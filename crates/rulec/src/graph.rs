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

fn word_for(kind: &str) -> String {
    match kind {
        "table" => tr!("表", "table"),
        "clause" => tr!("節", "clause"),
        "derive" => tr!("導出", "derive"),
        "define" => tr!("定義", "define"),
        "sum" => tr!("合計", "sum"),
        "count" => tr!("数え上げ", "count"),
        "fold" => tr!("畳み込み", "fold"),
        _ => String::new(),
    }
}

/// How a value is decided, as the line under its name on the card.
fn kind_line(n: &GNode) -> String {
    n.by
        .iter()
        .map(|(k, what)| if what.is_empty() { word_for(k) } else { format!("{} {what}", word_for(k)) })
        .collect::<Vec<_>>()
        .join(if crate::i18n::ja() { "／" } else { " / " })
}

/// **What the page needs to lay the rule out as a board.**
///
/// The drawing went through two versions before this one. A box per *value* did not read:
/// a rule with ten inputs spent three rows on boxes that carry no structure, and the arrows
/// out of them crossed everything. A box per **decider** — one table, one derive, one
/// definition — with the values it reads written inside it, is five boxes and four arrows
/// where the other was fifteen and twenty. And once the box is a card rather than a
/// rectangle, the table itself goes inside it, which is what the reader came for: the rows,
/// with the one that fired lit, in the place the picture says it belongs (§15.120).
///
/// Columns run left to right by how far a decider is from the inputs. Left to right, not
/// top to bottom, because a table shown whole is tall: side by side, two of them are one
/// glance apart however many rows they have.
///
/// The card sizes are the browser's business — a table's width is not knowable here — so
/// this is data and the page lays it out. Without a script the page is the document it
/// always was; the board is what the script makes of it.
pub fn data_json(f: &RuleFile, c: &Checked) -> String {
    let g = build(f, c);
    let dec: Vec<usize> = (0..g.nodes.len()).filter(|&i| !g.nodes[i].by.is_empty()).collect();
    let is_dec = |name: &str| g.nodes.iter().any(|n| n.name == name && !n.by.is_empty());

    let mut layer = vec![0usize; g.nodes.len()];
    for _ in 0..dec.len().max(1) {
        let want: Vec<usize> = g
            .nodes
            .iter()
            .map(|n| {
                g.edges
                    .iter()
                    .filter(|e| e.to == n.name && is_dec(&e.from))
                    .filter_map(|e| g.nodes.iter().position(|x| x.name == e.from))
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
    let depth = dec.iter().map(|&i| layer[i]).max().unwrap_or(0) + 1;
    let cols: Vec<String> = (0..depth)
        .map(|l| {
            crate::json::strs(
                &dec.iter().copied().filter(|&i| layer[i] == l).map(|i| g.nodes[i].name.clone()).collect::<Vec<_>>(),
            )
        })
        .collect();

    let nodes: Vec<String> = dec
        .iter()
        .map(|&i| {
            let n = &g.nodes[i];
            let reads: Vec<String> = g
                .edges
                .iter()
                .filter(|e| e.to == n.name && !is_dec(&e.from))
                .map(|e| e.from.clone())
                .collect();
            let tables: Vec<String> =
                n.by.iter().filter(|(k, _)| k == "table" || k == "clause").map(|(_, w)| w.clone()).collect();
            let mut o = Obj::new()
                .str("v", &n.name)
                .str("by", &kind_line(n))
                .raw("reads", crate::json::strs(&reads))
                .raw("tables", crate::json::strs(&tables));
            if n.output {
                o = o.bool("out", true);
            }
            if n.per_element {
                o = o.bool("per_element", true);
            }
            if let Some(a) = &n.from_apply {
                o = o.str("apply", a);
            }
            o.finish()
        })
        .collect();
    let edges: Vec<String> = g
        .edges
        .iter()
        .filter(|e| is_dec(&e.from) && is_dec(&e.to))
        .map(|e| Obj::new().str("from", &e.from).str("to", &e.to).finish())
        .collect();

    Obj::new()
        .raw("cols", arr(&cols))
        .raw("nodes", arr(&nodes))
        .raw("edges", arr(&edges))
        .raw(
            "text",
            Obj::new()
                .str(
                    "caption",
                    tr!(
                        "どの値が何から決まるかを、左から右へ描いています。順番に起きるのではなく、一回の呼び出しで全部が決まります。",
                        "Which value is decided from what, left to right. It does not happen in that order: all of it is decided in one call."
                    ),
                )
                .str("hint", tr!("箱を押すと、その表について確かめたことがここに出ます。", "Press a card to see what was verified about its table."))
                .str("close", tr!("閉じる", "close"))
                .str("row", tr!("行", "row "))
                .finish(),
        )
        .finish()
}

/// The board's own stylesheet. Two panes with a divider you can drag, a canvas that scrolls
/// both ways, and a dock along the bottom for what a card is too small to hold.
pub const APP_CSS: &str = r##"
/* The document is laid out for reading; the board is laid out for the window. The class is
   put on by the script, so a page with no script keeps the document's own measure. */
body.boarded { max-width: none; margin: 0; padding: 0; }
body.boarded main { display: none; }
.app { display: flex; height: 100vh; align-items: stretch; }
.app .side { width: 24rem; flex: none; min-width: 14rem; border-right: 1px solid #e0e0e0; background: #fafbfc; padding: 22px 24px; overflow-y: auto; }
.app .split { flex: none; width: 6px; cursor: col-resize; background: #ececec; }
.app .split:hover, .app .split.on { background: #c9d6e2; }
.app .board { flex: 1; min-width: 0; display: flex; flex-direction: column; background: #fff; }
.app .stage { flex: 1; min-height: 0; overflow: auto; padding: 30px 38px; }
.app .cap { font-size: 0.9rem; line-height: 1.8; color: #555; margin: 0; padding: 14px 38px; border-top: 1px solid #eee; }
.app .hsplit { flex: none; height: 6px; cursor: row-resize; background: #ececec; }
.app .hsplit:hover, .app .hsplit.on { background: #c9d6e2; }
.app .hsplit[hidden] { display: none; }
.app .dock { border-top: 1px solid #dcdcdc; background: #fafbfc; padding: 16px 38px; height: 34vh; overflow-y: auto; }
.app .dock .x { float: right; font-size: 0.85rem; color: #888; cursor: pointer; }
#canvas { position: relative; display: flex; gap: 60px; align-items: flex-start; padding: 4px; }
#wires { position: absolute; inset: 0; overflow: visible; pointer-events: none; }
.gcol { display: flex; flex-direction: column; gap: 34px; position: relative; z-index: 1; flex: none; }
.gcard { border: 1px solid #bdbdbd; border-radius: 10px; background: #fff; padding: 12px 14px; cursor: pointer; box-shadow: 0 1px 2px rgba(0,0,0,.04); }
.gcard.out { border-color: #8a8a8a; border-width: 1.5px; }
.gcard.sel { border-color: #c07800; border-width: 2px; background: #fffbf0; }
.gcard h3 { margin: 0; font-size: 1rem; display: flex; align-items: baseline; gap: 12px; }
.gcard h3 .v { margin-left: auto; font-size: 1.1rem; }
.gcard .by { font-size: 0.8rem; color: #888; margin: 3px 0 0; }
.gcard .by .r { color: #c07800; font-weight: 600; }
.gcard .rd { font-size: 0.8rem; color: #666; margin: 2px 0 0; }
.gcard table { margin: 10px 0 0; }
.gcard th, .gcard td { padding: 3px 8px; white-space: nowrap; }
.app .side table { width: 100%; }
"##;

/// The board, built from the document the page already is. **Without this the page is that
/// document**: `<main>` is what a reader without a script sees, and nothing here is needed
/// to read the rule — only to see its shape.
pub const APP_JS: &str = r##"
(() => {
  const m = document.querySelector("main");
  if (!m || typeof GRAPH === "undefined" || !GRAPH.nodes.length) return;
  const mk = (t, c) => { const e = document.createElement(t); if (c) e.className = c; return e; };
  const app = mk("div", "app"), side = mk("div", "side"), split = mk("div", "split"), board = mk("div", "board");
  const stage = mk("div", "stage"), cap = mk("p", "cap"), dock = mk("div", "dock"), hsplit = mk("div", "hsplit");
  cap.textContent = GRAPH.text.caption; dock.hidden = true; hsplit.hidden = true;
  board.append(stage, cap, hsplit, dock); app.append(side, split, board);
  m.hidden = true; document.body.classList.add("boarded"); document.body.append(app);

  // The panes are made of the document's own sections: nothing is written twice.
  const sec = (k) => m.querySelector('section[data-sec="' + k + '"]');
  side.append(document.querySelector("#try"));
  for (const k of ["inputs", "outputs", "types"]) { const s = sec(k); if (s) side.append(s); }

  const canvas = mk("div"); canvas.id = "canvas";
  const wires = document.createElementNS("http://www.w3.org/2000/svg", "svg"); wires.id = "wires";
  canvas.append(wires); stage.append(canvas);
  const byName = {}; for (const n of GRAPH.nodes) byName[n.v] = n;
  const el = {}, detail = {};
  for (const col of GRAPH.cols) {
    const c = mk("div", "gcol");
    for (const v of col) {
      const n = byName[v]; if (!n) continue;
      const card = mk("div", "gcard" + (n.out ? " out" : "")); card.dataset.v = v;
      const h = mk("h3");
      const nm = mk("span"); nm.textContent = v;
      const val = mk("span", "v");
      h.append(nm, val);
      const by = mk("p", "by"); by.textContent = n.by;
      const r = mk("span", "r"); by.append(r);
      card.append(h, by);
      if (n.reads.length) { const rd = mk("p", "rd"); rd.textContent = "← " + n.reads.join("　"); card.append(rd); }
      // The table itself, taken out of the document and put where the picture says it is.
      //
      // It is found through a row, not through a heading: a row carries `data-t` with the
      // name the trace uses, which is the same name whatever language the page is in and
      // whatever an `apply` renamed it to (§15.69). A heading would be neither.
      for (const t of n.tables) {
        const tr = m.querySelector('tr[data-t="' + CSS.escape(t) + '"]');
        const tbl = tr && tr.closest("table");
        if (!tbl) continue;
        // What is around that table — its heading, what `check` verified, the citation —
        // is too much for a card and goes to the dock.
        const sec = tbl.closest("section.sec");
        if (sec) { detail[v] = detail[v] || []; detail[v].push(sec); }
        else {
          const box = mk("div");
          let p = tbl.previousElementSibling, head = [];
          while (p && !/^H[1-6]$/.test(p.tagName)) { head.unshift(p); p = p.previousElementSibling; }
          if (p) head.unshift(p);
          let q = tbl.nextElementSibling, tail = [];
          while (q && !/^H[1-6]$/.test(q.tagName)) { tail.push(q); q = q.nextElementSibling; }
          box.append(...head, ...tail);
          detail[v] = detail[v] || []; detail[v].push(box);
        }
        for (const th of tbl.querySelectorAll("th")) th.textContent = th.textContent.replace(/（.*$/, "").replace(/ \(.*$/, "").trim();
        card.append(tbl);
      }
      c.append(card); el[v] = card;
    }
    canvas.append(c);
  }
  for (const s of m.querySelectorAll("section.sec")) if (s.parentElement === m) side.append(s);

  function wire() {
    const b = canvas.getBoundingClientRect();
    wires.setAttribute("width", canvas.offsetWidth); wires.setAttribute("height", canvas.offsetHeight);
    wires.setAttribute("viewBox", "0 0 " + canvas.offsetWidth + " " + canvas.offsetHeight);
    wires.innerHTML = '<defs><marker id="gw" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path d="M0 0 L8 4 L0 8 z" fill="#b0b0b0"/></marker></defs>';
    for (const e of GRAPH.edges) {
      const A = el[e.from], B = el[e.to]; if (!A || !B) continue;
      const ra = A.getBoundingClientRect(), rb = B.getBoundingClientRect();
      // A card shown whole can be very tall, so an edge leaves and lands at the height of
      // the card at the other end rather than at the middle of its own.
      const y1 = Math.min(Math.max(rb.top + rb.height / 2, ra.top + 18), ra.bottom - 18) - b.top;
      const y2 = Math.min(Math.max(ra.top + ra.height / 2, rb.top + 18), rb.bottom - 18) - b.top;
      const x1 = ra.right - b.left, x2 = rb.left - b.left, d = Math.max(18, (x2 - x1) / 2);
      const p = document.createElementNS("http://www.w3.org/2000/svg", "path");
      p.setAttribute("d", "M" + x1 + " " + y1 + " C" + (x1 + d) + " " + y1 + " " + (x2 - d) + " " + y2 + " " + (x2 - 4) + " " + y2);
      p.setAttribute("fill", "none"); p.setAttribute("stroke", "#b0b0b0"); p.setAttribute("stroke-width", "1.4");
      p.setAttribute("marker-end", "url(#gw)"); wires.append(p);
    }
  }
  window.rulecBoardFill = function (trace, outs) {
    const row = {}; for (const t of trace || []) row[t.table] = t.row;
    for (const n of GRAPH.nodes) {
      const card = el[n.v]; if (!card) continue;
      const hit = n.tables.map((t) => row[t]).find((x) => x !== undefined);
      card.querySelector(".r").textContent = hit === undefined ? "" : "　" + GRAPH.text.row + hit;
      card.querySelector(".v").textContent = outs && outs[n.v] !== undefined ? outs[n.v] : "";
    }
    requestAnimationFrame(() => requestAnimationFrame(wire));
  };
  for (const v in el) el[v].addEventListener("click", () => {
    const on = el[v].classList.contains("sel");
    for (const o of document.querySelectorAll(".gcard.sel")) o.classList.remove("sel");
    dock.innerHTML = ""; dock.hidden = true; hsplit.hidden = true;
    if (on) return;
    el[v].classList.add("sel");
    const parts = detail[v] || []; if (!parts.length) return;
    const x = mk("span", "x"); x.textContent = GRAPH.text.close;
    x.addEventListener("click", (ev) => { ev.stopPropagation(); el[v].classList.remove("sel"); dock.hidden = true; });
    dock.append(x, ...parts); dock.hidden = false; hsplit.hidden = false; wire();
  });
  // Both borders move: the side pane's width, and the dock's height.
  let drag = null;
  split.addEventListener("mousedown", (e) => { drag = "x"; split.classList.add("on"); e.preventDefault(); });
  hsplit.addEventListener("mousedown", (e) => { drag = "y"; hsplit.classList.add("on"); e.preventDefault(); });
  addEventListener("mousemove", (e) => {
    if (drag === "x") side.style.width = Math.max(220, Math.min(e.clientX, innerWidth * 0.6)) + "px";
    else if (drag === "y") dock.style.height = Math.max(80, Math.min(innerHeight - e.clientY, innerHeight * 0.8)) + "px";
  });
  addEventListener("mouseup", () => {
    if (!drag) return;
    split.classList.remove("on"); hsplit.classList.remove("on"); drag = null; wire();
  });
  addEventListener("resize", wire);
  new ResizeObserver(() => wire()).observe(canvas);
  wire();
})();
"##;
