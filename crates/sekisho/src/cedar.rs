//! The Cedar a `.gate` compiles to (DESIGN 5.1, 5.2, 5.7): its schema and its policies, as values
//! of ritsu-base's Cedar ([`ritsu_base::cedar`]), which writes them as Cedar's own tools do.
//!
//! What goes into Cedar is first gathered as a [`Shape`]: each type's attributes that a policy
//! reads (DESIGN 2.4: no other), the types an entity of it can be in (`Role`, and the groups a
//! policy asks it to be a member of), and each action's context — the inputs a policy reads and
//! every value the action computes, each with whether it may be absent. The schema is written
//! from the shape, the policies are compiled against it (an attribute that may be absent, or that
//! a type the policy can see does not have, is read behind `has`, DESIGN 3.1), and the vectors of
//! `sekisho vectors` fill requests and entities by it ([`crate::vectors`]).
//!
//! A policy is the condition the check walked, written in the small part of Cedar sekisho uses
//! (P2): scopes with `==`, `in` and `is`, `==`, the comparisons of numbers, `&&`, `||`, `!`,
//! `has`, and `in` between entities. The text is laid out as `cedar format` lays it out, and the
//! JSON forms are what `cedar translate-policy` and `cedar translate-schema` print for the text.

use crate::ast;
use crate::ast::Op;
use crate::checks::Checked;
use crate::model::*;
use crate::names::Scope;
use crate::walk::{Domain, Kn, Slot, Val};
use ritsu_base::cedar::{self as c, BinOp, CondKind, ExprKind, Var};
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;

/// What of a gate Cedar holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// `Shop`, `Acme::Shop`.
    pub namespace: c::Name,
    /// For each type of the gate (by index into [`Gate::types`]).
    pub types: Vec<TypeShape>,
    /// For each action of the file.
    pub actions: Vec<ActionShape>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TypeShape {
    /// The attributes a policy reads, by index into the type's, in the order declared.
    pub attrs: Vec<usize>,
    /// Whether an entity of it can be in a role (`in [Role]`): the type declares roles.
    pub roles: bool,
    /// The entity types a policy asks it to be a member of (`principal in resource.team`), by
    /// index into [`Gate::types`].
    pub member_of: Vec<usize>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActionShape {
    /// The inputs a policy reads, by index into the action's, in the order declared.
    pub inputs: Vec<usize>,
    /// Every value the action computes, in the order declared.
    pub computed: Vec<ComputedShape>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComputedShape {
    /// The values of a rule's enum, each by the rule's name of it and the string Cedar is given;
    /// None for a bool.
    pub values: Option<Vec<Named>>,
    /// The name of each value in the generated code (`WithinLimit`), which a condition may write
    /// too (DESIGN 3.2), in the order of `values`.
    pub members: Vec<String>,
    /// Whether it may be absent: not computed for a type that lacks what it reads, or reading an
    /// attribute or an input that may be absent.
    pub optional: bool,
}

impl Shape {
    /// Whether the context of `action` holds a value named `name` (an input or a computed value,
    /// by its name or its alias), and whether it may be absent there.
    fn context_value(&self, g: &Gate, action: usize, name: &str) -> Option<bool> {
        let a = &g.actions[action];
        if let Some((i, f)) = a.input(name) {
            return self.actions[action].inputs.contains(&i).then_some(f.optional);
        }
        let (i, _) = a.computed_value(name)?;
        Some(self.actions[action].computed[i].optional)
    }
}

/// Whether a policy whose `principal` line is `who` can apply to a principal of type `t`.
fn allows(g: &Gate, who: &Who, t: usize) -> bool {
    match who {
        Who::Anyone => true,
        // a type that holds no role is in none (Cedar's validator leaves it out of the scope)
        Who::InRoles(_) => !g.types[t].roles.is_empty(),
        Who::IsType(x) => *x == t,
        Who::IsWorkflow(_) => g.types[t].kind == Kind::Workflow,
    }
}

/// The principal types and the resource types a policy can apply to, over its actions.
fn envs(g: &Gate, p: &Policy) -> (Vec<usize>, Vec<usize>) {
    let mut ps: Vec<usize> = Vec::new();
    let mut rs: Vec<usize> = Vec::new();
    for &a in &p.actions {
        for &t in &g.actions[a].principals {
            if allows(g, &p.who, t) && !ps.contains(&t) {
                ps.push(t);
            }
        }
        for &t in &g.actions[a].resources {
            if !rs.contains(&t) {
                rs.push(t);
            }
        }
    }
    (ps, rs)
}

/// Every atom of a condition.
fn atoms<'a>(e: &'a Expr, out: &mut Vec<&'a Atom>) {
    match e {
        Expr::Atom(a) => out.push(a),
        Expr::Not(x) => atoms(x, out),
        Expr::And(xs) | Expr::Or(xs) => xs.iter().for_each(|x| atoms(x, out)),
    }
}

fn path_term(p: &Path) -> Option<Term> {
    match p {
        Path::Attr(o, n) => Some(Term::Attr(*o, n.clone())),
        Path::Value(_) => None,
    }
}

/// The shape of a gate's Cedar: what its policies read, and every value its actions compute.
pub fn shape(g: &Gate, scope: &Scope, checked: &Checked) -> Shape {
    let mut types: Vec<TypeShape> = g.types.iter().map(|t| TypeShape { roles: !t.roles.is_empty(), ..TypeShape::default() }).collect();
    let mut inputs: Vec<Vec<usize>> = vec![Vec::new(); g.actions.len()];
    let add = |v: &mut Vec<usize>, x: usize| {
        if !v.contains(&x) {
            v.push(x);
        }
    };
    for p in &g.policies {
        let mut found: Vec<&Atom> = Vec::new();
        for cond in &p.conds {
            atoms(&cond.expr, &mut found);
        }
        for &a in &p.actions {
            for &pt in g.actions[a].principals.iter().filter(|&&t| allows(g, &p.who, t)) {
                for &rt in &g.actions[a].resources {
                    let owner = |o: Owner| if o == Owner::Principal { pt } else { rt };
                    let read = |path: &Path, types: &mut Vec<TypeShape>, inputs: &mut Vec<Vec<usize>>| match path {
                        Path::Attr(o, n) => {
                            if let Some((i, f)) = g.types[owner(*o)].attr(n)
                                && !matches!(f.ty, FieldType::Date { .. })
                            {
                                add(&mut types[owner(*o)].attrs, i);
                            }
                        }
                        Path::Value(n) => {
                            if let Some((i, _)) = g.actions[a].input(n) {
                                add(&mut inputs[a], i);
                            }
                        }
                    };
                    for atom in &found {
                        match atom {
                            Atom::True(x) | Atom::Is(x, _) | Atom::Cmp(x, _, _) => read(x, &mut types, &mut inputs),
                            Atom::Eq(x, y) => {
                                read(x, &mut types, &mut inputs);
                                read(y, &mut types, &mut inputs);
                            }
                            Atom::Same(x, y) => {
                                for t in [x, y] {
                                    if let Term::Attr(o, n) = t {
                                        read(&Path::Attr(*o, n.clone()), &mut types, &mut inputs);
                                    }
                                }
                            }
                            Atom::Member(t) => {
                                if let Term::Attr(o, n) = t {
                                    read(&Path::Attr(*o, n.clone()), &mut types, &mut inputs);
                                    if let Some((_, f)) = g.types[owner(*o)].attr(n)
                                        && let FieldType::Entity(group) = f.ty
                                    {
                                        add(&mut types[pt].member_of, group);
                                    }
                                }
                            }
                            Atom::InRoles(_) | Atom::IsType(_) | Atom::IsWorkflow(_) => {}
                        }
                    }
                }
            }
        }
    }
    for t in &mut types {
        t.attrs.sort_unstable();
    }
    let actions = g
        .actions
        .iter()
        .enumerate()
        .map(|(ai, a)| {
            let mut ins = inputs[ai].clone();
            ins.sort_unstable();
            let decl = scope.file().actions.iter().find(|x| x.name.ascii() == a.named.alias);
            let computed = a
                .computed
                .iter()
                .enumerate()
                .map(|(ci, cv)| {
                    let (values, members) = match checked.known.computed.get(&(ai, ci)) {
                        Some(Kn::Rule { domain: Domain::Enum(vs), members, .. }) => (Some(vs.clone()), members.clone()),
                        _ => (None, Vec::new()),
                    };
                    ComputedShape { values, members, optional: may_be_absent(g, scope, checked, decl, ai, ci, cv) }
                })
                .collect();
            ActionShape { inputs: ins, computed }
        })
        .collect();
    Shape { namespace: c::Name::parse(&g.namespace), types, actions }
}

/// Whether a computed value may be absent: a principal or a resource type of the action lacks an
/// attribute it reads, an attribute or an input it reads may be absent, or the walk found it
/// absent (a date that stops).
fn may_be_absent(g: &Gate, scope: &Scope, checked: &Checked, decl: Option<&ast::ActionDecl>, ai: usize, ci: usize, cv: &Computed) -> bool {
    let a = &g.actions[ai];
    if let Some(d) = decl
        && let Some(cd) = d.context.iter().find(|x| x.name.ascii() == cv.named.alias)
    {
        let (ps, rs) = scope.computed_for(d, cd);
        if ps.len() < a.principals.len() || rs.len() < a.resources.len() {
            return true;
        }
    }
    let sources: Vec<&Source> = match &cv.how {
        How::Rule { args, .. } | How::Date { of: DateOf::Call { args, .. }, .. } => args.iter().map(|(_, s)| s).collect(),
        How::Date { of: DateOf::Attr(..), .. } | How::Open { .. } => Vec::new(),
    };
    let mut optional_source = sources.iter().any(|s| match s {
        Source::Attr(o, n) => {
            let ts = if *o == Owner::Principal { &a.principals } else { &a.resources };
            ts.iter().any(|&t| g.types[t].attr(n).is_some_and(|(_, f)| f.optional))
        }
        Source::Input(n) => a.input(n).is_some_and(|(_, f)| f.optional),
        Source::Lit(_) | Source::Today => false,
    });
    if let How::Date { of: DateOf::Attr(o, n), .. } = &cv.how {
        let ts = if *o == Owner::Principal { &a.principals } else { &a.resources };
        optional_source |= ts.iter().any(|&t| g.types[t].attr(n).is_some_and(|(_, f)| f.optional));
    }
    if optional_source {
        return true;
    }
    // what the walk found: a value fixed absent in a frame, or a point that leaves it absent
    let Some(Some(space)) = checked.report.spaces.get(ai) else { return false };
    space.frames.iter().any(|f| {
        let Some(at) = f.place(&Slot::Computed(ci)) else { return false };
        f.fixed.iter().any(|(p, v)| *p == at && *v == Val::Absent)
            || f.factors.iter().any(|x| x.places.iter().position(|p| *p == at).is_some_and(|k| x.points.iter().any(|pt| pt.vals[k] == Val::Absent)))
    })
}

// ---------------------------------------------------------------------------------------------
// Names

/// `Shop::Role`, `Shop::Order`.
fn qualified(shape: &Shape, id: &str) -> c::Name {
    let mut path = shape.namespace.path.clone();
    path.push(shape.namespace.id.clone());
    c::Name { path, id: id.to_string() }
}

/// `Shop::Role::"clerk"`.
pub fn role_uid(g: &Gate, shape: &Shape, r: usize) -> c::EntityUid {
    c::EntityUid::new(qualified(shape, ROLE), g.roles[r].named.alias.clone())
}

/// `Shop::Action::"refund_order"`.
pub fn action_uid(g: &Gate, shape: &Shape, a: usize) -> c::EntityUid {
    c::EntityUid::new(qualified(shape, "Action"), g.actions[a].named.alias.clone())
}

/// `Shop::Order`.
pub fn type_name(g: &Gate, shape: &Shape, t: usize) -> c::Name {
    qualified(shape, &g.types[t].named.alias)
}

/// The type of the roles.
pub const ROLE: &str = "Role";

/// A policy's `@id` (DESIGN 5.2): `<file>/<policy>`, and `<file>/<file read>/<policy>` for a
/// forbid of `action any` that a `use gate` file holds (DESIGN 2.10). `file` is the alias of the
/// file generated.
pub fn policy_id(g: &Gate, p: usize) -> String {
    let pol = &g.policies[p];
    match &pol.from {
        None => format!("{}/{}", g.named.alias, pol.named.alias),
        Some((from, _)) => format!("{}/{from}/{}", g.named.alias, pol.named.alias),
    }
}

// ---------------------------------------------------------------------------------------------
// The schema

fn note(key: &str, value: impl Into<String>) -> c::Annotation {
    c::Annotation { key: key.to_string(), value: Some(value.into()), line: 0, col: 0 }
}

/// `@name` when the name is not what Cedar calls it (DESIGN 5.7), and `@doc` when there is one.
fn notes(named: &Named, doc: Option<String>) -> Vec<c::Annotation> {
    let mut out = Vec::new();
    if let Some(d) = doc.filter(|d| !d.is_empty()) {
        out.push(note("doc", d));
    }
    if named.name != named.alias {
        out.push(note("name", named.name.clone()));
    }
    out
}

fn prim(name: &str) -> c::Type {
    c::Type::EntityOrCommon(c::Name::new(name))
}

/// What a value of a field is in Cedar: a bool, a string (an enum's value), a long (a number,
/// counted in its unit), an entity. A date never goes to Cedar.
fn field_type(g: &Gate, f: &Field) -> Option<c::Type> {
    Some(match &f.ty {
        FieldType::Bool => prim("Bool"),
        FieldType::Enum(_) => prim("String"),
        FieldType::Num { .. } => prim("Long"),
        FieldType::Entity(t) => c::Type::EntityOrCommon(c::Name::new(g.types[*t].named.alias.clone())),
        FieldType::Date { .. } => return None,
    })
}

/// `a, b or c`, `a、b か c`.
fn either(items: &[String]) -> Text {
    match items {
        [] => Text::same(String::new()),
        [one] => Text::same(one.clone()),
        [init @ .., last] => {
            let i = init.join(", ");
            let j = init.join("、");
            tr!("{j} か {last}", "{i} or {last}")
        }
    }
}

/// `a and b`, `a、b`.
fn both(items: &[String]) -> Text {
    match items {
        [] => Text::same(String::new()),
        [one] => Text::same(one.clone()),
        [init @ .., last] => {
            let i = init.join(", ");
            let j = items.join("、");
            Text { ja: j, en: format!("{i} and {last}") }
        }
    }
}

/// An enum's values as the strings Cedar is given, each with its own name when it has one.
fn values_text(vs: &[Named]) -> (Vec<String>, Vec<String>) {
    let en = vs.iter().map(|v| if v.name != v.alias { format!("{} ({})", v.alias, v.name) } else { v.alias.clone() }).collect();
    let ja = vs.iter().map(|v| if v.name != v.alias { format!("{}（{}）", v.alias, v.name) } else { v.alias.clone() }).collect();
    (en, ja)
}

/// The doc of an attribute or an input: what its values are in Cedar.
fn field_doc(g: &Gate, f: &Field) -> Option<Text> {
    match &f.ty {
        FieldType::Enum(e) => {
            let (en, ja) = values_text(&g.enums[*e].values);
            let (en, ja) = (en.join(", "), ja.join("、"));
            // no blank after a closing bracket of the Japanese
            let gap = if ja.ends_with('）') { "" } else { " " };
            Some(tr!("{ja}{gap}のどれか", "One of {en}"))
        }
        FieldType::Num { written, lo, hi, .. } => Some(tr!("{written}、{lo}〜{hi}", "{written}, {lo} to {hi}")),
        _ => None,
    }
}

/// What a value given to a rule or a date is, as the gate writes it.
fn source_text(g: &Gate, a: &Action, s: &Source) -> String {
    match s {
        Source::Attr(o, n) => {
            let ts = if *o == Owner::Principal { &a.principals } else { &a.resources };
            let shown = ts.iter().find_map(|&t| g.types[t].attr(n)).map(|(_, f)| f.named.name.clone()).unwrap_or_else(|| n.clone());
            format!("{}.{shown}", o.word())
        }
        Source::Input(n) => a.input(n).map(|(_, f)| f.named.name.clone()).unwrap_or_else(|| n.clone()),
        Source::Lit(Literal::Num(num)) => num.raw.clone(),
        Source::Lit(Literal::Date(d)) => crate::types::day_text(*d),
        Source::Lit(Literal::Bool(b)) => b.to_string(),
        Source::Lit(Literal::Word(w)) => w.clone(),
        Source::Today => "today".to_string(),
    }
}

/// `+00:00` from minutes east of UTC.
fn offset_text(m: i32) -> String {
    let sign = if m < 0 { '-' } else { '+' };
    format!("{sign}{:02}:{:02}", m.abs() / 60, m.abs() % 60)
}

/// The doc of a computed value (DESIGN 3.7): what computes it from what, the values it can be,
/// the types it is computed for, and that the generated code computes it.
fn computed_doc(g: &Gate, scope: &Scope, ai: usize, ci: usize, cs: &ComputedShape) -> Text {
    let a = &g.actions[ai];
    let cv = &a.computed[ci];
    let use_path = |u: usize| g.uses.get(u).map(|u| ritsu_base::naming::quote(&u.path)).unwrap_or_default();
    let from = |args: &[(String, Source)]| -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for (_, s) in args {
            let t = source_text(g, a, s);
            if !v.contains(&t) {
                v.push(t);
            }
        }
        v
    };
    let today = |v: &[String]| v.iter().any(|x| x == "today");
    let (what, sources, reads_today): (String, Vec<String>, bool) = match &cv.how {
        How::Rule { rule, args, output } => {
            let s = from(args);
            let t = today(&s);
            (format!("rulec {} output {}", use_path(*rule), ritsu_base::naming::word_or_quote(output)), s, t)
        }
        How::Date { op, of: DateOf::Call { dates, function, args } } => {
            let s = from(args);
            (format!("today {} koyomi {} date {}", op.symbol(), use_path(*dates), ritsu_base::naming::word_or_quote(function)), s, true)
        }
        How::Date { op, of: DateOf::Attr(o, n) } => {
            let s = source_text(g, a, &Source::Attr(*o, n.clone()));
            (format!("today {} {s}", op.symbol()), Vec::new(), true)
        }
        How::Open { calendar } => (format!("today is open in koyomi {}", use_path(*calendar)), Vec::new(), true),
    };
    let mut en: Vec<String> = Vec::new();
    let mut ja: Vec<String> = Vec::new();
    if sources.is_empty() {
        en.push(what.clone());
        ja.push(what.clone());
    } else {
        let b = both(&sources);
        en.push(format!("{what}, from {}", b.en));
        ja.push(format!("{what}。{} から計算する", b.ja));
    }
    if let Some(vs) = &cs.values {
        let names: Vec<String> = vs.iter().map(|v| v.alias.clone()).collect();
        let e = either(&names);
        // the values follow the sentence they belong to
        let last = en.len() - 1;
        en[last] = format!("{}: {}", en[last], e.en);
        ja.push(format!("値は {}", e.ja));
    }
    if reads_today && let Some(t) = &g.today {
        let o = offset_text(t.offset);
        let last = en.len() - 1;
        en[last] = format!("{}, the day changing at {o}", en[last]);
        ja.push(format!("日は {o} で変わる"));
    }
    // the types it is computed for, when not every type of the action
    if let Some(d) = scope.file().actions.iter().find(|x| x.name.ascii() == a.named.alias)
        && let Some(cd) = d.context.iter().find(|x| x.name.ascii() == cv.named.alias)
    {
        let (ps, rs) = scope.computed_for(d, cd);
        if ps.is_empty() || rs.is_empty() {
            en.push("Never computed: no principal or resource type of the action has what it reads".to_string());
            ja.push("計算することはない。読むものを持つ principal か resource の型が、action に無い".to_string());
        } else {
            let mut only_en: Vec<String> = Vec::new();
            let mut only_ja: Vec<String> = Vec::new();
            if ps.len() < a.principals.len() {
                only_en.push(format!("a principal of type {}", either(&ps).en));
                only_ja.push(format!("principal の型が {}", either(&ps).ja));
            }
            if rs.len() < a.resources.len() {
                only_en.push(format!("a resource of type {}", either(&rs).en));
                only_ja.push(format!("resource の型が {}", either(&rs).ja));
            }
            if !only_en.is_empty() {
                en.push(format!("Only for {}", only_en.join(" and ")));
                ja.push(format!("{} のときだけ計算する", only_ja.join("で、")));
            }
        }
    }
    en.push("Computed by the generated code, never taken from the caller".to_string());
    ja.push("生成したコードが計算し、呼ぶ側からは受け取らない".to_string());
    Text { ja: ja.join("。"), en: en.join(". ") }
}

/// The doc of an input in the context: an argument of the operation the action guards (or of the
/// request, for an action that guards none), and its values in Cedar.
fn input_doc(g: &Gate, a: &Action, f: &Field) -> Text {
    // an action that guards no operation takes its inputs with the request it is asked about
    let (ja, en) = if a.guards.is_empty() { ("リクエストの引数", "An argument of the request") } else { ("操作の引数", "An argument of the operation") };
    match field_doc(g, f) {
        Some(d) => {
            let lower = d.en.chars().next().map(|c| c.to_ascii_lowercase().to_string() + &d.en[c.len_utf8()..]).unwrap_or_default();
            tr!("{ja}。{}", "{en}: {}", d.ja; lower)
        }
        None => Text { ja: ja.to_string(), en: en.to_string() },
    }
}

/// The descriptions of the file and the files it reads, by what they declare.
struct Docs<'a> {
    scope: &'a Scope,
}

impl<'a> Docs<'a> {
    fn of(d: &Option<(String, ast::Span)>) -> Option<String> {
        d.as_ref().map(|(s, _)| s.clone())
    }

    fn role(&self, alias: &str) -> Option<&'a ast::RoleDecl> {
        self.scope.files.iter().find_map(|f| f.roles.iter().find(|r| r.name.ascii() == alias))
    }

    fn entity(&self, alias: &str) -> Option<&'a ast::EntityDecl> {
        self.scope.files.iter().find_map(|f| f.principals.iter().chain(f.resources.iter()).find(|e| e.name.ascii() == alias))
    }

    fn workflow(&self, alias: &str) -> Option<&'a ast::WorkflowDecl> {
        self.scope.files.iter().find_map(|f| f.workflows.iter().find(|w| w.name.ascii() == alias))
    }

    fn action(&self, alias: &str) -> Option<&'a ast::ActionDecl> {
        self.scope.file().actions.iter().find(|a| a.name.ascii() == alias)
    }

    /// The `.gate` that holds a policy: the file's own, or the file a forbid is read from.
    fn policy(&self, p: &Policy) -> Option<&'a ast::Policy> {
        let file = match &p.from {
            None => self.scope.file(),
            Some((from, _)) => self.scope.files.iter().find(|f| f.alias() == from)?,
        };
        file.policies.iter().find(|x| x.name.ascii() == p.named.alias)
    }
}

/// The doc of the type of the roles: each role, its own name, what it is, and what it includes.
fn roles_doc(g: &Gate, docs: &Docs) -> Text {
    let mut en: Vec<String> = Vec::new();
    let mut ja: Vec<String> = Vec::new();
    for r in &g.roles {
        let d = docs.role(&r.named.alias).and_then(|x| Docs::of(&x.description));
        let inc: Vec<String> = r.includes.iter().map(|&i| g.roles[i].named.alias.clone()).collect();
        let mut pe: Vec<String> = Vec::new();
        let mut pj: Vec<String> = Vec::new();
        if r.named.name != r.named.alias {
            pe.push(r.named.name.clone());
            pj.push(r.named.name.clone());
        }
        if let Some(d) = d {
            pe.push(d.clone());
            pj.push(d);
        }
        if !inc.is_empty() {
            pe.push(format!("includes {}", both(&inc).en));
            pj.push(format!("{} を含む", both(&inc).ja));
        }
        let alias = &r.named.alias;
        if pe.is_empty() {
            en.push(alias.clone());
            ja.push(alias.clone());
        } else {
            en.push(format!("{alias} ({})", pe.join("; ")));
            ja.push(format!("{alias}（{}）", pj.join("。")));
        }
    }
    Text { ja: format!("役割：{}", ja.join("、")), en: format!("The roles: {}", en.join(", ")) }
}

/// The doc of the type of the workflows: each workflow, what it does, and its flow.
fn workflows_doc(g: &Gate, docs: &Docs) -> Text {
    let mut en: Vec<String> = Vec::new();
    let mut ja: Vec<String> = Vec::new();
    for w in &g.workflows {
        let d = docs.workflow(&w.named.alias).and_then(|x| Docs::of(&x.description));
        let flow = format!("dandori {}", ritsu_base::naming::quote(&w.flow));
        let mut pe: Vec<String> = Vec::new();
        if w.named.name != w.named.alias {
            pe.push(w.named.name.clone());
        }
        if let Some(d) = d {
            pe.push(d);
        }
        pe.push(flow);
        let alias = &w.named.alias;
        en.push(format!("{alias} ({})", pe.join("; ")));
        ja.push(format!("{alias}（{}）", pe.join("。")));
    }
    if en.is_empty() {
        return tr!("自分の資格で呼ぶワークフロー", "A workflow, calling as itself");
    }
    Text { ja: format!("自分の資格で呼ぶワークフロー：{}", ja.join("、")), en: format!("The workflows, calling as themselves: {}", en.join(", ")) }
}

/// The schema of the gate (DESIGN 5.1).
pub fn schema(g: &Gate, scope: &Scope, shape: &Shape, lang: Lang) -> c::Schema {
    let docs = Docs { scope };
    let mut entity_types: Vec<c::EntityType> = Vec::new();
    if !g.roles.is_empty() {
        entity_types.push(c::EntityType {
            name: ROLE.to_string(),
            kind: c::EntityKind::Standard { member_of: vec![c::Name::new(ROLE)], shape: c::Type::empty_record(), tags: None },
            annotations: vec![note("doc", roles_doc(g, &docs).get(lang))],
            line: 0,
            col: 0,
        });
    }
    for (ti, t) in g.types.iter().enumerate() {
        let ts = &shape.types[ti];
        let mut member_of: Vec<c::Name> = Vec::new();
        if ts.roles {
            member_of.push(c::Name::new(ROLE));
        }
        member_of.extend(ts.member_of.iter().map(|&m| c::Name::new(g.types[m].named.alias.clone())));
        let attrs: Vec<c::Attr> = ts
            .attrs
            .iter()
            .filter_map(|&i| {
                let f = &t.attrs[i];
                let ty = field_type(g, f)?;
                Some(c::Attr { name: f.named.alias.clone(), ty, required: !f.optional, annotations: notes(&f.named, field_doc(g, f).map(|d| d.get(lang).to_string())), line: 0, col: 0 })
            })
            .collect();
        let annotations = match t.kind {
            Kind::Workflow => vec![note("doc", workflows_doc(g, &docs).get(lang))],
            _ => notes(&t.named, docs.entity(&t.named.alias).and_then(|e| Docs::of(&e.description))),
        };
        entity_types.push(c::EntityType {
            name: t.named.alias.clone(),
            kind: c::EntityKind::Standard { member_of, shape: c::Type::Record(c::RecordType { attrs, additional_attributes: false }), tags: None },
            annotations,
            line: 0,
            col: 0,
        });
    }
    let actions = g
        .actions
        .iter()
        .enumerate()
        .map(|(ai, a)| {
            let s = &shape.actions[ai];
            let mut attrs: Vec<c::Attr> = Vec::new();
            for &i in &s.inputs {
                let f = &a.inputs[i];
                if let Some(ty) = field_type(g, f) {
                    attrs.push(c::Attr { name: f.named.alias.clone(), ty, required: !f.optional, annotations: notes(&f.named, Some(input_doc(g, a, f).get(lang).to_string())), line: 0, col: 0 });
                }
            }
            for (ci, cs) in s.computed.iter().enumerate() {
                let cv = &a.computed[ci];
                let ty = if cs.values.is_some() { prim("String") } else { prim("Bool") };
                attrs.push(c::Attr { name: cv.named.alias.clone(), ty, required: !cs.optional, annotations: notes(&cv.named, Some(computed_doc(g, scope, ai, ci, cs).get(lang).to_string())), line: 0, col: 0 });
            }
            let context = if attrs.is_empty() { c::Type::empty_record() } else { c::Type::Record(c::RecordType { attrs, additional_attributes: false }) };
            let mut annotations = notes(&a.named, docs.action(&a.named.alias).and_then(|d| Docs::of(&d.description)));
            // the operations it guards, a reference a line (an annotation is one key a declaration)
            let refs: Vec<String> = a.references().iter().map(|(n, _)| n.text()).collect();
            if !refs.is_empty() {
                annotations.push(note("guards", refs.join("\n")));
            }
            c::Action {
                name: a.named.alias.clone(),
                member_of: None,
                applies_to: Some(c::AppliesTo {
                    principal_types: a.principals.iter().map(|&t| c::Name::new(g.types[t].named.alias.clone())).collect(),
                    resource_types: a.resources.iter().map(|&t| c::Name::new(g.types[t].named.alias.clone())).collect(),
                    context,
                }),
                annotations,
                line: 0,
                col: 0,
            }
        })
        .collect();
    let gate_doc = Docs::of(&scope.file().description);
    c::Schema {
        namespaces: vec![c::Namespace {
            name: Some(shape.namespace.clone()),
            annotations: gate_doc.map(|d| vec![note("doc", d)]).unwrap_or_default(),
            common_types: Vec::new(),
            entity_types,
            actions,
            line: 0,
            col: 0,
        }],
    }
}

// ---------------------------------------------------------------------------------------------
// The policies

fn ex(kind: ExprKind) -> c::Expr {
    c::Expr::new(kind)
}

fn bin(op: BinOp, l: c::Expr, r: c::Expr) -> c::Expr {
    ex(ExprKind::Binary { op, left: Box::new(l), right: Box::new(r) })
}

fn all_of(mut xs: Vec<c::Expr>) -> c::Expr {
    if xs.is_empty() {
        return ex(ExprKind::Bool(true));
    }
    let first = xs.remove(0);
    xs.into_iter().fold(first, |acc, x| bin(BinOp::And, acc, x))
}

fn any_of(mut xs: Vec<c::Expr>) -> c::Expr {
    if xs.is_empty() {
        return ex(ExprKind::Bool(false));
    }
    let first = xs.remove(0);
    xs.into_iter().fold(first, |acc, x| bin(BinOp::Or, acc, x))
}

fn var(v: Var) -> c::Expr {
    ex(ExprKind::Var(v))
}

fn get(of: c::Expr, attr: &str) -> c::Expr {
    ex(ExprKind::GetAttr { expr: Box::new(of), attr: attr.to_string() })
}

fn has(of: c::Expr, attr: &str) -> c::Expr {
    ex(ExprKind::Has { expr: Box::new(of), attrs: vec![attr.to_string()] })
}

fn cmp(op: Op) -> BinOp {
    match op {
        Op::Lt => BinOp::Less,
        Op::Le => BinOp::LessEq,
        Op::Gt => BinOp::Greater,
        Op::Ge => BinOp::GreaterEq,
        Op::Is => BinOp::Eq,
    }
}

/// What a path reads, in a policy: the expression, what guards it (`has`, when the value may be
/// absent), and its field (an attribute or an input) or its computed value.
struct Read<'g> {
    expr: c::Expr,
    guards: Vec<c::Expr>,
    field: Option<&'g Field>,
    computed: Option<&'g ComputedShape>,
}

/// A policy's conditions, compiled against the shape.
struct Compiler<'g> {
    g: &'g Gate,
    shape: &'g Shape,
    p: &'g Policy,
    principals: Vec<usize>,
    resources: Vec<usize>,
}

impl<'g> Compiler<'g> {
    fn owner_types(&self, o: Owner) -> &[usize] {
        if o == Owner::Principal { &self.principals } else { &self.resources }
    }

    /// An attribute of the principal or the resource: read behind `has` when a type the policy can
    /// see lacks it, or holds it as one that may be absent.
    fn attr(&self, o: Owner, name: &str) -> Option<Read<'g>> {
        let g = self.g;
        let mut field: Option<&'g Field> = None;
        let mut guard = false;
        for &t in self.owner_types(o) {
            match g.types[t].attr(name) {
                Some((i, f)) => {
                    if field.is_none() {
                        field = Some(f);
                    }
                    guard |= f.optional || !self.shape.types[t].attrs.contains(&i);
                }
                None => guard = true,
            }
        }
        let f = field?;
        let v = var(if o == Owner::Principal { Var::Principal } else { Var::Resource });
        let guards = if guard { vec![has(v.clone(), &f.named.alias)] } else { Vec::new() };
        Some(Read { expr: get(v, &f.named.alias), guards, field: Some(f), computed: None })
    }

    /// An input or a computed value: read behind `has` when an action of the policy has none of
    /// that name, or holds it as one that may be absent.
    fn value(&self, name: &str) -> Option<Read<'g>> {
        let g = self.g;
        // what the first action that has it reads, and its alias
        let mut found: Option<(String, Read<'g>)> = None;
        let mut guard = false;
        for &a in &self.p.actions {
            match self.shape.context_value(g, a, name) {
                Some(optional) => {
                    guard |= optional;
                    if found.is_none() {
                        let act = &g.actions[a];
                        let (alias, field, computed) = match act.input(name) {
                            Some((_, f)) => (f.named.alias.clone(), Some(f), None),
                            None => {
                                let (ci, cv) = act.computed_value(name)?;
                                (cv.named.alias.clone(), None, Some(&self.shape.actions[a].computed[ci]))
                            }
                        };
                        found = Some((alias.clone(), Read { expr: get(var(Var::Context), &alias), guards: Vec::new(), field, computed }));
                    }
                }
                None => guard = true,
            }
        }
        let (alias, mut r) = found?;
        if guard {
            r.guards.push(has(var(Var::Context), &alias));
        }
        Some(r)
    }

    fn read(&self, p: &Path) -> Option<Read<'g>> {
        match p {
            Path::Attr(o, n) => self.attr(*o, n),
            Path::Value(n) => self.value(n),
        }
    }

    fn term(&self, t: &Term) -> Option<Read<'g>> {
        match t {
            Term::Principal => Some(Read { expr: var(Var::Principal), guards: Vec::new(), field: None, computed: None }),
            Term::Attr(o, n) => self.attr(*o, n),
        }
    }

    fn guarded(mut guards: Vec<c::Expr>, test: c::Expr) -> c::Expr {
        guards.push(test);
        all_of(guards)
    }

    /// A constant, as the value read compares with it.
    fn literal(&self, r: &Read, lit: &Literal) -> Option<c::Expr> {
        match lit {
            Literal::Bool(b) => Some(ex(ExprKind::Bool(*b))),
            Literal::Num(num) => {
                let unit = match r.field.map(|f| &f.ty) {
                    Some(FieldType::Num { unit, .. }) => unit,
                    _ => return None,
                };
                let k = crate::types::count(num, unit).ok()?;
                Some(ex(ExprKind::Long(i64::try_from(k).ok()?)))
            }
            Literal::Word(w) => {
                if let Some(cs) = r.computed {
                    let vs = cs.values.as_ref()?;
                    // by the rule's name or the public name, or by the name of the generated code
                    let k = vs.iter().position(|v| v.is(w)).or_else(|| cs.members.iter().position(|m| m == w))?;
                    return Some(ex(ExprKind::Str(vs[k].alias.clone())));
                }
                match r.field.map(|f| &f.ty) {
                    Some(FieldType::Enum(e)) => self.g.enums[*e].values.iter().find(|v| v.is(w)).map(|v| ex(ExprKind::Str(v.alias.clone()))),
                    _ => None,
                }
            }
            Literal::Date(_) => None,
        }
    }

    fn atom(&self, a: &Atom) -> c::Expr {
        let never = || ex(ExprKind::Bool(false));
        let ns = self.shape;
        match a {
            Atom::InRoles(rs) => any_of(rs.iter().map(|&r| bin(BinOp::In, var(Var::Principal), ex(ExprKind::Entity(role_uid(self.g, ns, r))))).collect()),
            Atom::IsType(t) => ex(ExprKind::Is { expr: Box::new(var(Var::Principal)), ty: type_name(self.g, ns, *t), in_expr: None }),
            Atom::IsWorkflow(w) => bin(BinOp::Eq, var(Var::Principal), ex(ExprKind::Entity(workflow_uid(self.g, ns, *w)))),
            Atom::True(p) => match self.read(p) {
                Some(r) => Self::guarded(r.guards, r.expr),
                None => never(),
            },
            Atom::Is(p, lit) | Atom::Cmp(p, _, lit) => {
                let op = match a {
                    Atom::Cmp(_, op, _) => cmp(*op),
                    _ => BinOp::Eq,
                };
                match self.read(p) {
                    Some(r) => match self.literal(&r, lit) {
                        Some(k) => Self::guarded(r.guards, bin(op, r.expr, k)),
                        None => never(),
                    },
                    None => never(),
                }
            }
            Atom::Same(x, y) => match (self.term(x), self.term(y)) {
                (Some(a), Some(b)) => {
                    let mut guards = a.guards;
                    guards.extend(b.guards);
                    Self::guarded(guards, bin(BinOp::Eq, a.expr, b.expr))
                }
                _ => never(),
            },
            Atom::Member(t) => match self.term(t) {
                Some(r) => Self::guarded(r.guards, bin(BinOp::In, var(Var::Principal), r.expr)),
                None => never(),
            },
            Atom::Eq(p, q) => {
                let (x, y) = match (path_term(p), path_term(q)) {
                    (Some(x), Some(y)) => (self.term(&x), self.term(&y)),
                    _ => (self.read(p), self.read(q)),
                };
                match (x, y) {
                    (Some(a), Some(b)) => {
                        let mut guards = a.guards;
                        guards.extend(b.guards);
                        Self::guarded(guards, bin(BinOp::Eq, a.expr, b.expr))
                    }
                    _ => never(),
                }
            }
        }
    }

    fn expr(&self, e: &Expr) -> c::Expr {
        match e {
            Expr::Atom(a) => self.atom(a),
            Expr::Not(x) => ex(ExprKind::Not(Box::new(self.expr(x)))),
            Expr::And(xs) => all_of(xs.iter().map(|x| self.expr(x)).collect()),
            Expr::Or(xs) => any_of(xs.iter().map(|x| self.expr(x)).collect()),
        }
    }
}

/// `Shop::Workflow::"returns"`.
pub fn workflow_uid(g: &Gate, shape: &Shape, w: usize) -> c::EntityUid {
    c::EntityUid::new(qualified(shape, crate::ast::WORKFLOW_TYPE), g.workflows[w].named.alias.clone())
}

/// The policies of the gate (DESIGN 5.2), in the order written, a forbid read from a `use gate`
/// file after the file's own. A policy on no action (`action any` in a file with none) is left
/// out: it can apply to nothing.
pub fn policies(g: &Gate, scope: &Scope, shape: &Shape) -> c::PolicySet {
    let docs = Docs { scope };
    let mut out = Vec::new();
    for (pi, p) in g.policies.iter().enumerate() {
        if p.actions.is_empty() {
            continue;
        }
        let id = policy_id(g, pi);
        let mut annotations = vec![note("id", id.clone())];
        if p.named.name != p.named.alias {
            annotations.push(note("name", p.named.name.clone()));
        }
        if let Some(d) = docs.policy(p).and_then(|x| Docs::of(&x.body.description)) {
            annotations.push(note("doc", d));
        }
        let mut conditions: Vec<c::Condition> = Vec::new();
        let principal = match &p.who {
            Who::Anyone => c::Scope::Any,
            Who::InRoles(rs) if rs.len() == 1 => c::Scope::In(c::EntityOrSlot::Entity(role_uid(g, shape, rs[0]))),
            Who::InRoles(rs) => {
                let any = any_of(rs.iter().map(|&r| bin(BinOp::In, var(Var::Principal), ex(ExprKind::Entity(role_uid(g, shape, r))))).collect());
                conditions.push(c::Condition { kind: CondKind::When, body: any, line: 0, col: 0 });
                c::Scope::Any
            }
            Who::IsType(t) => c::Scope::Is(type_name(g, shape, *t)),
            Who::IsWorkflow(w) => c::Scope::Eq(c::EntityOrSlot::Entity(workflow_uid(g, shape, *w))),
        };
        let action = if p.actions.len() == 1 && !p.any { c::ActionScope::Eq(action_uid(g, shape, p.actions[0])) } else { c::ActionScope::InList(p.actions.iter().map(|&a| action_uid(g, shape, a)).collect()) };
        let (principals, resources) = envs(g, p);
        let resource = match resources.as_slice() {
            [one] => c::Scope::Is(type_name(g, shape, *one)),
            _ => c::Scope::Any,
        };
        let comp = Compiler { g, shape, p, principals, resources };
        for cond in &p.conds {
            conditions.push(c::Condition { kind: if cond.when { CondKind::When } else { CondKind::Unless }, body: comp.expr(&cond.expr), line: 0, col: 0 });
        }
        out.push(c::Policy { id, annotations, effect: if p.permit { c::Effect::Permit } else { c::Effect::Forbid }, principal, action, resource, conditions, line: 0, col: 0 });
    }
    c::PolicySet { policies: out }
}

// ---------------------------------------------------------------------------------------------
// The files

/// The four files of a gate's Cedar, each with its path under the directory `gen` writes to
/// (DESIGN 5.6): `cedar/<alias>.cedar`, `.cedarschema`, `.cedarschema.json`, `.policies.json`.
pub struct Files {
    pub policies: String,
    pub schema: String,
    pub schema_json: String,
    pub policies_json: String,
}

impl Files {
    /// Each file, with its path under the directory written to.
    pub fn paths(&self, alias: &str) -> Vec<(String, String)> {
        vec![
            (format!("cedar/{alias}.cedar"), self.policies.clone()),
            (format!("cedar/{alias}.cedarschema"), self.schema.clone()),
            (format!("cedar/{alias}.cedarschema.json"), self.schema_json.clone()),
            (format!("cedar/{alias}.policies.json"), self.policies_json.clone()),
        ]
    }
}

/// The head of a generated Cedar file (ritsu's DESIGN 9.2): the line Go's tools look for, and the
/// `.gate` with its digest. `shown` is the path the head names (the file's name for sekisho's own
/// command, the path from the project's root for `ritsu gen`).
pub fn head(scope: &Scope, shown: &str, lang: Lang) -> String {
    let f = scope.file();
    let sha = ritsu_base::sha256::hex(f.src.as_bytes());
    let version = f.version.strip_prefix('v').unwrap_or(&f.version);
    let source = ritsu_emit::header::Source { path: shown, kind: "gate", name: &f.name.text, version, sha256: &sha };
    let c = ritsu_emit::header::Comment::Slashes;
    c.line(&ritsu_emit::header::generated("sekisho")) + &c.line(source.line().get(lang))
}

/// The Cedar of a file that passes its check: its policies laid out as `cedar format` lays them
/// out, its schema as `cedar translate-schema` writes it, and the JSON forms of both, each as the
/// CLI prints it for the text. Err only when ritsu-base's Cedar does not read what it is given,
/// which is a bug of sekisho's.
pub fn files(scope: &Scope, checked: &Checked, shown: &str, lang: Lang) -> Result<Files, c::Error> {
    let g = &checked.gate;
    let shape = shape(g, scope, checked);
    let h = head(scope, shown, lang);
    let set = policies(g, scope, &shape);
    let body = if set.policies.is_empty() { String::new() } else { c::write_policies(&set)? };
    // the head goes right above the first policy, as `cedar format` puts a comment
    let policies = c::format_policies(&format!("{h}{body}"), 80, 2)?;
    let schema_text = c::write_schema(&schema(g, scope, &shape, lang))?;
    let schema = format!("{h}{schema_text}");
    let schema_json = c::schema_to_json(&c::parse_schema(&schema)?).compact() + "\n";
    let policies_json = c::policies_to_json(&c::parse_policies(&policies)?).compact() + "\n";
    Ok(Files { policies, schema, schema_json, policies_json })
}
