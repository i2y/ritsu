//! The glossaries (DESIGN 1.6, 3.6; PLAN B.9): a word that means two things in two contexts does
//! not cross the boundary unmapped (E406), nor is it mapped to a word of the same name (E407);
//! a term `means` an element of its own published language (E408); the words of a `term`
//! mapping are in the glossaries (E409); what `as` takes is there and from a context with a
//! relationship (E410); and every term crosses a boundary (W401).

use crate::ast::{Element, Role, Target, Term, ValueTo};
use crate::diag::{self, Diag, DiagExt, Ref};
use crate::elements::{At, Elements};
use ritsu_base::text::Text;
use crate::model::{Model, RelK};
use crate::naming::{Name, Tool};
use crate::proto::{Protos, Symbol};
use crate::refs::{Allowed, Crossing};
use std::collections::BTreeSet;

fn names(t: &Term) -> Vec<&str> {
    std::iter::once(t.name.as_str()).chain(t.also.iter().map(|a| a.value.as_str())).collect()
}

fn line_of(src: &str, l: usize) -> String {
    src.lines().nth(l - 1).unwrap_or("").trim().to_string()
}

/// Whether the element a name points at crosses with `reach` (DESIGN 2.5: an enum that crosses
/// takes its values along, a message its fields).
fn crosses(ps: &Protos, n: &Name, reach: &[Symbol]) -> bool {
    if n.tool != Tool::Proto {
        return false;
    }
    let Some(f) = ps.files.get(&n.path) else { return false };
    let Some((k, e)) = n.items.first() else { return false };
    matches!(k.as_str(), "message" | "enum") && reach.iter().any(|s| s.file == n.path && s.full == f.full(e))
}

/// `crossings_known` says whether sakai read every reference that could make a word cross: it is
/// false while a language is not joined, or did not answer for one of its files (E104, E105).
pub fn check(m: &Model, ps: &Protos, el: &Elements, crossings: &[Crossing], crossings_known: bool, read: &crate::suite::Read) -> Vec<Diag> {
    let mut diags = Vec::new();
    let at = |c: usize, p: crate::ast::Pos, code: &'static str, msg: Text| {
        let cx = &m.contexts[c];
        diag::at(code, &cx.file, p.line, p.col, msg).source(&cx.src)
    };
    // E408: what a term means is in its own published language.
    for (ci, c) in m.contexts.iter().enumerate() {
        for (ti, t) in c.ast.terms.iter().enumerate() {
            for (mi, me) in t.means.iter().enumerate() {
                let Some(n) = el.get(At::Means(ci, ti, mi)) else { continue };
                let ok = match n.tool {
                    Tool::Proto => c.published.iter().any(|p| p.protos.iter().any(|(f, _)| *f == n.path)),
                    Tool::Rulec => c.published.iter().any(|p| p.rulec.as_ref().is_some_and(|(f, _)| *f == n.path)),
                    _ => false,
                };
                if !ok {
                    let (tn, cn, nt) = (&t.name, &c.name, n.text());
                    let mut d = at(ci, me.pos(), "E408", tr!("語「{tn}」の `means` の {nt} は、「{cn}」の公表された言語にありません", "What the term {tn} `means`, {nt}, is not in a published language of {cn}"));
                    d = d.note(tr!(
                        "`means` は、語とともに境界を越えていく、自分の公表された言語の要素を指します。公表された言語にできるのは、proto の package と rulec の規則（Connect のサービス）です。",
                        "`means` points at an element of the context's own published language, which crosses the boundary with the term; a published language is a proto package or a rule (its Connect service)."
                    ));
                    diags.push(d);
                }
            }
        }
    }
    // E410: what `as` takes.
    for (ci, c) in m.contexts.iter().enumerate() {
        for t in &c.ast.terms {
            let Some(a) = &t.as_term else { continue };
            let Some(y) = m.ctx(&a.context) else { continue };
            let (tn, yn, an) = (&t.name, &a.context, &a.term);
            if !m.contexts[y].ast.terms.iter().any(|u| names(u).contains(&an.as_str())) {
                diags.push(at(ci, a.pos, "E410", tr!("`as` で取り入れる語「{an}」が、「{yn}」の用語集にありません", "The term {an} that `as` takes is not in {yn}'s glossary")));
                continue;
            }
            let related = |x: usize, z: usize| m.rels(x, z).any(|r| !matches!(r.kind, RelK::Separate));
            if !related(ci, y) && !related(y, ci) {
                let cn = &c.name;
                diags.push(at(ci, a.pos, "E410", tr!("「{cn}」と「{yn}」には関係が無いので、語「{tn}」を `as` で取り入れられません", "{cn} has no relationship with {yn}, so the term {tn} cannot be taken with `as`")).note(tr!(
                    "`as` は、関係のある相手の語を、同じ意味のまま取り入れます。",
                    "`as` takes a term, with its meaning, from a context there is a relationship with."
                )));
            }
        }
    }
    // E409: the words of a `term` mapping.
    for (ci, c) in m.contexts.iter().enumerate() {
        for r in &c.rels {
            let RelK::Upstream { terms, .. } = &r.kind else { continue };
            let y = &m.contexts[r.partner];
            for tm in terms {
                if !y.ast.terms.iter().any(|u| names(u).contains(&tm.from.as_str())) {
                    let (a, yn) = (&tm.from, &y.name);
                    diags.push(at(ci, tm.from_pos, "E409", tr!("`term` の対応の「{a}」が、「{yn}」の用語集にありません", "{a} of the `term` mapping is not in {yn}'s glossary")));
                }
                if !c.ast.terms.iter().any(|u| names(u).contains(&tm.to.as_str())) {
                    let (b, cn) = (&tm.to, &c.name);
                    diags.push(at(ci, tm.to_pos, "E409", tr!("`term` の対応の先の「{b}」が、「{cn}」の用語集にありません", "{b}, the target of the `term` mapping, is not in {cn}'s glossary")));
                }
            }
        }
    }
    // E406 and E407: the same word, meaning two things, crossing.
    let mut collide: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut said: BTreeSet<(usize, usize, usize, usize)> = BTreeSet::new();
    for cr in crossings {
        let ri = match cr.allowed {
            Some(Allowed::Upstream(ri)) => Some(ri),
            Some(Allowed::Partnership) => None,
            _ => continue,
        };
        let (x, y) = (cr.from_ctx, cr.to_ctx);
        let (xc, yc) = (&m.contexts[x], &m.contexts[y]);
        for (ti, t) in yc.ast.terms.iter().enumerate() {
            for mi in 0..t.means.len() {
                let Some(n) = el.get(At::Means(y, ti, mi)) else { continue };
                if !crosses(ps, n, &cr.reach) {
                    continue;
                }
                for (di, d) in xc.ast.terms.iter().enumerate() {
                    let shared: Vec<&str> = names(d).into_iter().filter(|w| names(t).contains(w)).collect();
                    if shared.is_empty() || d.as_term.as_ref().is_some_and(|a| a.context == yc.name && names(t).contains(&a.term.as_str())) {
                        continue;
                    }
                    collide.insert((x, di));
                    if !said.insert((x, di, y, ti)) {
                        continue;
                    }
                    let w = shared[0].to_string();
                    let rel = ri.map(|i| &xc.rels[i]);
                    let acl = rel.is_some_and(|r| r.has(Role::Acl));
                    let t_ref = Ref::line(Some(&yc.name), &yc.file, t.pos.line, Text::same(line_of(&yc.src, t.pos.line)));
                    let d_ref = Ref::line(Some(&xc.name), &xc.file, d.pos.line, Text::same(line_of(&xc.src, d.pos.line)));
                    let from_ref = cr.from_ref(&xc.name);
                    let el_ref = Ref::name(Some(&yc.name), n.clone(), tr!("「{}」の語「{}」が指すもの", "what {}'s term {} means", yc.name, t.name; yc.name, t.name));
                    // How the layer maps it, if it does.
                    let mut mapped_to: Option<(String, crate::ast::Pos)> = None;
                    let mut mapped = false;
                    if acl && let Some(r) = rel {
                        let RelK::Upstream { enums, terms, .. } = &r.kind else { unreachable!() };
                        if n.items.first().is_some_and(|(k, _)| k == "enum") {
                            for (ei, em) in enums.iter().enumerate() {
                                let Some(from) = el.get(At::From(x, ri.unwrap(), ei)) else { continue };
                                if from.path != n.path || from.items.first() != n.items.first() {
                                    continue;
                                }
                                mapped = true;
                                match n.items.get(1) {
                                    Some((_, v)) => {
                                        if let Some(vm) = em.values.iter().find(|vm| vm.from == *v) {
                                            if let ValueTo::Value(to, p) = &vm.to {
                                                mapped_to = Some((to.clone(), *p));
                                            }
                                        } else if let Some(tn) = el.get(At::To(x, ri.unwrap(), ei))
                                            && tn.tool == Tool::Rulec
                                            && let (Some((_, rule_enum)), Some(pf), Some((_, e))) = (tn.items.first(), ps.files.get(&from.path), from.items.first())
                                            && let Some(taken) = crate::mapping::rule_import(read, &tn.path, rule_enum, &from.path, &pf.full(e))
                                            && let Some((_, rv)) = taken.iter().find(|(wire, _)| wire == v)
                                        {
                                            // the rule's `import proto` is the mapping (DESIGN 1.7)
                                            mapped_to = Some((rv.clone(), em.pos));
                                        }
                                    }
                                    None => {
                                        mapped_to = match &em.target {
                                            Target::Name(nm, p) => Some((nm.clone(), *p)),
                                            Target::Element(te) => match te {
                                                Element::Long { written, pos } => written.items.last().map(|(_, _, nm, _)| (nm.clone(), *pos)),
                                                Element::Short { name, pos, .. } => Some((name.clone(), *pos)),
                                            },
                                        };
                                    }
                                }
                            }
                        }
                        if let Some(tm) = terms.iter().find(|tm| names(t).contains(&tm.from.as_str())) {
                            mapped = true;
                            mapped_to = Some((tm.to.clone(), tm.to_pos));
                        }
                    }
                    if mapped {
                        if let Some((to, p)) = mapped_to
                            && names(d).contains(&to.as_str())
                        {
                            let (yn, xn, tn) = (&yc.name, &xc.name, &t.name);
                            let mut dg = at(x, p, "E407", tr!("「{yn}」の語「{tn}」を、「{xn}」の違う意味の語と同じ名前の「{to}」に読み替えています", "{yn}'s term {tn} is mapped to {to}, the name of {xn}'s word of another meaning"));
                            if let Some(def) = &d.definition {
                                let dv = &def.value;
                                dg = dg.note(tr!("「{xn}」の「{to}」は「{dv}」で、「{yn}」の「{tn}」とは違う意味です。", "{xn}'s {to} is \"{dv}\", which is not what {yn}'s {tn} means."));
                            }
                            dg = dg.note(tr!("読み替えた先には、違う名前を付けます。", "Give what it is mapped to another name."));
                            diags.push(dg.refer(t_ref).refer(d_ref));
                        }
                        continue;
                    }
                    let (yn, xn, tn, dn) = (&yc.name, &xc.name, &t.name, &d.name);
                    let mut dg = at(x, d.pos, "E406", tr!("「{yn}」の「{w}」が、「{xn}」の違う意味の「{w}」とぶつかったまま、境界を越えています", "{yn}'s {w} crosses into {xn}, where {w} means something else, unmapped"));
                    let what = n.text();
                    match &t.definition {
                        Some(def) => {
                            let dv = &def.value;
                            dg = dg.note(tr!("「{yn}」の「{tn}」は「{dv}」で、{what} を指します。", "{yn}'s {tn} is \"{dv}\", and means {what}."));
                        }
                        None => dg = dg.note(tr!("「{yn}」の「{tn}」は {what} を指します。", "{yn}'s {tn} means {what}.")),
                    }
                    if let Some(def) = &d.definition {
                        let dv = &def.value;
                        dg = dg.note(tr!("「{xn}」の「{dn}」は「{dv}」です。", "{xn}'s {dn} is \"{dv}\"."));
                    }
                    if acl {
                        dg = dg.note(tr!(
                            "「{xn}」は「{yn}」とのあいだに腐敗防止層を置いているので、対応で読み替えます。`term {tn} -> <「{xn}」の語>` を書くか、列挙なら値の行で、違う名前の値に読み替えます。",
                            "{xn} keeps an anticorruption layer toward {yn}: map it, with `term {tn} -> <a term of {xn}>`, or for an enum with the value lines, to a value of another name."
                        ));
                    } else {
                        let role = match rel.map(|r| r.roles()) {
                            Some(rs) if rs.contains(&Role::Customer) => tr!("顧客", "a customer"),
                            Some(_) => tr!("順応者", "a conformist"),
                            None => tr!("パートナー", "a partner"),
                        };
                        dg = dg.note(tr!("「{xn}」は「{yn}」の{}なので、対応を書いて読み替えることはできません。", "{xn} is {} of {yn}, so it has no mapping to map it with.", role.ja; role.en));
                        dg = dg.note(tr!(
                            "直し方: 「{xn}」の語の名前を変える。同じ意味なら `{dn} as {yn}.{tn}` にする。読み替えるなら、`upstream {yn}` を anticorruption layer にして `term {tn} -> <「{xn}」の語>` を書く。",
                            "To fix it: rename {xn}'s term; if it means the same, write `{dn} as {yn}.{tn}`; to map it, make `upstream {yn}` an anticorruption layer and write `term {tn} -> <a term of {xn}>`."
                        ));
                    }
                    diags.push(dg.refer(from_ref).refer(el_ref).refer(t_ref).refer(d_ref));
                }
            }
        }
    }
    // W401: every term crosses a boundary (DESIGN 1.6). Not while an element the map names was
    // not found, nor while some references are not read: what crosses is then not known in full.
    for (ci, c) in m.contexts.iter().enumerate().filter(|_| el.complete && crossings_known) {
        let mut targets: Vec<&str> = Vec::new();
        for r in &c.rels {
            let RelK::Upstream { enums, terms, .. } = &r.kind else { continue };
            if !r.has(Role::Acl) {
                continue;
            }
            for em in enums {
                if let Target::Name(n, _) = &em.target {
                    targets.push(n);
                }
                for v in &em.values {
                    if let ValueTo::Value(x, _) = &v.to {
                        targets.push(x);
                    }
                }
            }
            targets.extend(terms.iter().map(|t| t.to.as_str()));
        }
        for (ti, t) in c.ast.terms.iter().enumerate() {
            let crossing = !t.means.is_empty() || t.as_term.is_some() || names(t).iter().any(|n| targets.contains(n)) || collide.contains(&(ci, ti));
            if !crossing {
                let tn = &t.name;
                diags.push(at(ci, t.pos, "W401", tr!("語「{tn}」は境界を越えません", "The term {tn} crosses no boundary")).note(tr!(
                    "用語集には、境界を越える語だけを書きます。語が境界を越えるのは、`means` で自分の公表された言語の要素を指すとき、`as` で相手の語を取り入れるとき、腐敗防止層の対応の先になるとき、上流から越えてくる同じ名前の語とぶつかるときです。",
                    "A glossary holds only the words that cross a boundary: a term that `means` an element of the context's published language, takes another's with `as`, is what an anticorruption layer maps to, or meets a word of the same name crossing from upstream."
                )));
            }
        }
    }
    diags
}
