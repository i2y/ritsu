//! The patterns, held to each other (DESIGN 1.4, 1.5, 3.4; PLAN B.6): E301 to E313 and W301.
//! Most of it is read from the `.ctx` files; E301 and E302 read the `.proto` files and, for a
//! rule's published language, the rule's Connect service as rulec says it (`Rules`), E302 and
//! E313 the owners, and E308 the bytes of the shared kernel's copies.

use crate::ast::{Pos, Role};
use crate::diag::{self, Diag, Ref};
use ritsu_base::text::Text;
use crate::model::{Model, Own, Rel, RelK};
use crate::owners::{self, Artifact};
use crate::paths;
use crate::proto::Protos;
use ritsu_base::sha256;

struct P<'a> {
    m: &'a Model,
    diags: Vec<Diag>,
}

impl P<'_> {
    fn at(&mut self, c: usize, p: Pos, code: &'static str, msg: Text) -> &mut Diag {
        let cx = &self.m.contexts[c];
        self.diags.push(diag::at(code, &cx.file, p.line, p.col, msg).source(&cx.src));
        self.diags.last_mut().unwrap()
    }

    fn name(&self, c: usize) -> &str {
        &self.m.contexts[c].name
    }

    fn rel_ref(&self, c: usize, r: &Rel) -> Ref {
        let cx = &self.m.contexts[c];
        Ref::line(Some(&cx.name), &cx.file, r.pos.line, Text::same(self.m.rel_line(c, r)))
    }
}

/// What the bytes of a kernel's entry come to: a file's digest, or a directory's, made of the
/// paths and digests of every file under it.
fn content_key(m: &Model, o: &Own) -> String {
    if !o.is_dir() {
        return sha256::hex(&std::fs::read(paths::on_disk(&m.root, &o.path)).unwrap_or_default());
    }
    let mut files = Vec::new();
    paths::walk(&m.root, &o.path, &[], &mut files);
    let mut listing = String::new();
    for f in files {
        let rel = paths::relative(&o.path, &f);
        let d = sha256::hex(&std::fs::read(paths::on_disk(&m.root, &f)).unwrap_or_default());
        listing.push_str(&format!("{rel}\0{d}\n"));
    }
    sha256::hex(listing.as_bytes())
}

fn kernel(m: &Model, a: usize, b: usize) -> Option<(&Rel, &Vec<Own>)> {
    m.rels(a, b).find_map(|r| match &r.kind {
        RelK::Kernel(items) => Some((r, items)),
        _ => None,
    })
}

pub fn check(m: &Model, ps: &Protos, arts: &[Artifact], read: &crate::suite::Read) -> Vec<Diag> {
    let mut p = P { m, diags: Vec::new() };
    let owner = |path: &str| arts.iter().find(|a| a.path == path).and_then(|a| a.ctx());
    // E301 and E302: the published languages.
    for (ci, c) in m.contexts.iter().enumerate() {
        for pl in &c.published {
            let k = &pl.package;
            for (f, at) in &pl.protos {
                match owner(f) {
                    Some(o) if o == ci => {}
                    o => {
                        let whose = match o {
                            Some(o) => tr!("「{}」のものです", "it belongs to {}", m.contexts[o].name; m.contexts[o].name),
                            None => tr!("どのコンテキストにも属しません", "it belongs to no context"),
                        };
                        let (me, sf) = (&c.name, paths::shown(f));
                        p.at(ci, *at, "E302", tr!("公表された言語 {k} の {sf} は「{me}」のものではありません（{}）", "The published language {k} lists {sf}, which is not {me}'s ({})", whose.ja; whose.en))
                            .notes
                            .push(tr!("公表された言語のファイルは、それを公表するコンテキストに属します。", "The files of a published language belong to the context that publishes it."));
                    }
                }
                if let Some(pf) = ps.files.get(f)
                    && pf.package != *k
                {
                    let got = if pf.package.is_empty() { "(none)".to_string() } else { pf.package.clone() };
                    let sf = paths::shown(f);
                    p.at(ci, *at, "E302", tr!("{sf} の package は {got} で、見出しの {k} と違います", "The package of {sf} is {got}, not {k} as the heading says"))
                        .notes
                        .push(tr!("公表された言語は proto の package を単位にします。その package を宣言するファイルだけを並べます。", "A published language is one proto package; list the files that declare that package."));
                }
            }
            if let Some((f, at)) = &pl.rulec
                && owner(f) != Some(ci)
            {
                let (me, sf) = (&c.name, paths::shown(f));
                p.at(ci, *at, "E302", tr!("公表された言語 {k} の {sf} は「{me}」のものではありません", "The published language {k} lists {sf}, which is not {me}'s"));
            }
            for (g, at) in &pl.generated {
                if owners::context_of(m, g) != Some(ci) {
                    let (me, g) = (&c.name, paths::shown(g));
                    p.at(ci, *at, "E302", tr!("公表された言語 {k} の生成したコードの置き場所 {g} は「{me}」のものではありません", "Where the code made from {k} goes, {g}, is not {me}'s"))
                        .notes
                        .push(tr!("生成したコードの置き場所は、公表するコンテキストに属するディレクトリにします。", "The code made from a published language goes in a directory of the context that publishes it."));
                }
            }
            // A rule's published language: the package and the service of its Connect, as rulec
            // says them (DESIGN 1.4), when rulec answers for the rule.
            if let Some((f, at)) = &pl.rulec
                && read.facts.contains_key(f)
            {
                let sf = paths::shown(f);
                if let Some(got) = crate::refs::rule_package(read, f)
                    && got != *k
                {
                    p.at(ci, *at, "E302", tr!("{sf} の Connect の package は {got} で、見出しの {k} と違います", "The package of the Connect of {sf} is {got}, not {k} as the heading says"))
                        .notes
                        .push(tr!("規則の公表された言語の package は、rulec が規則の別名と版から作る `rulec.<別名>.v<版>` です。", "The package of a rule's published language is the one rulec makes from the rule's alias and version, `rulec.<alias>.v<version>`."));
                }
                if let Some(svc) = crate::refs::rule_service(read, f) {
                    for (s, at) in &pl.services {
                        if *s != svc {
                            p.at(ci, *at, "E301", tr!("公開ホストサービス {s} は、{sf} の Connect のサービスではありません", "The open host service {s} is not the Connect service of {sf}"))
                                .notes
                                .push(tr!("{sf} の Connect のサービスは {svc} です。", "The Connect service of {sf} is {svc}."));
                        }
                    }
                }
            }
            if pl.rulec.is_none() {
                let services: Vec<&str> = pl.protos.iter().filter_map(|(f, _)| ps.files.get(f)).flat_map(|pf| pf.services.iter().map(|s| s.name.as_str())).collect();
                for (s, at) in &pl.services {
                    if !services.contains(&s.as_str()) && pl.protos.iter().all(|(f, _)| ps.files.contains_key(f)) {
                        let there = if services.is_empty() { tr!("その proto にサービスはありません。", "Its proto files have no service.") } else { tr!("その proto のサービスは {} です。", "Its proto files have the services {}.", services.join("、"); services.join(", ")) };
                        p.at(ci, *at, "E301", tr!("公開ホストサービス {s} が、公表された言語 {k} の proto にありません", "The open host service {s} is not in the proto files of the published language {k}")).notes.push(there);
                    }
                }
            }
        }
    }
    // The relationships.
    for (ci, c) in m.contexts.iter().enumerate() {
        for r in &c.rels {
            let pn = p.name(r.partner).to_string();
            let me = c.name.clone();
            if r.partner == ci {
                p.at(ci, r.partner_pos, "E311", tr!("自分自身との関係は書けません", "A context has no relationship with itself"));
                continue;
            }
            match &r.kind {
                RelK::Upstream { roles, through, layer, enums, terms } => {
                    let has = |x: Role| roles.iter().any(|(y, _)| *y == x);
                    if has(Role::Conformist) {
                        for (x, at) in roles {
                            if *x != Role::Conformist {
                                let w = x.word();
                                p.at(ci, *at, "E306", tr!("conformist と {w} は一緒に書けません", "conformist and {w} cannot go together")).notes.push(tr!(
                                    "順応者は上流のモデルにそのまま合わせます。要望を出して上流を動かせる顧客や、上流のモデルを読み替える腐敗防止層とは両立しません。",
                                    "A conformist takes the upstream's model as it is; that rules out a customer, who can move the upstream, and a layer that maps the upstream's model."
                                ));
                            }
                        }
                    }
                    let first_mapping: Option<Pos> = layer.first().map(|o| o.pos).or_else(|| enums.first().map(|e| e.pos)).or_else(|| terms.first().map(|t| t.pos));
                    if let Some(at) = first_mapping {
                        if has(Role::Conformist) {
                            p.at(ci, at, "E304", tr!("順応者には、対応も `layer` も書けません", "A conformist has no mapping and no `layer`")).notes.push(tr!(
                                "順応者は上流のモデルをそのまま使います。読み替えるなら、役割を anticorruption layer にします。",
                                "A conformist uses the upstream's model as it is; to map it, make the role anticorruption layer."
                            ));
                        } else if !has(Role::Acl) {
                            p.at(ci, at, "E305", tr!("腐敗防止層でない関係に、対応か `layer` があります", "A relationship that is not an anticorruption layer has a mapping or a `layer`"))
                                .notes
                                .push(tr!("対応と層を書けるのは腐敗防止層だけです。`, anticorruption layer` を役割に足します。", "Only an anticorruption layer has mappings and a layer; add `, anticorruption layer` to the role."));
                        }
                    }
                    let published: Vec<&str> = m.contexts[r.partner].published.iter().map(|x| x.package.as_str()).collect();
                    for (pkg, at) in through {
                        if !published.contains(&pkg.as_str()) {
                            let list = if published.is_empty() {
                                tr!("「{pn}」は何も公表していません。", "{pn} publishes nothing.")
                            } else {
                                tr!("「{pn}」が公表しているのは {} です。", "{pn} publishes {}.", published.join("、"); published.join(", "))
                            };
                            let d = p.at(ci, *at, "E312", tr!("「{pn}」は package {pkg} を公表していません", "{pn} does not publish the package {pkg}"));
                            d.notes.push(list);
                            if let Some(o) = m.contexts.iter().find(|x| x.published.iter().any(|y| y.package == *pkg)) {
                                let on = &o.name;
                                d.notes.push(tr!("{pkg} を公表しているのは「{on}」です。", "The package {pkg} is published by {on}."));
                            }
                        }
                    }
                    for o in layer {
                        let whose = owners::context_of(m, &o.path);
                        if whose != Some(ci) {
                            let t = o.text();
                            let w = match whose {
                                Some(x) => tr!("「{}」のものです", "it is {}'s", m.contexts[x].name; m.contexts[x].name),
                                None => tr!("どのコンテキストにも属しません", "it belongs to no context"),
                            };
                            p.at(ci, o.pos, "E313", tr!("腐敗防止層の {t} は「{me}」のものではありません（{}）", "The layer's {t} is not {me}'s ({})", w.ja; w.en))
                                .notes
                                .push(tr!("腐敗防止層は下流の側に置きます。層は、下流のコンテキストに属するディレクトリかファイルです。", "An anticorruption layer is on the downstream's side: a directory or file of the downstream."));
                        }
                    }
                    if has(Role::Customer) && !m.writes(r.partner, ci, |k| matches!(k, RelK::Downstream)) {
                        let d = p.at(ci, r.pos, "E303", tr!("「{me}」は「{pn}」の顧客だと書いていますが、「{pn}」の側に `downstream {me} supplier` がありません", "{me} says it is {pn}'s customer, and {pn} has no `downstream {me} supplier`"));
                        d.notes.push(tr!("顧客／供給者は二つのチームの合意なので、両方のファイルに書きます。", "Customer and supplier is an agreement of two teams, written in both files."));
                        d.fix = Some(ritsu_base::diag::Fix::Line(format!("downstream {me} supplier")));
                    }
                }
                RelK::Downstream => {
                    let up = m.upstream(r.partner, ci);
                    if !up.is_some_and(|u| u.has(Role::Customer)) {
                        let d = p.at(ci, r.pos, "E303", tr!("「{me}」は「{pn}」の供給者だと書いていますが、「{pn}」の側に `upstream {me} customer` がありません", "{me} says it is {pn}'s supplier, and {pn} has no `upstream {me} customer`"));
                        if let Some(u) = up {
                            let roles: Vec<&str> = u.roles().iter().map(|x| x.word()).collect();
                            let rs = roles.join(", ");
                            d.notes.push(tr!("「{pn}」は `upstream {me} {rs}` と書いています。", "{pn} writes `upstream {me} {rs}`."));
                        }
                        d.notes.push(tr!("顧客／供給者は二つのチームの合意なので、両方のファイルに書きます。", "Customer and supplier is an agreement of two teams, written in both files."));
                    }
                }
                RelK::Partnership => {
                    if !m.writes(r.partner, ci, |k| matches!(k, RelK::Partnership)) {
                        let d = p.at(ci, r.pos, "E309", tr!("パートナーシップが「{me}」の側にしか書かれていません", "The partnership is written on {me}'s side only"));
                        d.notes.push(tr!("パートナーシップは二つのチームの合意なので、「{pn}」のファイルにも `partnership with {me}` を書きます。", "A partnership is an agreement of two teams; write `partnership with {me}` in {pn}'s file too."));
                        d.fix = Some(ritsu_base::diag::Fix::Line(format!("partnership with {me}")));
                    }
                }
                RelK::Separate => {
                    let others: Vec<&Rel> = m.rels(ci, r.partner).chain(m.rels(r.partner, ci)).filter(|x| !matches!(x.kind, RelK::Separate)).collect();
                    if let Some(o) = others.first() {
                        let w = o.kind.words();
                        p.at(ci, r.pos, "E310", tr!("「{me}」と「{pn}」は別々の道なのに、`{w}` もあります", "{me} and {pn} go separate ways, and also have `{w}` between them"))
                            .notes
                            .push(tr!("別々の道は、関係を持たないという決定です。ほかの関係とは両立しません。", "Separate ways is the decision to have no relationship; it cannot go with another."));
                    }
                }
                RelK::Kernel(_) => {}
            }
        }
    }
    // E307 and E308: the shared kernels, once a pair.
    for a in 0..m.contexts.len() {
        for b in 0..m.contexts.len() {
            if a == b {
                continue;
            }
            let (ka, kb) = (kernel(m, a, b), kernel(m, b, a));
            match (ka, kb) {
                (Some((ra, _)), None) => {
                    let (me, pn) = (p.name(a).to_string(), p.name(b).to_string());
                    let r = p.rel_ref(a, ra);
                    let d = p.at(a, ra.pos, "E307", tr!("共有カーネルが「{me}」の側にしか書かれていません", "The shared kernel is written on {me}'s side only"));
                    d.notes.push(tr!(
                        "共有カーネルは二つのチームが一緒に持つものなので、「{pn}」のファイルにも `shared kernel with {me}` を書き、同じものを並べます。",
                        "A shared kernel is held by two teams together; write `shared kernel with {me}` in {pn}'s file too, with the same entries."
                    ));
                    d.extra.0.push(r);
                }
                (Some((ra, la)), Some((rb, lb))) if a < b => {
                    let pa: Vec<&str> = la.iter().map(|o| o.path.as_str()).collect();
                    let pb: Vec<&str> = lb.iter().map(|o| o.path.as_str()).collect();
                    let mut sa = pa.clone();
                    sa.sort();
                    let mut sb = pb.clone();
                    sb.sort();
                    if sa == sb {
                        for o in la {
                            let whose = owners::context_of(m, &o.path);
                            if whose != Some(a) && whose != Some(b) {
                                let t = o.text();
                                let (na, nb) = (p.name(a).to_string(), p.name(b).to_string());
                                let w = match whose {
                                    Some(x) => tr!("「{}」のものです", "it is {}'s", m.contexts[x].name; m.contexts[x].name),
                                    None => tr!("どのコンテキストにも属しません", "it belongs to no context"),
                                };
                                p.at(a, o.pos, "E307", tr!("共有カーネルの {t} は、「{na}」と「{nb}」のどちらのものでもありません（{}）", "The shared kernel's {t} is neither {na}'s nor {nb}'s ({})", w.ja; w.en))
                                    .notes
                                    .push(tr!("共有カーネルに並べるものは、二つのコンテキストのどちらかに属します。", "What a shared kernel lists belongs to one of its two contexts."));
                            }
                        }
                        continue;
                    }
                    let copies = la.len() == lb.len() && la.iter().all(|o| owners::context_of(m, &o.path) == Some(a)) && lb.iter().all(|o| owners::context_of(m, &o.path) == Some(b));
                    let (na, nb) = (p.name(a).to_string(), p.name(b).to_string());
                    if copies {
                        let mut ka: Vec<(String, &Own)> = la.iter().map(|o| (content_key(m, o), o)).collect();
                        let mut kb: Vec<(String, &Own)> = lb.iter().map(|o| (content_key(m, o), o)).collect();
                        let mut alone_a = Vec::new();
                        for (k, o) in ka.drain(..) {
                            match kb.iter().position(|(x, _)| *x == k) {
                                Some(i) => {
                                    kb.remove(i);
                                }
                                None => alone_a.push(o),
                            }
                        }
                        if !alone_a.is_empty() {
                            let ra_ref = p.rel_ref(a, ra);
                            let rb_ref = p.rel_ref(b, rb);
                            let xs: Vec<String> = alone_a.iter().map(|o| o.text()).collect();
                            let ys: Vec<String> = kb.iter().map(|(_, o)| o.text()).collect();
                            let d = p.at(b, rb.pos, "E308", tr!("「{na}」と「{nb}」の共有カーネルの写しの中身が違います", "The copies of the shared kernel of {na} and {nb} differ"));
                            d.notes.push(tr!(
                                "「{na}」の {} と同じ中身の写しが、「{nb}」の側にありません（「{nb}」の側は {}）。",
                                "No copy on {nb}'s side has the bytes of {na}'s {} ({nb} has {}).",
                                xs.join("、"), ys.join("、"); xs.join(", "), ys.join(", ")
                            ));
                            d.notes.push(tr!("写しを両側に置くときは、中身を同じに保ちます。", "When each side keeps a copy, the copies are kept the same."));
                            d.extra.0.push(ra_ref);
                            d.extra.0.push(rb_ref);
                        }
                        continue;
                    }
                    let only_a: Vec<String> = la.iter().filter(|o| !pb.contains(&o.path.as_str())).map(|o| o.text()).collect();
                    let only_b: Vec<String> = lb.iter().filter(|o| !pa.contains(&o.path.as_str())).map(|o| o.text()).collect();
                    let ra_ref = p.rel_ref(a, ra);
                    let rb_ref = p.rel_ref(b, rb);
                    let d = p.at(b, rb.pos, "E307", tr!("「{na}」と「{nb}」の共有カーネルの並びが、両側で違います", "The shared kernel of {na} and {nb} lists different things on each side"));
                    if !only_a.is_empty() {
                        d.notes.push(tr!("「{na}」の側にだけあるもの: {}", "only on {na}'s side: {}", only_a.join("、"); only_a.join(", ")));
                    }
                    if !only_b.is_empty() {
                        d.notes.push(tr!("「{nb}」の側にだけあるもの: {}", "only on {nb}'s side: {}", only_b.join("、"); only_b.join(", ")));
                    }
                    d.extra.0.push(ra_ref);
                    d.extra.0.push(rb_ref);
                }
                _ => {}
            }
        }
    }
    // W301: the upstream direction going round.
    let n = m.contexts.len();
    let edges: Vec<Vec<usize>> = (0..n).map(|d| m.contexts[d].rels.iter().filter(|r| matches!(r.kind, RelK::Upstream { .. }) && r.partner != d).map(|r| r.partner).collect()).collect();
    for s in 0..n {
        // A path back to s through contexts after it in the map: each circle is told once, from
        // the first of its contexts.
        let mut stack: Vec<(usize, Vec<usize>)> = vec![(s, vec![s])];
        let mut found: Option<Vec<usize>> = None;
        let mut seen = vec![false; n];
        while let Some((x, path)) = stack.pop() {
            for &y in edges[x].iter().rev() {
                if y == s && path.len() > 1 {
                    found = Some(path.clone());
                    break;
                }
                if y > s && !seen[y] && !path.contains(&y) {
                    seen[y] = true;
                    let mut np = path.clone();
                    np.push(y);
                    stack.push((y, np));
                }
            }
            if found.is_some() {
                break;
            }
        }
        if let Some(cycle) = found {
            let names: Vec<&str> = cycle.iter().chain(std::iter::once(&s)).map(|&i| m.contexts[i].name.as_str()).collect();
            let shown = names.join(" → ");
            let r = m.upstream(s, cycle[1]).unwrap().clone();
            let mut refs = Vec::new();
            for w in cycle.windows(2).chain(std::iter::once(&[cycle[cycle.len() - 1], s][..])) {
                if let Some(u) = m.upstream(w[0], w[1]) {
                    refs.push(p.rel_ref(w[0], u));
                }
            }
            let d = p.at(s, r.pos, "W301", tr!("上流をたどると、元のコンテキストに戻ります: {shown}", "Following the upstreams comes back where it started: {shown}"));
            d.notes.push(tr!(
                "互いに上流のコンテキストは、どちらも相手の変更に引きずられます。向きを一つにそろえるか、パートナーシップにすることを考えます。",
                "Contexts upstream of each other are each dragged along by the other's changes; consider one direction, or a partnership."
            ));
            d.extra.0 = refs;
        }
    }
    p.diags
}
