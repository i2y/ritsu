//! `sekisho vectors` (DESIGN 6.1): every combination the check walks, as a test of `cedar
//! run-tests` — the request and the entities of the combination, with the decision and the
//! policies that decide it by sekisho's reference evaluation ([`crate::eval`]), and no error.
//! Run against the generated Cedar, they hold the policies Cedar evaluates to the table the check
//! counted, combination by combination.
//!
//! A combination is a cell of each number a condition compares, not a number; a cell is tried at
//! both of its ends (DESIGN 6.1, as rulec's vectors step on both sides of every boundary): the
//! combination is written twice, once with every number at the low end of its cell and once at
//! the high end, so that a comparison off by one at any boundary turns one of them. A combination
//! with no number, or whose cells are single values, is written once.
//!
//! The entities are what the request needs: the roles (with the roles they include as their
//! parents), the principal (its roles and the groups it is a member of as its parents), the
//! resource, and an entity an attribute points to when its type holds no attribute in the schema.
//! Each holds exactly the attributes the schema gives its type ([`crate::cedar::Shape`]): a value
//! the combination does not walk (an attribute only another action's policies read) is given the
//! first value it can be.

use crate::cedar::{self, Shape};
use crate::checks::Report;
use crate::model::*;
use crate::walk::{Combo, Slot, Space, Val, bit};
use ritsu_base::cedar::{EntityUid, ExprKind, write_expr};
use ritsu_base::json::Json;
use std::collections::BTreeMap;

/// Which end of its cell each number is tried at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    Low,
    High,
}

/// The entities' ids of one test: `user1`, `order1`, `customer2`, by type.
#[derive(Default)]
struct Ids {
    next: BTreeMap<usize, usize>,
}

impl Ids {
    fn fresh(&mut self, g: &Gate, t: usize) -> String {
        let n = self.next.entry(t).or_insert(1);
        let id = format!("{}{n}", g.types[t].named.alias.to_lowercase());
        *n += 1;
        id
    }
}

fn uid_json(ty: &ritsu_base::cedar::Name, id: &str) -> Json {
    Json::obj([("type", Json::str(ty.to_string())), ("id", Json::str(id))])
}

/// `Shop::User::"user1"`, as a request writes it.
fn uid_text(ty: &ritsu_base::cedar::Name, id: &str) -> String {
    write_expr(&ritsu_base::cedar::Expr::new(ExprKind::Entity(EntityUid::new(ty.clone(), id))))
}

fn entity(ty: &ritsu_base::cedar::Name, id: &str, attrs: Vec<(String, Json)>, parents: Vec<Json>) -> Json {
    Json::obj([("uid", uid_json(ty, id)), ("attrs", Json::Obj(attrs)), ("parents", Json::Arr(parents))])
}

/// A value of a field as Cedar's JSON writes it, or None when it is absent.
fn value_of(g: &Gate, f: &Field, v: &Val, end: End) -> Option<Json> {
    match (v, &f.ty) {
        (Val::Bool(b), _) => Some(Json::Bool(*b)),
        (Val::Enum(k), FieldType::Enum(e)) => Some(Json::str(g.enums[*e].values[*k].alias.clone())),
        (Val::Cell(c), _) => Some(Json::Int(if end == End::High { c.hi } else { c.lo })),
        _ => None,
    }
}

/// The first value a field can be, for one the combination does not walk.
fn first_value(g: &Gate, f: &Field) -> Option<Json> {
    match &f.ty {
        FieldType::Bool => Some(Json::Bool(false)),
        FieldType::Enum(e) => g.enums[*e].values.first().map(|v| Json::str(v.alias.clone())),
        FieldType::Num { lo, .. } => Some(Json::Int(*lo)),
        FieldType::Date { .. } | FieldType::Entity(_) => None,
    }
}

/// What a test is named by: the action, the combination's number in it, and the numbers it sets.
fn place_name(g: &Gate, a: &Action, f: &crate::walk::Frame, s: &Slot) -> String {
    match s {
        Slot::Input(i) => a.inputs[*i].named.alias.clone(),
        Slot::Attr(o, i) => format!("{}.{}", o.word(), g.owner_type(*o, f.principal, f.resource).attrs[*i].named.alias),
        _ => String::new(),
    }
}

/// Whether the frame computes a value: every attribute and input it reads is there.
fn computable(g: &Gate, a: &Action, f: &crate::walk::Frame, cv: &Computed) -> bool {
    let sources: Vec<&Source> = match &cv.how {
        How::Rule { args, .. } | How::Date { of: DateOf::Call { args, .. }, .. } => args.iter().map(|(_, s)| s).collect(),
        How::Date { .. } | How::Open { .. } => Vec::new(),
    };
    let attr_there = |o: Owner, n: &str| g.owner_type(o, f.principal, f.resource).attr(n).is_some();
    let attr_ok = match &cv.how {
        How::Date { of: DateOf::Attr(o, n), .. } => attr_there(*o, n),
        _ => true,
    };
    attr_ok
        && sources.iter().all(|s| match s {
            Source::Attr(o, n) => attr_there(*o, n),
            Source::Input(n) => a.input(n).is_some(),
            Source::Lit(_) | Source::Today => true,
        })
}

/// The tests of one combination: one, or one at each end of the cells it sets.
fn tests_of(g: &Gate, shape: &Shape, s: &Space, n: usize, combo: &Combo, out: &mut Vec<Json>) {
    let ai = s.action;
    let a = &g.actions[ai];
    let f = &s.frames[combo.frame];
    let env = combo.env;
    let (pt, rt) = (f.principal, f.resource);
    // the numbers of the request, and whether a cell of one has two ends
    let mut numbers: Vec<(usize, String)> = Vec::new();
    for (at, slot) in f.slots.iter().enumerate() {
        let in_cedar = match slot {
            Slot::Input(i) => shape.actions[ai].inputs.contains(i),
            Slot::Attr(o, i) => shape.types[if *o == Owner::Principal { pt } else { rt }].attrs.contains(i),
            _ => false,
        };
        if in_cedar && matches!(&env[at], Val::Cell(_)) {
            numbers.push((at, place_name(g, a, f, slot)));
        }
    }
    let two_ends = numbers.iter().any(|(at, _)| matches!(&env[*at], Val::Cell(c) if c.lo < c.hi));
    let ends: &[End] = if two_ends { &[End::Low, End::High] } else { &[End::Low] };
    for &end in ends {
        let mut name = format!("{} {n}", a.named.alias);
        if two_ends {
            let set: Vec<String> = numbers
                .iter()
                .map(|(at, shown)| match &env[*at] {
                    Val::Cell(c) => format!("{shown} {}", if end == End::High { c.hi } else { c.lo }),
                    _ => shown.clone(),
                })
                .collect();
            name.push_str(&format!(" ({})", set.join(", ")));
        }
        out.push(test(g, shape, s, &name, combo, end));
    }
}

/// One test: the request, the entities, the decision and the policies that decide it.
fn test(g: &Gate, shape: &Shape, s: &Space, name: &str, combo: &Combo, end: End) -> Json {
    let ai = s.action;
    let a = &g.actions[ai];
    let f = &s.frames[combo.frame];
    let env = combo.env;
    let (pt, rt) = (f.principal, f.resource);
    let mut ids = Ids::default();
    let p_id = if g.types[pt].kind == Kind::Workflow {
        let w = f.place(&Slot::Workflow).and_then(|at| match &env[at] {
            Val::Workflow(w) => Some(*w),
            _ => None,
        });
        match w.or(if g.workflows.is_empty() { None } else { Some(0) }) {
            Some(w) => g.workflows[w].named.alias.clone(),
            None => ids.fresh(g, pt),
        }
    } else {
        ids.fresh(g, pt)
    };
    let r_id = ids.fresh(g, rt);
    // the entities the relations point to: each block of the same entity one id, the principal's
    // its own; the groups the principal is a member of
    let mut pointed: BTreeMap<(Owner, usize), Option<(usize, String)>> = BTreeMap::new();
    let mut groups: Vec<(usize, String)> = Vec::new();
    let mut others: Vec<(usize, String)> = Vec::new();
    for (&at, terms) in &f.terms {
        let (Slot::Relation(ty), Val::Relation(rel)) = (&f.slots[at], &env[at]) else { continue };
        let ty = *ty;
        let blocks = rel.blocks.iter().flatten().map(|b| *b as usize + 1).max().unwrap_or(0);
        let own = rel.principal.and_then(|p| rel.blocks[p]).map(|b| b as usize);
        let block_ids: Vec<String> = (0..blocks)
            .map(|b| {
                if Some(b) == own {
                    p_id.clone()
                } else {
                    let id = ids.fresh(g, ty);
                    others.push((ty, id.clone()));
                    id
                }
            })
            .collect();
        for (i, t) in terms.iter().enumerate() {
            if let Term::Attr(o, n) = t
                && let Some((k, _)) = g.owner_type(*o, pt, rt).attr(n)
            {
                pointed.entry((*o, k)).or_insert_with(|| rel.blocks[i].map(|b| (ty, block_ids[b as usize].clone())));
            }
        }
        if shape.types[pt].member_of.contains(&ty) {
            for (b, id) in block_ids.iter().enumerate() {
                if rel.members & (1u64 << b) != 0 {
                    groups.push((ty, id.clone()));
                }
            }
        }
    }
    let attrs_of = |o: Owner, t: usize, ids: &mut Ids, others: &mut Vec<(usize, String)>| -> Vec<(String, Json)> {
        let mut v = Vec::new();
        for &i in &shape.types[t].attrs {
            let fd = &g.types[t].attrs[i];
            let val = if let FieldType::Entity(et) = fd.ty {
                match pointed.get(&(o, i)) {
                    Some(Some((ty, id))) => Some(Json::obj([("__entity", uid_json(&cedar::type_name(g, shape, *ty), id))])),
                    Some(None) => None,
                    None if fd.optional => None,
                    None => {
                        let id = ids.fresh(g, et);
                        others.push((et, id.clone()));
                        Some(Json::obj([("__entity", uid_json(&cedar::type_name(g, shape, et), &id))]))
                    }
                }
            } else {
                match f.place(&Slot::Attr(o, i)) {
                    Some(at) => value_of(g, fd, &env[at], end),
                    None if fd.optional => None,
                    None => first_value(g, fd),
                }
            };
            if let Some(val) = val {
                v.push((fd.named.alias.clone(), val));
            }
        }
        v
    };
    let mut entities: Vec<Json> = Vec::new();
    let role_type = ritsu_base::cedar::Name { path: shape.namespace.path.iter().cloned().chain([shape.namespace.id.clone()]).collect(), id: cedar::ROLE.to_string() };
    for (ri, r) in g.roles.iter().enumerate() {
        let parents = r.includes.iter().map(|&x| uid_json(&role_type, &g.roles[x].named.alias)).collect();
        entities.push(entity(&role_type, &g.roles[ri].named.alias, Vec::new(), parents));
    }
    let p_type = cedar::type_name(g, shape, pt);
    let r_type = cedar::type_name(g, shape, rt);
    let p_attrs = if g.types[pt].kind == Kind::Workflow { Vec::new() } else { attrs_of(Owner::Principal, pt, &mut ids, &mut others) };
    let r_attrs = attrs_of(Owner::Resource, rt, &mut ids, &mut others);
    let mut parents: Vec<Json> = Vec::new();
    if let Val::Roles { direct, .. } = &env[0] {
        for ri in 0..g.roles.len() {
            if direct & bit(ri) != 0 {
                parents.push(uid_json(&role_type, &g.roles[ri].named.alias));
            }
        }
    }
    parents.extend(groups.iter().map(|(t, id)| uid_json(&cedar::type_name(g, shape, *t), id)));
    entities.push(entity(&p_type, &p_id, p_attrs, parents));
    entities.push(entity(&r_type, &r_id, r_attrs, Vec::new()));
    // an entity pointed to that holds no attribute in the schema is written too, so that the
    // request reads as the data does; one that holds some is left out (Cedar reads no attribute of
    // it, and a request may name an entity it does not give)
    let mut seen: Vec<(usize, String)> = vec![(pt, p_id.clone()), (rt, r_id.clone())];
    for (t, id) in others {
        if seen.contains(&(t, id.clone())) || !shape.types[t].attrs.is_empty() {
            continue;
        }
        entities.push(entity(&cedar::type_name(g, shape, t), &id, Vec::new(), Vec::new()));
        seen.push((t, id));
    }
    // the context: the inputs the policies read, and every value the action computes
    let mut context: Vec<(String, Json)> = Vec::new();
    for &i in &shape.actions[ai].inputs {
        let fd = &a.inputs[i];
        let val = match f.place(&Slot::Input(i)) {
            Some(at) => value_of(g, fd, &env[at], end),
            None if fd.optional => None,
            None => first_value(g, fd),
        };
        if let Some(val) = val {
            context.push((fd.named.alias.clone(), val));
        }
    }
    for (ci, cs) in shape.actions[ai].computed.iter().enumerate() {
        let cv = &a.computed[ci];
        let val = match f.place(&Slot::Computed(ci)) {
            Some(at) => match (&env[at], &cs.values) {
                (Val::Bool(b), _) => Some(Json::Bool(*b)),
                (Val::Enum(k), Some(vs)) => vs.get(*k).map(|v| Json::str(v.alias.clone())),
                _ => None,
            },
            None if !computable(g, a, f, cv) => None,
            None => match &cs.values {
                Some(vs) => vs.first().map(|v| Json::str(v.alias.clone())),
                None => Some(Json::Bool(false)),
            },
        };
        if let Some(val) = val {
            context.push((cv.named.alias.clone(), val));
        }
    }
    let request = Json::obj([
        ("principal", Json::str(uid_text(&p_type, &p_id))),
        ("action", Json::str(write_expr(&ritsu_base::cedar::Expr::new(ExprKind::Entity(cedar::action_uid(g, shape, ai)))))),
        ("resource", Json::str(uid_text(&r_type, &r_id))),
        ("context", Json::Obj(context)),
    ]);
    let reason: Vec<Json> = combo.decision.determining.iter().map(|&i| Json::str(cedar::policy_id(g, s.policies[i]))).collect();
    Json::obj([
        ("name", Json::str(name)),
        ("request", request),
        ("entities", Json::Arr(entities)),
        ("decision", Json::str(if combo.decision.allow { "allow" } else { "deny" })),
        ("reason", Json::Arr(reason)),
        ("num_errors", Json::Int(0)),
    ])
}

/// Every test of the actions walked (or of `only`), in the order of the actions and of the walk.
pub fn tests(g: &Gate, shape: &Shape, report: &Report, only: Option<usize>) -> Vec<Json> {
    let mut out = Vec::new();
    for (ai, space) in report.spaces.iter().enumerate() {
        if only.is_some_and(|x| x != ai) {
            continue;
        }
        let Some(s) = space else { continue };
        let mut n = 0usize;
        s.each(|combo| {
            n += 1;
            tests_of(g, shape, s, n, combo, &mut out);
        });
    }
    out
}

/// The tests as the file `cedar run-tests --tests` reads: a JSON array, a test a line.
pub fn text(tests: &[Json]) -> String {
    if tests.is_empty() {
        return "[]\n".to_string();
    }
    let lines: Vec<String> = tests.iter().map(|t| t.compact()).collect();
    format!("[\n{}\n]\n", lines.join(",\n"))
}
