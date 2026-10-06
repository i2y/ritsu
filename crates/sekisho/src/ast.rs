//! What a `.gate` says, as the parser read it (DESIGN 2, 9). Nothing here is resolved: a
//! reference is the word as written ([`Ref`]), a type is the word or the unit as written, a
//! constant is the number with the unit written after it. `names.rs` says what each reference
//! names and what is wrong with the names and the types (E006–E008, E101–E107).
//!
//! Once a file passes `names.rs` with no error, every reference finds what it names with the
//! lookups here ([`File::role`], [`File::entity`], [`EntityDecl::attribute`], [`EnumDecl::value`],
//! [`ActionDecl::local`] …), by the name or by the alias, as the file writes it; a number's unit is
//! read, and a constant counted in it, with `types.rs`. What another `.gate` declares (`use gate`)
//! is found through the scope `names.rs` builds, not through these.
//!
//! Every block keeps the lines it is written on ([`Lines`]), for the definitions `Items` gives.

use ritsu_ports::Day;
use ritsu_units::Rat;

/// Where something is written: 1-based line and column (columns count characters).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

/// The first and the last line a declaration is written on, from 1 (comments and blank lines
/// after its last line are not its).
pub type Lines = (usize, usize);

/// A declared name, and the ASCII alias written after it: `係は上限まで返金できる(clerks_refund_within_their_limit)`.
/// A name that is ASCII of the alias form is its own alias (`refund_order`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name {
    pub text: String,
    pub alias: Option<(String, Span)>,
    pub span: Span,
}

impl Name {
    /// Whether a reference written `word` names this: by the name, or by the alias.
    pub fn is(&self, word: &str) -> bool {
        self.text == word || self.alias.as_ref().is_some_and(|(a, _)| a == word)
    }

    /// What Cedar and the generated code call it: the alias, or the name itself when none is
    /// written (which, in a file that passes `names.rs`, is ASCII of the alias form).
    pub fn ascii(&self) -> &str {
        self.alias.as_ref().map(|(a, _)| a.as_str()).unwrap_or(&self.text)
    }

    /// Whether the name differs from what Cedar calls it: a Japanese name, which Cedar keeps in
    /// `@name` (DESIGN 5.7).
    pub fn has_own_name(&self) -> bool {
        self.ascii() != self.text
    }
}

/// A word that refers to something declared, by its name or its alias, and where it is written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ref {
    pub word: String,
    pub span: Span,
}

/// A `.gate` file.
#[derive(Clone, Debug)]
pub struct File {
    /// The path as diagnostics show it.
    pub path: String,
    pub src: String,
    /// `gate refunds v1`, `gate 返金(refunds_ja) v1`.
    pub name: Name,
    /// `v1`, as written: a mark for people, printed in the head of what is generated and by `api`.
    pub version: String,
    pub description: Option<(String, Span)>,
    /// `namespace Shop`, `namespace Acme::Shop`: the segments. None when the file writes none,
    /// and the namespace is the file's alias in Pascal case (DESIGN 5.1).
    pub namespace: Option<(Vec<String>, Span)>,
    pub uses: Vec<Use>,
    pub today: Option<Today>,
    pub enums: Vec<EnumDecl>,
    pub roles: Vec<RoleDecl>,
    pub principals: Vec<EntityDecl>,
    pub workflows: Vec<WorkflowDecl>,
    pub resources: Vec<EntityDecl>,
    pub actions: Vec<ActionDecl>,
    /// The permits and the forbids, in the order written.
    pub policies: Vec<Policy>,
    pub expects: Vec<Expect>,
    pub separates: Vec<Separate>,
}

/// What a `use` line reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UseKind {
    /// `use rule <name> from "<file.rule>"`: a rule of rulec.
    Rule,
    /// `use dates <name> from "<file.cal>"`: koyomi's date functions.
    Dates,
    /// `use calendar <name> from "<file.cal>"`: a koyomi calendar.
    Calendar,
    /// `use openapi <name> from "<file>"`: an OpenAPI document, the operations `guards` names.
    Openapi,
    /// `use proto <name> from "<file.proto>"`: the services of a `.proto`.
    Proto,
    /// `use asyncapi <name> from "<file>"`: an AsyncAPI document.
    Asyncapi,
    /// `use book <name> from "<file.book>"`: a chobo book, whose transfers' operations an action can guard.
    Book,
    /// `use gate "<file.gate>"`: the enums, roles, principals, workflows and resources of another
    /// `.gate`, and its forbids for `action any`.
    Gate,
}

impl UseKind {
    /// The word after `use`.
    pub fn word(self) -> &'static str {
        match self {
            UseKind::Rule => "rule",
            UseKind::Dates => "dates",
            UseKind::Calendar => "calendar",
            UseKind::Openapi => "openapi",
            UseKind::Proto => "proto",
            UseKind::Asyncapi => "asyncapi",
            UseKind::Book => "book",
            UseKind::Gate => "gate",
        }
    }

    /// The kind a word after `use` names.
    pub fn of(word: &str) -> Option<UseKind> {
        [
            UseKind::Rule,
            UseKind::Dates,
            UseKind::Calendar,
            UseKind::Openapi,
            UseKind::Proto,
            UseKind::Asyncapi,
            UseKind::Book,
            UseKind::Gate,
        ]
        .into_iter()
        .find(|k| k.word() == word)
    }

    /// Whether `guards` can name it: a contract's operations, or a book's transfers.
    pub fn has_operations(self) -> bool {
        matches!(self, UseKind::Openapi | UseKind::Proto | UseKind::Asyncapi | UseKind::Book)
    }
}

/// A `use` line.
#[derive(Clone, Debug)]
pub struct Use {
    pub kind: UseKind,
    /// The name the file uses it by; None for `use gate`, which has none.
    pub name: Option<(String, Span)>,
    /// The file, as written (from the `.gate`'s directory), and where the string is.
    pub path: (String, Span),
    /// Where `use` is.
    pub span: Span,
}

/// `today range >=2026-10-01 <=2028-10-31 offset +00:00`: the day of the request.
#[derive(Clone, Debug)]
pub struct Today {
    pub span: Span,
    /// The days the check walks and the generated code takes (`names.rs` holds both ends to be dates).
    pub range: Range,
    /// The offset as written (`+00:00`), and where; `names.rs` reads it (E107).
    pub offset: (String, Span),
}

/// `enum order_status = paid | shipped | returned | refunded`.
#[derive(Clone, Debug)]
pub struct EnumDecl {
    pub name: Name,
    pub span: Span,
    pub values: Vec<Name>,
}

impl EnumDecl {
    pub fn value(&self, word: &str) -> Option<&Name> {
        self.values.iter().find(|v| v.is(word))
    }
}

/// A `role` block.
#[derive(Clone, Debug)]
pub struct RoleDecl {
    pub name: Name,
    pub span: Span,
    pub lines: Lines,
    pub description: Option<(String, Span)>,
    /// `includes clerk`: whoever holds this role holds these too.
    pub includes: Vec<Ref>,
    /// `can view_order, refund_order`: the actions the role alone may be allowed (DESIGN 2.3,
    /// 4.4), and where `can` is. None when the block writes no `can`, and nothing is checked.
    pub can: Option<(Vec<Ref>, Span)>,
}

/// Whether a type is a principal's or a resource's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntityKind {
    Principal,
    Resource,
}

/// A `principal` or a `resource` block: a Cedar entity type.
#[derive(Clone, Debug)]
pub struct EntityDecl {
    pub kind: EntityKind,
    /// The type's name; its alias is of the form `[A-Z][A-Za-z0-9]*`.
    pub name: Name,
    pub span: Span,
    pub lines: Lines,
    pub description: Option<(String, Span)>,
    /// `roles clerk, manager, auditor`: the roles a principal of the type can hold (a principal's
    /// block only; empty for a resource and for a principal that holds none).
    pub roles: Vec<Ref>,
    /// Under `attributes`, in the order written.
    pub attributes: Vec<Field>,
}

impl EntityDecl {
    pub fn attribute(&self, word: &str) -> Option<&Field> {
        self.attributes.iter().find(|f| f.name.is(word))
    }
}

/// `workflow returns from "flows/returns.flow"`: a dandori workflow as a principal, the Cedar
/// entity `Workflow::"<alias>"`.
#[derive(Clone, Debug)]
pub struct WorkflowDecl {
    pub name: Name,
    pub span: Span,
    pub lines: Lines,
    /// The `.flow`, as written (from the `.gate`'s directory), and where the string is.
    pub flow: (String, Span),
    pub description: Option<(String, Span)>,
}

/// An attribute of a principal or a resource, or an input of an action:
/// `refund_limit : money[GBP, incl_tax]  range >=0GBP <=10_000GBP`.
#[derive(Clone, Debug)]
pub struct Field {
    pub name: Name,
    pub span: Span,
    pub ty: Type,
    pub ty_span: Span,
    /// `T?`: the value may be absent.
    pub optional: bool,
    pub range: Option<Range>,
}

/// The type of a field, as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    Bool,
    Date,
    /// A number of a unit, as rulec writes the type: `money[GBP, incl_tax]`, `mass[kg]`,
    /// `rate[step 0.1%]`, `rate`, `number`. The text is as written; `types.rs` reads the unit.
    Unit(String),
    /// An enum, or a principal's or a resource's type, by name.
    Named(Ref),
}

/// `range >=1GBP <=10_000GBP`: each end as written, None where the end is not written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Range {
    /// Where `range` is.
    pub span: Span,
    pub lo: Option<(Lit, Span)>,
    pub hi: Option<(Lit, Span)>,
}

/// A constant, as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lit {
    /// A number, with the unit written right after it: `50GBP`, `10_000GBP`, `100万円`, `50%`,
    /// `1.5kg`, `-5℃`, `3`.
    Num(Num),
    /// `2026-10-01`, as days since 1970-01-01 (the count ritsu's ports use).
    Date(Day),
    /// `true`, `false`.
    Bool(bool),
}

/// A number as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Num {
    /// Its value, exact, in the unit written: 10000 for `10_000GBP`, 3/2 for `1.5kg`, 1000000 for
    /// `100万円`, 50 for `50%`.
    pub value: Rat,
    /// The unit written right after the number (`GBP`, `円`, `kg`, `%`, `℃`); empty when none.
    pub unit: String,
    /// The constant as written (`10_000GBP`, `-5℃`).
    pub raw: String,
}

/// An `action` block.
#[derive(Clone, Debug)]
pub struct ActionDecl {
    pub name: Name,
    pub span: Span,
    pub lines: Lines,
    pub description: Option<(String, Span)>,
    /// `guards orders refundOrder`: the operations of contracts (and the operations of a book's
    /// transfers) the action guards, in the order written.
    pub guards: Vec<Guard>,
    /// `principal User, Workflow`: the principal types a request of the action can come from
    /// (`Workflow` is sekisho's own type), and where the line is.
    pub principals: (Vec<Ref>, Span),
    /// `resource Order from orderId`: the resource types, and where the line is.
    pub resources: (Vec<Ref>, Span),
    /// `from orderId`: the operation's parameter the resource's id is taken from, and where.
    pub resource_from: Option<(String, Span)>,
    /// `nobody "<reason>"`: the action is meant to be allowed to no one (E301 is not said).
    pub nobody: Option<(String, Span)>,
    /// Under `input`: the operation's parameters a policy or a computed value reads.
    pub input: Vec<Field>,
    /// Under `context`: the computed values, in the order written.
    pub context: Vec<Computed>,
}

/// What a bare name in an action's conditions names: an input, or a computed value.
#[derive(Clone, Copy, Debug)]
pub enum Local<'a> {
    Input(&'a Field),
    Computed(&'a Computed),
}

impl ActionDecl {
    pub fn input(&self, word: &str) -> Option<&Field> {
        self.input.iter().find(|f| f.name.is(word))
    }

    pub fn computed(&self, word: &str) -> Option<&Computed> {
        self.context.iter().find(|c| c.name.is(word))
    }

    /// The input or the computed value a bare name names (the two share Cedar's `context`, so a
    /// name is one or the other).
    pub fn local(&self, word: &str) -> Option<Local<'_>> {
        self.input(word).map(Local::Input).or_else(|| self.computed(word).map(Local::Computed))
    }
}

/// `guards orders refundOrder`.
#[derive(Clone, Debug)]
pub struct Guard {
    /// The name of a `use openapi|proto|asyncapi|book`.
    pub api: Ref,
    /// The operation as written: an OpenAPI `operationId` or `"POST /orders/{orderId}/refunds"`, a
    /// proto's `"Service/Method"`, an AsyncAPI operation's key, a book's `receive.do`.
    pub operation: (String, Span),
    /// Whether the operation is written in quotes.
    pub quoted: bool,
    /// Where `guards` is.
    pub span: Span,
}

/// A computed value: `refund_band = refund_limit(amount: amount, limit: principal.refund_limit).band`.
#[derive(Clone, Debug)]
pub struct Computed {
    pub name: Name,
    pub span: Span,
    pub value: Computation,
    /// What is written after `=`, as written.
    pub text: String,
}

/// How a computed value is computed (DESIGN 3.2, 3.3).
#[derive(Clone, Debug)]
pub enum Computation {
    /// `refund_limit(amount: amount, limit: principal.refund_limit).band`: a rule's output.
    Rule { rule: Ref, args: Vec<Arg>, output: Ref },
    /// `today <= refund_terms.last_day(paid_on: resource.paid_on)`, `today > resource.due_on`:
    /// today against a date.
    Date { op: Op, date: DateValue },
    /// `today is open in uk`: today is a business day of a calendar.
    Open { calendar: Ref },
}

/// The date `today` is compared with.
#[derive(Clone, Debug)]
pub enum DateValue {
    /// `refund_terms.last_day(paid_on: resource.paid_on)`: a date function of a koyomi dates file.
    Call { dates: Ref, date: Ref, args: Vec<Arg> },
    /// `resource.due_on`: a date attribute.
    Attr(Path),
}

/// One input given to a rule or a date function: `amount: amount`.
#[derive(Clone, Debug)]
pub struct Arg {
    /// The rule's or the date function's input.
    pub name: Ref,
    pub value: ArgValue,
}

/// What an input of a rule or a date function is given (DESIGN 3.6: an attribute of the principal
/// or the resource, an input of the action, a constant, today; nothing the caller writes).
#[derive(Clone, Debug)]
pub enum ArgValue {
    /// `principal.refund_limit`, `resource.paid_on`, or a bare name (an input of the action, or the
    /// value of an enum of the rule).
    Path(Path),
    /// `today`.
    Today(Span),
    /// `50GBP`, `true`, `2026-10-01`.
    Lit(Lit, Span),
}

/// A comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Op {
    Lt,
    Le,
    Gt,
    Ge,
    /// `is`: the same.
    Is,
}

impl Op {
    pub fn symbol(self) -> &'static str {
        match self {
            Op::Lt => "<",
            Op::Le => "<=",
            Op::Gt => ">",
            Op::Ge => ">=",
            Op::Is => "is",
        }
    }
}

/// A permit or a forbid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Effect {
    Permit,
    Forbid,
}

impl Effect {
    pub fn word(self) -> &'static str {
        match self {
            Effect::Permit => "permit",
            Effect::Forbid => "forbid",
        }
    }
}

/// A `permit` or a `forbid` block.
#[derive(Clone, Debug)]
pub struct Policy {
    pub effect: Effect,
    pub name: Name,
    pub span: Span,
    pub lines: Lines,
    pub body: Body,
}

/// An `expect allow|deny` block: what every combination it picks is answered (DESIGN 2.8).
#[derive(Clone, Debug)]
pub struct Expect {
    /// `expect allow` (true) or `expect deny`.
    pub allow: bool,
    pub name: Name,
    pub span: Span,
    pub lines: Lines,
    pub body: Body,
}

/// What a policy and an expectation pick, the same way (DESIGN 2.7).
#[derive(Clone, Debug)]
pub struct Body {
    pub description: Option<(String, Span)>,
    /// The `principal` line, and where; None when none is written (every principal of the actions).
    pub principal: Option<(Who, Span)>,
    /// The `action` line, and where.
    pub action: (What, Span),
    /// The `when` and `unless` lines, in the order written; the body holds when every one does.
    pub conds: Vec<Cond>,
}

/// The `principal` line of a policy or an expectation.
#[derive(Clone, Debug)]
pub enum Who {
    /// `principal in clerk, auditor`: holds one of the roles (or one that includes it).
    In(Vec<Ref>),
    /// `principal is Customer`: of the type (`Workflow` names every workflow).
    Is(Ref),
    /// `principal is workflow returns`: that workflow.
    Workflow(Ref),
}

/// The `action` line of a policy or an expectation.
#[derive(Clone, Debug)]
pub enum What {
    /// `action any`: every action of the file.
    Any,
    /// `action view_order, refund_order`.
    Actions(Vec<Ref>),
}

/// A `when` or an `unless` line.
#[derive(Clone, Debug)]
pub struct Cond {
    /// `when` (true) or `unless`.
    pub when: bool,
    pub expr: Expr,
    /// Where `when` or `unless` is.
    pub span: Span,
    /// The condition as written after the word.
    pub text: String,
}

/// A condition (DESIGN 3.1).
#[derive(Clone, Debug)]
pub enum Expr {
    /// `a or b or …`, two or more.
    Or(Vec<Expr>),
    /// `a and b and …`, two or more.
    And(Vec<Expr>),
    /// `not a`, and where `not` is.
    Not(Box<Expr>, Span),
    /// One condition, and where it starts.
    Atom(Atom, Span),
}

/// One condition on finitely many values.
#[derive(Clone, Debug)]
pub enum Atom {
    /// `principal in clerk`: holds the role, or one that includes it.
    InRole(Ref),
    /// `principal in resource.team`: a member of the group the attribute points to (DESIGN 2.5).
    InGroup(Path),
    /// `principal is Customer`: of the type (`Workflow`: any workflow).
    IsType(Ref),
    /// `principal is workflow returns`: that workflow.
    IsWorkflow(Ref),
    /// `<left> is <right>`, `<left> is not <right>`: an enum's value, a number, true or false, the
    /// principal, or another attribute (a relation).
    Is { left: Path, not: bool, right: Rhs },
    /// `amount <= 50GBP`: a number against a constant.
    Compare { left: Path, op: Op, right: (Lit, Span) },
    /// `principal.suspended`, `in_period`: true or false.
    Holds(Path),
}

/// A value a condition reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Path {
    /// `principal.suspended`: an attribute of the principal.
    Principal(Ref),
    /// `resource.status`: an attribute of the resource.
    Resource(Ref),
    /// `amount`, `refund_band`: an input or a computed value of the action.
    Local(Ref),
}

impl Path {
    /// The name after `principal.` or `resource.`, or the bare name.
    pub fn word(&self) -> &Ref {
        match self {
            Path::Principal(r) | Path::Resource(r) | Path::Local(r) => r,
        }
    }

    /// The path as written.
    pub fn text(&self) -> String {
        match self {
            Path::Principal(r) => format!("principal.{}", r.word),
            Path::Resource(r) => format!("resource.{}", r.word),
            Path::Local(r) => r.word.clone(),
        }
    }
}

/// The right side of `is`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rhs {
    /// A bare word: a value of the left side's enum (`refunded`, `within_limit`).
    Word(Ref),
    /// `principal`: the principal itself (`resource.customer is principal`).
    Principal(Span),
    /// `principal.tenant`, `resource.department`: another attribute (never a bare name, which is
    /// a [`Rhs::Word`]).
    Path(Path),
    /// `50GBP`, `true`.
    Lit(Lit, Span),
}

/// A `separate` block: no principal is allowed two of the actions (DESIGN 2.8, 4.5).
#[derive(Clone, Debug)]
pub struct Separate {
    pub name: Name,
    pub span: Span,
    pub lines: Lines,
    pub description: Option<(String, Span)>,
    /// `actions refund_order, export_refunds`: two or more, and where the line is.
    pub actions: (Vec<Ref>, Span),
}

/// The types sekisho declares itself: `Role`, whose entities are the roles, and `Workflow`, whose
/// entities are the workflows (DESIGN 2.2, 2.3, 5.1).
pub const ROLE_TYPE: &str = "Role";
pub const WORKFLOW_TYPE: &str = "Workflow";

/// A principal's or a resource's type, by name.
#[derive(Clone, Copy, Debug)]
pub enum EntityRef<'a> {
    Declared(&'a EntityDecl),
    /// `Workflow`, sekisho's own type of the workflows.
    Workflow,
}

impl File {
    /// The file's alias: the start of every `@id` (`refunds/clerks_refund_within_their_limit`).
    pub fn alias(&self) -> &str {
        self.name.ascii()
    }

    pub fn enum_decl(&self, word: &str) -> Option<&EnumDecl> {
        self.enums.iter().find(|e| e.name.is(word))
    }

    pub fn role(&self, word: &str) -> Option<&RoleDecl> {
        self.roles.iter().find(|r| r.name.is(word))
    }

    pub fn principal(&self, word: &str) -> Option<&EntityDecl> {
        self.principals.iter().find(|e| e.name.is(word))
    }

    pub fn resource(&self, word: &str) -> Option<&EntityDecl> {
        self.resources.iter().find(|e| e.name.is(word))
    }

    /// A principal's or a resource's type, or sekisho's own `Workflow`.
    pub fn entity(&self, word: &str) -> Option<EntityRef<'_>> {
        if word == WORKFLOW_TYPE {
            return Some(EntityRef::Workflow);
        }
        self.principal(word).or_else(|| self.resource(word)).map(EntityRef::Declared)
    }

    pub fn workflow(&self, word: &str) -> Option<&WorkflowDecl> {
        self.workflows.iter().find(|w| w.name.is(word))
    }

    pub fn action(&self, word: &str) -> Option<&ActionDecl> {
        self.actions.iter().find(|a| a.name.is(word))
    }

    /// The `use` line of the name a file uses something by (`refund_limit`, `orders`).
    pub fn used(&self, word: &str) -> Option<&Use> {
        self.uses.iter().find(|u| u.name.as_ref().is_some_and(|(n, _)| n == word))
    }

    pub fn policy(&self, word: &str) -> Option<&Policy> {
        self.policies.iter().find(|p| p.name.is(word))
    }

    pub fn expect(&self, word: &str) -> Option<&Expect> {
        self.expects.iter().find(|e| e.name.is(word))
    }

    pub fn separate(&self, word: &str) -> Option<&Separate> {
        self.separates.iter().find(|s| s.name.is(word))
    }

    /// The actions a policy's or an expectation's `action` line names, in the order of the file
    /// for `action any`. A word that names no action is passed over (`names.rs` says E101).
    pub fn actions_of(&self, what: &What) -> Vec<&ActionDecl> {
        match what {
            What::Any => self.actions.iter().collect(),
            What::Actions(refs) => refs.iter().filter_map(|r| self.action(&r.word)).collect(),
        }
    }
}
