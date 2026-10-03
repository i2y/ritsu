//! The mappings of an anticorruption layer, held to the upstream's enums (DESIGN 1.7, 3.5;
//! PLAN B.8): every value of the upstream enum has a value or `refuse` (E401), no value is
//! there that the enum lacks (E402), a proto target has the values on the right (E403), and an
//! upstream enum the downstream's artifacts refer to has a mapping (E404). The value 0 that says
//! nothing is set needs none (W402). A rule as the target is read from stage C.

use crate::ast::{Role, Target, ValueTo};
use crate::diag::{Diag, Ref};
use crate::elements::{At, Elements};
use crate::i18n::Text;
use crate::model::{Model, RelK};
use crate::naming::{Name, Tool};
use crate::proto::{self, Protos};
use crate::refs::{Allowed, Crossing};

/// The proto enum a name points at, with its full name.
fn enum_of<'a>(ps: &'a Protos, n: &Name) -> Option<(&'a proto::Enum, String, &'a proto::ProtoFile)> {
    if n.tool != Tool::Proto {
        return None;
    }
    let f = ps.files.get(&n.path)?;
    let (k, e) = n.items.first()?;
    if k != "enum" {
        return None;
    }
    Some((f.enumeration(e)?, f.full(e), f))
}

pub fn check(m: &Model, ps: &Protos, el: &Elements, crossings: &[Crossing]) -> Vec<Diag> {
    let mut diags = Vec::new();
    for (ci, c) in m.contexts.iter().enumerate() {
        for (ri, r) in c.rels.iter().enumerate() {
            let RelK::Upstream { enums, .. } = &r.kind else { continue };
            if !r.has(Role::Acl) || r.has(Role::Conformist) {
                continue;
            }
            let (xn, yn) = (c.name.clone(), m.contexts[r.partner].name.clone());
            let rel_ref = Ref::line(Some(&xn), &c.file, r.pos.line, Text::same(m.rel_line(ci, r)));
            for (ei, em) in enums.iter().enumerate() {
                let Some(from) = el.get(At::From(ci, ri, ei)) else { continue };
                let Some((e, full, f)) = enum_of(ps, from) else { continue };
                // A rule's enum as the target: its values are read from `rulec api` (stage C).
                let target = match &em.target {
                    Target::Name(..) => None,
                    Target::Element(_) => match el.get(At::To(ci, ri, ei)) {
                        Some(n) if n.tool == Tool::Rulec => continue,
                        Some(n) => enum_of(ps, n),
                        None => continue,
                    },
                };
                let written: Vec<&str> = em.values.iter().map(|v| v.from.as_str()).collect();
                let mut missing = Vec::new();
                for v in &e.values {
                    let w = written.contains(&v.name.as_str());
                    if proto::is_unset(e, v) {
                        if w {
                            let vm = em.values.iter().find(|x| x.from == v.name).unwrap();
                            let vn = &v.name;
                            diags.push(
                                Diag::at("W402", &c.file, vm.from_pos.line, vm.from_pos.col, tr!("値が無いことを表す 0 番の値 {vn} に、対応は要りません", "The value 0, {vn}, says nothing is set and needs no mapping"))
                                    .source(&c.src)
                                    .note(tr!(
                                        "0 番の値で、名前から列挙の接頭辞を外すと unspecified になるものは、値が設定されていないことを表す印です。rulec の `import proto` と dandori の proto から作る型も、同じ値を外します。",
                                        "A value 0 whose name, without the enum's prefix, is unspecified marks that nothing is set; rulec's `import proto` and dandori's types from a .proto leave it out the same way."
                                    )),
                            );
                        }
                    } else if !w {
                        missing.push(v);
                    }
                }
                if !missing.is_empty() {
                    let names: Vec<&str> = missing.iter().map(|v| v.name.as_str()).collect();
                    let (ja, en) = (names.join("、"), names.join(", "));
                    let mut d = Diag::at("E401", &c.file, em.pos.line, em.pos.col, tr!("「{xn}」の腐敗防止層の対応に、「{yn}」の列挙 {full} の値 {ja} がありません", "The anticorruption layer of {xn} maps no value for {en} of {yn}'s enum {full}")).source(&c.src);
                    for v in missing.iter().take(3) {
                        let (vn, fp, l) = (&v.name, crate::paths::shown(&f.path), v.line);
                        d = d.note(tr!("{vn} は {fp}:{l} の値です。", "{vn} is the value at {fp}:{l}."));
                    }
                    d = d.note(tr!(
                        "上流の列挙の値ごとに、下流の値か refuse（断る）を書きます。上流が値を足すと、その値をどう扱うかを決めるまで、検査は通りません。",
                        "Every value of the upstream enum gets a value of the downstream or refuse; when the upstream adds a value, the check fails until someone decides what it becomes."
                    ));
                    let first = names[0];
                    d = d.fix(format!("{first} -> refuse \"…\""));
                    let counted = e.values.iter().filter(|v| !proto::is_unset(e, v)).count();
                    let k = missing.len();
                    d = d.with(rel_ref.clone()).with(Ref::name(Some(&yn), from.clone(), tr!("値は {counted} 個で、対応が無いのは {k} 個", "{counted} values, {k} of them unmapped")));
                    diags.push(d);
                }
                for v in &em.values {
                    if !e.values.iter().any(|x| x.name == v.from) {
                        let vn = &v.from;
                        let have: Vec<&str> = e.values.iter().map(|x| x.name.as_str()).collect();
                        diags.push(
                            Diag::at("E402", &c.file, v.from_pos.line, v.from_pos.col, tr!("対応の {vn} は、列挙 {full} にありません", "The mapping names {vn}, which is not a value of the enum {full}"))
                                .source(&c.src)
                                .note(tr!("{full} の値は {} です。", "The values of {full} are {}.", have.join("、"); have.join(", "))),
                        );
                    }
                    if let (Some((te, tfull, _)), ValueTo::Value(x, at)) = (&target, &v.to)
                        && !te.values.iter().any(|y| y.name == *x)
                    {
                        let have: Vec<&str> = te.values.iter().map(|y| y.name.as_str()).collect();
                        diags.push(
                            Diag::at("E403", &c.file, at.line, at.col, tr!("対応の先の {x} は、列挙 {tfull} にありません", "The mapping maps to {x}, which is not a value of the target enum {tfull}"))
                                .source(&c.src)
                                .note(tr!("{tfull} の値は {} です。", "The values of {tfull} are {}.", have.join("、"); have.join(", "))),
                        );
                    }
                }
            }
            // E404: an upstream enum the downstream's artifacts refer to, with no mapping.
            let mapped: Vec<String> = (0..enums.len()).filter_map(|ei| el.get(At::From(ci, ri, ei))).filter_map(|n| enum_of(ps, n).map(|(_, full, _)| full)).collect();
            let mut said: Vec<String> = Vec::new();
            for cr in crossings.iter().filter(|cr| cr.from_ctx == ci && cr.to_ctx == r.partner && cr.allowed == Some(Allowed::Upstream(ri))) {
                for sym in &cr.reach {
                    let full = &sym.full;
                    if !sym.is_enum || mapped.contains(full) || said.contains(full) {
                        continue;
                    }
                    let owner_is_up = m.contexts[r.partner].published.iter().any(|p| p.protos.iter().any(|(f, _)| *f == sym.file));
                    if !owner_is_up {
                        continue;
                    }
                    said.push(full.clone());
                    let (p, l, imp) = (&cr.from, cr.line, &cr.import);
                    let short = sym.name.clone();
                    diags.push(
                        Diag::at("E404", &c.file, r.pos.line, r.pos.col, tr!("「{xn}」は「{yn}」の列挙 {full} を参照していますが、腐敗防止層に対応がありません", "{xn} refers to {yn}'s enum {full}, and its anticorruption layer has no mapping for it"))
                            .source(&c.src)
                            .note(tr!(
                                "腐敗防止層の下流は、参照している上流の列挙を、値ごとに自分の値か refuse に読み替えます。",
                                "Downstream of an anticorruption layer, every upstream enum referred to is mapped, value by value, to the downstream's values or refuse."
                            ))
                            .fix(format!("enum {short} -> <…>"))
                            .with(Ref::line(Some(&xn), p, l, Text::same(format!("import \"{imp}\""))).via("proto import"))
                            .with(Ref::name(Some(&yn), Name::file(Tool::Proto, sym.file.clone()).with("enum", short.clone()), Text::default())),
                    );
                }
            }
        }
    }
    diags
}
