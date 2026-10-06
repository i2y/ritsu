//! The finite model of a `.gate` (DESIGN 2 and 4.1): what the checks read of a file once its names
//! are resolved — the types with their roles and attributes, the actions with their inputs and the
//! values they compute, the policies, the expectations and the separations — each value of a kind
//! the checks can walk (P3). What a value can come to that only another language knows (a rule's
//! output, a koyomi date) is asked of that language when the walk is made ([`crate::walk`]); the
//! model holds where to ask.
//!
//! [`build`] makes the model from the syntax tree; the checks never read the tree.

use crate::ast::{Num, Op};
use ritsu_base::naming::Name;
use ritsu_units::Unit;
use std::path::PathBuf;

/// A day, as the number of days since 1970-01-01 (as rulec and koyomi count them).
pub type Day = i64;

/// A name as written, its ASCII alias (the name itself when it is ASCII already), and its line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Named {
    pub name: String,
    pub alias: String,
    pub line: usize,
}

impl Named {
    pub fn new(name: &str, alias: &str, line: usize) -> Named {
        Named { name: name.to_string(), alias: if alias.is_empty() { name.to_string() } else { alias.to_string() }, line }
    }

    /// Whether `word` names it: its name, or its alias.
    pub fn is(&self, word: &str) -> bool {
        self.name == word || self.alias == word
    }
}

/// One `.gate`, as the checks read it.
#[derive(Clone, Debug, PartialEq)]
pub struct Gate {
    /// The file, as the person gave it (what a diagnostic names), and its text.
    pub file: String,
    pub src: String,
    /// The root a reference's path is written from (ritsu's DESIGN 6.2, item 3; DESIGN 2.6): what
    /// `--root` says, else the nearest directory above the first path given that holds `.git`,
    /// else the directory of the file given.
    pub root: PathBuf,
    /// `gate 返金(refunds_ja) v1`
    pub named: Named,
    pub version: String,
    pub namespace: String,
    pub uses: Vec<Use>,
    pub today: Option<Today>,
    pub enums: Vec<Enum>,
    pub roles: Vec<Role>,
    /// Every entity type a request can name: the principals and the resources the file declares,
    /// and `Workflow` when it declares a workflow.
    pub types: Vec<Type>,
    pub workflows: Vec<Workflow>,
    pub actions: Vec<Action>,
    /// The policies, in the order written; a forbid of `action any` that a `use gate` file holds
    /// comes after this file's own, with [`Policy::from`] saying where it is written.
    pub policies: Vec<Policy>,
    pub expects: Vec<Expect>,
    pub separates: Vec<Separate>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseKind {
    Rule,
    Dates,
    Calendar,
    OpenApi,
    Proto,
    AsyncApi,
    Book,
    Gate,
}

impl UseKind {
    pub fn word(self) -> &'static str {
        match self {
            UseKind::Rule => "rule",
            UseKind::Dates => "dates",
            UseKind::Calendar => "calendar",
            UseKind::OpenApi => "openapi",
            UseKind::Proto => "proto",
            UseKind::AsyncApi => "asyncapi",
            UseKind::Book => "book",
            UseKind::Gate => "gate",
        }
    }
}

/// `use rule refund_limit from "rules/refund_limit.rule"`
#[derive(Clone, Debug, PartialEq)]
pub struct Use {
    pub kind: UseKind,
    /// The name the file uses it by; empty for `use gate`.
    pub name: String,
    /// The path as written.
    pub path: String,
    /// The file, as the tools reach it (from where they run).
    pub file: PathBuf,
    pub line: usize,
}

/// `today range >=2026-10-01 <=2028-10-31 offset +00:00`
#[derive(Clone, Debug, PartialEq)]
pub struct Today {
    pub lo: Day,
    pub hi: Day,
    /// Minutes east of UTC.
    pub offset: i32,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Enum {
    pub named: Named,
    pub values: Vec<Named>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Role {
    pub named: Named,
    /// The roles it includes, directly.
    pub includes: Vec<usize>,
    /// `can …`: the actions, and the line.
    pub can: Option<(Vec<usize>, usize)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Principal,
    Resource,
    /// sekisho's own type for the workflows (`workflow returns from …`).
    Workflow,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Type {
    pub named: Named,
    pub kind: Kind,
    /// The roles a principal of the type can hold.
    pub roles: Vec<usize>,
    pub attrs: Vec<Field>,
}

/// An attribute or an input.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub named: Named,
    pub ty: FieldType,
    /// `T?`: the value may be absent.
    pub optional: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FieldType {
    Bool,
    Enum(usize),
    /// A number counted in its unit, both ends in.
    Num { unit: Unit, written: String, lo: i128, hi: i128 },
    Date { lo: Day, hi: Day },
    Entity(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Workflow {
    pub named: Named,
    /// The `.flow` as written, and as the tools reach it.
    pub flow: String,
    pub file: PathBuf,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    pub named: Named,
    pub guards: Vec<Guard>,
    /// The types the request can name, by index into [`Gate::types`].
    pub principals: Vec<usize>,
    pub resources: Vec<usize>,
    /// `resource Order from orderId`: the argument, and the line.
    pub from: Option<(String, usize)>,
    /// `nobody "<reason>"`, and the line.
    pub nobody: Option<(String, usize)>,
    pub inputs: Vec<Field>,
    pub computed: Vec<Computed>,
    /// The line of `principal` and `resource` under the action.
    pub principal_line: usize,
    pub resource_line: usize,
}

/// `guards orders refundOrder`
#[derive(Clone, Debug, PartialEq)]
pub struct Guard {
    /// The `use` of the contract (or the book), by index into [`Gate::uses`].
    pub api: usize,
    /// The operation as written: an `operationId`, `"POST /orders/{orderId}/refunds"`,
    /// `"Service/Method"`, an AsyncAPI operation's key, or `<transfer>.<operation>`.
    pub operation: String,
    pub line: usize,
    /// The operation found, as a reference (ritsu's DESIGN 6.2): `openapi "api/orders.json"
    /// operation refundOrder`, `proto "shop/v1/orders.proto" service Orders method Refund`,
    /// `chobo "books/stock.book" transfer receive operation do`. The check of the contracts gives
    /// it (`contracts.rs`); None until then, and for an operation it did not find.
    pub reference: Option<Name>,
}

/// A value the action computes (`refund_band = refund_limit(...).band`).
#[derive(Clone, Debug, PartialEq)]
pub struct Computed {
    pub named: Named,
    pub how: How,
}

#[derive(Clone, Debug, PartialEq)]
pub enum How {
    /// `<rule>(<input>: <value>, …).<output>`: the `use rule`, by index into [`Gate::uses`].
    Rule { rule: usize, args: Vec<(String, Source)>, output: String },
    /// `today <op> <date>`
    Date { op: Op, of: DateOf },
    /// `today is open in <calendar>`: the `use calendar`, by index into [`Gate::uses`].
    Open { calendar: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub enum DateOf {
    /// `refund_terms.last_day(paid_on: resource.paid_on)`: the `use dates`, the date, its inputs.
    Call { dates: usize, function: String, args: Vec<(String, Source)> },
    /// A date attribute of the principal or the resource.
    Attr(Owner, String),
}

/// Where a value given to a rule or a date comes from (DESIGN 3.6): only these.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Attr(Owner, String),
    Input(String),
    Lit(Literal),
    /// `today`, the day of the request.
    Today,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Owner {
    Principal,
    Resource,
}

impl Owner {
    pub fn word(self) -> &'static str {
        match self {
            Owner::Principal => "principal",
            Owner::Resource => "resource",
        }
    }
}

/// A constant as written.
#[derive(Clone, Debug, PartialEq)]
pub enum Literal {
    /// A number and the unit written after it (`50GBP`, `10_000GBP`, `3`).
    Num(Num),
    Date(Day),
    Bool(bool),
    /// A word: an enum's value (the name or the alias).
    Word(String),
}

/// `principal …` in a policy or an expectation.
#[derive(Clone, Debug, PartialEq)]
pub enum Who {
    Anyone,
    /// `principal in clerk, auditor`: any of them.
    InRoles(Vec<usize>),
    IsType(usize),
    IsWorkflow(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Policy {
    pub named: Named,
    pub permit: bool,
    pub who: Who,
    /// The actions it is on, `action any` spelled out.
    pub actions: Vec<usize>,
    pub any: bool,
    pub conds: Vec<Cond>,
    /// The `use gate` file it is written in, for a forbid of `action any` read from another
    /// file: the file's alias, and the file as given.
    pub from: Option<(String, String)>,
}

/// `when …` or `unless …`.
#[derive(Clone, Debug, PartialEq)]
pub struct Cond {
    pub when: bool,
    pub expr: Expr,
    pub line: usize,
    /// The line as written, after `when` or `unless`, for messages.
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expect {
    pub name: String,
    pub line: usize,
    pub allow: bool,
    pub who: Who,
    pub actions: Vec<usize>,
    pub conds: Vec<Cond>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Separate {
    pub name: String,
    pub line: usize,
    pub actions: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Atom(Atom),
    Not(Box<Expr>),
    And(Vec<Expr>),
    Or(Vec<Expr>),
}

/// One condition, as the checks read it (DESIGN 3.1).
#[derive(Clone, Debug, PartialEq)]
pub enum Atom {
    /// `principal in clerk`: holds the role, or one that includes it.
    InRoles(Vec<usize>),
    /// `principal is Customer`
    IsType(usize),
    /// `principal is workflow returns`
    IsWorkflow(usize),
    /// `principal.suspended`, `in_period`: a value that is true.
    True(Path),
    /// `resource.status is refunded`: a value is an enum's value, or a bool is a literal.
    Is(Path, Literal),
    /// `amount <= 50GBP`
    Cmp(Path, Op, Literal),
    /// `resource.customer is principal`, `resource.tenant is principal.tenant`
    Same(Term, Term),
    /// `principal in resource.team`
    Member(Term),
    /// `principal.department is resource.department`: two enum values.
    Eq(Path, Path),
}

/// What a condition reads.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Path {
    Attr(Owner, String),
    /// An `input` of the action, or a value it computes: which one is the action's to say.
    Value(String),
}

/// What points to an entity: the principal, or an attribute of an entity type.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Term {
    Principal,
    Attr(Owner, String),
}

impl Gate {
    /// The type of an owner in a request of these types.
    pub fn owner_type(&self, owner: Owner, principal: usize, resource: usize) -> &Type {
        &self.types[match owner {
            Owner::Principal => principal,
            Owner::Resource => resource,
        }]
    }

    /// The roles `r` includes, itself among them, following `includes` through.
    pub fn closure(&self, r: usize) -> Vec<usize> {
        let mut out = vec![r];
        let mut i = 0;
        while i < out.len() {
            for &x in &self.roles[out[i]].includes {
                if !out.contains(&x) {
                    out.push(x);
                }
            }
            i += 1;
        }
        out
    }

    /// The policies on an action, in the order written.
    pub fn policies_on(&self, action: usize) -> Vec<usize> {
        (0..self.policies.len()).filter(|&p| self.policies[p].actions.contains(&action)).collect()
    }

    pub fn expects_on(&self, action: usize) -> Vec<usize> {
        (0..self.expects.len()).filter(|&e| self.expects[e].actions.contains(&action)).collect()
    }

    /// The type `Workflow`, when the file declares a workflow.
    pub fn workflow_type(&self) -> Option<usize> {
        self.types.iter().position(|t| t.kind == Kind::Workflow)
    }

    /// A file the gate reads, as a path from the root (`api/orders.json`); None when it is outside
    /// the root, where no reference can name it.
    pub fn from_root(&self, file: &std::path::Path) -> Option<String> {
        ritsu_base::paths::from_root(&self.root, file)
    }
}

impl Type {
    pub fn attr(&self, name: &str) -> Option<(usize, &Field)> {
        self.attrs.iter().enumerate().find(|(_, f)| f.named.is(name))
    }
}

impl Action {
    pub fn input(&self, name: &str) -> Option<(usize, &Field)> {
        self.inputs.iter().enumerate().find(|(_, f)| f.named.is(name))
    }

    pub fn computed_value(&self, name: &str) -> Option<(usize, &Computed)> {
        self.computed.iter().enumerate().find(|(_, c)| c.named.is(name))
    }

    /// The operations the action guards, each as its reference with the line of its `guards`:
    /// what the schema's `@guards`, `sekisho api` and the port `Gates` (`GateAction::guards`) hold.
    /// Every guard of a file that passes its check has one.
    pub fn references(&self) -> Vec<(Name, usize)> {
        self.guards.iter().filter_map(|g| g.reference.clone().map(|n| (n, g.line))).collect()
    }
}

// ---------------------------------------------------------------------------------------------
// From the syntax tree

use crate::ast;

/// The model of a file whose names, types and units pass `names.rs` (no error), with the files its
/// `use gate` lines read, parsed, in the order written, and the root its references are written
/// from. A name the scope does not find (which `names.rs` has said) is read as a condition that
/// never holds.
pub fn build(f: &ast::File, imports: &[&ast::File], root: &std::path::Path) -> Gate {
    // the scope a name is found in: the file, then the files its `use gate` lines read
    let mut scope: Vec<&ast::File> = vec![f];
    scope.extend(imports.iter().copied());
    let dir = std::path::Path::new(&f.path).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let reach = |written: &str| -> PathBuf { if dir.as_os_str().is_empty() { PathBuf::from(written) } else { dir.join(written) } };

    // the enums, the roles, the types and the workflows of the scope, in the order of the files
    let mut enums: Vec<Enum> = Vec::new();
    let mut roles_ast: Vec<&ast::RoleDecl> = Vec::new();
    let mut principals: Vec<&ast::EntityDecl> = Vec::new();
    let mut resources: Vec<&ast::EntityDecl> = Vec::new();
    let mut workflows: Vec<Workflow> = Vec::new();
    for x in &scope {
        let xdir = std::path::Path::new(&x.path).parent().map(|p| p.to_path_buf()).unwrap_or_default();
        for e in &x.enums {
            enums.push(Enum { named: named(&e.name), values: e.values.iter().map(named).collect() });
        }
        roles_ast.extend(x.roles.iter());
        principals.extend(x.principals.iter());
        resources.extend(x.resources.iter());
        for w in &x.workflows {
            let file = if xdir.as_os_str().is_empty() { PathBuf::from(&w.flow.0) } else { xdir.join(&w.flow.0) };
            workflows.push(Workflow { named: named(&w.name), flow: w.flow.0.clone(), file });
        }
    }
    let role_of = |word: &str| roles_ast.iter().position(|r| r.name.is(word));
    let enum_of = |word: &str| enums.iter().position(|e| e.named.is(word));
    let entities: Vec<&ast::EntityDecl> = principals.iter().chain(resources.iter()).copied().collect();
    let needs_workflow = !workflows.is_empty() || scope.iter().any(|x| x.actions.iter().any(|a| a.principals.0.iter().any(|r| r.word == ast::WORKFLOW_TYPE)));
    let workflow_type = needs_workflow.then_some(entities.len());
    let type_of = |word: &str| -> Option<usize> {
        if word == ast::WORKFLOW_TYPE {
            return workflow_type;
        }
        entities.iter().position(|e| e.name.is(word))
    };
    let field = |x: &ast::Field| -> Field {
        let ty = match &x.ty {
            ast::Type::Bool => FieldType::Bool,
            ast::Type::Date => {
                let (lo, hi) = date_range(x.range.as_ref());
                FieldType::Date { lo, hi }
            }
            ast::Type::Unit(written) => {
                let unit = crate::types::unit(written).unwrap_or_else(|_| Unit::number());
                let end = |e: Option<&(ast::Lit, ast::Span)>, dflt: i128| match e {
                    Some((ast::Lit::Num(n), _)) => crate::types::count(n, &unit).unwrap_or(dflt),
                    _ => dflt,
                };
                let r = x.range.as_ref();
                FieldType::Num { unit: unit.clone(), written: written.clone(), lo: end(r.and_then(|r| r.lo.as_ref()), -crate::types::MAX_SAFE), hi: end(r.and_then(|r| r.hi.as_ref()), crate::types::MAX_SAFE) }
            }
            ast::Type::Named(r) => match (enum_of(&r.word), type_of(&r.word)) {
                (Some(e), _) => FieldType::Enum(e),
                (None, Some(t)) => FieldType::Entity(t),
                // E101 says it
                (None, None) => FieldType::Bool,
            },
        };
        Field { named: named(&x.name), ty, optional: x.optional }
    };
    let mut types: Vec<Type> = Vec::new();
    for e in &entities {
        types.push(Type {
            named: named(&e.name),
            kind: if e.kind == ast::EntityKind::Principal { Kind::Principal } else { Kind::Resource },
            roles: e.roles.iter().filter_map(|r| role_of(&r.word)).collect(),
            attrs: e.attributes.iter().map(&field).collect(),
        });
    }
    if needs_workflow {
        types.push(Type { named: Named::new(ast::WORKFLOW_TYPE, "", 0), kind: Kind::Workflow, roles: vec![], attrs: vec![] });
    }

    // the uses and the actions are the file's own
    let uses: Vec<Use> = f
        .uses
        .iter()
        .map(|u| Use {
            kind: use_kind(u.kind),
            name: u.name.as_ref().map(|(n, _)| n.clone()).unwrap_or_default(),
            path: u.path.0.clone(),
            file: reach(&u.path.0),
            line: u.span.line,
        })
        .collect();
    let use_of = |word: &str| uses.iter().position(|u| u.name == word && u.kind != UseKind::Gate);
    let action_of = |word: &str| f.actions.iter().position(|a| a.name.is(word));
    let actions: Vec<Action> = f
        .actions
        .iter()
        .map(|a| {
            let source = |v: &ast::ArgValue| -> Source {
                match v {
                    ast::ArgValue::Path(ast::Path::Principal(r)) => Source::Attr(Owner::Principal, r.word.clone()),
                    ast::ArgValue::Path(ast::Path::Resource(r)) => Source::Attr(Owner::Resource, r.word.clone()),
                    // a bare name: an input of the action, or a value of the rule's enum
                    ast::ArgValue::Path(ast::Path::Local(r)) => {
                        if a.input(&r.word).is_some() {
                            Source::Input(r.word.clone())
                        } else {
                            Source::Lit(Literal::Word(r.word.clone()))
                        }
                    }
                    ast::ArgValue::Today(_) => Source::Today,
                    ast::ArgValue::Lit(l, _) => Source::Lit(literal(l)),
                }
            };
            let computed = a
                .context
                .iter()
                .map(|c| Computed {
                    named: named(&c.name),
                    how: match &c.value {
                        ast::Computation::Rule { rule, args, output } => How::Rule {
                            rule: use_of(&rule.word).unwrap_or(usize::MAX),
                            args: args.iter().map(|x| (x.name.word.clone(), source(&x.value))).collect(),
                            output: output.word.clone(),
                        },
                        ast::Computation::Date { op, date } => How::Date {
                            op: *op,
                            of: match date {
                                ast::DateValue::Call { dates, date, args } => {
                                    DateOf::Call { dates: use_of(&dates.word).unwrap_or(usize::MAX), function: date.word.clone(), args: args.iter().map(|x| (x.name.word.clone(), source(&x.value))).collect() }
                                }
                                ast::DateValue::Attr(ast::Path::Principal(r)) => DateOf::Attr(Owner::Principal, r.word.clone()),
                                ast::DateValue::Attr(ast::Path::Resource(r) | ast::Path::Local(r)) => DateOf::Attr(Owner::Resource, r.word.clone()),
                            },
                        },
                        ast::Computation::Open { calendar } => How::Open { calendar: use_of(&calendar.word).unwrap_or(usize::MAX) },
                    },
                })
                .collect();
            Action {
                named: named(&a.name),
                guards: a.guards.iter().map(|x| Guard { api: use_of(&x.api.word).unwrap_or(usize::MAX), operation: x.operation.0.clone(), line: x.span.line, reference: None }).collect(),
                principals: a.principals.0.iter().filter_map(|r| type_of(&r.word)).collect(),
                resources: a.resources.0.iter().filter_map(|r| type_of(&r.word)).collect(),
                from: a.resource_from.as_ref().map(|(s, sp)| (s.clone(), sp.line)),
                nobody: a.nobody.as_ref().map(|(s, sp)| (s.clone(), sp.line)),
                inputs: a.input.iter().map(&field).collect(),
                computed,
                principal_line: a.principals.1.line,
                resource_line: a.resources.1.line,
            }
        })
        .collect();

    let who = |w: &Option<(ast::Who, ast::Span)>| -> Who {
        match w {
            None => Who::Anyone,
            Some((ast::Who::In(rs), _)) => Who::InRoles(rs.iter().filter_map(|r| role_of(&r.word)).collect()),
            Some((ast::Who::Is(r), _)) => type_of(&r.word).map(Who::IsType).unwrap_or(Who::InRoles(vec![])),
            Some((ast::Who::Workflow(r), _)) => workflows.iter().position(|w| w.named.is(&r.word)).map(Who::IsWorkflow).unwrap_or(Who::InRoles(vec![])),
        }
    };
    let conds = |b: &ast::Body| -> Vec<Cond> {
        b.conds
            .iter()
            .map(|c| Cond {
                when: c.when,
                expr: expr(&c.expr, &|w| role_of(w), &|w| type_of(w), &|w| workflows.iter().position(|x| x.named.is(w))),
                line: c.span.line,
                text: c.text.clone(),
            })
            .collect()
    };
    let actions_of = |what: &ast::What| -> (Vec<usize>, bool) {
        match what {
            ast::What::Any => ((0..f.actions.len()).collect(), true),
            ast::What::Actions(rs) => (rs.iter().filter_map(|r| action_of(&r.word)).collect(), false),
        }
    };
    let mut policies: Vec<Policy> = f
        .policies
        .iter()
        .map(|p| {
            let (acts, any) = actions_of(&p.body.action.0);
            Policy { named: named(&p.name), permit: p.effect == ast::Effect::Permit, who: who(&p.body.principal), actions: acts, any, conds: conds(&p.body), from: None }
        })
        .collect();
    // a forbid of `action any` another file holds stands for this file's actions too
    for x in imports {
        for p in &x.policies {
            if p.effect == ast::Effect::Forbid && matches!(p.body.action.0, ast::What::Any) {
                policies.push(Policy {
                    named: named(&p.name),
                    permit: false,
                    who: who(&p.body.principal),
                    actions: (0..f.actions.len()).collect(),
                    any: true,
                    conds: conds(&p.body),
                    from: Some((x.alias().to_string(), x.path.clone())),
                });
            }
        }
    }
    let expects = f
        .expects
        .iter()
        .map(|e| Expect { name: e.name.text.clone(), line: e.span.line, allow: e.allow, who: who(&e.body.principal), actions: actions_of(&e.body.action.0).0, conds: conds(&e.body) })
        .collect();
    let separates = f.separates.iter().map(|s| Separate { name: s.name.text.clone(), line: s.span.line, actions: s.actions.0.iter().filter_map(|r| action_of(&r.word)).collect() }).collect();
    let roles = roles_ast
        .iter()
        .map(|r| Role {
            named: named(&r.name),
            includes: r.includes.iter().filter_map(|x| role_of(&x.word)).collect(),
            can: r.can.as_ref().map(|(rs, sp)| (rs.iter().filter_map(|x| action_of(&x.word)).collect(), sp.line)),
        })
        .collect();
    let today = f.today.as_ref().map(|t| {
        let (lo, hi) = date_range(Some(&t.range));
        Today { lo, hi, offset: offset_minutes(&t.offset.0).unwrap_or(0), line: t.span.line }
    });
    let namespace = match &f.namespace {
        Some((segs, _)) => segs.join("::"),
        None => pascal(f.alias()),
    };
    Gate {
        file: f.path.clone(),
        src: f.src.clone(),
        root: root.to_path_buf(),
        named: named(&f.name),
        version: f.version.clone(),
        namespace,
        uses,
        today,
        enums,
        roles,
        types,
        workflows,
        actions,
        policies,
        expects,
        separates,
    }
}

fn named(n: &ast::Name) -> Named {
    Named { name: n.text.clone(), alias: n.ascii().to_string(), line: n.span.line }
}

fn use_kind(k: ast::UseKind) -> UseKind {
    match k {
        ast::UseKind::Rule => UseKind::Rule,
        ast::UseKind::Dates => UseKind::Dates,
        ast::UseKind::Calendar => UseKind::Calendar,
        ast::UseKind::Openapi => UseKind::OpenApi,
        ast::UseKind::Proto => UseKind::Proto,
        ast::UseKind::Asyncapi => UseKind::AsyncApi,
        ast::UseKind::Book => UseKind::Book,
        ast::UseKind::Gate => UseKind::Gate,
    }
}

fn literal(l: &ast::Lit) -> Literal {
    match l {
        ast::Lit::Num(n) => Literal::Num(n.clone()),
        ast::Lit::Date(d) => Literal::Date(*d),
        ast::Lit::Bool(b) => Literal::Bool(*b),
    }
}

/// Both ends of a range of days; an end not written is as far as a date goes.
fn date_range(r: Option<&ast::Range>) -> (Day, Day) {
    let end = |e: Option<&(ast::Lit, ast::Span)>, dflt: Day| match e {
        Some((ast::Lit::Date(d), _)) => *d,
        _ => dflt,
    };
    // 0001-01-01 and 9999-12-31
    (end(r.and_then(|r| r.lo.as_ref()), -719_162), end(r.and_then(|r| r.hi.as_ref()), 2_932_896))
}

/// `+09:00` as minutes east of UTC.
pub fn offset_minutes(s: &str) -> Option<i32> {
    let (sign, rest) = match s.as_bytes().first()? {
        b'+' => (1, &s[1..]),
        b'-' => (-1, &s[1..]),
        _ => return None,
    };
    let (h, m) = rest.split_once(':')?;
    if h.len() != 2 || m.len() != 2 {
        return None;
    }
    Some(sign * (h.parse::<i32>().ok()? * 60 + m.parse::<i32>().ok()?))
}

/// `refunds_ja` as `RefundsJa`: the namespace of a file that writes none.
pub fn pascal(alias: &str) -> String {
    alias
        .split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut cs = p.chars();
            match cs.next() {
                Some(c) => c.to_ascii_uppercase().to_string() + cs.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

fn expr(e: &ast::Expr, role: &dyn Fn(&str) -> Option<usize>, ty: &dyn Fn(&str) -> Option<usize>, wf: &dyn Fn(&str) -> Option<usize>) -> Expr {
    let never = || Expr::Or(vec![]);
    match e {
        ast::Expr::Or(xs) => Expr::Or(xs.iter().map(|x| expr(x, role, ty, wf)).collect()),
        ast::Expr::And(xs) => Expr::And(xs.iter().map(|x| expr(x, role, ty, wf)).collect()),
        ast::Expr::Not(x, _) => Expr::Not(Box::new(expr(x, role, ty, wf))),
        ast::Expr::Atom(a, _) => match a {
            ast::Atom::InRole(r) => role(&r.word).map(|i| Expr::Atom(Atom::InRoles(vec![i]))).unwrap_or_else(never),
            ast::Atom::InGroup(p) => match term_of(p) {
                Some(t) => Expr::Atom(Atom::Member(t)),
                None => never(),
            },
            ast::Atom::IsType(r) => ty(&r.word).map(|t| Expr::Atom(Atom::IsType(t))).unwrap_or_else(never),
            ast::Atom::IsWorkflow(r) => wf(&r.word).map(|w| Expr::Atom(Atom::IsWorkflow(w))).unwrap_or_else(never),
            ast::Atom::Is { left, not, right } => {
                let atom = match right {
                    ast::Rhs::Word(w) => Some(Atom::Is(path_of(left), Literal::Word(w.word.clone()))),
                    ast::Rhs::Principal(_) => term_of(left).map(|t| Atom::Same(t, Term::Principal)),
                    ast::Rhs::Path(p) => Some(Atom::Eq(path_of(left), path_of(p))),
                    ast::Rhs::Lit(l, _) => Some(Atom::Is(path_of(left), literal(l))),
                };
                match atom {
                    Some(a) if *not => Expr::Not(Box::new(Expr::Atom(a))),
                    Some(a) => Expr::Atom(a),
                    None => never(),
                }
            }
            ast::Atom::Compare { left, op, right } => Expr::Atom(Atom::Cmp(path_of(left), *op, literal(&right.0))),
            ast::Atom::Holds(p) => Expr::Atom(Atom::True(path_of(p))),
        },
    }
}

fn path_of(p: &ast::Path) -> Path {
    match p {
        ast::Path::Principal(r) => Path::Attr(Owner::Principal, r.word.clone()),
        ast::Path::Resource(r) => Path::Attr(Owner::Resource, r.word.clone()),
        ast::Path::Local(r) => Path::Value(r.word.clone()),
    }
}

fn term_of(p: &ast::Path) -> Option<Term> {
    match p {
        ast::Path::Principal(r) => Some(Term::Attr(Owner::Principal, r.word.clone())),
        ast::Path::Resource(r) => Some(Term::Attr(Owner::Resource, r.word.clone())),
        ast::Path::Local(_) => None,
    }
}
