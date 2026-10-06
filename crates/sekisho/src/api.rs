//! `sekisho api` (DESIGN 11): what a `.gate` that passes its check declares, as JSON, for the tools
//! that read it without reading the file — the shape follows koyomi's and chobo's `api` (the tool
//! and its version, the file's name, alias, version, digest and description first). It holds what
//! the Cedar is made of: the namespace and the four files `gen --target cedar` writes, each role,
//! type and workflow with the entity it is in Cedar, each action with the operations it guards
//! (as ritsu names them, ritsu's DESIGN 6.2) and its context, each policy with its `@id`, and the
//! expectations and the separations, each with its line.

use crate::cedar::{self, Shape};
use crate::checks::Checked;
use crate::model::*;
use crate::names::Scope;
use ritsu_base::cedar::{EntityUid, Expr, ExprKind, write_expr};
use ritsu_base::json::Json;

fn uid(u: EntityUid) -> Json {
    Json::str(write_expr(&Expr::new(ExprKind::Entity(u))))
}

fn names(g: &Gate, xs: &[usize], of: impl Fn(&Gate, usize) -> &Named) -> Json {
    Json::arr(xs.iter().map(|&x| Json::str(of(g, x).alias.clone())))
}

/// `to` as seen from the directory of the file `from` (both as sekisho reaches them), as koyomi's
/// `api` writes the files it reads: no path of the machine that ran it.
fn beside(from: &str, to: &str) -> String {
    use std::path::{Component, Path, PathBuf};
    let base: Vec<Component> = Path::new(from).parent().map(|p| p.components().collect()).unwrap_or_default();
    let target: Vec<Component> = Path::new(to).components().collect();
    let common = base.iter().zip(&target).take_while(|(x, y)| x == y).count();
    let mut out = PathBuf::new();
    for _ in common..base.len() {
        out.push("..");
    }
    for c in &target[common..] {
        out.push(c.as_os_str());
    }
    out.to_string_lossy().to_string()
}

fn line(n: usize) -> Json {
    if n == 0 { Json::Null } else { Json::from(n) }
}

/// A field's type as the gate writes it, and its range.
fn field_json(g: &Gate, f: &Field, in_cedar: bool) -> Json {
    let (ty, range) = match &f.ty {
        FieldType::Bool => ("bool".to_string(), Json::Null),
        FieldType::Enum(e) => (g.enums[*e].named.alias.clone(), Json::Null),
        FieldType::Num { written, lo, hi, .. } => (written.clone(), Json::obj([("min", Json::Int(*lo)), ("max", Json::Int(*hi))])),
        FieldType::Date { lo, hi } => ("date".to_string(), Json::obj([("min", Json::str(crate::types::day_text(*lo))), ("max", Json::str(crate::types::day_text(*hi)))])),
        FieldType::Entity(t) => (g.types[*t].named.alias.clone(), Json::Null),
    };
    let values = match &f.ty {
        FieldType::Enum(e) => Json::arr(g.enums[*e].values.iter().map(|v| Json::str(v.alias.clone()))),
        _ => Json::Null,
    };
    Json::obj([
        ("name", Json::str(f.named.name.clone())),
        ("alias", Json::str(f.named.alias.clone())),
        ("type", Json::str(ty)),
        ("range", range),
        ("values", values),
        ("optional", Json::Bool(f.optional)),
        ("in_cedar", Json::Bool(in_cedar)),
    ])
}

/// The JSON of `sekisho api` for a file that passes.
pub fn json(scope: &Scope, checked: &Checked) -> Json {
    let g = &checked.gate;
    let f = scope.file();
    let shape: Shape = cedar::shape(g, scope, checked);
    let alias = &g.named.alias;
    let kind = |k: Kind| match k {
        Kind::Principal => "principal",
        Kind::Resource => "resource",
        Kind::Workflow => "workflow",
    };
    let roles = g.roles.iter().enumerate().map(|(ri, r)| {
        Json::obj([
            ("name", Json::str(r.named.name.clone())),
            ("alias", Json::str(r.named.alias.clone())),
            ("line", line(r.named.line)),
            ("entity", uid(cedar::role_uid(g, &shape, ri))),
            ("includes", names(g, &r.includes, |g, x| &g.roles[x].named)),
            ("can", r.can.as_ref().map(|(xs, _)| names(g, xs, |g, x| &g.actions[x].named)).unwrap_or(Json::Null)),
        ])
    });
    let types = g.types.iter().enumerate().map(|(ti, t)| {
        let ts = &shape.types[ti];
        let mut member_of: Vec<String> = Vec::new();
        if ts.roles {
            member_of.push(cedar::ROLE.to_string());
        }
        member_of.extend(ts.member_of.iter().map(|&m| g.types[m].named.alias.clone()));
        Json::obj([
            ("name", Json::str(t.named.name.clone())),
            ("alias", Json::str(t.named.alias.clone())),
            ("kind", Json::str(kind(t.kind))),
            ("line", line(t.named.line)),
            ("entity_type", Json::str(cedar::type_name(g, &shape, ti).to_string())),
            ("roles", names(g, &t.roles, |g, x| &g.roles[x].named)),
            ("member_of", Json::arr(member_of.into_iter().map(Json::str))),
            ("attributes", Json::arr(t.attrs.iter().enumerate().map(|(i, a)| field_json(g, a, ts.attrs.contains(&i))))),
        ])
    });
    let workflows = g.workflows.iter().enumerate().map(|(wi, w)| {
        Json::obj([
            ("name", Json::str(w.named.name.clone())),
            ("alias", Json::str(w.named.alias.clone())),
            ("line", line(w.named.line)),
            ("entity", uid(cedar::workflow_uid(g, &shape, wi))),
            ("flow", Json::str(w.flow.clone())),
        ])
    });
    let actions = g.actions.iter().enumerate().map(|(ai, a)| {
        let s = &shape.actions[ai];
        let decl = f.actions.iter().find(|x| x.name.ascii() == a.named.alias);
        let context = a.computed.iter().enumerate().map(|(ci, cv)| {
            let cs = &s.computed[ci];
            let how = decl.and_then(|d| d.context.iter().find(|x| x.name.ascii() == cv.named.alias)).map(|x| x.text.clone()).unwrap_or_default();
            Json::obj([
                ("name", Json::str(cv.named.name.clone())),
                ("alias", Json::str(cv.named.alias.clone())),
                ("line", line(cv.named.line)),
                ("computed", Json::str(how)),
                ("type", Json::str(if cs.values.is_some() { "enum" } else { "bool" })),
                ("values", cs.values.as_ref().map(|vs| Json::arr(vs.iter().map(|v| Json::str(v.alias.clone())))).unwrap_or(Json::Null)),
                ("optional", Json::Bool(cs.optional)),
            ])
        });
        Json::obj([
            ("name", Json::str(a.named.name.clone())),
            ("alias", Json::str(a.named.alias.clone())),
            ("line", line(a.named.line)),
            ("entity", uid(cedar::action_uid(g, &shape, ai))),
            ("guards", Json::arr(a.guards.iter().map(|gd| Json::obj([("reference", Json::str(cedar::guard_reference(g, gd))), ("line", line(gd.line))])))),
            ("principals", names(g, &a.principals, |g, x| &g.types[x].named)),
            ("resources", names(g, &a.resources, |g, x| &g.types[x].named)),
            ("from", a.from.as_ref().map(|(x, _)| Json::str(x.clone())).unwrap_or(Json::Null)),
            ("nobody", a.nobody.as_ref().map(|(x, _)| Json::str(x.clone())).unwrap_or(Json::Null)),
            ("inputs", Json::arr(a.inputs.iter().enumerate().map(|(i, x)| field_json(g, x, s.inputs.contains(&i))))),
            ("context", Json::arr(context)),
        ])
    });
    let policies = g.policies.iter().enumerate().map(|(pi, p)| {
        Json::obj([
            ("name", Json::str(p.named.name.clone())),
            ("alias", Json::str(p.named.alias.clone())),
            ("id", Json::str(cedar::policy_id(g, pi))),
            ("effect", Json::str(if p.permit { "permit" } else { "forbid" })),
            ("actions", names(g, &p.actions, |g, x| &g.actions[x].named)),
            ("file", Json::str(beside(&g.file, p.from.as_ref().map(|(_, file)| file.as_str()).unwrap_or(&g.file)))),
            ("line", line(p.named.line)),
        ])
    });
    let expects = g.expects.iter().map(|e| {
        Json::obj([
            ("name", Json::str(e.name.clone())),
            ("expect", Json::str(if e.allow { "allow" } else { "deny" })),
            ("actions", names(g, &e.actions, |g, x| &g.actions[x].named)),
            ("line", line(e.line)),
        ])
    });
    let separations = g.separates.iter().map(|s| Json::obj([("name", Json::str(s.name.clone())), ("actions", names(g, &s.actions, |g, x| &g.actions[x].named)), ("line", line(s.line))]));
    Json::obj([
        ("sekisho", Json::str(env!("CARGO_PKG_VERSION"))),
        ("name", Json::str(f.name.text.clone())),
        ("alias", Json::str(alias.clone())),
        ("version", Json::str(f.version.clone())),
        ("source_sha256", Json::str(ritsu_base::sha256::hex(f.src.as_bytes()))),
        ("description", Json::opt_str(f.description.as_ref().map(|(d, _)| d.clone()))),
        ("namespace", Json::str(g.namespace.clone())),
        ("uses", Json::arr(g.uses.iter().map(|u| Json::obj([("kind", Json::str(u.kind.word())), ("name", if u.name.is_empty() { Json::Null } else { Json::str(u.name.clone()) }), ("path", Json::str(u.path.clone())), ("line", line(u.line))])))),
        (
            "today",
            match &g.today {
                Some(t) => {
                    let sign = if t.offset < 0 { '-' } else { '+' };
                    let offset = format!("{sign}{:02}:{:02}", t.offset.abs() / 60, t.offset.abs() % 60);
                    Json::obj([("min", Json::str(crate::types::day_text(t.lo))), ("max", Json::str(crate::types::day_text(t.hi))), ("offset", Json::str(offset)), ("line", line(t.line))])
                }
                None => Json::Null,
            },
        ),
        (
            "cedar",
            Json::obj([
                ("policies", Json::str(format!("cedar/{alias}.cedar"))),
                ("schema", Json::str(format!("cedar/{alias}.cedarschema"))),
                ("schema_json", Json::str(format!("cedar/{alias}.cedarschema.json"))),
                ("policies_json", Json::str(format!("cedar/{alias}.policies.json"))),
            ]),
        ),
        ("roles", Json::arr(roles)),
        ("types", Json::arr(types)),
        ("workflows", Json::arr(workflows)),
        ("actions", Json::arr(actions)),
        ("policies", Json::arr(policies)),
        ("expects", Json::arr(expects)),
        ("separations", Json::arr(separations)),
    ])
}
