//! The references that cross a boundary (DESIGN 3.3; PLAN B.7). In stage B they are the
//! imports of the `.proto` files; stage C adds what `rulec api` and `koyomi api` say. A crossing
//! is allowed by a shared kernel both sides write, or by a relationship that lets the one side
//! reach the other's published language (DESIGN 1.5's table); anything else is E201 to E206, at
//! the line of the import.

use crate::ast::Role;
use crate::diag::{self, Diag, DiagExt, Ref};
use ritsu_base::text::Text;
use crate::model::{Model, RelK};
use crate::naming::{Name, Tool};
use crate::owners::Artifact;
use crate::paths::shown;
use crate::proto::{Protos, Resolved, Symbol, Type};
use std::collections::BTreeSet;

/// What allows a crossing.
#[derive(Clone, Debug, PartialEq)]
pub enum Allowed {
    /// The shared kernel of the two; the context whose declaration is cited.
    Kernel(usize),
    /// The `upstream` relationship of the referring context: its index in `Ctx::rels`.
    Upstream(usize),
    Partnership,
}

/// One reference from an artifact of one context to an artifact of another.
#[derive(Clone, Debug, PartialEq)]
pub struct Crossing {
    pub from: String,
    pub from_ctx: usize,
    pub to: String,
    pub to_ctx: usize,
    pub line: usize,
    pub col: usize,
    /// The import, as written.
    pub import: String,
    /// The messages and enums of the target the source names.
    pub uses: Vec<Symbol>,
    /// Those and everything they reach through fields (DESIGN 3.3).
    pub reach: Vec<Symbol>,
    pub allowed: Option<Allowed>,
}

impl Crossing {
    pub fn tool(&self) -> Tool {
        Tool::Proto
    }

    pub fn from_name(&self) -> Name {
        Name::file(Tool::Proto, self.from.clone())
    }

    pub fn to_name(&self) -> Name {
        Name::file(Tool::Proto, self.to.clone())
    }
}

/// The files whose types an import of `file` lets one name: it, and what it passes on with
/// `import public`.
fn through_import(ps: &Protos, file: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut todo = vec![file.to_string()];
    while let Some(f) = todo.pop() {
        if !out.insert(f.clone()) {
            continue;
        }
        if let (Some(pf), Some(at)) = (ps.files.get(&f), ps.imports.get(&f)) {
            for (imp, a) in pf.imports.iter().zip(at) {
                if imp.public
                    && let Some(a) = a
                {
                    todo.push(a.clone());
                }
            }
        }
    }
    out
}

/// Every type a file names, resolved, in the order it names them.
fn named_types(ps: &Protos, file: &str) -> Vec<Symbol> {
    let f = &ps.files[file];
    let mut out: Vec<Symbol> = Vec::new();
    let mut add = |r: Resolved| {
        if let Resolved::Found(n) = r
            && !out.contains(&n)
        {
            out.push(n);
        }
    };
    for m in &f.messages {
        let scope = f.full(&m.name);
        for fl in &m.fields {
            let n = match &fl.ty {
                Type::Named(n) => Some(n),
                Type::Map(_, v) => match v.as_ref() {
                    Type::Named(n) => Some(n),
                    _ => None,
                },
                Type::Scalar(_) => None,
            };
            if let Some(n) = n {
                add(ps.resolve(file, &scope, n));
            }
        }
    }
    for s in &f.services {
        for m in &s.methods {
            add(ps.resolve(file, &f.package, &m.input));
            add(ps.resolve(file, &f.package, &m.output));
        }
    }
    out
}

/// The crossings of the `.proto` imports, in the order of the files and their imports.
pub fn proto_crossings(ps: &Protos, arts: &[Artifact]) -> Vec<Crossing> {
    let owner = |p: &str| arts.iter().find(|a| a.path == p).and_then(|a| a.ctx());
    let mut out = Vec::new();
    for (file, f) in &ps.files {
        let Some(x) = owner(file) else { continue };
        let named = named_types(ps, file);
        for (imp, at) in f.imports.iter().zip(ps.imports.get(file).into_iter().flatten()) {
            let Some(q) = at else { continue };
            let Some(y) = owner(q) else { continue };
            if x == y {
                continue;
            }
            let via = through_import(ps, q);
            let uses: Vec<Symbol> = named.iter().filter(|n| via.contains(&n.file)).cloned().collect();
            let reach = ps.reach(&uses);
            out.push(Crossing { from: file.clone(), from_ctx: x, to: q.clone(), to_ctx: y, line: imp.line, col: imp.col, import: imp.path.clone(), uses, reach, allowed: None });
        }
    }
    out
}

/// The published package of the context's that holds the file, if any.
pub fn published_package(m: &Model, c: usize, file: &str) -> Option<String> {
    m.contexts[c].published.iter().find(|p| p.protos.iter().any(|(f, _)| f == file)).map(|p| p.package.clone())
}

/// Whether both sides write a shared kernel toward each other that holds `file`.
fn kernel_holds(m: &Model, a: usize, b: usize, file: &str) -> bool {
    let holds = |x: usize, y: usize| m.rels(x, y).any(|r| matches!(&r.kind, RelK::Kernel(items) if items.iter().any(|o| o.holds(file))));
    holds(a, b) && holds(b, a)
}

fn one_sided_kernel(m: &Model, a: usize, b: usize) -> Option<usize> {
    let w = |x: usize, y: usize| m.writes(x, y, |k| matches!(k, RelK::Kernel(_)));
    match (w(a, b), w(b, a)) {
        (true, false) => Some(a),
        (false, true) => Some(b),
        _ => None,
    }
}

/// Hold every crossing to the map (DESIGN 3.3): fill `allowed`, and say what is not allowed.
pub fn check(m: &Model, crossings: &mut [Crossing]) -> Vec<Diag> {
    let mut diags = Vec::new();
    for c in crossings.iter_mut() {
        let (x, y) = (c.from_ctx, c.to_ctx);
        let (xn, yn) = (m.contexts[x].name.clone(), m.contexts[y].name.clone());
        let src = std::fs::read_to_string(crate::paths::on_disk(&m.root, &c.from)).unwrap_or_default();
        let (p, q, imp) = (c.from.clone(), c.to.clone(), c.import.clone());
        let (sp, sq) = (shown(&p), shown(&q));
        let from_ref = Ref::line(Some(&xn), &p, c.line, Text::same(format!("import \"{imp}\""))).via("proto import");
        let pkg = published_package(m, y, &q);
        let to_what = match &pkg {
            Some(k) => tr!("公表された言語 {k} のファイル", "a file of the published language {k}"),
            None => tr!("「{yn}」の内側のファイル", "a file inside {yn}"),
        };
        let to_ref = Ref::name(Some(&yn), c.to_name(), to_what);
        let diag = |code: &'static str, msg: Text| diag::at(code, &p, c.line, c.col, msg).source(&src).refer(from_ref.clone()).refer(to_ref.clone());
        let separate = m.writes(x, y, |k| matches!(k, RelK::Separate)) || m.writes(y, x, |k| matches!(k, RelK::Separate));
        if separate {
            diags.push(diag("E206", tr!("「{xn}」の {sp} が、別々の道の相手「{yn}」の {sq} を import しています", "The file {sp} of {xn} imports {sq} of {yn}, and the two go separate ways")).note(tr!(
                "別々の道は、二つのあいだに何の参照も持たないという決定です。参照が要るなら、別々の道をやめて関係を書きます。",
                "Separate ways is the decision that nothing between the two refers to the other; if the reference is needed, write a relationship instead."
            )));
            continue;
        }
        if kernel_holds(m, x, y, &q) {
            c.allowed = Some(Allowed::Kernel(x));
            continue;
        }
        let up = m.contexts[x].rels.iter().position(|r| r.partner == y && matches!(r.kind, RelK::Upstream { .. }));
        let partners = m.writes(x, y, |k| matches!(k, RelK::Partnership)) && m.writes(y, x, |k| matches!(k, RelK::Partnership));
        let kernel_note = one_sided_kernel(m, x, y).map(|s| {
            let sn = &m.contexts[s].name;
            tr!("二つのあいだの共有カーネルは、「{sn}」の側にしか書かれていません（E307 も出ています）。", "The shared kernel of the two is written on {sn}'s side only (E307 is told too).")
        });
        if up.is_none() && !partners {
            let mut d = diag("E201", tr!("「{xn}」の {sp} が、関係の無い「{yn}」の {sq} を import しています", "The file {sp} of {xn} imports {sq} of {yn}, which {xn} has no relationship with"));
            if m.upstream(y, x).is_some() {
                d = d.note(tr!(
                    "「{yn}」が「{xn}」の下流で、参照の向きが逆です。上流と下流の関係が許すのは、下流から上流への参照です。",
                    "{yn} is downstream of {xn}, the other way round: an upstream relationship lets the downstream refer to the upstream."
                ));
            }
            if m.writes(x, y, |k| matches!(k, RelK::Partnership)) != m.writes(y, x, |k| matches!(k, RelK::Partnership)) {
                d = d.note(tr!("二つのあいだのパートナーシップは、片側にしか書かれていません（E309 も出ています）。", "The partnership of the two is written on one side only (E309 is told too)."));
            }
            if let Some(n) = &kernel_note {
                d = d.note(n.clone());
            }
            d = d.note(tr!(
                "境界を越えて参照するには、上流と下流（下流が `upstream` を書く）、パートナーシップ、共有カーネルのどれかの関係が要ります。",
                "A reference across the boundary needs a relationship: upstream and downstream (the downstream writes `upstream`), a partnership, or a shared kernel."
            ));
            diags.push(d);
            continue;
        }
        let Some(k) = pkg else {
            let mut d = diag("E202", tr!("「{xn}」の {sp} が、「{yn}」の内側の {sq} を import しています", "The file {sp} of {xn} imports {sq}, which is inside {yn}")).note(tr!(
                "{sq} は「{yn}」の公表された言語に入っていません。境界の向こうから参照できるのは、公表された言語と共有カーネルだけです。",
                "The file {sq} is in no published language of {yn}; across a boundary only the published languages and a shared kernel can be referred to."
            ));
            if let Some(n) = kernel_note {
                d = d.note(n);
            }
            diags.push(d);
            continue;
        };
        if partners {
            c.allowed = Some(Allowed::Partnership);
            continue;
        }
        let ri = up.unwrap();
        let r = &m.contexts[x].rels[ri];
        let RelK::Upstream { through, layer, .. } = &r.kind else { unreachable!() };
        let rel_ref = Ref::line(Some(&xn), &m.contexts[x].file, r.pos.line, Text::same(m.rel_line(x, r)));
        if !through.iter().any(|(t, _)| *t == k) {
            let now: Vec<&str> = through.iter().map(|(t, _)| t.as_str()).collect();
            let fix = format!("through {}, {k}", now.join(", "));
            diags.push(diag("E203", tr!("「{xn}」が、`through` に無い package {k} を通って「{yn}」を参照しています", "{xn} refers to {yn} through the package {k}, which its `through` does not list")).note(tr!(
                "上流と下流の関係は、下流が通ってよい上流の公表された言語を `through` に並べます。",
                "An upstream relationship lists under `through` the upstream's published languages the downstream may go through."
            )).fix_line(fix).refer(rel_ref));
            continue;
        }
        if r.has(Role::Acl) {
            if let Some(own) = published_package(m, x, &p) {
                let used: Vec<String> = c.uses.iter().map(|s| s.full.clone()).collect();
                let used = if used.is_empty() { k.clone() } else { used.join(", ") };
                diags.push(diag("E205", tr!("「{xn}」の公表された言語 {own} に、上流「{yn}」の型が出ています", "The published language {own} of {xn} shows the upstream {yn}'s types")).note(tr!(
                    "{sp} は {used} を使っています。腐敗防止層の下流は、上流のモデルを自分の公表された言語に出しません。層の中で自分の型に読み替えます。",
                    "The file {sp} uses {used}. Downstream of an anticorruption layer, the upstream's model stays out of the downstream's own published language; the layer maps it to the downstream's types."
                )).refer(rel_ref));
                continue;
            }
            if !layer.is_empty() && !layer.iter().any(|o| o.holds(&p)) {
                let ls: Vec<String> = layer.iter().map(|o| o.text()).collect();
                diags.push(diag("E204", tr!("腐敗防止層の外から、「{yn}」の公表された言語 {k} を参照しています", "The file {sp} refers to {yn}'s published language {k} from outside the anticorruption layer")).note(tr!(
                    "「{xn}」は `layer` を書いているので、上流の公表された言語を参照できるのは層（{}）の中だけです。",
                    "{xn} writes a `layer`, so only the layer ({}) may refer to the upstream's published language.",
                    ls.join("、"); ls.join(", ")
                )).refer(rel_ref));
                continue;
            }
        }
        c.allowed = Some(Allowed::Upstream(ri));
    }
    diags
}
