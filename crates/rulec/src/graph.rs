//! The rule as a graph: what decides each value, and which values it reads (§15.117).
//!
//! **Nothing here is new knowledge.** `rulec doc` already prints, under every table, each
//! column it reads and where that column comes from. This is the same relation gathered
//! into one place instead of scattered over the sections — which is what it takes to *see*
//! the shape of a rule rather than read it a table at a time. A rule whose two upstream
//! tables cut the same input at different thresholds is a diamond, and a diamond is
//! something you notice; in the text it is two tables you have to read side by side
//! (§15.114).
//!
//! **It is a graph of dependency, not of time.** Every edge says "this value is read while
//! that one is decided", and all of it happens in one call: there is no step between two
//! nodes for anything to happen in. What happens outside is at the *edge* of the picture —
//! the inputs and the fields of an element arrive from somewhere, and where the caller is a
//! durable workflow that somewhere is a step with side effects. So the crossings carry
//! their guards and the rule's preconditions come with them (§15.116): a drawing that stops
//! at the rule's own boundary hides the half the caller has to get right.
//!
//! A node is a **value**, not an item. A table is not a node: it is how one or more values
//! are decided, and it rides on them in `by`. That keeps the picture in the reader's own
//! vocabulary — the names in the rule are values — and it is what lets one value carry two
//! deciders when a clause takes precedence over a table (§15.66).

use crate::ast::*;
use crate::json::{arr, Obj};
use crate::types::Checked;

/// One value of the rule, before it is written out. Two deciders of one value are one of
/// these with two entries in `by`, in the order they are written — which is the order
/// precedence is declared in (§15.66).
struct Val {
    name: String,
    alias: String,
    kind: &'static str,
    /// Extra fields this kind carries: `of` for an element, and nothing for the rest.
    extra: Vec<(&'static str, String)>,
    by: Vec<String>,
    /// `(the value read, the kind of edge, the decider that reads it)`.
    reads: Vec<(String, &'static str, String)>,
}

/// The `apply` a name came in through, when it did. The expansion prefixes every name it
/// brought with `<apply>:` (§15.69), which is the only mark the graph needs to draw them
/// inside a border of their own.
fn from_apply(f: &RuleFile, name: &str) -> Option<String> {
    let (head, _) = name.split_once(':')?;
    f.applies.iter().find(|a| a.name.text == head).map(|a| a.name.text.clone())
}

fn names(e: &Expr) -> Vec<(String, &'static str, String)> {
    crate::apply::expr_names(e).into_iter().map(|n| (n, "reads", String::new())).collect()
}

fn val(n: &Name, kind: &'static str) -> Val {
    Val {
        name: n.text.clone(),
        alias: n.ascii.clone().unwrap_or_default(),
        kind,
        extra: Vec::new(),
        by: Vec::new(),
        reads: Vec::new(),
    }
}

/// One value, as both renderings need it.
pub struct GNode {
    pub name: String,
    pub alias: String,
    pub kind: &'static str,
    pub of: Option<String>,
    pub ty: Option<String>,
    pub range: Option<(i128, i128)>,
    pub output: bool,
    pub per_element: bool,
    pub from_apply: Option<String>,
    /// `(kind, name)` per decider, in the order precedence is declared.
    pub by: Vec<(String, String)>,
    pub by_json: Vec<String>,
}

pub struct GEdge {
    pub from: String,
    pub to: String,
    pub kind: &'static str,
    pub via: String,
}

pub struct Graph {
    pub nodes: Vec<GNode>,
    pub edges: Vec<GEdge>,
}

/// The graph itself. `json` writes it out; `svg` draws it. Two renderings of one structure,
/// built once so they cannot be about different things (§15.31).
pub fn build(f: &RuleFile, c: &Checked) -> Graph {
    let mut vals: Vec<Val> = Vec::new();

    for i in &f.inputs {
        vals.push(val(&i.name, "input"));
    }
    if let Some(el) = &f.elements {
        vals.push(val(&el.name, "sequence"));
        for fd in &el.fields {
            let mut v = val(&fd.name, "element");
            v.extra.push(("of", el.name.text.clone()));
            vals.push(v);
        }
    }
    for it in &f.items {
        match it {
            Item::Agg(d) => {
                let mut b = Obj::new()
                    .str("kind", if d.kind == AggKind::Sum { "sum" } else { "count" })
                    .str("over", &d.over)
                    .str("of", &d.column.text);
                if let Some(w) = &d.value {
                    b = b.str("where", &w.text);
                }
                let mut v = val(&d.name, "value");
                v.by.push(b.finish());
                // The one edge that leaves the element frame: many elements in, one value out.
                v.reads.push((d.column.text.clone(), "walk", String::new()));
                vals.push(v);
            }
            Item::Derived(d) => {
                let mut v = val(&d.name, "value");
                v.by.push(Obj::new().str("kind", "derive").finish());
                v.reads = names(&d.expr);
                vals.push(v);
            }
            Item::Define(d) => {
                let mut v = val(&d.name, "value");
                v.by.push(Obj::new().str("kind", "define").finish());
                v.reads = names(&d.expr);
                vals.push(v);
            }
            Item::Table(t) => {
                let tn = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
                let mut b = Obj::new()
                    .str("kind", if t.clause { "clause" } else { "table" })
                    .str("name", &tn)
                    .str("policy", if t.policy == Policy::Unique { crate::kw::UNIQUE } else { crate::kw::FIRST })
                    .int("rows", t.rows.len() as i128);
                if !t.overrides.is_empty() {
                    b = b.raw(
                        "overrides",
                        crate::json::strs(&t.overrides.iter().map(|r| r.table.clone()).collect::<Vec<_>>()),
                    );
                }
                let by = b.finish();
                let mut reads: Vec<(String, &'static str, String)> =
                    t.inputs.iter().map(|(n, _)| (n.clone(), "reads", tn.clone())).collect();
                // A cell to the right of `->` may name a value instead of writing a literal
                // — that is how a clause hands on what a `define` worked out — and naming it
                // is reading it. Only the left side is in `inputs`, so without this the
                // edge that carries the answer out of a clause is missing.
                for r in &t.rows {
                    for oc in &r.outs {
                        if let OutCell::Name(w) | OutCell::Lit(Lit::Word(w)) = oc {
                            reads.push((w.clone(), "reads", tn.clone()));
                        }
                    }
                }
                for oc in &t.outputs {
                    let mut v = val(&oc.name, "value");
                    v.by.push(by.clone());
                    v.reads = reads.clone();
                    vals.push(v);
                }
            }
        }
    }
    // The walk's answer is the rule's output, and nothing else names it: a fold is the one
    // item whose product is written only in `outputs` (§15.56).
    if let Some(fd) = &f.fold {
        if let Some(o) = f.outputs.iter().find(|o| !vals.iter().any(|v| v.name == o.name.text)) {
            let mut v = val(&o.name, "value");
            v.by.push(
                Obj::new().str("kind", "fold").str("verdict", &fd.verdict).str("over", &fd.over).finish(),
            );
            v.reads.push((fd.verdict.clone(), "reads", String::new()));
            let walk = |e: &Expr, v: &mut Val| {
                for n in crate::apply::expr_names(e) {
                    v.reads.push((n, "walk", String::new()));
                }
            };
            for (_, arm, _) in &fd.arms {
                match arm {
                    Arm::Stop(Some(e)) => walk(e, &mut v),
                    Arm::Take { expr, .. } => walk(expr, &mut v),
                    Arm::KeepMax { expr, key } => {
                        walk(expr, &mut v);
                        walk(key, &mut v);
                    }
                    _ => {}
                }
            }
            for e in fd.empty.iter().chain(fd.exhausted.iter()) {
                walk(e, &mut v);
            }
            vals.push(v);
        }
    }

    // One node per value: a second decider joins the first rather than standing beside it.
    let mut merged: Vec<Val> = Vec::new();
    for v in vals {
        match merged.iter_mut().find(|m| m.name == v.name) {
            Some(m) => {
                m.by.extend(v.by);
                m.reads.extend(v.reads);
            }
            None => merged.push(v),
        }
    }

    // **Inside the element frame, or outside it.** A value decided from a field of an
    // element is decided once per element; a `sum`, a `count` and a `fold` are the three
    // ways out, and everything downstream of one of those is decided once per call. The
    // text does not say which is which — you have to know that `elements` declares a
    // repeated record and that a fold collapses it — so the graph does, and a drawing can
    // put a border where the text has none.
    let ways_out: Vec<&str> = vec!["sum", "count", "fold"];
    let mut per_element: Vec<String> = f
        .elements
        .iter()
        .flat_map(|e| e.fields.iter().map(|fd| fd.name.text.clone()))
        .collect();
    loop {
        let mut grew = false;
        for m in &merged {
            if per_element.contains(&m.name)
                || m.by.iter().any(|b| ways_out.iter().any(|w| b.contains(&format!("\"kind\":\"{w}\""))))
            {
                continue;
            }
            if m.reads.iter().any(|(from, _, _)| per_element.contains(from)) {
                per_element.push(m.name.clone());
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }

    let known: Vec<String> = merged.iter().map(|m| m.name.clone()).collect();
    let outs: Vec<&str> = f.outputs.iter().map(|o| o.name.text.as_str()).collect();
    let mut edges: Vec<GEdge> = Vec::new();
    let mut nodes: Vec<GNode> = Vec::new();
    for m in &merged {
        for (from, kind, via) in &m.reads {
            // A cell may name a group or an enum value, which is not a value of the rule.
            if !known.contains(from) || from == &m.name {
                continue;
            }
            if edges.iter().any(|e| &e.from == from && e.to == m.name && e.kind == *kind && &e.via == via) {
                continue;
            }
            edges.push(GEdge { from: from.clone(), to: m.name.clone(), kind, via: via.clone() });
        }
        nodes.push(GNode {
            name: m.name.clone(),
            alias: m.alias.clone(),
            kind: m.kind,
            of: m.extra.iter().find(|(k, _)| *k == "of").map(|(_, v)| v.clone()),
            ty: c.ty_of(&m.name).map(|t| t.to_string()),
            range: c.ranges.get(&m.name).and_then(|(lo, hi)| {
                let sc = c.wire_scale(&m.name);
                Some((crate::types::wire_int(*lo.as_ref()?, sc), crate::types::wire_int(*hi.as_ref()?, sc)))
            }),
            output: outs.contains(&m.name.as_str()),
            per_element: per_element.contains(&m.name) && m.kind != "element",
            from_apply: from_apply(f, &m.name),
            by: m
                .by
                .iter()
                .map(|b| {
                    let k = json_str(b, "kind");
                    // A summary's own name is on the box already; what the second line is
                    // for is what it summarises.
                    let what = match k.as_str() {
                        "sum" | "count" => tr!(
                            "{}の{}",
                            "{1} over {0}",
                            json_str(b, "over"),
                            json_str(b, "of")
                        ),
                        _ => json_str(b, "name"),
                    };
                    (k, what)
                })
                .collect(),
            by_json: m.by.clone(),
        });
    }
    Graph { nodes, edges }
}

/// One string field out of an object this module wrote a moment ago. Reading back what was
/// just written is cheaper than carrying the pieces twice, and the two shapes cannot drift
/// because only this file writes them.
fn json_str(obj: &str, key: &str) -> String {
    let pat = format!("\"{key}\":\"");
    let Some(i) = obj.find(&pat) else { return String::new() };
    let rest = &obj[i + pat.len()..];
    rest.find('"').map(|j| rest[..j].to_string()).unwrap_or_default()
}

pub fn json(f: &RuleFile, c: &Checked, src_hash: &str) -> String {
    let g = build(f, c);
    let nodes: Vec<String> = g
        .nodes
        .iter()
        .map(|n| {
            let mut o = Obj::new().str("name", &n.name);
            if !n.alias.is_empty() {
                o = o.str("alias", &n.alias);
            }
            o = o.str("kind", n.kind);
            if let Some(of) = &n.of {
                o = o.str("of", of);
            }
            if let Some(t) = &n.ty {
                o = o.str("type", t);
            }
            if let Some((lo, hi)) = n.range {
                o = o.raw("range", Obj::new().int("min", lo).int("max", hi).finish());
            }
            if n.output {
                o = o.bool("output", true);
            }
            if n.per_element {
                o = o.bool("per_element", true);
            }
            if let Some(ap) = &n.from_apply {
                o = o.str("from_apply", ap);
            }
            if !n.by_json.is_empty() {
                o = o.raw("by", arr(&n.by_json));
            }
            o.finish()
        })
        .collect();
    let edges: Vec<String> = g
        .edges
        .iter()
        .map(|e| {
            let mut o = Obj::new().str("from", &e.from).str("to", &e.to).str("kind", e.kind);
            if !e.via.is_empty() {
                o = o.str("via", &e.via);
            }
            o.finish()
        })
        .collect();
    Obj::new()
        .str("rule", &f.name.text)
        .str("alias", f.name.ascii.as_deref().unwrap_or(""))
        .str("version", &f.version)
        .str("source_sha256", src_hash)
        .raw("nodes", arr(&nodes))
        .raw("edges", arr(&edges))
        // The crossings carry their guards in the nodes; what a shape cannot say about them
        // is here, in the same form `rulec api` gives it (§15.116).
        .raw("preconditions", crate::verify::preconditions_json(f, c))
        .finish()
        + "\n"
}

/// How wide a label is, roughly: a full-width character takes two columns.
fn vis(s: &str) -> usize {
    s.chars().map(|ch| if (ch as u32) >= 0x1100 && !ch.is_ascii() { 2 } else { 1 }).sum()
}

fn word_for(kind: &str) -> String {
    match kind {
        "table" => tr!("表", "table"),
        "clause" => tr!("節", "clause"),
        "derive" => tr!("導出", "derive"),
        "define" => tr!("定義", "define"),
        "sum" => tr!("合計", "sum"),
        "count" => tr!("数え上げ", "count"),
        "fold" => tr!("畳み込み", "fold"),
        "input" => tr!("入力", "input"),
        "element" => tr!("要素の欄", "element field"),
        "sequence" => tr!("並び", "sequence"),
        _ => String::new(),
    }
}

/// The second line of a box: how the value is decided, or — at a crossing — what the caller
/// is held to there.
fn sub_of(n: &GNode) -> String {
    if n.by.is_empty() {
        let mut s = word_for(n.kind);
        if let Some(t) = &n.ty {
            s = format!("{s} · {t}");
        }
        // At a crossing the range is not decoration: it is what the caller is refused for.
        if let Some((lo, hi)) = n.range {
            s = format!("{s} {lo}–{hi}");
        }
        return s;
    }
    n.by
        .iter()
        .map(|(k, name)| if name.is_empty() { word_for(k) } else { format!("{} {name}", word_for(k)) })
        .collect::<Vec<_>>()
        .join(if crate::i18n::ja() { "／" } else { " / " })
}

/// **The rule, drawn.** One box per value, one arrow per "read while deciding".
///
/// Top to bottom, by how far a value is from the ones that arrive from outside — so the top
/// row is the wall the caller crosses and the bottom row is what comes back. Nothing here
/// is a step in time: the whole picture happens in one call, and the only thing that
/// happens *between* anything is above the top row, outside the frame (§15.117).
pub fn svg(f: &RuleFile, c: &Checked) -> String {
    let g = build(f, c);
    if g.nodes.is_empty() {
        return String::new();
    }
    let at = |n: &str| g.nodes.iter().position(|x| x.name == n);

    // How far from the outside each value is. The walk is bounded by the node count: the
    // items of a rule are written define-before-use (§5.1), so there is no cycle to spin on.
    let mut layer = vec![0usize; g.nodes.len()];
    for _ in 0..g.nodes.len() {
        let want: Vec<usize> = g
            .nodes
            .iter()
            .map(|n| {
                g.edges
                    .iter()
                    .filter(|e| e.to == n.name)
                    .filter_map(|e| at(&e.from))
                    .map(|j| layer[j] + 1)
                    .max()
                    .unwrap_or(0)
            })
            .collect();
        if want == layer {
            break;
        }
        for (i, w) in want.into_iter().enumerate() {
            layer[i] = layer[i].max(w);
        }
    }

    const BH: f64 = 42.0;
    const LH: f64 = 82.0;
    const GX: f64 = 16.0;
    const PAD: f64 = 22.0;
    let width_of = |n: &GNode| -> f64 {
        let w = vis(&n.name).max(vis(&sub_of(n))) as f64;
        (10.0 + 6.4 * w).clamp(108.0, 208.0)
    };

    // The sequence itself gets no box: it has no arrow of its own, and a box with nothing
    // attached reads as something forgotten. Its name goes on the frame instead.
    let drawn = |i: usize| g.nodes[i].kind != "sequence";
    let depth = layer.iter().copied().max().unwrap_or(0) + 1;
    let rows: Vec<Vec<usize>> =
        (0..depth).map(|l| (0..g.nodes.len()).filter(|&i| layer[i] == l && drawn(i)).collect()).collect();
    let row_w: Vec<f64> = rows
        .iter()
        .map(|r| r.iter().map(|&i| width_of(&g.nodes[i])).sum::<f64>() + GX * (r.len().max(1) - 1) as f64)
        .collect();
    let inner = row_w.iter().cloned().fold(0.0_f64, f64::max);
    let w_total = inner + PAD * 2.0;
    let h_total = LH * depth as f64 + PAD * 2.0 - (LH - BH);

    let mut x = vec![0.0_f64; g.nodes.len()];
    let mut y = vec![0.0_f64; g.nodes.len()];
    for (l, r) in rows.iter().enumerate() {
        let mut cx = PAD + (inner - row_w[l]) / 2.0;
        for &i in r {
            x[i] = cx;
            y[i] = PAD + LH * l as f64;
            cx += width_of(&g.nodes[i]) + GX;
        }
    }

    let mut o = String::new();
    o.push_str(&format!(
        "<figure id=\"rule-graph\">\n<svg viewBox=\"0 0 {:.0} {:.0}\" width=\"{:.0}\" height=\"{:.0}\" \
         font-family=\"system-ui, sans-serif\" role=\"img\" aria-label=\"{}\">\n\
         <defs><marker id=\"ar\" viewBox=\"0 0 8 8\" refX=\"7\" refY=\"4\" markerWidth=\"6\" markerHeight=\"6\" orient=\"auto\">\
         <path d=\"M0 0 L8 4 L0 8 z\" fill=\"#9a9a9a\"/></marker>\
         <marker id=\"arl\" viewBox=\"0 0 8 8\" refX=\"7\" refY=\"4\" markerWidth=\"6\" markerHeight=\"6\" orient=\"auto\">\
         <path d=\"M0 0 L8 4 L0 8 z\" fill=\"#c07800\"/></marker></defs>\n",
        w_total,
        h_total,
        w_total,
        h_total,
        crate::doc::html_esc(&tr!(
            "値の出どころと読み先の図",
            "what decides each value, and which values it reads"
        ))
    ));

    // The frames, behind everything: what is decided once per element, and what another
    // rule brought in. Neither is anywhere in the text of the rule.
    let frame = |members: Vec<usize>, label: String, o: &mut String| {
        if members.is_empty() {
            return;
        }
        let x0 = members.iter().map(|&i| x[i]).fold(f64::MAX, f64::min) - 9.0;
        let x1 = members.iter().map(|&i| x[i] + width_of(&g.nodes[i])).fold(0.0_f64, f64::max) + 9.0;
        let y0 = members.iter().map(|&i| y[i]).fold(f64::MAX, f64::min) - 24.0;
        let y1 = members.iter().map(|&i| y[i] + BH).fold(0.0_f64, f64::max) + 9.0;
        o.push_str(&format!(
            "<rect x=\"{x0:.1}\" y=\"{y0:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"8\" fill=\"#fbfbfb\" \
             stroke=\"#c8c8c8\" stroke-dasharray=\"4 3\"/>\
             <text x=\"{:.1}\" y=\"{:.1}\" font-size=\"10\" fill=\"#777\">{}</text>\n",
            x1 - x0,
            y1 - y0,
            x0 + 7.0,
            y0 + 12.0,
            crate::doc::html_esc(&label)
        ));
    };
    let seq = g.nodes.iter().find(|n| n.kind == "sequence").map(|n| n.name.clone()).unwrap_or_default();
    frame(
        (0..g.nodes.len()).filter(|&i| drawn(i) && (g.nodes[i].per_element || g.nodes[i].kind == "element")).collect(),
        tr!("{} の一件ごと", "once per element of {}", seq),
        &mut o,
    );
    for a in &f.applies {
        frame(
            (0..g.nodes.len())
                .filter(|&i| drawn(i) && g.nodes[i].from_apply.as_deref() == Some(a.name.text.as_str()))
                .collect(),
            tr!("準用 {}", "applied: {}", a.name.text),
            &mut o,
        );
    }

    for e in &g.edges {
        let (Some(i), Some(j)) = (at(&e.from), at(&e.to)) else { continue };
        if !drawn(i) || !drawn(j) {
            continue;
        }
        let (x1, y1) = (x[i] + width_of(&g.nodes[i]) / 2.0, y[i] + BH);
        let (x2, y2) = (x[j] + width_of(&g.nodes[j]) / 2.0, y[j]);
        let d = ((y2 - y1) / 2.0).max(10.0);
        let dash = if e.kind == "walk" { " stroke-dasharray=\"5 3\"" } else { "" };
        o.push_str(&format!(
            "<path class=\"ge\" data-from=\"{}\" data-to=\"{}\" d=\"M{x1:.1} {y1:.1} C{x1:.1} {:.1} {x2:.1} {:.1} {x2:.1} {:.1}\" \
             fill=\"none\" stroke=\"#9a9a9a\"{dash} marker-end=\"url(#ar)\"/>\n",
            crate::doc::html_esc(&e.from),
            crate::doc::html_esc(&e.to),
            y1 + d,
            y2 - d,
            y2 - 3.0
        ));
    }

    for (i, n) in g.nodes.iter().enumerate() {
        if !drawn(i) {
            continue;
        }
        let w = width_of(n);
        let outside = n.kind == "input" || n.kind == "element" || n.kind == "sequence";
        let deciders: Vec<&str> =
            n.by.iter().filter(|(k, _)| k == "table" || k == "clause").map(|(_, name)| name.as_str()).collect();
        o.push_str(&format!(
            "<g class=\"gn\" data-v=\"{}\"{}>\
             <rect x=\"{:.1}\" y=\"{:.1}\" width=\"{w:.1}\" height=\"{BH}\" rx=\"5\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>\
             <text x=\"{:.1}\" y=\"{:.1}\" font-size=\"12\" fill=\"#222\">{}</text>\
             <text x=\"{:.1}\" y=\"{:.1}\" font-size=\"10\" fill=\"#777\">{}</text></g>\n",
            crate::doc::html_esc(&n.name),
            if deciders.is_empty() {
                String::new()
            } else {
                format!(" data-t=\"{}\"", crate::doc::html_esc(&deciders.join("\u{1f}")))
            },
            x[i],
            y[i],
            if outside { "#f4f6f8" } else { "#ffffff" },
            if n.output { "#8a8a8a" } else { "#c8c8c8" },
            if n.output { 1.6 } else { 1.0 },
            x[i] + 8.0,
            y[i] + 17.0,
            crate::doc::html_esc(&n.name),
            x[i] + 8.0,
            y[i] + 32.0,
            crate::doc::html_esc(&sub_of(n))
        ));
    }
    o.push_str("</svg>\n<figcaption>");
    o.push_str(&crate::doc::html_esc(&tr!(
        "どの値が、どの値を読んで決まるかの図です。上の段が呼び手から渡るもので、下の段がこの規則の答えです。矢印は順番ではありません——全部が一回の呼び出しの中で決まります。試すと、当てはまった行を出した表の値に色が付きます。",
        "Which value is read while which other is decided. The top row is what arrives from the caller and the bottom row is what comes back. An arrow is not a step: all of it happens in one call. Try a case and the values whose rows matched light up."
    )));
    let pre: Vec<String> = crate::verify::preconditions(f, c)
        .iter()
        .map(|p| match p {
            crate::verify::Pre::Rel { left, op, right } => format!("{left} {op} {right}"),
            crate::verify::Pre::Sum { name: _, over, of, max } => {
                tr!("{over}の{of}の合計 ≤ {max}", "the total of {of} over {over} ≤ {max}")
            }
            crate::verify::Pre::Length { sequence, max } => {
                tr!("{sequence}は {max} 件まで", "at most {max} elements of {sequence}")
            }
        })
        .collect();
    if !pre.is_empty() {
        o.push(' ');
        o.push_str(&crate::doc::html_esc(&tr!(
            "上の段を渡すとき、呼び手はこれも満たします：{}。形だけでは言えない前提で、破ると入口で断られます。",
            "Handing the top row over also means satisfying this: {}. None of it is a shape, and breaking it is refused at the door.",
            pre.join(if crate::i18n::ja() { "、" } else { "; " })
        )));
    }
    o.push_str("</figcaption>\n</figure>\n");
    o
}
