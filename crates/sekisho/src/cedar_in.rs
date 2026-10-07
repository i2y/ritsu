//! Cedar's policies and schemas written by hand, as the port `Gates` answers for them (DESIGN
//! 1.3): a policy set (`.cedar`) and the schema beside it (`.cedarschema`, else
//! `.cedarschema.json`, of the same name), read with ritsu-base's reader ([`ritsu_base::cedar`]).
//!
//! - What it holds ([`facts`]): every action of the schema, with the operations its `@guards`
//!   names (a reference a line, from the root, as sekisho writes the annotation in the schemas it
//!   generates), the principal and resource types it applies to, and every policy with its `@id`.
//! - What it names ([`references`]): the operations of the `@guards` of the schema's actions, for
//!   sakai, which holds an operation to be guarded in its own context (sakai's E211).
//! - How far someone who asks is allowed an action ([`allowed`]), over every combination of the
//!   values the action's policies read, counted as a `.gate`'s are when the policies are written in
//!   the finite part sekisho's own conditions are made of: the scope; the roles and the groups a
//!   principal is in (`principal in Role::"clerk"`); an attribute of the principal, the resource or
//!   the context (one step, as declared in the schema) compared with a string, a boolean or an
//!   integer; two such attributes of strings or of booleans compared; `has`; an entity attribute
//!   that is the principal, or another such attribute, and the principal in an entity attribute;
//!   the resource in an entity; `&&`, `||`, `!` and `if … then … else`. A question that reaches
//!   anything else (arithmetic, `like`, sets and their methods, the extensions, an attribute of an
//!   attribute, two integers of attributes compared, a template's slot) is undecided, with the
//!   policy and what it reads.
//! - Every combination of a question, with the decision, the policies that decide it and what each
//!   policy on the action comes to, and the request and the entities that make it, as tests of
//!   `cedar run-tests` ([`cases`]): the tests hold them to the official CLI (DESIGN 6.5).
//!
//! A combination is counted only when a request makes it, as the schema says what a request can be:
//! which types an entity can be in, which attributes each type has and of what type, which terms
//! can be the same entity. The roles of a principal are the entities it is in: Cedar keeps who is
//! in what with the entities, which the policies and the schema do not hold, so an asker's roles
//! are counted as given (those the schema lets its type be in), with no role inside another (a
//! gate's `includes` is in the `.gate`; for Cedar written by hand, the asker lists every role the
//! principal is in).

use ritsu_base::cedar::{self, ActionScope, BinOp, EntityOrSlot, EntityUid, Expr, ExprKind, PolicySet, Schema, Scope, Var};
use ritsu_base::json::Json;
use ritsu_base::naming::{self, Name};
use ritsu_base::text::Text;
use ritsu_ports::{Allowance, Asker, Found, GateAction, GateFacts, GatePolicy, Reference, Said};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The endings of the files of a pair, the policies' last.
const SCHEMAS: [&str; 2] = [".cedarschema.json", ".cedarschema"];
const POLICIES: &str = ".cedar";

/// The name a pair's files share: `policies/refunds` of `policies/refunds.cedarschema`.
pub fn stem(file: &str) -> Option<&str> {
    SCHEMAS.iter().chain([POLICIES].iter()).find_map(|x| file.strip_suffix(x))
}

/// A policy set and its schema, as read.
struct Pair {
    /// The policies, when the pair has them: the file (from the root) and its text.
    policies: Option<(String, String, PolicySet)>,
    /// The schema: the file (from the root), its text, and what it declares.
    schema: (String, String, Schema),
}

fn said(file: &str, line: Option<usize>, message: Text) -> Vec<Said> {
    vec![Said { code: String::new(), file: file.to_string(), line, message }]
}

/// Where a text of Cedar's does not read, and why.
fn unread_at(file: &str, e: cedar::Error) -> Vec<Said> {
    said(file, Some(e.line), tr!("{}:{}: {}", "{}:{}: {}", e.line, e.col, e.message.ja; e.line, e.col, e.message.en))
}

fn text(root: &Path, f: &str) -> Option<String> {
    ritsu_base::fs::read_to_string(ritsu_base::paths::on_disk(root, f)).ok()
}

fn schema_of(file: &str, text: &str) -> Result<Schema, Vec<Said>> {
    if file.ends_with(".json") { cedar::parse_schema_json(text) } else { cedar::parse_schema(text) }.map_err(|e| unread_at(file, e))
}

/// The pair of `file` (either of its files, from `root`): the schema beside the policies, or the
/// policies beside the schema (none is a set of no policies).
fn read(root: &Path, file: &str) -> Result<Pair, Vec<Said>> {
    let Some(stem) = stem(file) else {
        return Err(said(file, None, tr!("Cedar のファイルではありません（.cedar、.cedarschema、.cedarschema.json）", "not a file of Cedar's (.cedar, .cedarschema, .cedarschema.json)")));
    };
    // the schema: the Cedar format first, as `sekisho gen` writes both
    let Some((sf, st)) = [format!("{stem}.cedarschema"), format!("{stem}.cedarschema.json")].into_iter().find_map(|f| text(root, &f).map(|t| (f, t))) else {
        return Err(said(
            file,
            None,
            tr!("{stem}.cedar の横に、スキーマ（{stem}.cedarschema か {stem}.cedarschema.json）がありません", "There is no schema ({stem}.cedarschema or {stem}.cedarschema.json) beside {stem}.cedar"),
        ));
    };
    let schema = schema_of(&sf, &st)?;
    let pf = format!("{stem}{POLICIES}");
    let policies = match text(root, &pf) {
        Some(pt) => {
            let set = cedar::parse_policies(&pt).map_err(|e| unread_at(&pf, e))?;
            Some((pf, pt, set))
        }
        None => None,
    };
    Ok(Pair { policies, schema: (sf, st, schema) })
}

/// The operations an action's `@guards` names, a reference a line, each with the annotation's
/// line; what is wrong with a line that is no reference.
fn guards(file: &str, a: &cedar::Action) -> Result<Vec<(Name, usize)>, Vec<Said>> {
    let mut out = Vec::new();
    for an in a.annotations.iter().filter(|x| x.key == "guards") {
        for (i, l) in an.value.as_deref().unwrap_or("").lines().enumerate() {
            let l = l.trim();
            if l.is_empty() {
                continue;
            }
            match naming::parse_one(l) {
                Ok(n) => out.push((n, an.line)),
                Err(e) => {
                    let (n, why) = (i + 1, e.text());
                    return Err(said(
                        file,
                        Some(an.line),
                        tr!(
                            "action `{}` の `@guards` の {n} 行目 `{l}` を、参照として読めません: {}",
                            "Line {n} of the `@guards` of the action `{}`, `{l}`, does not read as a reference: {}",
                            a.name, why.ja; a.name, why.en
                        ),
                    ));
                }
            }
        }
    }
    Ok(out)
}

/// What sekisho knows of a policy set and its schema: each action with the operations it guards,
/// and each policy.
pub fn facts(root: &Path, file: &str) -> Result<GateFacts, Vec<Said>> {
    let p = read(root, file)?;
    let (sf, st, schema) = &p.schema;
    let mut actions = Vec::new();
    let mut namespace = String::new();
    for ns in &schema.namespaces {
        if !ns.actions.is_empty() && namespace.is_empty() {
            namespace = ns.name.as_ref().map(|n| n.to_string()).unwrap_or_default();
        }
        for a in &ns.actions {
            let applies = a.applies_to.as_ref();
            actions.push(GateAction {
                name: a.name.clone(),
                alias: a.name.clone(),
                line: a.line,
                guards: guards(sf, a)?,
                principals: applies.map(|t| t.principal_types.iter().map(|n| n.id.clone()).collect()).unwrap_or_default(),
                resources: applies.map(|t| t.resource_types.iter().map(|n| n.id.clone()).collect()).unwrap_or_default(),
                nobody: None,
            });
        }
    }
    // the pair's SHA-256 is of its policies and its schema, one after the other
    let bytes = match &p.policies {
        Some((_, pt, _)) => format!("{pt}{st}"),
        None => st.clone(),
    };
    let stem = stem(sf).unwrap_or(sf).rsplit('/').next().unwrap_or("").to_string();
    Ok(GateFacts {
        name: stem.clone(),
        alias: stem,
        version: String::new(),
        sha256: ritsu_base::sha256::hex(bytes.as_bytes()),
        namespace,
        actions,
        workflows: Vec::new(),
        policies: p.policies.as_ref().map(|(_, _, set)| set.policies.iter().map(|x| GatePolicy { name: x.id.clone(), id: x.id.clone(), permit: x.effect == cedar::Effect::Permit, line: x.line }).collect()).unwrap_or_default(),
        expects: Vec::new(),
        separations: Vec::new(),
    })
}

/// The operations the `@guards` of the actions of a schema names (`guards`); a policy set names
/// none, and is only held to reading.
pub fn references(root: &Path, file: &str) -> Result<Vec<Reference>, Vec<Said>> {
    let t = text(root, file).ok_or_else(|| vec![Said::unreadable(file, "no such file")])?;
    if file.ends_with(POLICIES) {
        cedar::parse_policies(&t).map_err(|e| unread_at(file, e))?;
        return Ok(Vec::new());
    }
    let schema = schema_of(file, &t)?;
    let mut out = Vec::new();
    for ns in &schema.namespaces {
        for a in &ns.actions {
            for (n, line) in guards(file, a)? {
                out.push(Reference { line, target: n, how: "guards".into() });
            }
        }
    }
    out.sort_by_key(|r| r.line);
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// How far an asker is allowed an action

/// The most combinations an action's question walks before it is undecided.
const BUDGET: u128 = 1_000_000;

/// The full name of the type `id` declared in the namespace `ns`: `Shop::User`, or `User` outside
/// any namespace.
fn full(ns: Option<&cedar::Name>, id: &str) -> String {
    match ns {
        Some(n) => format!("{n}::{id}"),
        None => id.to_string(),
    }
}

/// Whether two entity literals name the same entity, wherever each is written.
fn same_uid(a: &EntityUid, b: &EntityUid) -> bool {
    a.ty == b.ty && a.id == b.id
}

/// The types a schema declares, by their full names, and the names it writes, made full as Cedar
/// resolves them.
#[derive(Clone, Copy)]
struct Types<'a> {
    schema: &'a Schema,
}

impl<'a> Types<'a> {
    /// The entity type of a full name, with the namespace it is declared in.
    fn entity(&self, name: &str) -> Option<(Option<&'a cedar::Name>, &'a cedar::EntityType)> {
        self.schema.namespaces.iter().find_map(|ns| ns.entity_types.iter().find(|e| full(ns.name.as_ref(), &e.name) == name).map(|e| (ns.name.as_ref(), e)))
    }

    /// The common type a name written in the namespace `ns` reaches: with a path, as written;
    /// without, the namespace's own, else the one outside any namespace.
    fn common(&self, ns: Option<&cedar::Name>, n: &cedar::Name) -> Option<(Option<&'a cedar::Name>, &'a cedar::CommonType)> {
        let find = |name: &str| self.schema.namespaces.iter().find_map(|x| x.common_types.iter().find(|c| full(x.name.as_ref(), &c.name) == name).map(|c| (x.name.as_ref(), c)));
        if n.path.is_empty() { find(&full(ns, &n.id)).or_else(|| find(&n.id)) } else { find(&n.to_string()) }
    }

    /// The full name of an entity type written in the namespace `ns`: with a path, as written;
    /// without, the namespace's own type when it declares one, else the type outside any namespace.
    fn resolve(&self, ns: Option<&cedar::Name>, n: &cedar::Name) -> String {
        if !n.path.is_empty() {
            return n.to_string();
        }
        let own = full(ns, &n.id);
        if self.entity(&own).is_some() { own } else { n.id.clone() }
    }

    /// The types an entity of the type `name` may have as its parents (`in [...]`).
    fn parents(&self, name: &str) -> Vec<String> {
        match self.entity(name) {
            Some((ns, e)) => match &e.kind {
                cedar::EntityKind::Standard { member_of, .. } => member_of.iter().map(|m| self.resolve(ns, m)).collect(),
                cedar::EntityKind::Enum(_) => Vec::new(),
            },
            None => Vec::new(),
        }
    }

    /// The types from an entity of the type `from` up to an ancestor of the type `to`, its parent's
    /// first and `to` last: the shortest such chain; None when the schema lets no entity of `from`
    /// be in one of `to`.
    fn chain(&self, from: &str, to: &str) -> Option<Vec<String>> {
        let mut paths: Vec<Vec<String>> = vec![Vec::new()];
        let mut seen: Vec<String> = vec![from.to_string()];
        let mut i = 0;
        while i < paths.len() {
            let at = paths[i].last().map(String::as_str).unwrap_or(from).to_string();
            for p in self.parents(&at) {
                let mut next = paths[i].clone();
                next.push(p.clone());
                if p == to {
                    return Some(next);
                }
                if !seen.contains(&p) {
                    seen.push(p);
                    paths.push(next);
                }
            }
            i += 1;
        }
        None
    }

    /// Whether an entity of the type `from` can be in one of the type `to`, other than by being it.
    fn can_be_in(&self, from: &str, to: &str) -> bool {
        self.chain(from, to).is_some()
    }

    /// Whether the type is one whose entities the schema lists (`entity Color enum [...]`).
    fn enumerated(&self, name: &str) -> bool {
        matches!(self.entity(name), Some((_, e)) if matches!(e.kind, cedar::EntityKind::Enum(_)))
    }
}

/// An entity term: the principal, the resource, or the entity an attribute of one names.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Term {
    Principal,
    Resource,
    Attr(Var, String),
}

impl Term {
    fn shown(&self) -> String {
        match self {
            Term::Principal => "principal".into(),
            Term::Resource => "resource".into(),
            Term::Attr(v, a) => format!("{}.{a}", v.as_str()),
        }
    }
}

/// The type of an attribute a policy reads, as the schema declares it: an entity type by its full
/// name.
#[derive(Clone, Debug, PartialEq)]
enum AttrTy {
    Bool,
    Long,
    String,
    Entity(String),
}

/// One value an attribute can be in a combination.
#[derive(Clone, Debug, PartialEq)]
enum AttrVal {
    Absent,
    Bool(bool),
    Long(i64),
    Str(String),
    /// A string none of the policies compares it with.
    OtherStr,
    /// An entity attribute that is there (what it is, the relations say).
    Present,
}

/// One dimension of the combinations.
#[derive(Clone, Debug)]
enum Dim {
    /// The type of the resource: an index into the action's resource types.
    ResourceType,
    /// Who the principal is, of the literals of its type the policies compare it with (the last
    /// choice: none of them).
    PrincipalId(Vec<EntityUid>),
    /// Which the resource is, of the literals of its types the policies compare it with.
    ResourceId(Vec<EntityUid>),
    /// Whether the resource is in an entity, other than by being it.
    ResourceIn(EntityUid),
    /// An attribute, its type, the values it can be, and the integers a policy compares it with.
    Attr { owner: Var, name: String, ty: AttrTy, values: Vec<AttrVal>, ints: Vec<i64> },
    /// Whether two entity terms of one type are the same entity.
    Same(Term, Term),
    /// Whether two attributes of strings, each a string no policy compares it with, are the same
    /// string.
    SameText(Term, Term),
    /// Whether the principal is in the entity an attribute names, other than by being it.
    Member(Term),
}

/// What a value of an expression is, in one combination.
#[derive(Clone, Debug, PartialEq)]
enum Val {
    Bool(bool),
    Long(i64),
    Str(String),
    /// The string of an attribute that no policy names: which attribute's.
    OtherStr(Term),
    Ent(Term),
    Lit(EntityUid),
}

/// What a policy reads that sekisho does not count, and where: what, its line, its column.
type Out = (String, usize, usize);

fn outside(what: &str, e: &Expr) -> Out {
    (what.to_string(), e.line, e.col)
}

/// The first part of an expression, in the order Cedar reads it, that no condition sekisho counts
/// is made of: a pattern, arithmetic, a method, a function, a set, a record, a template's slot.
fn never_counted(e: &Expr) -> Option<Out> {
    let mut found = None;
    cedar::walk(e, &mut |x| {
        if found.is_some() {
            return;
        }
        found = match &x.kind {
            ExprKind::Like { .. } => Some(outside("like", x)),
            ExprKind::Binary { op: op @ (BinOp::Add | BinOp::Sub | BinOp::Mul), .. } => Some(outside(op.as_str(), x)),
            ExprKind::Neg(inner) if !matches!(inner.kind, ExprKind::Long(_)) => Some(outside("-", x)),
            ExprKind::Method { name, .. } => Some(outside(&format!(".{name}()"), x)),
            ExprKind::Call { func, .. } => Some(outside(&format!("{func}()"), x)),
            ExprKind::Set(_) => Some(outside("set", x)),
            ExprKind::Record(_) => Some(outside("record", x)),
            ExprKind::Slot(_) => Some(outside("slot", x)),
            _ => None,
        };
    });
    found
}

/// Why sekisho does not decide a question about Cedar written by hand (P7).
#[derive(Clone, Debug, PartialEq)]
pub enum Undecided {
    /// A policy on the action reads what sekisho does not count: the policy, what it is (`like`,
    /// `*`, `.contains()`, `resource.tags`, …), and where.
    Outside { policy: String, what: String, line: usize, col: usize },
    /// A policy on the action is a template, which a link gives what it is for.
    Template { policy: String },
    /// The principal or the resource is of a type whose entities the schema lists.
    Enumerated { ty: String },
    /// More combinations than sekisho counts.
    Over { total: u128 },
}

impl Undecided {
    /// Why, in both languages, as the port says it.
    pub fn text(&self) -> Text {
        match self {
            Undecided::Outside { policy, what, line, col } => tr!(
                "ポリシー {policy} の、{line} 行 {col} 列の {what} は、sekisho が数える有限の部分の外です",
                "the {what} at line {line}, column {col} is outside the finite part sekisho counts, in the policy {policy}"
            ),
            Undecided::Template { policy } => tr!("ポリシー {policy} はテンプレートで、何に当てはまるかはリンクで決まります", "the policy {policy} is a template, which a link gives what it is for"),
            Undecided::Enumerated { ty } => {
                tr!("{ty} は、スキーマが ID を並べたエンティティ型で、sekisho が数える有限の部分の外です", "{ty} is an entity type whose ids the schema lists, outside the finite part sekisho counts")
            }
            Undecided::Over { total } => tr!("組み合わせが {total} 通りあり、数える上限の {BUDGET} 通りを超えます", "there are {total} combinations, past the {BUDGET} counted at most"),
        }
    }
}

/// One policy on the action, as one condition: its scope and its `when` and `unless`, in Cedar's
/// order (the principal, the resource, then each condition as written).
struct On {
    id: String,
    permit: bool,
    cond: Expr,
}

/// The action's question: what the request can be, and the dimensions its policies read.
struct World<'a> {
    types: Types<'a>,
    /// The namespace the action is declared in, which its types and its context are written in.
    action_ns: Option<&'a cedar::Name>,
    /// The action, as a request names it.
    action_uid: EntityUid,
    /// The principal's type (its full name), and the roles (the ids of the entities) it is in.
    principal_ty: String,
    roles: Vec<String>,
    /// The workflow, when the asker is one (`Workflow::"<name>"`).
    workflow: Option<String>,
    resource_types: Vec<String>,
    context: Option<&'a cedar::Type>,
    dims: Vec<Dim>,
    /// The entities the policies ask the principal to be in (`principal in Role::"clerk"`).
    groups: Vec<EntityUid>,
}

impl<'a> World<'a> {
    fn record(&self, ns: Option<&'a cedar::Name>, t: &'a cedar::Type) -> Option<(Option<&'a cedar::Name>, &'a cedar::RecordType)> {
        match t {
            cedar::Type::Record(r) => Some((ns, r)),
            cedar::Type::CommonRef(n) | cedar::Type::EntityOrCommon(n) => {
                let (cns, c) = self.types.common(ns, n)?;
                match &c.ty {
                    cedar::Type::Record(r) => Some((cns, r)),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// The attributes an entity of the type `ty` has, with the namespace their types are written in.
    fn shape(&self, ty: &str) -> Option<(Option<&'a cedar::Name>, &'a cedar::RecordType)> {
        let (ns, e) = self.types.entity(ty)?;
        match &e.kind {
            cedar::EntityKind::Standard { shape, .. } => self.record(ns, shape),
            cedar::EntityKind::Enum(_) => None,
        }
    }

    /// The declaration of the attribute `name` of `owner` — of the resource type `rt`, of the
    /// principal's type, of the action's context — with the namespace its type is written in.
    fn decl(&self, owner: Var, rt: &str, name: &str) -> Option<(Option<&'a cedar::Name>, &'a cedar::Attr)> {
        let (ns, r) = match owner {
            Var::Principal => self.shape(&self.principal_ty)?,
            Var::Resource => self.shape(rt)?,
            Var::Context => self.record(self.action_ns, self.context?)?,
            Var::Action => return None,
        };
        r.attrs.iter().find(|a| a.name == name).map(|a| (ns, a))
    }

    /// The declarations of the attribute, one for each type of its owner that has it.
    fn decls(&self, owner: Var, name: &str) -> Vec<(Option<&'a cedar::Name>, &'a cedar::Attr)> {
        match owner {
            Var::Resource => self.resource_types.iter().filter_map(|rt| self.decl(owner, rt, name)).collect(),
            _ => self.decl(owner, "", name).into_iter().collect(),
        }
    }

    /// The type of an attribute, the same in every type of its owner that declares it; None when
    /// none declares it. Err for one sekisho does not count: a set, a record, an extension, an
    /// entity type whose entities the schema lists, or two types.
    fn attr_ty(&self, owner: Var, name: &str) -> Result<Option<AttrTy>, ()> {
        let mut ty: Option<AttrTy> = None;
        for (ns, d) in self.decls(owner, name) {
            let t = self.ty_of(ns, &d.ty).ok_or(())?;
            match &ty {
                Some(x) if *x != t => return Err(()),
                _ => ty = Some(t),
            }
        }
        Ok(ty)
    }

    /// Whether the attribute may be absent: a type of its owner lacks it or declares it optional.
    fn optional(&self, owner: Var, name: &str) -> bool {
        match owner {
            Var::Resource => self.resource_types.iter().any(|rt| self.decl(owner, rt, name).is_none_or(|(_, d)| !d.required)),
            _ => self.decl(owner, "", name).is_none_or(|(_, d)| !d.required),
        }
    }

    fn ty_of(&self, ns: Option<&'a cedar::Name>, t: &'a cedar::Type) -> Option<AttrTy> {
        let entity = |n: String| (self.types.entity(&n).is_some() && !self.types.enumerated(&n)).then_some(AttrTy::Entity(n));
        match t {
            cedar::Type::Long => Some(AttrTy::Long),
            cedar::Type::String => Some(AttrTy::String),
            cedar::Type::Bool => Some(AttrTy::Bool),
            cedar::Type::Entity(n) => entity(self.types.resolve(ns, n)),
            cedar::Type::EntityOrCommon(n) => {
                if n.path.is_empty() {
                    match n.id.as_str() {
                        "Long" => return Some(AttrTy::Long),
                        "String" => return Some(AttrTy::String),
                        "Bool" | "Boolean" => return Some(AttrTy::Bool),
                        _ => {}
                    }
                }
                match self.types.common(ns, n) {
                    Some((cns, c)) => self.ty_of(cns, &c.ty),
                    None => entity(self.types.resolve(ns, n)),
                }
            }
            cedar::Type::CommonRef(n) => {
                let (cns, c) = self.types.common(ns, n)?;
                self.ty_of(cns, &c.ty)
            }
            _ => None,
        }
    }

    /// The type of an entity term in a combination whose resource is of the type `rt`.
    fn term_ty(&self, t: &Term, rt: &str) -> Option<String> {
        match t {
            Term::Principal => Some(self.principal_ty.clone()),
            Term::Resource => Some(rt.to_string()),
            Term::Attr(v, a) => {
                let (ns, d) = self.decl(*v, rt, a)?;
                match self.ty_of(ns, &d.ty)? {
                    AttrTy::Entity(n) => Some(n),
                    _ => None,
                }
            }
        }
    }

    /// The entity type of a term, whatever the resource's type: an attribute's, when every type
    /// that declares it gives it one.
    fn entity_ty(&self, t: &Term) -> Option<String> {
        match t {
            Term::Principal => Some(self.principal_ty.clone()),
            Term::Resource => None,
            Term::Attr(v, n) => match self.attr_ty(*v, n) {
                Ok(Some(AttrTy::Entity(e))) => Some(e),
                _ => None,
            },
        }
    }

    /// Whether a term is an entity: the principal, the resource, or an attribute declared as one.
    fn is_entity(&self, t: &Term) -> bool {
        matches!(t, Term::Principal | Term::Resource) || self.entity_ty(t).is_some()
    }

    fn dim_of(&self, f: impl Fn(&Dim) -> bool) -> Option<usize> {
        self.dims.iter().position(f)
    }

    /// The attribute `owner.name`: its dimension, made when first read, with `lit` among the values
    /// a policy compares it with. An attribute no type declares has none, and errs in every
    /// combination: the policy holds in none.
    fn attr(&mut self, owner: Var, name: &str, lit: Option<&AttrVal>, e: &Expr) -> Result<(), Out> {
        let ty = match self.attr_ty(owner, name) {
            Ok(Some(t)) => t,
            Ok(None) => return Ok(()),
            Err(()) => return Err(outside(&format!("{}.{name}", owner.as_str()), e)),
        };
        let at = match self.dim_of(|d| matches!(d, Dim::Attr { owner: o, name: n, .. } if *o == owner && n == name)) {
            Some(i) => i,
            None => {
                let mut values = Vec::new();
                if self.optional(owner, name) {
                    values.push(AttrVal::Absent);
                }
                match &ty {
                    AttrTy::Bool => values.extend([AttrVal::Bool(false), AttrVal::Bool(true)]),
                    AttrTy::String => values.push(AttrVal::OtherStr),
                    AttrTy::Long => {}
                    AttrTy::Entity(_) => values.push(AttrVal::Present),
                }
                self.dims.push(Dim::Attr { owner, name: name.to_string(), ty: ty.clone(), values, ints: Vec::new() });
                self.dims.len() - 1
            }
        };
        let Dim::Attr { values, ints, .. } = &mut self.dims[at] else { unreachable!("an attribute's dimension") };
        match (lit, &ty) {
            // a value on each side of an integer, and the integer: every interval its comparisons cut
            (Some(AttrVal::Long(n)), AttrTy::Long) => {
                if !ints.contains(n) {
                    ints.push(*n);
                }
                for v in [n.saturating_sub(1), *n, n.saturating_add(1)] {
                    if !values.contains(&AttrVal::Long(v)) {
                        values.push(AttrVal::Long(v));
                    }
                }
            }
            (Some(v @ AttrVal::Str(_)), AttrTy::String) if !values.contains(v) => values.push(v.clone()),
            // a literal of another type: never equal to the attribute, and an error to order
            _ => {}
        }
        Ok(())
    }

    /// The literal an expression is, when it is one an attribute is compared with.
    fn literal(e: &Expr) -> Option<AttrVal> {
        match &e.kind {
            ExprKind::Long(n) => Some(AttrVal::Long(*n)),
            ExprKind::Neg(x) => match x.kind {
                ExprKind::Long(n) => Some(AttrVal::Long(n.checked_neg()?)),
                _ => None,
            },
            ExprKind::Str(s) => Some(AttrVal::Str(s.clone())),
            ExprKind::Bool(b) => Some(AttrVal::Bool(*b)),
            _ => None,
        }
    }

    /// An entity term: the principal, the resource, or an attribute of one of the three variables.
    fn term(e: &Expr) -> Option<Term> {
        match &e.kind {
            ExprKind::Var(Var::Principal) => Some(Term::Principal),
            ExprKind::Var(Var::Resource) => Some(Term::Resource),
            ExprKind::GetAttr { expr, attr } => match expr.kind {
                ExprKind::Var(v @ (Var::Principal | Var::Resource | Var::Context)) => Some(Term::Attr(v, attr.clone())),
                _ => None,
            },
            _ => None,
        }
    }

    /// A literal the principal is compared with: one of its type, when the asker is no workflow (a
    /// workflow is the one it names). A literal of another type is never the principal.
    fn principal_literal(&mut self, uid: &EntityUid) {
        if self.workflow.is_some() || uid.ty.to_string() != self.principal_ty {
            return;
        }
        match self.dims.iter_mut().find_map(|d| if let Dim::PrincipalId(us) = d { Some(us) } else { None }) {
            Some(us) if !us.iter().any(|u| same_uid(u, uid)) => us.push(uid.clone()),
            Some(_) => {}
            None => self.dims.push(Dim::PrincipalId(vec![uid.clone()])),
        }
    }

    /// A literal the resource is compared with, when it is of one of the resource's types.
    fn resource_literal(&mut self, uid: &EntityUid) {
        if !self.resource_types.contains(&uid.ty.to_string()) {
            return;
        }
        match self.dims.iter_mut().find_map(|d| if let Dim::ResourceId(us) = d { Some(us) } else { None }) {
            Some(us) if !us.iter().any(|u| same_uid(u, uid)) => us.push(uid.clone()),
            Some(_) => {}
            None => self.dims.push(Dim::ResourceId(vec![uid.clone()])),
        }
    }

    /// Two entity terms compared: whether they are the same, when they can be (they are of one
    /// type).
    fn relate(&mut self, x: Term, y: Term) {
        if x == y {
            return;
        }
        let (tx, ty) = (self.entity_ty(&x), self.entity_ty(&y));
        if tx.is_none() || tx != ty {
            return;
        }
        if self.dim_of(|d| matches!(d, Dim::Same(a, b) if (*a == x && *b == y) || (*a == y && *b == x))).is_none() {
            self.dims.push(Dim::Same(x, y));
        }
    }

    /// Whether either side of a comparison or of `in` is an attribute no type declares: reading it
    /// errs in every combination, so the comparison does too (Cedar reads the left side first, and
    /// either side's error is the comparison's).
    fn undeclared(&self, left: &Expr, right: &Expr) -> bool {
        [left, right].into_iter().any(|x| matches!(Self::term(x), Some(Term::Attr(v, n)) if self.attr_ty(v, &n) == Ok(None)))
    }

    /// `left in right`, read: the principal in an entity literal or in what an attribute names, the
    /// resource in an entity literal.
    fn scan_in(&mut self, left: &Expr, right: &Expr, e: &Expr) -> Result<(), Out> {
        if self.undeclared(left, right) {
            return Ok(());
        }
        match (Self::term(left), &right.kind) {
            (Some(Term::Principal), ExprKind::Entity(uid)) => {
                // the principal is it (when it is of its type), or is in it (as the asker says)
                self.principal_literal(uid);
                if !self.groups.iter().any(|g| same_uid(g, uid)) {
                    self.groups.push(uid.clone());
                }
                Ok(())
            }
            (Some(Term::Resource), ExprKind::Entity(uid)) => {
                // the resource is it, or is in it: when a resource type can be
                self.resource_literal(uid);
                let ty = uid.ty.to_string();
                if self.resource_types.iter().any(|rt| self.types.can_be_in(rt, &ty)) && self.dim_of(|d| matches!(d, Dim::ResourceIn(u) if same_uid(u, uid))).is_none() {
                    self.dims.push(Dim::ResourceIn(uid.clone()));
                }
                Ok(())
            }
            (Some(Term::Principal), _) => match Self::term(right) {
                Some(t @ Term::Attr(..)) if self.is_entity(&t) => {
                    if let Term::Attr(v, n) = &t {
                        self.attr(*v, n, None, right)?;
                    }
                    // the principal is in what the attribute names by being it, or as a member
                    self.relate(Term::Principal, t.clone());
                    let can = self.entity_ty(&t).is_some_and(|ty| self.types.can_be_in(&self.principal_ty, &ty));
                    if can && self.dim_of(|d| matches!(d, Dim::Member(x) if *x == t)).is_none() {
                        self.dims.push(Dim::Member(t));
                    }
                    Ok(())
                }
                _ => Err(outside("in", e)),
            },
            _ => Err(outside("in", e)),
        }
    }

    /// Read a condition: every dimension it needs is made, or what it reads that sekisho does not
    /// count is said.
    fn scan(&mut self, e: &Expr) -> Result<(), Out> {
        match &e.kind {
            ExprKind::Bool(_) => Ok(()),
            ExprKind::Not(x) => self.scan(x),
            ExprKind::If { cond, then, els } => {
                self.scan(cond)?;
                self.scan(then)?;
                self.scan(els)
            }
            ExprKind::Binary { op: BinOp::And | BinOp::Or, left, right } => {
                self.scan(left)?;
                self.scan(right)
            }
            ExprKind::Binary { op: op @ (BinOp::Eq | BinOp::NotEq | BinOp::Less | BinOp::LessEq | BinOp::Greater | BinOp::GreaterEq), left, right } => {
                // what no form counts, inside what is compared: that, rather than the comparison
                if let Some(o) = never_counted(left).or_else(|| never_counted(right)) {
                    return Err(o);
                }
                if self.undeclared(left, right) {
                    return Ok(());
                }
                let ordering = !matches!(op, BinOp::Eq | BinOp::NotEq);
                // an attribute against a literal, either way round
                for (a, b) in [(left, right), (right, left)] {
                    if let (Some(Term::Attr(v, n)), Some(lit)) = (Self::term(a), Self::literal(b)) {
                        if ordering && !matches!(lit, AttrVal::Long(_)) {
                            return Err(outside(op.as_str(), e));
                        }
                        return self.attr(v, &n, Some(&lit), a);
                    }
                }
                if ordering {
                    return Err(outside(op.as_str(), e));
                }
                // the principal or the resource against an entity literal
                for (a, b) in [(left, right), (right, left)] {
                    if let ExprKind::Entity(uid) = &b.kind {
                        return match Self::term(a) {
                            Some(Term::Principal) => {
                                self.principal_literal(uid);
                                Ok(())
                            }
                            Some(Term::Resource) => {
                                self.resource_literal(uid);
                                Ok(())
                            }
                            _ => Err(outside(op.as_str(), e)),
                        };
                    }
                }
                match (Self::term(left), Self::term(right)) {
                    // two entity terms, neither the resource
                    (Some(x), Some(y)) if self.is_entity(&x) && self.is_entity(&y) && x != Term::Resource && y != Term::Resource => {
                        for (t, at) in [(&x, left), (&y, right)] {
                            if let Term::Attr(v, n) = t {
                                self.attr(*v, n, None, at)?;
                            }
                        }
                        self.relate(x, y);
                        Ok(())
                    }
                    // two attributes of strings or of booleans, or of two types (never equal); two
                    // of integers are not counted
                    (Some(x @ Term::Attr(..)), Some(y @ Term::Attr(..))) => {
                        let ty = |t: &Term| match t {
                            Term::Attr(v, n) => self.attr_ty(*v, n),
                            _ => Ok(None),
                        };
                        let (tx, ty) = (ty(&x), ty(&y));
                        if matches!((&tx, &ty), (Ok(Some(AttrTy::Long)), Ok(Some(AttrTy::Long)))) {
                            return Err(outside(op.as_str(), e));
                        }
                        for (t, at) in [(&x, left), (&y, right)] {
                            if let Term::Attr(v, n) = t {
                                self.attr(*v, n, None, at)?;
                            }
                        }
                        if matches!((&tx, &ty), (Ok(Some(AttrTy::String)), Ok(Some(AttrTy::String))))
                            && x != y
                            && self.dim_of(|d| matches!(d, Dim::SameText(a, b) if (*a == x && *b == y) || (*a == y && *b == x))).is_none()
                        {
                            self.dims.push(Dim::SameText(x, y));
                        }
                        Ok(())
                    }
                    _ => Err(outside(op.as_str(), e)),
                }
            }
            ExprKind::Binary { op: BinOp::In, left, right } => {
                if let Some(o) = never_counted(left).or_else(|| never_counted(right)) {
                    return Err(o);
                }
                self.scan_in(left, right, e)
            }
            ExprKind::Has { expr, attrs } => match (&expr.kind, attrs.as_slice()) {
                (ExprKind::Var(v @ (Var::Principal | Var::Resource | Var::Context)), [a]) => self.attr(*v, a, None, e),
                _ => Err(outside("has", e)),
            },
            ExprKind::Is { expr, in_expr, .. } => match (Self::term(expr), in_expr) {
                (Some(Term::Principal | Term::Resource), None) => Ok(()),
                (Some(t @ (Term::Principal | Term::Resource)), Some(x)) => {
                    let var = if t == Term::Principal { Var::Principal } else { Var::Resource };
                    let as_in = Expr { kind: ExprKind::Binary { op: BinOp::In, left: Box::new(Expr::new(ExprKind::Var(var))), right: x.clone() }, line: e.line, col: e.col };
                    self.scan(&as_in)
                }
                _ => Err(outside("is", e)),
            },
            // a boolean attribute on its own
            ExprKind::GetAttr { .. } => match Self::term(e) {
                Some(Term::Attr(v, n)) => self.attr(v, &n, None, e),
                _ => Err(outside("attribute", e)),
            },
            ExprKind::Like { .. } => Err(outside("like", e)),
            ExprKind::Binary { op, .. } => Err(outside(op.as_str(), e)),
            ExprKind::Neg(_) => Err(outside("-", e)),
            ExprKind::Method { name, .. } => Err(outside(&format!(".{name}()"), e)),
            ExprKind::Call { func, .. } => Err(outside(&format!("{func}()"), e)),
            ExprKind::Set(_) => Err(outside("set", e)),
            ExprKind::Record(_) => Err(outside("record", e)),
            ExprKind::Slot(_) => Err(outside("slot", e)),
            ExprKind::Long(_) | ExprKind::Str(_) | ExprKind::Entity(_) | ExprKind::Var(_) => Err(outside("value", e)),
        }
    }

    /// Once every policy is read: an attribute of strings compared with another can be every string
    /// the other is compared with; an integer no policy compares is one value.
    fn settle(&mut self) {
        let pairs: Vec<(Term, Term)> = self.dims.iter().filter_map(|d| if let Dim::SameText(a, b) = d { Some((a.clone(), b.clone())) } else { None }).collect();
        let strings = |dims: &[Dim], t: &Term| -> Vec<AttrVal> {
            dims.iter()
                .find_map(|d| match (d, t) {
                    (Dim::Attr { owner, name, values, .. }, Term::Attr(o, n)) if owner == o && name == n => Some(values.iter().filter(|v| matches!(v, AttrVal::Str(_))).cloned().collect()),
                    _ => None,
                })
                .unwrap_or_default()
        };
        loop {
            let mut grew = false;
            for (x, y) in &pairs {
                for (from, to) in [(x, y), (y, x)] {
                    let have = strings(&self.dims, from);
                    for d in &mut self.dims {
                        if let (Dim::Attr { owner, name, values, .. }, Term::Attr(o, n)) = (d, to)
                            && owner == o
                            && name == n
                        {
                            for v in &have {
                                if !values.contains(v) {
                                    values.push(v.clone());
                                    grew = true;
                                }
                            }
                        }
                    }
                }
            }
            if !grew {
                break;
            }
        }
        for d in &mut self.dims {
            if let Dim::Attr { ty: AttrTy::Long, values, .. } = d
                && !values.iter().any(|v| matches!(v, AttrVal::Long(_)))
            {
                values.push(AttrVal::Long(0));
            }
        }
    }

    /// How many choices a dimension has.
    fn size(&self, d: &Dim) -> usize {
        match d {
            Dim::ResourceType => self.resource_types.len(),
            Dim::PrincipalId(us) | Dim::ResourceId(us) => us.len() + 1,
            Dim::ResourceIn(_) | Dim::Same(..) | Dim::SameText(..) | Dim::Member(_) => 2,
            Dim::Attr { values, .. } => values.len(),
        }
    }

    /// `f` on each combination that can be, in order, while it says to go on.
    fn each(&self, mut f: impl FnMut(&Combo) -> bool) {
        let sizes: Vec<usize> = self.dims.iter().map(|d| self.size(d)).collect();
        if sizes.contains(&0) {
            return;
        }
        let mut at = vec![0usize; sizes.len()];
        loop {
            {
                let c = Combo { w: self, at: &at };
                if c.consistent() && !f(&c) {
                    return;
                }
            }
            // the next combination
            let mut i = 0;
            loop {
                if i == at.len() {
                    return;
                }
                at[i] += 1;
                if at[i] < sizes[i] {
                    break;
                }
                at[i] = 0;
                i += 1;
            }
        }
    }
}

/// Pairs of terms, said to be the same or to differ.
type Pairs<'t> = Vec<(&'t Term, &'t Term)>;

/// Whether the pairs said to be the same and the pairs said to differ can hold together: two
/// terms that are each the same as a third are the same as each other.
fn classes_hold(same: &[(&Term, &Term)], differ: &[(&Term, &Term)]) -> bool {
    if same.is_empty() {
        return true;
    }
    let mut terms: Vec<&Term> = Vec::new();
    for (a, b) in same.iter().chain(differ) {
        for t in [*a, *b] {
            if !terms.contains(&t) {
                terms.push(t);
            }
        }
    }
    let ix = |t: &Term| terms.iter().position(|x| *x == t).unwrap_or(0);
    fn find(p: &mut [usize], x: usize) -> usize {
        if p[x] != x {
            let r = find(p, p[x]);
            p[x] = r;
        }
        p[x]
    }
    let mut parent: Vec<usize> = (0..terms.len()).collect();
    for (a, b) in same {
        let (x, y) = (find(&mut parent, ix(a)), find(&mut parent, ix(b)));
        parent[x] = y;
    }
    differ.iter().all(|(a, b)| find(&mut parent, ix(a)) != find(&mut parent, ix(b)))
}

/// One combination: a choice for each dimension.
struct Combo<'w, 'a> {
    w: &'w World<'a>,
    at: &'w [usize],
}

impl Combo<'_, '_> {
    fn resource_type(&self) -> &str {
        let i = self.w.dims.iter().position(|d| matches!(d, Dim::ResourceType)).map(|d| self.at[d]).unwrap_or(0);
        &self.w.resource_types[i]
    }

    /// The value the attribute's dimension takes, when it has one.
    fn value(&self, owner: Var, name: &str) -> Option<&AttrVal> {
        let i = self.w.dims.iter().position(|d| matches!(d, Dim::Attr { owner: o, name: n, .. } if *o == owner && n == name))?;
        let Dim::Attr { values, .. } = &self.w.dims[i] else { unreachable!("an attribute's dimension") };
        Some(&values[self.at[i]])
    }

    fn attr(&self, owner: Var, name: &str) -> Result<AttrVal, ()> {
        // what the type of the resource in this combination does not declare is not there
        if owner == Var::Resource && self.w.decl(owner, self.resource_type(), name).is_none() {
            return Ok(AttrVal::Absent);
        }
        self.value(owner, name).cloned().ok_or(())
    }

    fn flag(&self, f: impl Fn(&Dim) -> bool) -> Option<bool> {
        self.w.dims.iter().position(f).map(|i| self.at[i] == 1)
    }

    /// The literal a dimension of literals chooses, when it chooses one.
    fn chosen(&self, f: impl Fn(&Dim) -> bool) -> Option<&EntityUid> {
        let i = self.w.dims.iter().position(f)?;
        match &self.w.dims[i] {
            Dim::PrincipalId(us) | Dim::ResourceId(us) => us.get(self.at[i]),
            _ => None,
        }
    }

    /// Whether two entity terms are the same entity.
    fn same(&self, x: &Term, y: &Term) -> Result<bool, ()> {
        if x == y {
            return Ok(true);
        }
        for t in [x, y] {
            if let Term::Attr(v, n) = t
                && self.attr(*v, n)? == AttrVal::Absent
            {
                return Err(());
            }
        }
        Ok(self.flag(|d| matches!(d, Dim::Same(a, b) if (a == x && b == y) || (a == y && b == x))).unwrap_or(false))
    }

    fn eval(&self, e: &Expr) -> Result<Val, ()> {
        Ok(match &e.kind {
            ExprKind::Bool(b) => Val::Bool(*b),
            ExprKind::Long(n) => Val::Long(*n),
            ExprKind::Neg(x) => match self.eval(x)? {
                Val::Long(n) => Val::Long(n.checked_neg().ok_or(())?),
                _ => return Err(()),
            },
            ExprKind::Str(s) => Val::Str(s.clone()),
            ExprKind::Entity(u) => Val::Lit(u.clone()),
            ExprKind::Var(Var::Principal) => Val::Ent(Term::Principal),
            ExprKind::Var(Var::Resource) => Val::Ent(Term::Resource),
            ExprKind::Not(x) => Val::Bool(!self.bool(x)?),
            ExprKind::If { cond, then, els } => {
                if self.bool(cond)? {
                    self.eval(then)?
                } else {
                    self.eval(els)?
                }
            }
            // `&&` and `||` read their right side only when the left does not decide
            ExprKind::Binary { op: BinOp::And, left, right } => Val::Bool(self.bool(left)? && self.bool(right)?),
            ExprKind::Binary { op: BinOp::Or, left, right } => Val::Bool(self.bool(left)? || self.bool(right)?),
            ExprKind::Binary { op: BinOp::In, left, right } => Val::Bool(self.member(left, right)?),
            ExprKind::Binary { op, left, right } => {
                let (l, r) = (self.eval(left)?, self.eval(right)?);
                let eq = |l: &Val, r: &Val| -> Result<bool, ()> {
                    Ok(match (l, r) {
                        (Val::Bool(a), Val::Bool(b)) => a == b,
                        (Val::Long(a), Val::Long(b)) => a == b,
                        (Val::Str(a), Val::Str(b)) => a == b,
                        // two strings no policy names: the same string, or not
                        (Val::OtherStr(x), Val::OtherStr(y)) => x == y || self.flag(|d| matches!(d, Dim::SameText(a, b) if (a == x && b == y) || (a == y && b == x))).ok_or(())?,
                        (Val::Ent(x), Val::Ent(y)) => self.same(x, y)?,
                        (Val::Ent(t), Val::Lit(u)) | (Val::Lit(u), Val::Ent(t)) => self.is_literal(t, u)?,
                        (Val::Lit(a), Val::Lit(b)) => same_uid(a, b),
                        // a string no policy names is none of the strings they name; values of two
                        // types are never equal
                        _ => false,
                    })
                };
                match (op, &l, &r) {
                    (BinOp::Eq, ..) => Val::Bool(eq(&l, &r)?),
                    (BinOp::NotEq, ..) => Val::Bool(!eq(&l, &r)?),
                    (BinOp::Less, Val::Long(a), Val::Long(b)) => Val::Bool(a < b),
                    (BinOp::LessEq, Val::Long(a), Val::Long(b)) => Val::Bool(a <= b),
                    (BinOp::Greater, Val::Long(a), Val::Long(b)) => Val::Bool(a > b),
                    (BinOp::GreaterEq, Val::Long(a), Val::Long(b)) => Val::Bool(a >= b),
                    _ => return Err(()),
                }
            }
            ExprKind::GetAttr { expr, attr } => {
                let ExprKind::Var(v) = expr.kind else { return Err(()) };
                match self.attr(v, attr)? {
                    AttrVal::Absent => return Err(()),
                    AttrVal::Bool(b) => Val::Bool(b),
                    AttrVal::Long(n) => Val::Long(n),
                    AttrVal::Str(s) => Val::Str(s),
                    AttrVal::OtherStr => Val::OtherStr(Term::Attr(v, attr.clone())),
                    AttrVal::Present => Val::Ent(Term::Attr(v, attr.clone())),
                }
            }
            ExprKind::Has { expr, attrs } => {
                let ExprKind::Var(v) = expr.kind else { return Err(()) };
                let [a] = attrs.as_slice() else { return Err(()) };
                Val::Bool(self.attr(v, a).map(|x| x != AttrVal::Absent).unwrap_or(false))
            }
            ExprKind::Is { expr, ty, in_expr } => {
                let is = match &expr.kind {
                    ExprKind::Var(Var::Principal) => self.w.principal_ty == ty.to_string(),
                    ExprKind::Var(Var::Resource) => self.resource_type() == ty.to_string(),
                    _ => return Err(()),
                };
                match in_expr {
                    Some(x) => Val::Bool(is && self.member(expr, x)?),
                    None => Val::Bool(is),
                }
            }
            _ => return Err(()),
        })
    }

    fn bool(&self, e: &Expr) -> Result<bool, ()> {
        match self.eval(e)? {
            Val::Bool(b) => Ok(b),
            _ => Err(()),
        }
    }

    /// Whether the principal or the resource is the entity literal `u`.
    fn is_literal(&self, t: &Term, u: &EntityUid) -> Result<bool, ()> {
        let ty = u.ty.to_string();
        let chosen = |want: fn(&Dim) -> bool| self.chosen(want).is_some_and(|c| same_uid(c, u));
        match t {
            Term::Principal => Ok(ty == self.w.principal_ty
                && match &self.w.workflow {
                    Some(w) => u.id == *w,
                    None => chosen(|d| matches!(d, Dim::PrincipalId(_))),
                }),
            Term::Resource => Ok(ty == self.resource_type() && chosen(|d| matches!(d, Dim::ResourceId(_)))),
            Term::Attr(..) => Err(()),
        }
    }

    /// `left in right`: the principal in a role or a group (as the asker says, when its type can be
    /// in one of that type), or in what an attribute names; the resource in an entity.
    fn member(&self, left: &Expr, right: &Expr) -> Result<bool, ()> {
        let t = World::term(left).ok_or(())?;
        let w = self.w;
        match (&t, &right.kind) {
            (Term::Principal, ExprKind::Entity(u)) => Ok(self.is_literal(&t, u)? || (w.workflow.is_none() && w.roles.contains(&u.id) && w.types.can_be_in(&w.principal_ty, &u.ty.to_string()))),
            (Term::Resource, ExprKind::Entity(u)) => Ok(self.is_literal(&t, u)? || self.flag(|d| matches!(d, Dim::ResourceIn(x) if same_uid(x, u))).unwrap_or(false)),
            (Term::Principal, _) => {
                let a = World::term(right).ok_or(())?;
                if let Term::Attr(v, n) = &a
                    && self.attr(*v, n)? == AttrVal::Absent
                {
                    return Err(());
                }
                Ok(self.same(&t, &a)? || self.flag(|d| matches!(d, Dim::Member(x) if *x == a)).unwrap_or(false))
            }
            _ => Err(()),
        }
    }

    /// Whether some request makes the combination: the resource is a literal of its type and has
    /// the attributes its type declares; terms said to be the same are of one type and there, and
    /// two that are each the same as a third are the same as each other; the principal is in what
    /// an attribute names, or the resource in an entity, only when the schema lets an entity of its
    /// type be in one of that type. A choice that changes nothing (a flag of an attribute that is
    /// not there) is taken once.
    fn consistent(&self) -> bool {
        let w = self.w;
        let rt = self.resource_type();
        let there = |t: &Term| match t {
            Term::Attr(v, n) => self.attr(*v, n).is_ok_and(|x| x != AttrVal::Absent),
            _ => true,
        };
        let other = |t: &Term| matches!(t, Term::Attr(v, n) if self.attr(*v, n) == Ok(AttrVal::OtherStr));
        let (mut same, mut differ): (Pairs, Pairs) = (Vec::new(), Vec::new());
        let (mut text_same, mut text_differ): (Pairs, Pairs) = (Vec::new(), Vec::new());
        for (i, d) in w.dims.iter().enumerate() {
            let on = self.at[i] == 1;
            match d {
                Dim::ResourceId(us) => {
                    if us.get(self.at[i]).is_some_and(|u| u.ty.to_string() != rt) {
                        return false;
                    }
                }
                Dim::Attr { owner: Var::Resource, name, values, .. } => {
                    let v = &values[self.at[i]];
                    match w.decl(Var::Resource, rt, name) {
                        None if *v != AttrVal::Absent => return false,
                        Some((_, a)) if a.required && *v == AttrVal::Absent => return false,
                        _ => {}
                    }
                }
                Dim::Same(x, y) if on => {
                    if !there(x) || !there(y) || w.term_ty(x, rt) != w.term_ty(y, rt) {
                        return false;
                    }
                    same.push((x, y));
                }
                Dim::Same(x, y) => differ.push((x, y)),
                Dim::SameText(x, y) if on => {
                    if !other(x) || !other(y) {
                        return false;
                    }
                    text_same.push((x, y));
                }
                Dim::SameText(x, y) => text_differ.push((x, y)),
                Dim::Member(t) if on => {
                    if !there(t) || self.same(&Term::Principal, t) == Ok(true) || !w.term_ty(t, rt).is_some_and(|ty| w.types.can_be_in(&w.principal_ty, &ty)) {
                        return false;
                    }
                }
                Dim::ResourceIn(u) if on => {
                    if self.is_literal(&Term::Resource, u) == Ok(true) || !w.types.can_be_in(rt, &u.ty.to_string()) {
                        return false;
                    }
                }
                _ => {}
            }
        }
        classes_hold(&same, &differ) && classes_hold(&text_same, &text_differ)
    }

    /// The combination, a value at a time, in both languages.
    fn shown(&self) -> Text {
        let (mut ja, mut en): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
        let uid = |u: &EntityUid| format!("{}::\"{}\"", u.ty, u.id);
        for (i, d) in self.w.dims.iter().enumerate() {
            let c = self.at[i];
            let (j, e) = match d {
                Dim::ResourceType => (format!("resource の型: {}", self.w.resource_types[c]), format!("resource of type {}", self.w.resource_types[c])),
                Dim::PrincipalId(us) | Dim::ResourceId(us) => {
                    let who = if matches!(d, Dim::PrincipalId(_)) { "principal" } else { "resource" };
                    match us.get(c) {
                        Some(u) => (format!("{who}: {}", uid(u)), format!("{who}: {}", uid(u))),
                        None => {
                            let all: Vec<String> = us.iter().map(uid).collect();
                            (format!("{who}: {} のどれでもない", all.join("、")), format!("{who}: none of {}", all.join(", ")))
                        }
                    }
                }
                // whether the resource is in the entity, by being it too
                Dim::ResourceIn(u) => {
                    let is = c == 1 || self.is_literal(&Term::Resource, u) == Ok(true);
                    (format!("resource in {}: {is}", uid(u)), format!("resource in {}: {is}", uid(u)))
                }
                Dim::Same(a, b) | Dim::SameText(a, b) => (format!("{} == {}: {}", a.shown(), b.shown(), c == 1), format!("{} == {}: {}", a.shown(), b.shown(), c == 1)),
                // whether the principal is in what the attribute names, by being it too
                Dim::Member(t) => {
                    let is = c == 1 || self.same(&Term::Principal, t) == Ok(true);
                    (format!("principal in {}: {is}", t.shown()), format!("principal in {}: {is}", t.shown()))
                }
                Dim::Attr { owner, name, values, .. } => {
                    let at = format!("{}.{name}", owner.as_str());
                    match &values[c] {
                        AttrVal::Absent => (format!("{at}: 無し"), format!("{at}: absent")),
                        AttrVal::Bool(b) => (format!("{at}: {b}"), format!("{at}: {b}")),
                        AttrVal::Long(n) => (format!("{at}: {n}"), format!("{at}: {n}")),
                        AttrVal::Str(s) => (format!("{at}: \"{s}\""), format!("{at}: \"{s}\"")),
                        AttrVal::OtherStr => (format!("{at}: ポリシーが比べるどの文字列でもない"), format!("{at}: a string no policy compares it with")),
                        AttrVal::Present => (format!("{at}: 有り"), format!("{at}: present")),
                    }
                }
            };
            ja.push(j);
            en.push(e);
        }
        Text { ja: ja.join("、"), en: en.join("; ") }
    }
}

/// Whether a policy's scope takes the action (as a request names it), the groups it is in (`in`)
/// followed through the schema.
fn takes_action(schema: &Schema, action: &EntityUid, scope: &ActionScope) -> bool {
    let key = |u: &EntityUid| (u.ty.to_string(), u.id.clone());
    let mut groups: Vec<(String, String)> = vec![key(action)];
    let mut i = 0;
    while i < groups.len() {
        let (ty, id) = groups[i].clone();
        for ns in schema.namespaces.iter().filter(|ns| full(ns.name.as_ref(), "Action") == ty) {
            for x in ns.actions.iter().filter(|x| x.name == id) {
                for r in x.member_of.iter().flatten() {
                    // a parent of no type, or of the type `Action`, is of the namespace's own
                    let pty = match &r.ty {
                        Some(n) if !n.path.is_empty() => n.to_string(),
                        _ => ty.clone(),
                    };
                    let g = (pty, r.id.clone());
                    if !groups.contains(&g) {
                        groups.push(g);
                    }
                }
            }
        }
        i += 1;
    }
    match scope {
        ActionScope::Any => true,
        ActionScope::Eq(u) => key(u) == key(action),
        ActionScope::In(u) => groups.contains(&key(u)),
        ActionScope::InList(us) => us.iter().any(|u| groups.contains(&key(u))),
    }
}

/// A policy's scope of the principal or the resource, as a condition; None for a template's slot.
fn scope_expr(s: &Scope, var: Var) -> Option<Expr> {
    let v = || Box::new(Expr::new(ExprKind::Var(var)));
    let ent = |e: &EntityOrSlot| match e {
        EntityOrSlot::Entity(u) => Some(Box::new(Expr::new(ExprKind::Entity(u.clone())))),
        EntityOrSlot::Slot(_) => None,
    };
    Some(match s {
        Scope::Any => Expr::new(ExprKind::Bool(true)),
        Scope::Eq(e) => Expr::new(ExprKind::Binary { op: BinOp::Eq, left: v(), right: ent(e)? }),
        Scope::In(e) => Expr::new(ExprKind::Binary { op: BinOp::In, left: v(), right: ent(e)? }),
        Scope::Is(n) => Expr::new(ExprKind::Is { expr: v(), ty: n.clone(), in_expr: None }),
        Scope::IsIn(n, e) => Expr::new(ExprKind::Is { expr: v(), ty: n.clone(), in_expr: Some(ent(e)?) }),
    })
}

/// What a question comes to before it is walked.
enum Built<'a> {
    /// No principal of the asker's type, or no resource, comes with the action.
    Never,
    Undecided(Undecided),
    Walk(Box<World<'a>>, Vec<On>),
}

/// The question of how far `asker` is allowed `action` by a pair: the dimensions the action's
/// policies read, and the policies, each as one condition.
fn question<'a>(p: &'a Pair, action: &str, asker: &Asker) -> Result<Built<'a>, Vec<Said>> {
    let schema = &p.schema.2;
    let types = Types { schema };
    let Some((action_ns, a)) = schema.namespaces.iter().find_map(|ns| ns.actions.iter().find(|x| x.name == action).map(|x| (ns.name.as_ref(), x))) else {
        let f = &p.schema.0;
        return Err(said(f, None, tr!("{f} に action `{action}` はありません", "There is no action `{action}` in {f}")));
    };
    let applies = a.applies_to.as_ref();
    let resolved = |ns: &[cedar::Name]| -> Vec<String> { ns.iter().map(|n| types.resolve(action_ns, n)).collect() };
    let principals = applies.map(|t| resolved(&t.principal_types)).unwrap_or_default();
    let resource_types = applies.map(|t| resolved(&t.resource_types)).unwrap_or_default();
    // the asker's type among the action's, by its name with its namespace or without
    let of_type = |ty: &str| principals.iter().find(|p| p.as_str() == ty || p.rsplit("::").next() == Some(ty)).cloned();
    let (principal_ty, roles, workflow) = match asker {
        Asker::Workflow(w) => (of_type("Workflow"), Vec::new(), Some(w.clone())),
        Asker::Roles { ty, roles } => (of_type(ty), roles.clone(), None),
    };
    let Some(principal_ty) = principal_ty else { return Ok(Built::Never) };
    if resource_types.is_empty() {
        return Ok(Built::Never);
    }
    if let Some(t) = std::iter::once(&principal_ty).chain(&resource_types).find(|t| types.enumerated(t)) {
        return Ok(Built::Undecided(Undecided::Enumerated { ty: t.clone() }));
    }
    let action_ty = cedar::Name { path: action_ns.map(|n| n.path.iter().cloned().chain([n.id.clone()]).collect()).unwrap_or_default(), id: "Action".into() };
    let mut w = World {
        types,
        action_ns,
        action_uid: EntityUid::new(action_ty, action),
        principal_ty,
        roles,
        workflow,
        resource_types,
        context: applies.map(|t| &t.context),
        dims: vec![Dim::ResourceType],
        groups: Vec::new(),
    };
    let mut on: Vec<On> = Vec::new();
    for x in p.policies.iter().flat_map(|(_, _, set)| set.policies.iter()) {
        if !takes_action(schema, &w.action_uid, &x.action) {
            continue;
        }
        let (Some(pe), Some(re)) = (scope_expr(&x.principal, Var::Principal), scope_expr(&x.resource, Var::Resource)) else {
            return Ok(Built::Undecided(Undecided::Template { policy: x.id.clone() }));
        };
        let mut all = vec![pe, re];
        for c in &x.conditions {
            all.push(match c.kind {
                cedar::CondKind::When => c.body.clone(),
                cedar::CondKind::Unless => Expr::new(ExprKind::Not(Box::new(c.body.clone()))),
            });
        }
        for e in &all {
            if let Err((what, line, col)) = w.scan(e) {
                return Ok(Built::Undecided(Undecided::Outside { policy: x.id.clone(), what, line, col }));
            }
        }
        let cond = all.into_iter().reduce(|l, r| Expr::new(ExprKind::Binary { op: BinOp::And, left: Box::new(l), right: Box::new(r) })).unwrap_or(Expr::new(ExprKind::Bool(true)));
        on.push(On { id: x.id.clone(), permit: x.effect == cedar::Effect::Permit, cond });
    }
    w.settle();
    let total: u128 = w.dims.iter().map(|d| w.size(d) as u128).product();
    if total > BUDGET {
        return Ok(Built::Undecided(Undecided::Over { total }));
    }
    Ok(Built::Walk(Box::new(w), on))
}

/// What the policies on the action come to in a combination, and the decision, as Cedar's
/// authorizer makes it: a forbid that holds denies, with every forbid that holds; else a permit
/// that holds allows, with every permit that holds; else the request is denied, by none. A policy
/// whose condition errs holds in no combination, and is counted among the errors.
struct Decided {
    allow: bool,
    /// The policies that decide it, by their place among those on the action.
    reason: Vec<usize>,
    /// What each policy on the action comes to: true or false, or None for an error.
    policies: Vec<Option<bool>>,
}

fn decide(c: &Combo, on: &[On]) -> Decided {
    let policies: Vec<Option<bool>> = on.iter().map(|p| c.bool(&p.cond).ok()).collect();
    let holding = |permit: bool| -> Vec<usize> { on.iter().enumerate().filter(|(i, p)| p.permit == permit && policies[*i] == Some(true)).map(|(i, _)| i).collect() };
    let forbids = holding(false);
    if !forbids.is_empty() {
        return Decided { allow: false, reason: forbids, policies };
    }
    let permits = holding(true);
    Decided { allow: !permits.is_empty(), reason: permits, policies }
}

/// How far `asker` is allowed `action` by the policy set and the schema of `file`'s pair, over
/// every combination of what the action's policies read. A principal of a type the action does
/// not take is never allowed it.
pub fn allowed(root: &Path, file: &str, action: &str, asker: &Asker) -> Result<Found<Allowance>, Vec<Said>> {
    let p = read(root, file)?;
    let (w, on) = match question(&p, action, asker)? {
        Built::Never => return Ok(Found::Value(Allowance::Never)),
        Built::Undecided(u) => return Ok(Found::Undecided(u.text())),
        Built::Walk(w, on) => (w, on),
    };
    let (mut allowed, mut denied): (Option<Text>, Option<Text>) = (None, None);
    w.each(|c| {
        let slot = if decide(c, &on).allow { &mut allowed } else { &mut denied };
        if slot.is_none() {
            *slot = Some(c.shown());
        }
        allowed.is_none() || denied.is_none()
    });
    Ok(Found::Value(match (allowed, denied) {
        (Some(a), Some(d)) => Allowance::Sometimes { allowed: a, denied: d },
        (Some(_), None) => Allowance::Always,
        _ => Allowance::Never,
    }))
}

// ---------------------------------------------------------------------------------------------
// Every combination, as tests of `cedar run-tests`

/// One combination of a question, as sekisho answers it, with the tests of `cedar run-tests` that
/// make it (DESIGN 6.5).
#[derive(Clone, Debug)]
pub struct Case {
    /// The tests: one, or one at each end of the cells its integers are in — each with the
    /// decision, the policies that decide it and the number of policies whose evaluation errs. Err
    /// says why no request makes the combination (one sekisho should not count).
    pub tests: Result<Vec<Json>, Text>,
    pub allow: bool,
    /// The `@id` of each policy that decides it, in the order of the file.
    pub reason: Vec<String>,
    /// What each policy on the action comes to: true or false, or None when its evaluation errs.
    pub policies: Vec<(String, Option<bool>)>,
    /// The combination, a value at a time, as `allowed` shows one.
    pub shown: Text,
}

/// Every combination sekisho counts for how far `asker` is allowed `action` by `file`'s pair, with
/// the tests that make each; or why the question is undecided. No combination when the action
/// takes no principal of the asker's type.
pub fn cases(root: &Path, file: &str, action: &str, asker: &Asker) -> Result<Result<Vec<Case>, Undecided>, Vec<Said>> {
    let p = read(root, file)?;
    let (w, on) = match question(&p, action, asker)? {
        Built::Never => return Ok(Ok(Vec::new())),
        Built::Undecided(u) => return Ok(Err(u)),
        Built::Walk(w, on) => (w, on),
    };
    let lits = Lits::of(p.policies.as_ref().map(|(_, _, s)| s));
    let who = match asker {
        Asker::Workflow(n) => format!("workflow {n:?}"),
        Asker::Roles { ty, roles } => format!("{ty} in {roles:?}"),
    };
    let mut out = Vec::new();
    let mut n = 0usize;
    w.each(|c| {
        n += 1;
        let d = decide(c, &on);
        let reason: Vec<String> = d.reason.iter().map(|&i| on[i].id.clone()).collect();
        let errors = d.policies.iter().filter(|x| x.is_none()).count();
        let tests = c.tests(&format!("{action} ({who}) {n}"), d.allow, &reason, errors, &lits);
        out.push(Case { tests, allow: d.allow, reason, policies: on.iter().zip(&d.policies).map(|(o, v)| (o.id.clone(), *v)).collect(), shown: c.shown() });
        true
    });
    Ok(Ok(out))
}

/// Which end of the cells of its integers a test sets them at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    Low,
    High,
}

/// What a policy set names: its strings and its entities, so that what a test makes up is none of
/// them.
struct Lits {
    strings: BTreeSet<String>,
    uids: BTreeSet<(String, String)>,
}

impl Lits {
    fn of(set: Option<&PolicySet>) -> Lits {
        let mut l = Lits { strings: BTreeSet::new(), uids: BTreeSet::new() };
        for p in set.iter().flat_map(|s| s.policies.iter()) {
            for s in [&p.principal, &p.resource] {
                if let Scope::Eq(EntityOrSlot::Entity(u)) | Scope::In(EntityOrSlot::Entity(u)) | Scope::IsIn(_, EntityOrSlot::Entity(u)) = s {
                    l.uids.insert((u.ty.to_string(), u.id.clone()));
                }
            }
            for c in &p.conditions {
                cedar::walk(&c.body, &mut |e| match &e.kind {
                    ExprKind::Str(s) => {
                        l.strings.insert(s.clone());
                    }
                    ExprKind::Entity(u) => {
                        l.uids.insert((u.ty.to_string(), u.id.clone()));
                    }
                    _ => {}
                });
            }
        }
        l
    }
}

/// An entity's type (its full name) and its id.
type Uid = (String, String);

fn uid_json(u: &Uid) -> Json {
    Json::obj([("type", Json::str(u.0.clone())), ("id", Json::str(u.1.clone()))])
}

/// An entity as the value of an attribute.
fn entity_ref(u: &Uid) -> Json {
    Json::obj([("__entity", uid_json(u))])
}

/// `Shop::User::"user1"`, as a request names it.
fn uid_text(u: &Uid) -> String {
    cedar::write_expr(&Expr::new(ExprKind::Entity(EntityUid::new(cedar::Name::parse(&u.0), u.1.clone()))))
}

/// What one test is made of, as it is made: the ids and the strings made up, the entity each
/// entity term names, the string each attribute of strings no policy names is, and the entities
/// besides the principal and the resource.
struct Made<'l> {
    lits: &'l Lits,
    used: BTreeSet<Uid>,
    next: BTreeMap<String, usize>,
    strings: usize,
    named: Vec<(Term, Uid)>,
    texts: Vec<(Term, String)>,
    entities: Vec<(Uid, Json)>,
}

impl Made<'_> {
    /// An id of the type that no policy names and no entity of the test has yet: `user1`.
    fn id(&mut self, ty: &str) -> String {
        let base = ty.rsplit("::").next().unwrap_or(ty).to_lowercase();
        loop {
            let n = self.next.entry(ty.to_string()).or_insert(1);
            let key = (ty.to_string(), format!("{base}{n}"));
            *n += 1;
            if !self.lits.uids.contains(&key) && !self.used.contains(&key) {
                self.used.insert(key.clone());
                return key.1;
            }
        }
    }

    /// A string no policy names, and no attribute of the test is yet.
    fn text(&mut self) -> String {
        loop {
            self.strings += 1;
            let s = format!("other{}", self.strings);
            if !self.lits.strings.contains(&s) {
                return s;
            }
        }
    }

    fn named(&self, t: &Term) -> Option<Uid> {
        self.named.iter().find(|(x, _)| x == t).map(|(_, u)| u.clone())
    }

    fn add(&mut self, u: &Uid, entity: Json) {
        if !self.entities.iter().any(|(x, _)| x == u) {
            self.entities.push((u.clone(), entity));
        }
    }
}

fn entity_json(u: &Uid, attrs: Json, parents: Vec<Json>) -> Json {
    Json::obj([("uid", uid_json(u)), ("attrs", attrs), ("parents", Json::Arr(parents))])
}

/// Why a combination is no request, in both languages.
fn no_request(why: Text) -> Text {
    tr!("どのリクエストもこの組み合わせになりません: {}", "no request makes the combination: {}", why.ja; why.en)
}

impl<'w, 'a> Combo<'w, 'a> {
    /// The integers of the combination, each with the cell of the values that compare as it does
    /// with every integer the policies compare it with: its name, the low end and the high end.
    fn longs(&self) -> Vec<(String, i64, i64)> {
        let mut out = Vec::new();
        for (i, d) in self.w.dims.iter().enumerate() {
            if let Dim::Attr { owner, name, values, ints, .. } = d
                && let AttrVal::Long(v) = values[self.at[i]]
            {
                let (lo, hi) = if ints.contains(&v) {
                    (v, v)
                } else {
                    (ints.iter().filter(|x| **x < v).max().map(|x| x + 1).unwrap_or(i64::MIN), ints.iter().filter(|x| **x > v).min().map(|x| x - 1).unwrap_or(i64::MAX))
                };
                out.push((format!("{}.{name}", owner.as_str()), lo, hi));
            }
        }
        out
    }

    /// The integer an attribute is in a test at `end` of its cell.
    fn number(&self, owner: Var, name: &str, end: End) -> Option<i64> {
        let at = format!("{}.{name}", owner.as_str());
        self.longs().into_iter().find(|(n, _, _)| *n == at).map(|(_, lo, hi)| if end == End::High { hi } else { lo })
    }

    /// The tests of the combination: one, or one at each end of the cells its integers are in.
    fn tests(&self, name: &str, allow: bool, reason: &[String], errors: usize, lits: &Lits) -> Result<Vec<Json>, Text> {
        let longs = self.longs();
        let wide = longs.iter().any(|(_, lo, hi)| lo < hi);
        let ends: &[End] = if wide { &[End::Low, End::High] } else { &[End::Low] };
        let mut out = Vec::new();
        for &end in ends {
            let (request, entities) = self.realize(end, lits).map_err(no_request)?;
            let name = if wide {
                let set: Vec<String> = longs.iter().map(|(n, lo, hi)| format!("{n} {}", if end == End::High { hi } else { lo })).collect();
                format!("{name} ({})", set.join(", "))
            } else {
                name.to_string()
            };
            out.push(Json::obj([
                ("name", Json::str(name)),
                ("request", request),
                ("entities", entities),
                ("decision", Json::str(if allow { "allow" } else { "deny" })),
                ("reason", Json::arr(reason.iter().map(Json::str))),
                ("num_errors", Json::Int(errors as i128)),
            ]));
        }
        Ok(out)
    }

    /// The first value of a type, for an attribute the combination does not set and the schema
    /// requires.
    fn first(&self, ns: Option<&'a cedar::Name>, t: &'a cedar::Type, made: &mut Made) -> Result<Json, Text> {
        let types = self.w.types;
        Ok(match t {
            cedar::Type::Long => Json::Int(0),
            cedar::Type::String => Json::str(""),
            cedar::Type::Bool => Json::Bool(false),
            cedar::Type::Set(_) => Json::Arr(Vec::new()),
            cedar::Type::Record(r) => {
                let mut out = Vec::new();
                for a in r.attrs.iter().filter(|a| a.required) {
                    out.push((a.name.clone(), self.first(ns, &a.ty, made)?));
                }
                Json::Obj(out)
            }
            cedar::Type::Entity(n) => self.some_entity(&types.resolve(ns, n), made)?,
            cedar::Type::EntityOrCommon(n) => {
                if n.path.is_empty() {
                    match n.id.as_str() {
                        "Long" => return Ok(Json::Int(0)),
                        "String" => return Ok(Json::str("")),
                        "Bool" | "Boolean" => return Ok(Json::Bool(false)),
                        _ => {}
                    }
                }
                match types.common(ns, n) {
                    Some((cns, c)) => self.first(cns, &c.ty, made)?,
                    None => self.some_entity(&types.resolve(ns, n), made)?,
                }
            }
            cedar::Type::CommonRef(n) => {
                let (cns, c) = types.common(ns, n).ok_or_else(|| tr!("型 {n} がスキーマにありません", "the schema has no type {n}"))?;
                self.first(cns, &c.ty, made)?
            }
            cedar::Type::Extension(x) => {
                let arg = match x.as_str() {
                    "decimal" => "0.0",
                    "ipaddr" => "0.0.0.0",
                    "datetime" => "1970-01-01",
                    "duration" => "0ms",
                    _ => return Err(tr!("拡張の型 {x} の値を作れません", "no value is made of the extension type {x}")),
                };
                Json::obj([("__extn", Json::obj([("fn", Json::str(x.clone())), ("arg", Json::str(arg))]))])
            }
        })
    }

    /// An entity of the type, made up: the first id the schema lists, for a type that lists them.
    fn some_entity(&self, ty: &str, made: &mut Made) -> Result<Json, Text> {
        let id = match self.w.types.entity(ty) {
            Some((_, e)) => match &e.kind {
                cedar::EntityKind::Enum(ids) => ids.first().cloned().ok_or_else(|| tr!("{ty} の ID がありません", "{ty} lists no id"))?,
                cedar::EntityKind::Standard { .. } => made.id(ty),
            },
            None => return Err(tr!("エンティティ型 {ty} がスキーマにありません", "the schema has no entity type {ty}")),
        };
        Ok(entity_ref(&(ty.to_string(), id)))
    }

    /// An attribute of the principal, the resource or the context as the test gives it, at `end` of
    /// the cell of an integer; None when it is absent.
    fn value_json(&self, owner: Var, a: &'a cedar::Attr, ns: Option<&'a cedar::Name>, end: End, made: &mut Made) -> Result<Option<Json>, Text> {
        let Some(v) = self.value(owner, &a.name) else {
            return Ok(if a.required { Some(self.first(ns, &a.ty, made)?) } else { None });
        };
        let term = Term::Attr(owner, a.name.clone());
        Ok(match v {
            AttrVal::Absent => None,
            AttrVal::Bool(b) => Some(Json::Bool(*b)),
            AttrVal::Long(_) => Some(Json::Int(self.number(owner, &a.name, end).unwrap_or(0).into())),
            AttrVal::Str(s) => Some(Json::str(s.clone())),
            AttrVal::OtherStr => Some(Json::str(made.texts.iter().find(|(t, _)| *t == term).map(|(_, s)| s.clone()).ok_or_else(|| tr!("{} の文字列がありません", "no string for {}", term.shown(); term.shown()))?)),
            AttrVal::Present => Some(entity_ref(&made.named(&term).ok_or_else(|| tr!("{} のエンティティがありません", "no entity for {}", term.shown(); term.shown()))?)),
        })
    }

    /// The attributes of an entity of the type `ty`: the principal's or the resource's as the test
    /// gives them (`owner`); another's, each the schema requires at its first value.
    fn attrs(&self, owner: Option<Var>, ty: &str, end: End, made: &mut Made) -> Result<Json, Text> {
        let Some((ns, rec)) = self.w.shape(ty) else { return Ok(Json::Obj(Vec::new())) };
        let mut out = Vec::new();
        for a in &rec.attrs {
            let v = match owner {
                Some(o) => self.value_json(o, a, ns, end, made)?,
                None if a.required => Some(self.first(ns, &a.ty, made)?),
                None => None,
            };
            if let Some(v) = v {
                out.push((a.name.clone(), v));
            }
        }
        Ok(Json::Obj(out))
    }

    /// The parent an entity of the type `from` has, for it to be in `target`: the target itself, or
    /// the first of the entities between, made up and added to the test.
    fn up(&self, from: &str, target: &Uid, made: &mut Made) -> Result<Json, Text> {
        let chain = self.w.types.chain(from, &target.0).ok_or_else(|| tr!("{from} のエンティティは {} に入れません", "an entity of {from} cannot be in {}", target.0; target.0))?;
        let attrs = self.attrs(None, &target.0, End::Low, made)?;
        made.add(target, entity_json(target, attrs, Vec::new()));
        let mut next = target.clone();
        for ty in chain[..chain.len() - 1].iter().rev() {
            let me = (ty.clone(), made.id(ty));
            let attrs = self.attrs(None, ty, End::Low, made)?;
            made.add(&me, entity_json(&me, attrs, vec![uid_json(&next)]));
            next = me;
        }
        Ok(uid_json(&next))
    }

    /// The request and the entities of the combination, its integers at `end` of their cells; or
    /// why no request makes it.
    fn realize(&self, end: End, lits: &Lits) -> Result<(Json, Json), Text> {
        let w = self.w;
        let rt = self.resource_type().to_string();
        let mut made = Made { lits, used: BTreeSet::new(), next: BTreeMap::new(), strings: 0, named: Vec::new(), texts: Vec::new(), entities: Vec::new() };
        // the principal and the resource
        let p: Uid = (
            w.principal_ty.clone(),
            match (&w.workflow, self.chosen(|d| matches!(d, Dim::PrincipalId(_)))) {
                (Some(name), _) => name.clone(),
                (None, Some(u)) => u.id.clone(),
                (None, None) => made.id(&w.principal_ty),
            },
        );
        made.used.insert(p.clone());
        let r: Uid = (rt.clone(), self.chosen(|d| matches!(d, Dim::ResourceId(_))).map(|u| u.id.clone()).unwrap_or_else(|| made.id(&rt)));
        made.used.insert(r.clone());
        // the entity each entity term that is there names: the terms said to be the same name one,
        // the principal's its own
        let mut terms: Vec<Term> = vec![Term::Principal];
        for d in &w.dims {
            let ts: Vec<Term> = match d {
                Dim::Same(a, b) => vec![a.clone(), b.clone()],
                Dim::Member(t) => vec![t.clone()],
                Dim::Attr { owner, name, ty: AttrTy::Entity(_), .. } => vec![Term::Attr(*owner, name.clone())],
                _ => Vec::new(),
            };
            for t in ts {
                if !terms.contains(&t) {
                    terms.push(t);
                }
            }
        }
        terms.retain(|t| match t {
            Term::Attr(v, n) => self.attr(*v, n) == Ok(AttrVal::Present),
            _ => true,
        });
        let groups = |pairs: &[(&Term, &Term)], terms: &[Term]| -> Vec<usize> {
            let mut class: Vec<usize> = (0..terms.len()).collect();
            loop {
                let mut joined = false;
                for (a, b) in pairs {
                    let (Some(i), Some(j)) = (terms.iter().position(|t| t == *a), terms.iter().position(|t| t == *b)) else { continue };
                    let (x, y) = (class[i].min(class[j]), class[i].max(class[j]));
                    if x != y {
                        class.iter_mut().filter(|c| **c == y).for_each(|c| *c = x);
                        joined = true;
                    }
                }
                if !joined {
                    return class;
                }
            }
        };
        let same: Vec<(&Term, &Term)> = w.dims.iter().enumerate().filter_map(|(i, d)| if let Dim::Same(a, b) = d && self.at[i] == 1 { Some((a, b)) } else { None }).collect();
        let class = groups(&same, &terms);
        for (k, t) in terms.iter().enumerate() {
            let u = match made.named.iter().find(|(x, _)| class[terms.iter().position(|y| y == x).unwrap_or(0)] == class[k]) {
                Some((_, u)) => u.clone(),
                None if class[k] == class[0] => p.clone(),
                None => {
                    let ty = w.term_ty(t, &rt).ok_or_else(|| tr!("{} の型がわかりません", "the type of {} is not known", t.shown(); t.shown()))?;
                    (ty.clone(), made.id(&ty))
                }
            };
            made.named.push((t.clone(), u));
        }
        // the string each attribute of strings no policy names is: those said to be the same, one
        let texts: Vec<Term> = w
            .dims
            .iter()
            .filter_map(|d| match d {
                Dim::Attr { owner, name, ty: AttrTy::String, .. } => Some(Term::Attr(*owner, name.clone())),
                _ => None,
            })
            .filter(|t| matches!(t, Term::Attr(v, n) if self.attr(*v, n) == Ok(AttrVal::OtherStr)))
            .collect();
        let same_text: Vec<(&Term, &Term)> = w.dims.iter().enumerate().filter_map(|(i, d)| if let Dim::SameText(a, b) = d && self.at[i] == 1 { Some((a, b)) } else { None }).collect();
        let class = groups(&same_text, &texts);
        for (k, t) in texts.iter().enumerate() {
            let s = match made.texts.iter().find(|(x, _)| class[texts.iter().position(|y| y == x).unwrap_or(0)] == class[k]) {
                Some((_, s)) => s.clone(),
                None => made.text(),
            };
            made.texts.push((t.clone(), s));
        }
        // the principal: its attributes, and the entities it is in (the asker's roles the schema
        // lets it be in, and what an attribute names when the combination says so)
        let p_attrs = self.attrs(Some(Var::Principal), &p.0, end, &mut made)?;
        let mut p_parents: Vec<Json> = Vec::new();
        let mut in_p: Vec<Uid> = Vec::new();
        if w.workflow.is_none() {
            for g in &w.groups {
                let gu: Uid = (g.ty.to_string(), g.id.clone());
                if w.roles.contains(&g.id) && gu != p && w.types.can_be_in(&w.principal_ty, &gu.0) {
                    in_p.push(gu);
                }
            }
        }
        for (i, d) in w.dims.iter().enumerate() {
            if let Dim::Member(t) = d
                && self.at[i] == 1
            {
                in_p.push(made.named(t).ok_or_else(|| tr!("{} のエンティティがありません", "no entity for {}", t.shown(); t.shown()))?);
            }
        }
        for target in &in_p {
            let parent = self.up(&w.principal_ty, target, &mut made)?;
            if !p_parents.contains(&parent) {
                p_parents.push(parent);
            }
        }
        // the resource: its attributes, and the entities it is in
        let r_attrs = self.attrs(Some(Var::Resource), &rt, end, &mut made)?;
        let mut r_parents: Vec<Json> = Vec::new();
        for (i, d) in w.dims.iter().enumerate() {
            if let Dim::ResourceIn(u) = d
                && self.at[i] == 1
            {
                let parent = self.up(&rt, &(u.ty.to_string(), u.id.clone()), &mut made)?;
                if !r_parents.contains(&parent) {
                    r_parents.push(parent);
                }
            }
        }
        // the context
        let mut context: Vec<(String, Json)> = Vec::new();
        if let Some(t) = w.context
            && let Some((ns, rec)) = w.record(w.action_ns, t)
        {
            for a in &rec.attrs {
                if let Some(v) = self.value_json(Var::Context, a, ns, end, &mut made)? {
                    context.push((a.name.clone(), v));
                }
            }
        }
        // an entity an attribute names that holds no attribute in the schema is written too, so that
        // the request reads as the data does; one that holds some is left out (no policy sekisho
        // counts reads an attribute of it, and a request may name an entity it does not give)
        let named: Vec<Uid> = made.named.iter().map(|(_, u)| u.clone()).collect();
        for u in named {
            if u != p && u != r && w.shape(&u.0).is_none_or(|(_, rec)| rec.attrs.is_empty()) {
                made.add(&u, entity_json(&u, Json::Obj(Vec::new()), Vec::new()));
            }
        }
        let mut entities = vec![entity_json(&p, p_attrs, p_parents), entity_json(&r, r_attrs, r_parents)];
        entities.extend(made.entities.into_iter().filter(|(u, _)| *u != p && *u != r).map(|(_, e)| e));
        let request = Json::obj([
            ("principal", Json::str(uid_text(&p))),
            ("action", Json::str(cedar::write_expr(&Expr::new(ExprKind::Entity(w.action_uid.clone()))))),
            ("resource", Json::str(uid_text(&r))),
            ("context", Json::Obj(context)),
        ]);
        Ok((request, Json::Arr(entities)))
    }
}
