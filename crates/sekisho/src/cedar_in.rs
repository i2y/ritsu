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
//!   integer; `has`; an entity attribute that is the principal, or another such attribute, and the
//!   principal in an entity attribute; `&&`, `||`, `!` and `if … then … else`. A question that
//!   reaches anything else (arithmetic, `like`, sets and their methods, the extensions, an
//!   attribute of an attribute, a template's slot) is undecided, with the policy and what it reads.
//!
//! The roles of a principal are the entities it is in: Cedar keeps who is in what with the
//! entities, which the policies and the schema do not hold, so an asker's roles are counted as
//! given, with no role inside another (a gate's `includes` is in the `.gate`; for Cedar written by
//! hand, the asker lists every role the principal is in).

use ritsu_base::cedar::{self, ActionScope, BinOp, EntityOrSlot, EntityUid, Expr, ExprKind, PolicySet, Schema, Scope, Var};
use ritsu_base::naming::{self, Name};
use ritsu_base::text::Text;
use ritsu_ports::{Allowance, Asker, Found, GateAction, GateFacts, GatePolicy, Reference, Said};
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

/// The type of an attribute a policy reads, as the schema declares it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum AttrTy {
    Bool,
    Long,
    String,
    Entity,
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
    /// Who the principal is, of the literals the policies compare it with (the last choice: none
    /// of them).
    PrincipalId(Vec<EntityUid>),
    /// Which the resource is, of the literals the policies compare it with.
    ResourceId(Vec<EntityUid>),
    /// Whether the resource is in an entity.
    ResourceIn(EntityUid),
    /// An attribute and the values it can be.
    Attr { owner: Var, name: String, values: Vec<AttrVal> },
    /// Whether two entity terms are the same entity.
    Same(Term, Term),
    /// Whether the principal is in the entity an attribute names.
    Member(Term),
}

/// What a value of an expression is, in one combination.
#[derive(Clone, Debug, PartialEq)]
enum Val {
    Bool(bool),
    Long(i64),
    Str(String),
    OtherStr,
    Ent(Term),
    Lit(EntityUid),
}

/// The action's question: what the request can be, and the dimensions its policies read.
struct World<'a> {
    schema: &'a Schema,
    /// The principal's type, and the roles (the ids of the entities) it is in.
    principal_ty: String,
    roles: Vec<String>,
    /// The workflow, when the asker is one (`Workflow::"<name>"`).
    workflow: Option<String>,
    resource_types: Vec<String>,
    context: Option<&'a cedar::Type>,
    dims: Vec<Dim>,
}

/// What a policy reads that sekisho does not count, in both languages.
fn outside(what: &str, e: &Expr) -> Text {
    let (l, c) = (e.line, e.col);
    tr!("{l} 行 {c} 列の {what} は、sekisho が数える有限の部分の外です", "the {what} at line {l}, column {c} is outside the finite part sekisho counts")
}

impl<'a> World<'a> {
    fn record(&self, t: &'a cedar::Type) -> Option<&'a cedar::RecordType> {
        match t {
            cedar::Type::Record(r) => Some(r),
            cedar::Type::CommonRef(n) | cedar::Type::EntityOrCommon(n) => self.schema.namespaces.iter().flat_map(|ns| ns.common_types.iter()).find(|c| c.name == n.id).and_then(|c| match &c.ty {
                cedar::Type::Record(r) => Some(r),
                _ => None,
            }),
            _ => None,
        }
    }

    /// The declaration of the attribute `name` of `owner`: of the resource type `rt`, of the
    /// principal's own type, of the action's context.
    fn decl(&self, owner: Var, rt: &str, name: &str) -> Option<&'a cedar::Attr> {
        let entity = |ty: &str| -> Option<&'a cedar::RecordType> {
            let e = self.schema.namespaces.iter().flat_map(|ns| ns.entity_types.iter()).find(|e| e.name == ty)?;
            match &e.kind {
                cedar::EntityKind::Standard { shape, .. } => self.record(shape),
                cedar::EntityKind::Enum(_) => None,
            }
        };
        let r = match owner {
            Var::Principal => entity(&self.principal_ty)?,
            Var::Resource => entity(rt)?,
            Var::Context => self.record(self.context?)?,
            Var::Action => return None,
        };
        r.attrs.iter().find(|a| a.name == name)
    }

    /// The declaration of the attribute for any resource type that has it.
    fn any_decl(&self, owner: Var, name: &str) -> Option<&'a cedar::Attr> {
        if owner == Var::Resource { self.resource_types.iter().find_map(|rt| self.decl(owner, rt, name)) } else { self.decl(owner, "", name) }
    }

    fn ty_of(&self, t: &cedar::Type) -> Option<AttrTy> {
        let common = |id: &str| self.schema.namespaces.iter().flat_map(|ns| ns.common_types.iter()).find(|c| c.name == id);
        match t {
            cedar::Type::Long => Some(AttrTy::Long),
            cedar::Type::String => Some(AttrTy::String),
            cedar::Type::Bool => Some(AttrTy::Bool),
            cedar::Type::Entity(_) => Some(AttrTy::Entity),
            cedar::Type::EntityOrCommon(n) => match n.id.as_str() {
                "Long" => Some(AttrTy::Long),
                "String" => Some(AttrTy::String),
                "Bool" | "Boolean" => Some(AttrTy::Bool),
                id if self.schema.namespaces.iter().any(|ns| ns.entity_types.iter().any(|e| e.name == id)) => Some(AttrTy::Entity),
                id => common(id).and_then(|c| self.ty_of(&c.ty)),
            },
            cedar::Type::CommonRef(n) => common(&n.id).and_then(|c| self.ty_of(&c.ty)),
            _ => None,
        }
    }

    fn dim_of(&self, f: impl Fn(&Dim) -> bool) -> Option<usize> {
        self.dims.iter().position(f)
    }

    /// The attribute `owner.name`: its dimension, made when first read, with `lit` among the values
    /// a policy compares it with.
    fn attr(&mut self, owner: Var, name: &str, lit: Option<AttrVal>, e: &Expr) -> Result<(), Text> {
        let Some(decl) = self.any_decl(owner, name) else {
            // what no type declares errs in every combination: the policy holds in none
            return Ok(());
        };
        let Some(ty) = self.ty_of(&decl.ty) else {
            return Err(outside(&format!("{}.{name}", owner.as_str()), e));
        };
        let optional = !decl.required || (owner == Var::Resource && self.resource_types.iter().any(|rt| self.decl(owner, rt, name).is_none()));
        let at = match self.dim_of(|d| matches!(d, Dim::Attr { owner: o, name: n, .. } if *o == owner && n == name)) {
            Some(i) => i,
            None => {
                let mut values = Vec::new();
                if optional {
                    values.push(AttrVal::Absent);
                }
                match ty {
                    AttrTy::Bool => values.extend([AttrVal::Bool(false), AttrVal::Bool(true)]),
                    AttrTy::String => values.push(AttrVal::OtherStr),
                    AttrTy::Long => {}
                    AttrTy::Entity => values.push(AttrVal::Present),
                }
                self.dims.push(Dim::Attr { owner, name: name.to_string(), values });
                self.dims.len() - 1
            }
        };
        let Dim::Attr { values, .. } = &mut self.dims[at] else { unreachable!("an attribute's dimension") };
        match lit {
            // a value on each side of an integer, and the integer: every interval its comparisons cut
            Some(AttrVal::Long(n)) => {
                for v in [n.saturating_sub(1), n, n.saturating_add(1)] {
                    if !values.contains(&AttrVal::Long(v)) {
                        values.push(AttrVal::Long(v));
                    }
                }
            }
            Some(v @ AttrVal::Str(_)) if !values.contains(&v) => values.push(v),
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

    /// Whether a term is an entity: the principal, or an attribute declared as an entity.
    fn entity(&self, t: &Term) -> bool {
        match t {
            Term::Attr(v, a) => self.any_decl(*v, a).and_then(|d| self.ty_of(&d.ty)) == Some(AttrTy::Entity),
            _ => true,
        }
    }

    fn principal_literal(&mut self, uid: &EntityUid) {
        match self.dims.iter_mut().find_map(|d| if let Dim::PrincipalId(us) = d { Some(us) } else { None }) {
            Some(us) if !us.contains(uid) => us.push(uid.clone()),
            Some(_) => {}
            None => self.dims.push(Dim::PrincipalId(vec![uid.clone()])),
        }
    }

    fn resource_literal(&mut self, uid: &EntityUid) {
        match self.dims.iter_mut().find_map(|d| if let Dim::ResourceId(us) = d { Some(us) } else { None }) {
            Some(us) if !us.contains(uid) => us.push(uid.clone()),
            Some(_) => {}
            None => self.dims.push(Dim::ResourceId(vec![uid.clone()])),
        }
    }

    /// Read a condition: every dimension it needs is made, or what it reads that sekisho does not
    /// count is said.
    fn scan(&mut self, e: &Expr) -> Result<(), Text> {
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
                let ordering = !matches!(op, BinOp::Eq | BinOp::NotEq);
                // an attribute against a literal, either way round
                for (a, b) in [(left, right), (right, left)] {
                    if let (Some(Term::Attr(v, n)), Some(lit)) = (Self::term(a), Self::literal(b)) {
                        if ordering && !matches!(lit, AttrVal::Long(_)) {
                            return Err(outside(op.as_str(), e));
                        }
                        return self.attr(v, &n, Some(lit), a);
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
                // two entity terms, neither the resource
                match (Self::term(left), Self::term(right)) {
                    (Some(x), Some(y)) if self.entity(&x) && self.entity(&y) && x != Term::Resource && y != Term::Resource => {
                        for t in [&x, &y] {
                            if let Term::Attr(v, n) = t {
                                self.attr(*v, n, None, e)?;
                            }
                        }
                        if x != y && self.dim_of(|d| matches!(d, Dim::Same(a, b) if (*a == x && *b == y) || (*a == y && *b == x))).is_none() {
                            self.dims.push(Dim::Same(x, y));
                        }
                        Ok(())
                    }
                    _ => Err(outside(op.as_str(), e)),
                }
            }
            ExprKind::Binary { op: BinOp::In, left, right } => match (Self::term(left), &right.kind) {
                (Some(Term::Principal), ExprKind::Entity(uid)) => {
                    // the principal in an entity of its own type is the principal being it
                    if uid.ty.id == self.principal_ty {
                        self.principal_literal(uid);
                    }
                    Ok(())
                }
                (Some(Term::Resource), ExprKind::Entity(uid)) => {
                    if self.dim_of(|d| matches!(d, Dim::ResourceIn(u) if u == uid)).is_none() {
                        self.dims.push(Dim::ResourceIn(uid.clone()));
                    }
                    Ok(())
                }
                (Some(Term::Principal), _) => match Self::term(right) {
                    Some(t @ Term::Attr(..)) if self.entity(&t) => {
                        if let Term::Attr(v, n) = &t {
                            self.attr(*v, n, None, e)?;
                        }
                        if self.dim_of(|d| matches!(d, Dim::Member(x) if *x == t)).is_none() {
                            self.dims.push(Dim::Member(t));
                        }
                        Ok(())
                    }
                    _ => Err(outside("in", e)),
                },
                _ => Err(outside("in", e)),
            },
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

    /// How many choices a dimension has.
    fn size(&self, d: &Dim) -> usize {
        match d {
            Dim::ResourceType => self.resource_types.len(),
            Dim::PrincipalId(us) | Dim::ResourceId(us) => us.len() + 1,
            Dim::ResourceIn(_) | Dim::Same(..) | Dim::Member(_) => 2,
            Dim::Attr { values, .. } => values.len(),
        }
    }
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

    fn attr(&self, owner: Var, name: &str) -> Result<AttrVal, ()> {
        // what the type of the resource in this combination does not declare is not there
        if owner == Var::Resource && self.w.decl(owner, self.resource_type(), name).is_none() {
            return Ok(AttrVal::Absent);
        }
        match self.w.dims.iter().position(|d| matches!(d, Dim::Attr { owner: o, name: n, .. } if *o == owner && n == name)) {
            Some(i) => {
                let Dim::Attr { values, .. } = &self.w.dims[i] else { unreachable!("an attribute's dimension") };
                Ok(values[self.at[i]].clone())
            }
            None => Err(()),
        }
    }

    fn flag(&self, f: impl Fn(&Dim) -> bool) -> Option<bool> {
        self.w.dims.iter().position(f).map(|i| self.at[i] == 1)
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
        self.flag(|d| matches!(d, Dim::Same(a, b) if (a == x && b == y) || (a == y && b == x))).ok_or(())
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
                        (Val::Ent(x), Val::Ent(y)) => self.same(x, y)?,
                        (Val::Ent(t), Val::Lit(u)) | (Val::Lit(u), Val::Ent(t)) => self.is_literal(t, u)?,
                        (Val::Lit(a), Val::Lit(b)) => a.ty.id == b.ty.id && a.id == b.id,
                        // a string no policy names is none of the strings they name
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
                    AttrVal::OtherStr => Val::OtherStr,
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
                    ExprKind::Var(Var::Principal) => self.w.principal_ty == ty.id,
                    ExprKind::Var(Var::Resource) => self.resource_type() == ty.id,
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
        let chosen = |want: fn(&Dim) -> bool| -> bool {
            self.w.dims.iter().position(want).is_some_and(|i| match &self.w.dims[i] {
                Dim::PrincipalId(us) | Dim::ResourceId(us) => us.get(self.at[i]) == Some(u),
                _ => false,
            })
        };
        match t {
            Term::Principal => Ok(match &self.w.workflow {
                Some(w) => u.ty.id == "Workflow" && u.id == *w,
                None => u.ty.id == self.w.principal_ty && chosen(|d| matches!(d, Dim::PrincipalId(_))),
            }),
            Term::Resource => Ok(u.ty.id == self.resource_type() && chosen(|d| matches!(d, Dim::ResourceId(_)))),
            Term::Attr(..) => Err(()),
        }
    }

    /// `left in right`: the principal in a role or a group, or in what an attribute names; the
    /// resource in an entity.
    fn member(&self, left: &Expr, right: &Expr) -> Result<bool, ()> {
        let t = World::term(left).ok_or(())?;
        match (&t, &right.kind) {
            (Term::Principal, ExprKind::Entity(u)) => Ok((self.w.workflow.is_none() && self.w.roles.contains(&u.id)) || self.is_literal(&t, u)?),
            (Term::Resource, ExprKind::Entity(u)) => self.flag(|d| matches!(d, Dim::ResourceIn(x) if x == u)).ok_or(()),
            (Term::Principal, _) => {
                let a = World::term(right).ok_or(())?;
                if let Term::Attr(v, n) = &a
                    && self.attr(*v, n)? == AttrVal::Absent
                {
                    return Err(());
                }
                Ok(self.same(&t, &a).unwrap_or(false) || self.flag(|d| matches!(d, Dim::Member(x) if *x == a)).ok_or(())?)
            }
            _ => Err(()),
        }
    }

    /// Whether the relations of the combination can hold together: two terms that are each the same
    /// as a third are the same as each other.
    fn consistent(&self) -> bool {
        let pairs: Vec<(&Term, &Term, bool)> = self.w.dims.iter().enumerate().filter_map(|(i, d)| if let Dim::Same(a, b) = d { Some((a, b, self.at[i] == 1)) } else { None }).collect();
        if pairs.is_empty() {
            return true;
        }
        let mut terms: Vec<&Term> = Vec::new();
        for (a, b, _) in &pairs {
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
        for (a, b, same) in &pairs {
            if *same {
                let (x, y) = (find(&mut parent, ix(a)), find(&mut parent, ix(b)));
                parent[x] = y;
            }
        }
        pairs.iter().all(|(a, b, same)| *same || find(&mut parent, ix(a)) != find(&mut parent, ix(b)))
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
                Dim::ResourceIn(u) => (format!("resource in {}: {}", uid(u), c == 1), format!("resource in {}: {}", uid(u), c == 1)),
                Dim::Same(a, b) => (format!("{} == {}: {}", a.shown(), b.shown(), c == 1), format!("{} == {}: {}", a.shown(), b.shown(), c == 1)),
                Dim::Member(t) => (format!("principal in {}: {}", t.shown(), c == 1), format!("principal in {}: {}", t.shown(), c == 1)),
                Dim::Attr { owner, name, values } => {
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

/// Whether a policy's scope takes the action `a` of the schema (by its name), the groups it is in
/// (`in`) followed through the schema.
fn takes_action(schema: &Schema, scope: &ActionScope, a: &str) -> bool {
    let mut groups = vec![a.to_string()];
    let mut i = 0;
    while i < groups.len() {
        let parents: Vec<String> = schema.namespaces.iter().flat_map(|ns| ns.actions.iter()).filter(|x| x.name == groups[i]).flat_map(|x| x.member_of.iter().flatten().map(|r| r.id.clone())).collect();
        for p in parents {
            if !groups.contains(&p) {
                groups.push(p);
            }
        }
        i += 1;
    }
    match scope {
        ActionScope::Any => true,
        ActionScope::Eq(u) => u.id == a,
        ActionScope::In(u) => groups.contains(&u.id),
        ActionScope::InList(us) => us.iter().any(|u| groups.contains(&u.id)),
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

/// How far `asker` is allowed `action` by the policy set and the schema of `file`'s pair, over
/// every combination of what the action's policies read. A principal of a type the action does
/// not take is never allowed it.
pub fn allowed(root: &Path, file: &str, action: &str, asker: &Asker) -> Result<Found<Allowance>, Vec<Said>> {
    let p = read(root, file)?;
    let schema = &p.schema.2;
    let Some(a) = schema.namespaces.iter().flat_map(|ns| ns.actions.iter()).find(|x| x.name == action) else {
        let f = &p.schema.0;
        return Err(said(f, None, tr!("{f} に action `{action}` はありません", "There is no action `{action}` in {f}")));
    };
    let applies = a.applies_to.as_ref();
    let principals: Vec<String> = applies.map(|t| t.principal_types.iter().map(|n| n.id.clone()).collect()).unwrap_or_default();
    let (principal_ty, roles, workflow) = match asker {
        Asker::Workflow(w) => ("Workflow".to_string(), Vec::new(), Some(w.clone())),
        Asker::Roles { ty, roles } => (ty.clone(), roles.clone(), None),
    };
    let resource_types: Vec<String> = applies.map(|t| t.resource_types.iter().map(|n| n.id.clone()).collect()).unwrap_or_default();
    if !principals.contains(&principal_ty) || resource_types.is_empty() {
        return Ok(Found::Value(Allowance::Never));
    }
    let mut w = World { schema, principal_ty, roles, workflow, resource_types, context: applies.map(|t| &t.context), dims: vec![Dim::ResourceType] };
    // the policies on the action, each as one condition: its scope and its `when` and `unless`
    let mut on: Vec<(bool, Expr)> = Vec::new();
    for x in p.policies.iter().flat_map(|(_, _, set)| set.policies.iter()) {
        if !takes_action(schema, &x.action, action) {
            continue;
        }
        let id = &x.id;
        let (Some(pe), Some(re)) = (scope_expr(&x.principal, Var::Principal), scope_expr(&x.resource, Var::Resource)) else {
            return Ok(Found::Undecided(tr!("ポリシー {id} はテンプレートで、何に当てはまるかはリンクで決まります", "the policy {id} is a template, which a link gives what it is for")));
        };
        let mut all = vec![pe, re];
        for c in &x.conditions {
            all.push(match c.kind {
                cedar::CondKind::When => c.body.clone(),
                cedar::CondKind::Unless => Expr::new(ExprKind::Not(Box::new(c.body.clone()))),
            });
        }
        for e in &all {
            if let Err(why) = w.scan(e) {
                return Ok(Found::Undecided(tr!("ポリシー {id} の、{}", "{}, in the policy {id}", why.ja; why.en)));
            }
        }
        let cond = all.into_iter().reduce(|l, r| Expr::new(ExprKind::Binary { op: BinOp::And, left: Box::new(l), right: Box::new(r) })).unwrap_or(Expr::new(ExprKind::Bool(true)));
        on.push((x.effect == cedar::Effect::Permit, cond));
    }
    // an integer no policy compares is one value
    for d in &mut w.dims {
        if let Dim::Attr { values, .. } = d
            && values.is_empty()
        {
            values.push(AttrVal::Long(0));
        }
    }
    let sizes: Vec<usize> = w.dims.iter().map(|d| w.size(d)).collect();
    let total: u128 = sizes.iter().map(|s| *s as u128).product();
    if total > BUDGET {
        return Ok(Found::Undecided(tr!("組み合わせが {total} 通りあり、数える上限の {BUDGET} 通りを超えます", "there are {total} combinations, past the {BUDGET} counted at most")));
    }
    let (mut allowed, mut denied): (Option<Text>, Option<Text>) = (None, None);
    let mut at = vec![0usize; sizes.len()];
    'walk: loop {
        let c = Combo { w: &w, at: &at };
        if c.consistent() {
            // a policy whose condition errs is not satisfied, as Cedar evaluates it
            let holds = |permit: bool| on.iter().filter(|(p, _)| *p == permit).any(|(_, e)| c.bool(e).unwrap_or(false));
            let slot = if holds(true) && !holds(false) { &mut allowed } else { &mut denied };
            if slot.is_none() {
                *slot = Some(c.shown());
            }
            if allowed.is_some() && denied.is_some() {
                break 'walk;
            }
        }
        // the next combination
        let mut i = 0;
        loop {
            if i == at.len() {
                break 'walk;
            }
            at[i] += 1;
            if at[i] < sizes[i] {
                break;
            }
            at[i] = 0;
            i += 1;
        }
    }
    Ok(Found::Value(match (allowed, denied) {
        (Some(a), Some(d)) => Allowance::Sometimes { allowed: a, denied: d },
        (Some(_), None) => Allowance::Always,
        _ => Allowance::Never,
    }))
}
