//! The references that cross a boundary (DESIGN 3.3; PLAN B.7, C.2–C.4). They are the imports of
//! the `.proto` files, and what the other languages say their artifacts name: a rule's `import
//! proto` and `shape` (rulec's `Rules` and `References`), a calendar's `use calendar` (koyomi's
//! `References`), and a workflow's rules, APIs, methods and child workflows (dandori's
//! `References`), and the dependencies of Rust's crates on each other (the lines of their
//! `Cargo.toml`, as Cargo says them; DESIGN 7.7). A crossing is allowed by a shared kernel both
//! sides write, or by a relationship that lets the one side reach the other's published language
//! (DESIGN 1.5's table); anything else is E201 to E206, or for a workflow E207 to E209, at the
//! line of the reference.

use crate::ast::Role;
use crate::diag::{self, Diag, DiagExt, Ref};
use ritsu_base::text::Text;
use crate::model::{Model, RelK};
use crate::naming::{Name, Tool};
use crate::owners::Artifact;
use crate::paths::shown;
use crate::proto::{Protos, Resolved, Symbol, Type};
use crate::suite::Read;
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

/// How an artifact refers to another (DESIGN 3.3's table).
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// A `.proto`'s `import`.
    ProtoImport,
    /// A rule's `import proto`: an enum of a `.proto`, with its values.
    RuleImportProto,
    /// A rule's `shape`: a message of a `.proto`, and what it reaches.
    RuleShape,
    /// A rule's `apply`: another rule, expanded into this one when rulec checks it.
    RuleApply,
    /// A dates file's or a calendar's `use calendar`.
    CalendarUse,
    /// A workflow's `use rule`, in the words dandori says it with (`use rule`, `use rule …
    /// connect`, `use rule … lambda, local`): called as the rule's Connect service when `connect`
    /// is among them, else the rule's own code (bundled, or deployed as a function).
    FlowRule { connect: bool, how: String },
    /// A workflow's `use proto`: the `.proto` its tasks call the services of.
    FlowProto,
    /// A task's `connect`: a method of a service of a `.proto`.
    FlowConnect,
    /// A task's child `flow`.
    FlowChild,
    /// A Rust crate's dependency on another crate by its path, in its manifest's
    /// `[dependencies]` or `[build-dependencies]` (the table's name).
    Crate { table: &'static str },
}

impl Kind {
    /// The words a reference is read in: what `api` writes under `via`.
    pub fn via(&self) -> String {
        match self {
            Kind::ProtoImport => "proto import".into(),
            Kind::RuleImportProto => "import proto".into(),
            Kind::RuleShape => "shape".into(),
            Kind::RuleApply => "apply".into(),
            Kind::CalendarUse => "use calendar".into(),
            Kind::FlowRule { how, .. } => how.clone(),
            Kind::FlowProto => "use proto".into(),
            Kind::FlowConnect => "connect".into(),
            Kind::FlowChild => "flow".into(),
            Kind::Crate { table } => table.to_string(),
        }
    }
}

/// One reference from an artifact of one context to an artifact of another.
#[derive(Clone, Debug, PartialEq)]
pub struct Crossing {
    pub from: String,
    pub from_tool: Tool,
    pub from_ctx: usize,
    /// The file referred to.
    pub to: String,
    pub to_ctx: usize,
    /// What is referred to: the file, or a thing in it (an enum, a message, a service's method).
    pub target: Name,
    pub kind: Kind,
    pub line: usize,
    pub col: usize,
    /// The import as written (`.proto`), or the reference as its language says it.
    pub import: String,
    /// The messages and enums of the target the source names.
    pub uses: Vec<Symbol>,
    /// Those and everything they reach through fields (DESIGN 3.3).
    pub reach: Vec<Symbol>,
    pub allowed: Option<Allowed>,
}

impl Crossing {
    /// The tool of the referring artifact.
    pub fn tool(&self) -> Tool {
        self.from_tool
    }

    pub fn from_name(&self) -> Name {
        Name::file(self.from_tool, self.from.clone())
    }

    pub fn to_name(&self) -> Name {
        self.target.clone()
    }

    /// The line of the reference, as a diagnostic says what is involved.
    pub fn from_ref(&self, context: &str) -> Ref {
        let what = match self.kind {
            Kind::ProtoImport => Text::same(format!("import \"{}\"", self.import)),
            _ => Text::same(self.import.clone()),
        };
        Ref::line(Some(context), &self.from, self.line, what).via(&self.kind.via())
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
            out.push(Crossing { from: file.clone(), from_tool: Tool::Proto, from_ctx: x, to: q.clone(), to_ctx: y, target: Name::file(Tool::Proto, q.clone()), kind: Kind::ProtoImport, line: imp.line, col: imp.col, import: imp.path.clone(), uses, reach, allowed: None });
        }
    }
    out
}

/// What the other languages say their artifacts name, as crossings (DESIGN 3.3): a rule's `import
/// proto` (the enum, its values among what crosses), `shape` (the message, and what it reaches)
/// and `apply` (another rule), a calendar's `use calendar`, and a workflow's rules, `.proto`
/// files, methods and child workflows. A reference to a file outside the map's scope is E103.
/// What no context owns is part of the artifact that reads it (DESIGN 1.3): a JSON Schema, a
/// source's copy, a table of holidays, an OpenAPI or Smithy description. A workflow's `implements`
/// is no crossing: it is held to the workflow's own published language (E208) by [`implements`].
pub fn suite_crossings(m: &Model, ps: &Protos, arts: &[Artifact], read: &Read) -> (Vec<Crossing>, Vec<Diag>) {
    let owner = |p: &str| arts.iter().find(|a| a.path == p).and_then(|a| a.ctx());
    let mut out = Vec::new();
    let mut diags = Vec::new();
    for (from, tool, refs) in &read.refs {
        let Some(x) = owner(from) else { continue };
        for r in refs {
            let kind = match (tool, r.how.as_str()) {
                (Tool::Rulec, "import proto") => Kind::RuleImportProto,
                (Tool::Rulec, "shape") if r.target.tool == Tool::Proto => Kind::RuleShape,
                (Tool::Rulec, "apply") => Kind::RuleApply,
                (Tool::Koyomi, "use calendar") => Kind::CalendarUse,
                (Tool::Dandori, h) if h.starts_with("use rule") => Kind::FlowRule { connect: h.contains("connect"), how: h.to_string() },
                (Tool::Dandori, "use proto") => Kind::FlowProto,
                (Tool::Dandori, "connect") => Kind::FlowConnect,
                (Tool::Dandori, "flow") => Kind::FlowChild,
                _ => continue,
            };
            let to = r.target.path.clone();
            let Some(y) = owner(&to) else {
                // A file the map does not hold: outside its scope (E103), or in it and no artifact
                // (a JSON or a Smithy model), which no context owns and nothing can be held to.
                let known = r.target.tool == Tool::Proto && crate::owners::is_known_proto(m, &to);
                if !known && !crate::owners::in_scope(m, &to) {
                    let (f, t, how) = (shown(from), r.target.text(), r.how.clone());
                    let src = ritsu_base::fs::read_to_string(crate::paths::on_disk(&m.root, from)).unwrap_or_default();
                    diags.push(diag::at("E103", from, r.line, 1, tr!("{f} が、地図の範囲の外の {t} を参照しています（{how}）", "The file {f} refers to {t}, which is outside the map's scope ({how})")).source(&src).note(crate::owners::scope_note(m)));
                }
                continue;
            };
            if x == y {
                continue;
            }
            let symbol = |kind_word: &str| -> Option<Symbol> {
                let f = ps.files.get(&to)?;
                let (k, local) = r.target.items.first()?;
                if k != kind_word {
                    return None;
                }
                match ps.resolve(&to, &f.package, &f.full(local)) {
                    Resolved::Found(s) => Some(s),
                    _ => None,
                }
            };
            let uses: Vec<Symbol> = match kind {
                Kind::RuleImportProto => symbol("enum").into_iter().collect(),
                Kind::RuleShape => symbol("message").into_iter().collect(),
                _ => vec![],
            };
            let reach = ps.reach(&uses);
            let import = match &kind {
                Kind::FlowConnect => format!("connect {}", r.target.items.iter().map(|(_, v)| v.as_str()).collect::<Vec<_>>().join("/")),
                k => format!("{} {}", k.via(), crate::naming::quote(&ritsu_base::paths::relative(&ritsu_base::paths::parent(from), &to))),
            };
            out.push(Crossing { from: from.clone(), from_tool: *tool, from_ctx: x, to, to_ctx: y, target: r.target.clone(), kind, line: r.line, col: 1, import, uses, reach, allowed: None });
        }
    }
    (out, diags)
}

/// The dependencies of the Rust crates on each other, as crossings (DESIGN 7.7): from the line of
/// the manifest of one crate to the manifest of the crate it depends on, when the two belong to
/// different contexts. A crate outside the map's scope that one in it depends on is E103; one
/// outside the root, or in the scope and no artifact (under no `code rust` place), is in no context.
pub fn crate_crossings(m: &Model, arts: &[Artifact], crates: Option<&crate::cargo::Crates>) -> (Vec<Crossing>, Vec<Diag>) {
    let owner = |p: &str| arts.iter().find(|a| a.path == p).and_then(|a| a.ctx());
    let (mut out, mut diags) = (Vec::new(), Vec::new());
    for c in crates.map(|c| c.list.as_slice()).unwrap_or_default() {
        let Some(x) = owner(&c.manifest) else { continue };
        for d in &c.deps {
            let Some(dir) = &d.dir else { continue };
            let to = crate::cargo::manifest_of(dir);
            let Some(y) = owner(&to) else {
                if !crate::owners::in_scope(m, &to) {
                    let (f, t) = (shown(&c.manifest), shown(dir));
                    let src = ritsu_base::fs::read_to_string(crate::paths::on_disk(&m.root, &c.manifest)).unwrap_or_default();
                    diags.push(
                        diag::at("E103", &c.manifest, d.line, d.col, tr!("{f} が、地図の範囲の外のクレート {t} に依存しています（{}）", "The file {f} depends on the crate at {t}, which is outside the map's scope ({})", d.table; d.table))
                            .source(&src)
                            .note(crate::owners::scope_note(m)),
                    );
                }
                continue;
            };
            if x == y {
                continue;
            }
            out.push(Crossing {
                from: c.manifest.clone(),
                from_tool: Tool::File,
                from_ctx: x,
                to: to.clone(),
                to_ctx: y,
                target: Name::file(Tool::File, to),
                kind: Kind::Crate { table: d.table },
                line: d.line,
                col: d.col,
                import: d.text.clone(),
                uses: vec![],
                reach: vec![],
                allowed: None,
            });
        }
    }
    (out, diags)
}

/// A workflow's `implements` (DESIGN 4.7): the service it implements is an open host service of a
/// published language of its own context (E208).
pub fn implements(m: &Model, arts: &[Artifact], read: &Read) -> Vec<Diag> {
    let mut diags = Vec::new();
    for (from, tool, refs) in &read.refs {
        if *tool != Tool::Dandori {
            continue;
        }
        let Some(x) = arts.iter().find(|a| a.path == *from).and_then(|a| a.ctx()) else { continue };
        for r in refs.iter().filter(|r| r.how == "implements") {
            let svc = r.target.items.first().map(|(_, v)| v.clone()).unwrap_or_default();
            let own = m.contexts[x].published.iter().find(|p| p.protos.iter().any(|(f, _)| *f == r.target.path));
            if own.is_some_and(|p| p.services.iter().any(|(s, _)| *s == svc)) {
                continue;
            }
            let (xn, sp, t) = (m.contexts[x].name.clone(), shown(from), r.target.text());
            let src = ritsu_base::fs::read_to_string(crate::paths::on_disk(&m.root, from)).unwrap_or_default();
            let mut d = diag::at("E208", from, r.line, 1, tr!("「{xn}」の {sp} が実装する {t} は、「{xn}」の公表された言語の公開ホストサービスではありません", "The workflow {sp} of {xn} implements {t}, which is no open host service of a published language of {xn}")).source(&src);
            d = d.note(match own {
                Some(p) => {
                    let k = &p.package;
                    tr!("その proto は「{xn}」の公表された言語 {k} にありますが、`open host service` に {svc} がありません。", "The .proto is in {xn}'s published language {k}, and its `open host service` does not list {svc}.")
                }
                None => tr!("その proto は「{xn}」のどの公表された言語にも入っていません。", "The .proto is in no published language of {xn}."),
            });
            d = d.note(tr!(
                "ワークフローが実装するサービスは、そのワークフローを持つコンテキストが公表するものです。自分の公表された言語に proto を並べ、`open host service` にサービスを書いてください。",
                "A service a workflow implements is one the workflow's context publishes: list the .proto in its published language, and the service under `open host service`."
            ));
            diags.push(d.refer(Ref::line(Some(&xn), from, r.line, Text::same(format!("implements {t}"))).via("implements")));
        }
    }
    diags
}

/// The published package of the context's that holds the file, if any.
pub fn published_package(m: &Model, c: usize, file: &str) -> Option<String> {
    m.contexts[c].published.iter().find(|p| p.protos.iter().any(|(f, _)| f == file)).map(|p| p.package.clone())
}

/// The published package whose `.proto` files, rule or crate hold the file: for the referring
/// side of an anticorruption layer (E205), whose own published language may be a rule's service
/// or a crate.
fn published_package_any(m: &Model, c: usize, file: &str) -> Option<String> {
    m.contexts[c]
        .published
        .iter()
        .find(|p| p.protos.iter().any(|(f, _)| f == file) || p.rulec.as_ref().is_some_and(|(f, _)| f == file) || p.krate.as_ref().is_some_and(|(d, _)| crate::cargo::manifest_of(d) == file))
        .map(|p| p.package.clone())
}

/// The published package of the context's that is the crate of the manifest `file`, if any.
fn published_crate(m: &Model, c: usize, file: &str) -> Option<String> {
    m.contexts[c].published.iter().find(|p| p.krate.as_ref().is_some_and(|(d, _)| crate::cargo::manifest_of(d) == file)).map(|p| p.package.clone())
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

/// The service a child workflow implements, when it does: its `.proto` and its name.
fn child_service(read: &Read, flow: &str) -> Option<(String, String)> {
    read.refs_of(flow).iter().find(|r| r.how == "implements").and_then(|r| Some((r.target.path.clone(), r.target.items.first()?.1.clone())))
}

/// The service of a rule's Connect, from rulec's facts: `/rulec.urgency.v1.UrgencyService/Decide`
/// is `UrgencyService`.
pub fn rule_service(read: &Read, rule: &str) -> Option<String> {
    let path = &read.facts.get(rule)?.connect.as_ref()?.path;
    let qualified = path.trim_start_matches('/').split('/').next()?;
    Some(qualified.rsplit('.').next()?.to_string())
}

/// The package of a rule's Connect, from rulec's facts: `rulec.urgency.v1`.
pub fn rule_package(read: &Read, rule: &str) -> Option<String> {
    let path = &read.facts.get(rule)?.connect.as_ref()?.path;
    let qualified = path.trim_start_matches('/').split('/').next()?;
    Some(qualified.rsplit_once('.')?.0.to_string())
}

/// Hold every crossing to the map (DESIGN 3.3, 4.7): fill `allowed`, and say what is not allowed.
pub fn check(m: &Model, crossings: &mut [Crossing], read: &Read) -> Vec<Diag> {
    let mut diags = Vec::new();
    for c in crossings.iter_mut() {
        let (x, y) = (c.from_ctx, c.to_ctx);
        let (xn, yn) = (m.contexts[x].name.clone(), m.contexts[y].name.clone());
        let src = ritsu_base::fs::read_to_string(crate::paths::on_disk(&m.root, &c.from)).unwrap_or_default();
        let (p, q) = (c.from.clone(), c.to.clone());
        let krate = matches!(c.kind, Kind::Crate { .. });
        // a crate is shown by its directory, as a dependency by its path writes it
        let (sp, sq) = (shown(&p), if krate { shown(&ritsu_base::paths::parent(&q)) } else { shown(&q) });
        let from_ref = c.from_ref(&xn);
        // what a reference does, after what it refers to: a `.proto` imports, a crate depends,
        // the rest refer
        let via = c.kind.via();
        let proto = c.kind == Kind::ProtoImport;
        let tail_ja = if proto {
            " を import しています".to_string()
        } else if krate {
            format!(" に依存しています（{via}）")
        } else {
            format!(" を参照しています（{via}）")
        };
        let act_en = |obj: &str| if proto { format!("imports {obj}") } else if krate { format!("depends on {obj} ({via})") } else { format!("refers to {obj} ({via})") };
        // The file a reference lands on for the published languages: a child workflow that
        // implements an open host service lands on that service's `.proto`.
        let mut lands = q.clone();
        let mut child_published = false;
        if c.kind == Kind::FlowChild
            && let Some((pf, svc)) = child_service(read, &q)
            && m.contexts[y].published.iter().any(|pl| pl.protos.iter().any(|(f, _)| *f == pf) && pl.services.iter().any(|(s, _)| *s == svc))
        {
            lands = pf;
            child_published = true;
        }
        let pkg = match &c.kind {
            Kind::FlowRule { connect: true, .. } => m.contexts[y].published.iter().find(|pl| pl.rulec.as_ref().is_some_and(|(f, _)| *f == q)).map(|pl| pl.package.clone()),
            Kind::FlowRule { connect: false, .. } | Kind::RuleApply | Kind::CalendarUse => None,
            Kind::FlowChild if !child_published => None,
            Kind::Crate { .. } => published_crate(m, y, &q),
            _ => published_package(m, y, &lands),
        };
        let to_what = match &pkg {
            Some(k) => tr!("公表された言語 {k} のもの", "a part of the published language {k}"),
            None => tr!("「{yn}」の内側のもの", "a part of the inside of {yn}"),
        };
        let to_what = if proto {
            match &pkg {
                Some(k) => tr!("公表された言語 {k} のファイル", "a file of the published language {k}"),
                None => tr!("「{yn}」の内側のファイル", "a file inside {yn}"),
            }
        } else if krate {
            match &pkg {
                Some(k) => tr!("公表された言語 {k} のクレート", "the crate of the published language {k}"),
                None => tr!("「{yn}」の内側のクレート", "a crate inside {yn}"),
            }
        } else {
            to_what
        };
        let to_ref = Ref::name(Some(&yn), c.to_name(), to_what);
        let diag = |code: &'static str, msg: Text| diag::at(code, &p, c.line, c.col, msg).source(&src).refer(from_ref.clone()).refer(to_ref.clone());
        let separate = m.writes(x, y, |k| matches!(k, RelK::Separate)) || m.writes(y, x, |k| matches!(k, RelK::Separate));
        if separate {
            let obj = format!("{sq} of {yn}");
            diags.push(diag("E206", tr!("「{xn}」の {sp} が、別々の道の相手「{yn}」の {sq}{tail_ja}", "The file {sp} of {xn} {}, and the two go separate ways", ; act_en(&obj))).note(tr!(
                "別々の道は、二つのあいだに何の参照も持たないという決定です。参照が要るなら、別々の道をやめて関係を書いてください。",
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
            let obj = format!("{sq} of {yn}");
            let mut d = diag("E201", tr!("「{xn}」の {sp} が、関係の無い「{yn}」の {sq}{tail_ja}", "The file {sp} of {xn} {}, which {xn} has no relationship with", ; act_en(&obj)));
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
        // A child workflow that is no published service: partners may run each other's workflows.
        if c.kind == Kind::FlowChild && !child_published {
            if partners {
                c.allowed = Some(Allowed::Partnership);
                continue;
            }
            let mut d = diag("E209", tr!("「{xn}」の {sp} が、「{yn}」のワークフロー {sq} を子として走らせています", "The workflow {sp} of {xn} runs {yn}'s workflow {sq} as its child"));
            if let Some(n) = &kernel_note {
                d = d.note(n.clone());
            }
            d = d.note(tr!(
                "境界の向こうのワークフローを子として走らせてよいのは、二つがパートナーシップのとき、子のフローが二つの共有カーネルにあるとき、子のフローが「{yn}」の公開ホストサービスを `implements` で実装しているときです。そうでなければ、「{yn}」が公表したサービスを `connect` で呼んでください。",
                "Another context's workflow may run as a child when the two are partners, when the child is in their shared kernel, or when the child `implements` an open host service of {yn}; else call a service {yn} publishes with `connect`."
            ));
            diags.push(d);
            continue;
        }
        let Some(k) = pkg else {
            let mut d = diag("E202", tr!("「{xn}」の {sp} が、「{yn}」の内側の {sq}{tail_ja}", "The file {sp} of {xn} {}, which is inside {yn}", ; act_en(&sq)));
            d = d.note(match &c.kind {
                Kind::FlowRule { connect: false, .. } => tr!(
                    "規則を同梱するか Lambda で呼ぶと、規則そのもの（「{yn}」の内側）を使います。境界の向こうの規則は、「{yn}」にその規則を公表された言語（`published language rulec.…`）に入れてもらい、`use rule … connect` で、その Connect のサービスとして呼んでください。",
                    "A rule bundled, or called as a Lambda function, is the rule itself, inside {yn}; a rule across the boundary is called as its Connect service, with `use rule … connect`, once {yn} puts it in a published language (`published language rulec.…`)."
                ),
                Kind::FlowRule { connect: true, .. } => tr!(
                    "{sq} は「{yn}」の公表された言語に入っていません。規則を Connect で呼ぶには、「{yn}」がその規則を公表された言語（`published language rulec.…`）に入れる必要があります。",
                    "The rule {sq} is in no published language of {yn}; to call it by Connect, {yn} puts it in a published language (`published language rulec.…`)."
                ),
                Kind::CalendarUse => tr!(
                    "カレンダーは公表された言語にできません（DESIGN 1.4）。境界の向こうのカレンダーを読めるのは、二つの共有カーネルに並べたときだけです。",
                    "A calendar cannot be a published language (DESIGN 1.4); a calendar across the boundary is read only from the two's shared kernel."
                ),
                Kind::Crate { .. } => tr!(
                    "{sq} のクレートは「{yn}」の公表された言語に入っていません。境界の向こうのクレートに依存できるのは、「{yn}」がそのクレートを公表された言語（`published language` の下の `crate \"…\"`）に入れたときと、二つの共有カーネルに並べたときだけです。",
                    "The crate at {sq} is in no published language of {yn}; a crate across the boundary is depended on only once {yn} puts it in a published language (`crate \"…\"` under a `published language`), or from the two's shared kernel."
                ),
                Kind::RuleApply => tr!(
                    "`apply` は、呼び先の規則をこの規則の中に展開します。使うのは規則そのもの（「{yn}」の内側）です。境界の向こうの規則は、二つの共有カーネルに並べて展開するか、「{yn}」が公表された言語（`published language rulec.…`）に入れた規則を、ワークフローから `use rule … connect` で呼んでください。",
                    "An `apply` expands the rule it names into this one: the rule itself, inside {yn}. A rule across the boundary is expanded from the two's shared kernel, or called from a workflow with `use rule … connect` once {yn} puts it in a published language (`published language rulec.…`)."
                ),
                _ => tr!(
                    "{sq} は「{yn}」の公表された言語に入っていません。境界の向こうから参照できるのは、公表された言語と共有カーネルだけです。",
                    "The file {sq} is in no published language of {yn}; across a boundary only the published languages and a shared kernel can be referred to."
                ),
            });
            if let Some(n) = kernel_note {
                d = d.note(n);
            }
            diags.push(d);
            continue;
        };
        // A service called by `connect` is one the other side offers (E207).
        let called = match &c.kind {
            Kind::FlowConnect => c.target.items.first().map(|(_, v)| v.clone()),
            Kind::FlowRule { connect: true, .. } => rule_service(read, &q),
            _ => None,
        };
        if let Some(svc) = called {
            let offered: Vec<String> = m.contexts[y].published.iter().filter(|pl| pl.package == k).flat_map(|pl| pl.services.iter().map(|(s, _)| s.clone())).collect();
            if !offered.contains(&svc) {
                let (ja, en) = (offered.join("、"), offered.join(", "));
                let mut d = diag("E207", tr!("「{xn}」の {sp} が、「{yn}」の公開ホストサービスでない {svc} を呼んでいます", "The workflow {sp} of {xn} calls {svc}, which is no open host service of {yn}"));
                d = d.note(if offered.is_empty() {
                    tr!("公表された言語 {k} に、公開ホストサービスはありません。", "The published language {k} has no open host service.")
                } else {
                    tr!("公表された言語 {k} の公開ホストサービスは {ja} です。", "The open host services of the published language {k} are {en}.")
                });
                d = d.note(tr!(
                    "境界の向こうで呼べるサービスは、相手が `open host service` に並べたものだけです。",
                    "Across a boundary a workflow calls only the services the other side lists under `open host service`."
                ));
                diags.push(d);
                continue;
            }
        }
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
                "上流と下流の関係では、下流が通ってよい上流の公表された言語を `through` に並べてください。",
                "An upstream relationship lists under `through` the upstream's published languages the downstream may go through."
            )).fix_line(fix).refer(rel_ref));
            continue;
        }
        if r.has(Role::Acl) {
            if let Some(own) = published_package_any(m, x, &p) {
                let used: Vec<String> = c.uses.iter().map(|s| s.full.clone()).collect();
                let used = if used.is_empty() { k.clone() } else { used.join(", ") };
                diags.push(diag("E205", tr!("「{xn}」の公表された言語 {own} に、上流「{yn}」の型が出ています", "The published language {own} of {xn} shows the upstream {yn}'s types")).note(tr!(
                    "{sp} は {used} を使っています。腐敗防止層の下流の公表された言語には、上流のモデルを出せません。層の中で自分の型に読み替えてください。",
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
