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

fn range_json(c: &Checked, name: &str) -> Option<String> {
    let (lo, hi) = c.ranges.get(name)?;
    let (lo, hi) = (lo.as_ref()?, hi.as_ref()?);
    let sc = c.wire_scale(name);
    Some(Obj::new().int("min", crate::types::wire_int(*lo, sc)).int("max", crate::types::wire_int(*hi, sc)).finish())
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

pub fn json(f: &RuleFile, c: &Checked, src_hash: &str) -> String {
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
            let mut walk = |e: &Expr, v: &mut Val| {
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
    let mut edges: Vec<String> = Vec::new();
    let mut nodes: Vec<String> = Vec::new();
    for m in &merged {
        for (from, kind, via) in &m.reads {
            // A cell may name a group or an enum value, which is not a value of the rule.
            if !known.contains(from) || from == &m.name {
                continue;
            }
            let mut e = Obj::new().str("from", from).str("to", &m.name).str("kind", *kind);
            if !via.is_empty() {
                e = e.str("via", via);
            }
            let line = e.finish();
            if !edges.contains(&line) {
                edges.push(line);
            }
        }
        let mut o = Obj::new().str("name", &m.name);
        if !m.alias.is_empty() {
            o = o.str("alias", &m.alias);
        }
        o = o.str("kind", m.kind);
        for (k, v) in &m.extra {
            o = o.str(k, v);
        }
        if let Some(t) = c.ty_of(&m.name) {
            o = o.str("type", t.to_string());
        }
        if let Some(r) = range_json(c, &m.name) {
            o = o.raw("range", r);
        }
        if outs.contains(&m.name.as_str()) {
            o = o.bool("output", true);
        }
        if let Some(ap) = from_apply(f, &m.name) {
            o = o.str("from_apply", &ap);
        }
        if per_element.contains(&m.name) && m.kind != "element" {
            o = o.bool("per_element", true);
        }
        if !m.by.is_empty() {
            o = o.raw("by", arr(&m.by));
        }
        nodes.push(o.finish());
    }

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
