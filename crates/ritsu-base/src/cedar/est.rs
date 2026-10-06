//! The JSON policy format (Cedar's EST, `cedar-policy-core/src/est/` at 4.13.0), as the CLI
//! writes it: the keys in the order of Cedar's structs, a record's keys and a policy's
//! annotations sorted (they are `BTreeMap`s there), and `action in [A]` as `in` one entity.

use super::ast::*;
use crate::json::Json;

fn obj<const N: usize>(pairs: [(&str, Json); N]) -> Json {
    Json::Obj(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

pub(crate) fn entity(e: &EntityUid) -> Json {
    obj([("type", Json::Str(e.ty.to_string())), ("id", Json::Str(e.id.clone()))])
}

fn entity_or_slot(e: &EntityOrSlot) -> (&'static str, Json) {
    match e {
        EntityOrSlot::Entity(u) => ("entity", entity(u)),
        EntityOrSlot::Slot(s) => ("slot", Json::Str(s.as_str().to_string())),
    }
}

fn scope(s: &Scope) -> Json {
    match s {
        Scope::Any => obj([("op", Json::str("All"))]),
        Scope::Eq(e) => {
            let (k, v) = entity_or_slot(e);
            obj([("op", Json::str("==")), (k, v)])
        }
        Scope::In(e) => {
            let (k, v) = entity_or_slot(e);
            obj([("op", Json::str("in")), (k, v)])
        }
        Scope::Is(t) => obj([("op", Json::str("is")), ("entity_type", Json::Str(t.to_string()))]),
        Scope::IsIn(t, e) => {
            let (k, v) = entity_or_slot(e);
            obj([("op", Json::str("is")), ("entity_type", Json::Str(t.to_string())), ("in", obj([(k, v)]))])
        }
    }
}

fn action_scope(s: &ActionScope) -> Json {
    match s {
        ActionScope::Any => obj([("op", Json::str("All"))]),
        ActionScope::Eq(e) => obj([("op", Json::str("==")), ("entity", entity(e))]),
        ActionScope::In(e) => obj([("op", Json::str("in")), ("entity", entity(e))]),
        ActionScope::InList(es) if es.len() == 1 => obj([("op", Json::str("in")), ("entity", entity(&es[0]))]),
        ActionScope::InList(es) => obj([("op", Json::str("in")), ("entities", Json::Arr(es.iter().map(entity).collect()))]),
    }
}

fn sorted<T>(mut kv: Vec<(String, T)>) -> Vec<(String, T)> {
    kv.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    kv
}

pub(crate) fn expr(e: &Expr) -> Json {
    let lr = |op: &str, l: &Expr, r: &Expr| obj([(op, obj([("left", expr(l)), ("right", expr(r))]))]);
    match &e.kind {
        ExprKind::Bool(b) => obj([("Value", Json::Bool(*b))]),
        ExprKind::Long(n) => obj([("Value", Json::Int(*n as i128))]),
        ExprKind::Str(s) => obj([("Value", Json::Str(s.clone()))]),
        ExprKind::Entity(u) => obj([("Value", obj([("__entity", entity(u))]))]),
        ExprKind::Var(v) => obj([("Var", Json::str(v.as_str()))]),
        ExprKind::Slot(s) => obj([("Slot", Json::str(s.as_str()))]),
        ExprKind::Not(x) => obj([("!", obj([("arg", expr(x))]))]),
        ExprKind::Neg(x) => obj([("neg", obj([("arg", expr(x))]))]),
        ExprKind::Binary { op, left, right } => lr(op.as_str(), left, right),
        ExprKind::GetAttr { expr: x, attr } => obj([(".", obj([("left", expr(x)), ("attr", Json::Str(attr.clone()))]))]),
        ExprKind::Has { expr: x, attrs } => {
            let attr = if attrs.len() == 1 { Json::Str(attrs[0].clone()) } else { Json::Arr(attrs.iter().cloned().map(Json::Str).collect()) };
            obj([("has", obj([("left", expr(x)), ("attr", attr)]))])
        }
        ExprKind::Like { expr: x, pattern } => {
            let p = pattern
                .iter()
                .map(|p| match p {
                    PatternElem::Wildcard => Json::str("Wildcard"),
                    PatternElem::Char(c) => obj([("Literal", Json::Str(c.to_string()))]),
                })
                .collect();
            obj([("like", obj([("left", expr(x)), ("pattern", Json::Arr(p))]))])
        }
        ExprKind::Is { expr: x, ty, in_expr } => {
            let mut body = vec![("left".to_string(), expr(x)), ("entity_type".to_string(), Json::Str(ty.to_string()))];
            if let Some(i) = in_expr {
                body.push(("in".to_string(), expr(i)));
            }
            obj([("is", Json::Obj(body))])
        }
        ExprKind::If { cond, then, els } => obj([("if-then-else", obj([("if", expr(cond)), ("then", expr(then)), ("else", expr(els))]))]),
        ExprKind::Set(xs) => obj([("Set", Json::Arr(xs.iter().map(expr).collect()))]),
        ExprKind::Record(kv) => {
            let m = sorted(kv.iter().map(|(k, v)| (k.clone(), expr(v))).collect());
            obj([("Record", Json::Obj(m))])
        }
        ExprKind::Method { expr: recv, name, args } => match name.as_str() {
            "contains" | "containsAll" | "containsAny" | "getTag" | "hasTag" => lr(name, recv, &args[0]),
            "isEmpty" => obj([("isEmpty", obj([("arg", expr(recv))]))]),
            _ => {
                let mut v = vec![expr(recv)];
                v.extend(args.iter().map(expr));
                obj([(name.as_str(), Json::Arr(v))])
            }
        },
        ExprKind::Call { func, args } => Json::Obj(vec![(func.to_string(), Json::Arr(args.iter().map(expr).collect()))]),
    }
}

pub(crate) fn policy(p: &Policy) -> Json {
    let effect = match p.effect {
        Effect::Permit => "permit",
        Effect::Forbid => "forbid",
    };
    let conditions = p
        .conditions
        .iter()
        .map(|c| {
            let kind = match c.kind {
                CondKind::When => "when",
                CondKind::Unless => "unless",
            };
            obj([("kind", Json::str(kind)), ("body", expr(&c.body))])
        })
        .collect();
    let mut out = vec![
        ("effect".to_string(), Json::str(effect)),
        ("principal".to_string(), scope(&p.principal)),
        ("action".to_string(), action_scope(&p.action)),
        ("resource".to_string(), scope(&p.resource)),
        ("conditions".to_string(), Json::Arr(conditions)),
    ];
    if !p.annotations.is_empty() {
        let m = sorted(p.annotations.iter().map(|a| (a.key.clone(), a.value.clone().map(Json::Str).unwrap_or(Json::Null))).collect());
        out.push(("annotations".to_string(), Json::Obj(m)));
    }
    Json::Obj(out)
}

pub(crate) fn policy_set(set: &PolicySet) -> Json {
    let templates = set.policies.iter().filter(|p| p.is_template()).map(|p| (p.id.clone(), policy(p))).collect();
    let statics = set.policies.iter().filter(|p| !p.is_template()).map(|p| (p.id.clone(), policy(p))).collect();
    obj([("templates", Json::Obj(templates)), ("staticPolicies", Json::Obj(statics)), ("templateLinks", Json::Arr(vec![]))])
}
