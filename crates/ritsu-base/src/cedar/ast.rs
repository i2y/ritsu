//! The abstract syntax of a policy set, and the reading of the concrete one into it with every
//! check Cedar 4.13.0 makes on the way (`cedar-policy-core/src/parser/cst_to_ast.rs`): the
//! effect and the clauses' words, the three variables of the scope and what each may be compared
//! with, the reserved words, the known functions and methods and their counts of arguments,
//! integers in `i64`, escapes, duplicate keys and annotations, and slots only in the scope.

use super::cst::{self, Node, Span};
use super::lexer::Lines;
use super::Error;
use crate::text::Text;
use crate::tr;
use std::fmt;

/// A policy set: static policies and templates, in the order they are written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicySet {
    pub policies: Vec<Policy>,
}

/// One policy or template (a template has a slot in its scope).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Policy {
    /// The id the CLI gives it: the value of `@id("…")`, or `policy<n>` for the n-th policy of
    /// the file (from 0).
    pub id: String,
    pub annotations: Vec<Annotation>,
    pub effect: Effect,
    pub principal: Scope,
    pub action: ActionScope,
    pub resource: Scope,
    pub conditions: Vec<Condition>,
    pub line: usize,
    pub col: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Annotation {
    pub key: String,
    /// `None` for `@key` written without a value.
    pub value: Option<String>,
    pub line: usize,
    pub col: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Permit,
    Forbid,
}

/// What the scope says of the principal or the resource.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    Any,
    Eq(EntityOrSlot),
    In(EntityOrSlot),
    Is(Name),
    IsIn(Name, EntityOrSlot),
}

/// What the scope says of the action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionScope {
    Any,
    Eq(EntityUid),
    In(EntityUid),
    /// `action in [A, B]` (a list of one is written in the JSON format as `in` one entity).
    InList(Vec<EntityUid>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntityOrSlot {
    Entity(EntityUid),
    Slot(Slot),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Principal,
    Resource,
}

impl Slot {
    pub fn as_str(self) -> &'static str {
        match self {
            Slot::Principal => "?principal",
            Slot::Resource => "?resource",
        }
    }
}

/// `Type::"id"`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityUid {
    pub ty: Name,
    pub id: String,
    pub line: usize,
    pub col: usize,
}

impl EntityUid {
    pub fn new(ty: Name, id: impl Into<String>) -> EntityUid {
        EntityUid { ty, id: id.into(), line: 0, col: 0 }
    }
}

/// `A::B::C`: `path` is `[A, B]`, `id` is `C`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name {
    pub path: Vec<String>,
    pub id: String,
}

impl Name {
    pub fn new(id: impl Into<String>) -> Name {
        Name { path: vec![], id: id.into() }
    }

    /// `"A::B::C"` read into its parts.
    pub fn parse(s: &str) -> Name {
        let mut parts: Vec<String> = s.split("::").map(str::to_string).collect();
        let id = parts.pop().unwrap_or_default();
        Name { path: parts, id }
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for p in &self.path {
            write!(f, "{p}::")?;
        }
        write!(f, "{}", self.id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CondKind {
    When,
    Unless,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Condition {
    pub kind: CondKind,
    pub body: Expr,
    pub line: usize,
    pub col: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expr {
    pub kind: ExprKind,
    pub line: usize,
    pub col: usize,
}

impl Expr {
    /// An expression made rather than read: line and column 0.
    pub fn new(kind: ExprKind) -> Expr {
        Expr { kind, line: 0, col: 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Var {
    Principal,
    Action,
    Resource,
    Context,
}

impl Var {
    pub fn as_str(self) -> &'static str {
        match self {
            Var::Principal => "principal",
            Var::Action => "action",
            Var::Resource => "resource",
            Var::Context => "context",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Eq,
    NotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    In,
    And,
    Or,
    Add,
    Sub,
    Mul,
}

impl BinOp {
    pub fn as_str(self) -> &'static str {
        match self {
            BinOp::Eq => "==",
            BinOp::NotEq => "!=",
            BinOp::Less => "<",
            BinOp::LessEq => "<=",
            BinOp::Greater => ">",
            BinOp::GreaterEq => ">=",
            BinOp::In => "in",
            BinOp::And => "&&",
            BinOp::Or => "||",
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternElem {
    Char(char),
    Wildcard,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExprKind {
    Bool(bool),
    Long(i64),
    Str(String),
    Entity(EntityUid),
    Var(Var),
    Slot(Slot),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Binary { op: BinOp, left: Box<Expr>, right: Box<Expr> },
    /// `e.a` and `e["a"]`.
    GetAttr { expr: Box<Expr>, attr: String },
    /// `e has a` and `e has a.b.c`.
    Has { expr: Box<Expr>, attrs: Vec<String> },
    Like { expr: Box<Expr>, pattern: Vec<PatternElem> },
    Is { expr: Box<Expr>, ty: Name, in_expr: Option<Box<Expr>> },
    If { cond: Box<Expr>, then: Box<Expr>, els: Box<Expr> },
    Set(Vec<Expr>),
    /// The keys in the order they are written (no key twice).
    Record(Vec<(String, Expr)>),
    /// `e.contains(x)`, `e.isEmpty()`, `e.getTag(k)`, and the extensions' methods
    /// (`isInRange`, `lessThan`, `toDate`, …).
    Method { expr: Box<Expr>, name: String, args: Vec<Expr> },
    /// The extensions' functions: `decimal`, `ip`, `datetime`, `duration`, `unknown`.
    Call { func: Name, args: Vec<Expr> },
}

impl Policy {
    /// A template: a slot in its scope.
    pub fn is_template(&self) -> bool {
        let slot = |s: &Scope| matches!(s, Scope::Eq(EntityOrSlot::Slot(_)) | Scope::In(EntityOrSlot::Slot(_)) | Scope::IsIn(_, EntityOrSlot::Slot(_)));
        slot(&self.principal) || slot(&self.resource)
    }
}

/// The methods every value has (they are not extensions).
pub(crate) const BUILTIN_METHODS: [&str; 6] = ["contains", "containsAll", "containsAny", "isEmpty", "getTag", "hasTag"];
/// The extensions' methods.
pub(crate) const EXT_METHODS: [&str; 18] = [
    "lessThan",
    "lessThanOrEqual",
    "greaterThan",
    "greaterThanOrEqual",
    "isIpv4",
    "isIpv6",
    "isLoopback",
    "isMulticast",
    "isInRange",
    "offset",
    "durationSince",
    "toDate",
    "toTime",
    "toMilliseconds",
    "toSeconds",
    "toMinutes",
    "toHours",
    "toDays",
];
/// The extensions' functions.
pub(crate) const EXT_FUNCTIONS: [&str; 5] = ["decimal", "ip", "datetime", "duration", "unknown"];

type R<T> = Result<T, Error>;

struct Cx<'a> {
    lines: Lines<'a>,
}

impl Cx<'_> {
    fn pos(&self, s: Span) -> (usize, usize) {
        self.lines.at(s.start)
    }

    fn err(&self, s: Span, message: Text) -> Error {
        self.lines.err(s.start, message)
    }

    fn mk(&self, s: Span, kind: ExprKind) -> Expr {
        let (line, col) = self.pos(s);
        Expr { kind, line, col }
    }
}

pub(crate) fn convert(src: &str, parsed: &cst::Parsed) -> R<PolicySet> {
    let cx = Cx { lines: Lines::new(src) };
    let mut policies = Vec::new();
    for (n, p) in parsed.policies.iter().enumerate() {
        policies.push(cx.policy(p, n)?);
    }
    // the CLI renames a policy after its `@id`, templates first, and stops at an id taken twice
    let mut seen: Vec<&str> = Vec::new();
    let order: Vec<&Policy> = policies.iter().filter(|p| p.is_template()).chain(policies.iter().filter(|p| !p.is_template())).collect();
    for p in order {
        if seen.contains(&p.id.as_str()) {
            let id = &p.id;
            return Err(Error { line: p.line, col: p.col, message: tr!("ポリシーの id `{id}` が二度あります", "the policy id `{id}` is used twice") });
        }
        seen.push(&p.id);
    }
    Ok(PolicySet { policies })
}

fn reserved(id: &cst::Ident) -> bool {
    matches!(id, cst::Ident::If | cst::Ident::True | cst::Ident::False | cst::Ident::Then | cst::Ident::Else | cst::Ident::In | cst::Ident::Is | cst::Ident::Has | cst::Ident::Like)
}

/// An expression, or a word that is one only in some places (Cedar's `ExprOrSpecial`).
enum Eos {
    Expr(Expr, Span),
    Var(Var, Span),
    Name(Name, Span),
    StrLit(String, Span),
    BoolLit(bool, Span),
}

impl Eos {
    fn span(&self) -> Span {
        match self {
            Eos::Expr(_, s) | Eos::Var(_, s) | Eos::Name(_, s) | Eos::StrLit(_, s) | Eos::BoolLit(_, s) => *s,
        }
    }
}

/// The text of a string read as Rust reads a string literal: `\n \r \t \\ \0 \' \"`, `\x7F`
/// (ASCII), `\u{…}` (one to six hex digits); a bare carriage return is not read. In a pattern
/// (`like`), `*` is the wildcard and `\*` a star.
fn unescape(s: &str, pattern: bool) -> Result<Vec<PatternElem>, Text> {
    let mut out = Vec::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\r' {
            return Err(tr!("文字列の中に、エスケープしていない CR があります（`\\r` と書いてください）", "a bare carriage return in a string (write `\\r`)"));
        }
        if c != '\\' {
            out.push(if pattern && c == '*' { PatternElem::Wildcard } else { PatternElem::Char(c) });
            continue;
        }
        let e = it.next().unwrap_or('\\');
        let ch = match e {
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            '\\' => '\\',
            '0' => '\0',
            '\'' => '\'',
            '"' => '"',
            '*' if pattern => {
                out.push(PatternElem::Char('*'));
                continue;
            }
            'x' => {
                let h: String = it.by_ref().take(2).collect();
                match u32::from_str_radix(&h, 16) {
                    Ok(v) if h.len() == 2 && h.chars().all(|c| c.is_ascii_hexdigit()) && v <= 0x7f => char::from_u32(v).unwrap(),
                    _ => return Err(tr!("`\\x{h}` は読めないエスケープです（`\\x` は 7F までの 2 桁の 16 進です）", "`\\x{h}` is not a valid escape (`\\x` takes two hexadecimal digits up to 7F)")),
                }
            }
            'u' => {
                if it.next() != Some('{') {
                    return Err(tr!("`\\u` のあとに `{{` がありません", "no `{{` after `\\u`"));
                }
                let mut h = String::new();
                let mut closed = false;
                for d in it.by_ref() {
                    if d == '}' {
                        closed = true;
                        break;
                    }
                    h.push(d);
                }
                let digits: String = h.chars().filter(|c| *c != '_').collect();
                let ok = closed && !h.starts_with('_') && !digits.is_empty() && digits.len() <= 6 && h.chars().all(|c| c == '_' || c.is_ascii_hexdigit());
                match (ok, u32::from_str_radix(&digits, 16).ok().and_then(char::from_u32)) {
                    (true, Some(c)) => c,
                    _ => return Err(tr!("`\\u{{{h}}}` は文字ではありません", "`\\u{{{h}}}` is not a character")),
                }
            }
            other => return Err(tr!("`\\{other}` は読めないエスケープです", "`\\{other}` is not a valid escape")),
        };
        out.push(PatternElem::Char(ch));
    }
    Ok(out)
}

/// The text of a string literal of either grammar (policy or schema), its escapes read.
pub(crate) fn unescape_text(s: &str) -> Result<String, Text> {
    unescape_str(s)
}

fn unescape_str(s: &str) -> Result<String, Text> {
    Ok(unescape(s, false)?.into_iter().map(|e| if let PatternElem::Char(c) = e { c } else { '*' }).collect())
}

impl Cx<'_> {
    fn string(&self, n: &Node<String>) -> R<String> {
        unescape_str(&n.node).map_err(|m| self.err(n.span, m))
    }

    fn policy(&self, node: &Node<cst::Policy>, n: usize) -> R<Policy> {
        let p = &node.node;
        let effect = match p.effect.node {
            cst::Ident::Permit => Effect::Permit,
            cst::Ident::Forbid => Effect::Forbid,
            ref other => {
                let w = other.as_str();
                return Err(self.err(p.effect.span, tr!("ポリシーは `permit` か `forbid` で始めてください（`{w}` があります）", "a policy starts with `permit` or `forbid`, not `{w}`")));
            }
        };
        let mut annotations: Vec<Annotation> = Vec::new();
        for a in &p.annotations {
            let key = a.node.key.node.as_str().to_string();
            if annotations.iter().any(|b| b.key == key) {
                return Err(self.err(a.span, tr!("注釈 `@{key}` が二度あります", "the annotation `@{key}` is written twice")));
            }
            let value = match &a.node.value {
                Some(v) => Some(self.string(v)?),
                None => None,
            };
            let (line, col) = self.pos(a.span);
            annotations.push(Annotation { key, value, line, col });
        }
        let vars = &p.variables;
        if vars.len() < 3 {
            let missing = ["principal", "action", "resource"][vars.len()];
            return Err(self.err(p.effect.span, tr!("スコープに `{missing}` がありません", "the scope has no `{missing}`")));
        }
        if vars.len() > 3 {
            return Err(self.err(vars[3].span, tr!("スコープに四つ目の要素があります。スコープは principal、action、resource の三つです", "a fourth element in the scope; the scope is principal, action and resource")));
        }
        let principal = self.scope(&vars[0], cst::Ident::Principal)?;
        let action = self.action_scope(&vars[1])?;
        let resource = self.scope(&vars[2], cst::Ident::Resource)?;
        let mut conditions = Vec::new();
        for c in &p.conds {
            let kind = match c.node.cond.node {
                cst::Ident::When => CondKind::When,
                cst::Ident::Unless => CondKind::Unless,
                ref other => {
                    let w = other.as_str();
                    return Err(self.err(c.node.cond.span, tr!("条件は `when` か `unless` で書いてください（`{w}` があります）", "a condition is `when` or `unless`, not `{w}`")));
                }
            };
            let Some(e) = &c.node.expr else {
                let w = c.node.cond.node.as_str();
                return Err(self.err(c.span, tr!("`{w}` の中が空です", "the `{w}` clause is empty")));
            };
            let body = self.expr(e)?;
            if let Some(span) = first_slot(&body) {
                let w = c.node.cond.node.as_str();
                return Err(Error { line: span.0, col: span.1, message: tr!("`{w}` の中にテンプレートのスロットがあります。スロットはスコープにだけ書けます", "a template slot in a `{w}` clause; slots go only in the scope") });
            }
            let (line, col) = self.pos(c.span);
            conditions.push(Condition { kind, body, line, col });
        }
        let id = annotations.iter().find(|a| a.key == "id").and_then(|a| a.value.clone()).unwrap_or_else(|| format!("policy{n}"));
        let (line, col) = self.pos(node.span);
        Ok(Policy { id, annotations, effect, principal, action, resource, conditions, line, col })
    }

    fn scope_var(&self, v: &Node<cst::VariableDef>, want: cst::Ident) -> R<()> {
        let got = &v.node.variable.node;
        if !matches!(got, cst::Ident::Principal | cst::Ident::Action | cst::Ident::Resource) {
            let w = got.as_str();
            return Err(self.err(v.node.variable.span, tr!("`{w}` はスコープの変数ではありません", "`{w}` is not a variable of the scope")));
        }
        if *got != want {
            let (w, g) = (want.as_str(), got.as_str());
            return Err(self.err(v.node.variable.span, tr!("ここには `{w}` が要ります（`{g}` があります）", "`{w}` goes here, not `{g}`")));
        }
        if let Some(t) = &v.node.unused_type_name {
            return Err(self.err(t.span, tr!("`:` で型を書く形は読みません", "type constraints written with `:` are not read")));
        }
        Ok(())
    }

    fn scope(&self, v: &Node<cst::VariableDef>, want: cst::Ident) -> R<Scope> {
        self.scope_var(v, want.clone())?;
        let var = if want == cst::Ident::Principal { Var::Principal } else { Var::Resource };
        let entity_type = match &v.node.entity_type {
            Some(et) => Some(self.entity_type(et)?),
            None => None,
        };
        let Some((op, rhs)) = &v.node.ineq else {
            return Ok(match entity_type {
                Some(et) => Scope::Is(et),
                None => Scope::Any,
            });
        };
        if *op == cst::RelOp::In && is_is_relation(rhs) {
            return Err(self.err(v.span, tr!("`is` は `in` の前に書いてください（`{0} is T in E`）", "`is` goes before `in` (`{0} is T in E`)", var.as_str())));
        }
        let r = self.ref_or_slot(rhs, var)?;
        match (op, entity_type) {
            (cst::RelOp::Eq, None) => Ok(Scope::Eq(r)),
            (cst::RelOp::Eq, Some(_)) => Err(self.err(v.span, tr!("`is` と `==` は一緒に書けません", "`is` and `==` cannot be written together"))),
            (cst::RelOp::In, None) => Ok(Scope::In(r)),
            (cst::RelOp::In, Some(et)) => Ok(Scope::IsIn(et, r)),
            (cst::RelOp::InvalidSingleEq, _) => Err(self.err(v.span, tr!("`=` ではなく `==` と書いてください", "write `==`, not `=`"))),
            (op, _) => {
                let o = op.as_str();
                Err(self.err(v.span, tr!("スコープに `{o}` は書けません（書けるのは `==`、`in`、`is` です）", "`{o}` cannot be in the scope (`==`, `in` and `is` can)")))
            }
        }
    }

    fn entity_type(&self, a: &Node<cst::Add>) -> R<Name> {
        match self.add(a)? {
            Eos::Var(v, _) => Ok(Name::new(v.as_str())),
            Eos::Name(n, _) => Ok(n),
            other => Err(self.err(other.span(), tr!("`is` のあとにはエンティティタイプの名前を書いてください", "the name of an entity type goes after `is`"))),
        }
    }

    fn action_scope(&self, v: &Node<cst::VariableDef>) -> R<ActionScope> {
        self.scope_var(v, cst::Ident::Action)?;
        if v.node.entity_type.is_some() {
            return Err(self.err(v.span, tr!("action のスコープに `is` は書けません", "`is` cannot be in the scope of the action")));
        }
        let Some((op, rhs)) = &v.node.ineq else { return Ok(ActionScope::Any) };
        let check = |e: &EntityUid| -> R<()> {
            if e.ty.id != "Action" {
                let t = e.ty.to_string();
                return Err(Error { line: e.line, col: e.col, message: tr!("action のスコープに書けるのは、型が `Action` のエンティティだけです（`{t}` があります）", "the entities of the action's scope are of type `Action` (`{t}` is not)") });
            }
            Ok(())
        };
        match op {
            cst::RelOp::In => {
                if is_is_relation(rhs) {
                    return Err(self.err(v.span, tr!("action のスコープに `is` は書けません", "`is` cannot be in the scope of the action")));
                }
                match self.refs(rhs)? {
                    Refs::One(e) => {
                        check(&e)?;
                        Ok(ActionScope::In(e))
                    }
                    Refs::Many(es) => {
                        for e in &es {
                            check(e)?;
                        }
                        Ok(ActionScope::InList(es))
                    }
                }
            }
            cst::RelOp::Eq => {
                let e = self.single_ref(rhs, "action")?;
                check(&e)?;
                Ok(ActionScope::Eq(e))
            }
            cst::RelOp::InvalidSingleEq => Err(self.err(v.span, tr!("`=` ではなく `==` と書いてください", "write `==`, not `=`"))),
            op => {
                let o = op.as_str();
                Err(self.err(v.span, tr!("action のスコープに `{o}` は書けません（書けるのは `==` と `in` です）", "`{o}` cannot be in the scope of the action (`==` and `in` can)")))
            }
        }
    }

    /// The primary under an expression with no operator, access or call: what the scope reads.
    fn scope_primary<'n>(&self, e: &'n Node<cst::Expr>) -> R<&'n Node<cst::Primary>> {
        let bad = |s: Span| self.err(s, tr!("スコープにはエンティティ（`Type::\"id\"`）かスロットを書いてください", "the scope takes an entity (`Type::\"id\"`) or a slot"));
        let cst::Expr::Or(or) = &e.node else { return Err(bad(e.span)) };
        if !or.node.extended.is_empty() || !or.node.initial.node.extended.is_empty() {
            return Err(bad(e.span));
        }
        let cst::Relation::Common { initial, extended } = &or.node.initial.node.initial.node else { return Err(bad(e.span)) };
        if !extended.is_empty() || !initial.node.extended.is_empty() || !initial.node.initial.node.extended.is_empty() {
            return Err(bad(e.span));
        }
        let unary = &initial.node.initial.node.initial;
        if unary.node.op.is_some() || !unary.node.item.node.access.is_empty() {
            return Err(bad(e.span));
        }
        let prim = &unary.node.item.node.item;
        if let cst::Primary::Expr(inner) = &prim.node {
            return self.scope_primary(inner);
        }
        Ok(prim)
    }

    fn ref_or_slot(&self, e: &Node<cst::Expr>, var: Var) -> R<EntityOrSlot> {
        let prim = self.scope_primary(e)?;
        match &prim.node {
            cst::Primary::Ref(r) => Ok(EntityOrSlot::Entity(self.entity_ref(r)?)),
            cst::Primary::Slot(s) => match (&s.node, var) {
                (cst::Slot::Principal, Var::Principal) => Ok(EntityOrSlot::Slot(Slot::Principal)),
                (cst::Slot::Resource, Var::Resource) => Ok(EntityOrSlot::Slot(Slot::Resource)),
                (other, var) => {
                    let (o, v) = (other.as_str(), var.as_str());
                    Err(self.err(s.span, tr!("ここに書けるスロットは `?{v}` です（`{o}` があります）", "the slot here is `?{v}`, not `{o}`")))
                }
            },
            _ => Err(self.err(prim.span, tr!("スコープにはエンティティ（`Type::\"id\"`）かスロットを書いてください", "the scope takes an entity (`Type::\"id\"`) or a slot"))),
        }
    }

    fn single_ref(&self, e: &Node<cst::Expr>, var: &str) -> R<EntityUid> {
        let prim = self.scope_primary(e)?;
        match &prim.node {
            cst::Primary::Ref(r) => self.entity_ref(r),
            _ => Err(self.err(prim.span, tr!("{var} のスコープにはエンティティ（`Type::\"id\"`）を一つ書いてください", "the scope of the {var} takes one entity (`Type::\"id\"`)"))),
        }
    }

    fn refs(&self, e: &Node<cst::Expr>) -> R<Refs> {
        let prim = self.scope_primary(e)?;
        match &prim.node {
            cst::Primary::Ref(r) => Ok(Refs::One(self.entity_ref(r)?)),
            cst::Primary::EList(es) => {
                let mut out = Vec::new();
                for x in es {
                    out.push(self.single_ref(x, "action")?);
                }
                // `action in [A]` is the JSON format's `in` one entity
                if out.len() == 1 { Ok(Refs::One(out.pop().unwrap())) } else { Ok(Refs::Many(out)) }
            }
            _ => Err(self.err(prim.span, tr!("action のスコープにはエンティティか、エンティティのリストを書いてください", "the scope of the action takes an entity or a list of entities"))),
        }
    }

    fn entity_ref(&self, r: &Node<cst::Ref>) -> R<EntityUid> {
        match &r.node {
            cst::Ref::Uid { path, eid } => {
                let ty = self.name(path)?;
                let id = self.string(eid)?;
                let (line, col) = self.pos(r.span);
                Ok(EntityUid { ty, id, line, col })
            }
            cst::Ref::Ref { .. } => Err(self.err(r.span, tr!("`Type::{{…}}` の形のエンティティは読みません。`Type::\"id\"` と書いてください", "an entity written `Type::{{…}}` is not read; write `Type::\"id\"`"))),
        }
    }

    fn ident(&self, id: &Node<cst::Ident>) -> R<String> {
        if reserved(&id.node) {
            let w = id.node.as_str();
            return Err(self.err(id.span, tr!("`{w}` は予約語なので、名前に使えません", "`{w}` is reserved and cannot be a name")));
        }
        Ok(id.node.as_str().to_string())
    }

    /// An attribute after `.` or in the path after `has`: not a reserved word, and not `__cedar`
    /// either (Cedar's `UnreservedId`; `["__cedar"]` is read).
    fn attr_ident(&self, id: &Node<cst::Ident>) -> R<String> {
        let w = self.ident(id)?;
        if w == "__cedar" {
            return Err(self.err(id.span, tr!("名前 `__cedar` は予約されているので、使えません", "the name `__cedar` is reserved")));
        }
        Ok(w)
    }

    fn name(&self, n: &Node<cst::Name>) -> R<Name> {
        let mut path = Vec::new();
        for p in &n.node.path {
            path.push(self.ident(p)?);
        }
        let id = self.ident(&n.node.name)?;
        let name = Name { path, id };
        if name.path.iter().chain(std::iter::once(&name.id)).any(|p| p == "__cedar") {
            let s = name.to_string();
            return Err(self.err(n.span, tr!("名前 `{s}` に、予約された `__cedar` が含まれています", "the name `{s}` contains `__cedar`, which is reserved")));
        }
        Ok(name)
    }

    fn expr_of(&self, e: Eos) -> R<Expr> {
        match e {
            Eos::Expr(x, _) => Ok(x),
            Eos::Var(v, s) => Ok(self.mk(s, ExprKind::Var(v))),
            Eos::Name(n, s) => {
                let n = n.to_string();
                Err(self.err(s, tr!("`{n}` は変数ではありません。変数は principal、action、resource、context です", "`{n}` is not a variable; the variables are principal, action, resource and context")))
            }
            Eos::StrLit(raw, s) => {
                let v = unescape_str(&raw).map_err(|m| self.err(s, m))?;
                Ok(self.mk(s, ExprKind::Str(v)))
            }
            Eos::BoolLit(b, s) => Ok(self.mk(s, ExprKind::Bool(b))),
        }
    }

    fn expr(&self, e: &Node<cst::Expr>) -> R<Expr> {
        let x = self.expr_eos(e)?;
        self.expr_of(x)
    }

    fn expr_eos(&self, e: &Node<cst::Expr>) -> R<Eos> {
        match &e.node {
            cst::Expr::Or(or) => self.or(or),
            cst::Expr::If(c, t, f) => {
                let (c, t, f) = (self.expr(c)?, self.expr(t)?, self.expr(f)?);
                Ok(Eos::Expr(self.mk(e.span, ExprKind::If { cond: Box::new(c), then: Box::new(t), els: Box::new(f) }), e.span))
            }
        }
    }

    fn fold(&self, span: Span, op: BinOp, first: Expr, rest: Vec<Expr>) -> Expr {
        rest.into_iter().fold(first, |acc, next| self.mk(span, ExprKind::Binary { op, left: Box::new(acc), right: Box::new(next) }))
    }

    fn or(&self, or: &Node<cst::Or>) -> R<Eos> {
        let first = self.and(&or.node.initial)?;
        if or.node.extended.is_empty() {
            return Ok(first);
        }
        let mut rest = Vec::new();
        for x in &or.node.extended {
            let e = self.and(x)?;
            rest.push(self.expr_of(e)?);
        }
        let first = self.expr_of(first)?;
        Ok(Eos::Expr(self.fold(or.span, BinOp::Or, first, rest), or.span))
    }

    fn and(&self, and: &Node<cst::And>) -> R<Eos> {
        let first = self.relation(&and.node.initial)?;
        if and.node.extended.is_empty() {
            return Ok(first);
        }
        let mut rest = Vec::new();
        for x in &and.node.extended {
            let e = self.relation(x)?;
            rest.push(self.expr_of(e)?);
        }
        let first = self.expr_of(first)?;
        Ok(Eos::Expr(self.fold(and.span, BinOp::And, first, rest), and.span))
    }

    fn add_expr(&self, a: &Node<cst::Add>) -> R<Expr> {
        let e = self.add(a)?;
        self.expr_of(e)
    }

    fn relation(&self, r: &Node<cst::Relation>) -> R<Eos> {
        let span = r.span;
        match &r.node {
            cst::Relation::Common { initial, extended } => {
                let first = self.add(initial)?;
                if extended.len() > 1 {
                    return Err(self.err(span, tr!("比べる演算子が続いています。括弧で区切ってください", "comparison operators in a row; put parentheses around one of them")));
                }
                let Some((op, second)) = extended.first() else { return Ok(first) };
                let second = self.add_expr(second)?;
                let first = self.expr_of(first)?;
                let op = match op {
                    cst::RelOp::Less => BinOp::Less,
                    cst::RelOp::LessEq => BinOp::LessEq,
                    cst::RelOp::GreaterEq => BinOp::GreaterEq,
                    cst::RelOp::Greater => BinOp::Greater,
                    cst::RelOp::NotEq => BinOp::NotEq,
                    cst::RelOp::Eq => BinOp::Eq,
                    cst::RelOp::In => BinOp::In,
                    cst::RelOp::InvalidSingleEq => return Err(self.err(span, tr!("`=` ではなく `==` と書いてください", "write `==`, not `=`"))),
                };
                Ok(Eos::Expr(self.mk(span, ExprKind::Binary { op, left: Box::new(first), right: Box::new(second) }), span))
            }
            cst::Relation::Has { target, field } => {
                let t = self.add_expr(target)?;
                let attrs = self.has_rhs(field)?;
                Ok(Eos::Expr(self.mk(span, ExprKind::Has { expr: Box::new(t), attrs }), span))
            }
            cst::Relation::Like { target, pattern } => {
                let t = self.add_expr(target)?;
                let p = match self.add(pattern)? {
                    Eos::StrLit(raw, s) => unescape(&raw, true).map_err(|m| self.err(s, m))?,
                    other => return Err(self.err(other.span(), tr!("`like` のあとにはパターンの文字列を書いてください", "a pattern string goes after `like`"))),
                };
                Ok(Eos::Expr(self.mk(span, ExprKind::Like { expr: Box::new(t), pattern: p }), span))
            }
            cst::Relation::IsIn { target, entity_type, in_entity } => {
                let t = self.add_expr(target)?;
                let ty = self.entity_type(entity_type)?;
                let in_expr = match in_entity {
                    Some(x) => Some(Box::new(self.add_expr(x)?)),
                    None => None,
                };
                Ok(Eos::Expr(self.mk(span, ExprKind::Is { expr: Box::new(t), ty, in_expr }), span))
            }
        }
    }

    /// What `has` tests: a string, or identifiers joined by `.`.
    fn has_rhs(&self, a: &Node<cst::Add>) -> R<Vec<String>> {
        let bad = |s: Span| self.err(s, tr!("`has` のあとには属性の名前か文字列を書いてください", "the name of an attribute or a string goes after `has`"));
        if !a.node.extended.is_empty() || !a.node.initial.node.extended.is_empty() {
            return Err(bad(a.span));
        }
        let unary = &a.node.initial.node.initial;
        if unary.node.op.is_some() {
            return Err(bad(a.span));
        }
        let member = &unary.node.item.node;
        let item = &member.item;
        let first = match &item.node {
            cst::Primary::Literal(_) | cst::Primary::Name(_) => match self.primary(item)? {
                Eos::StrLit(raw, s) if member.access.is_empty() => {
                    return Ok(vec![unescape_str(&raw).map_err(|m| self.err(s, m))?]);
                }
                Eos::Var(v, _) => v.as_str().to_string(),
                Eos::Name(n, s) => {
                    if !n.path.is_empty() {
                        let n = n.to_string();
                        return Err(self.err(s, tr!("`{n}` は属性の名前に使えません（`::` があります）", "`{n}` cannot name an attribute (it has `::`)")));
                    }
                    n.id
                }
                other => return Err(bad(other.span())),
            },
            _ => return Err(bad(item.span)),
        };
        let mut attrs = vec![first];
        for acc in &member.access {
            match &acc.node {
                cst::MemAccess::Field(id) => attrs.push(self.attr_ident(id)?),
                _ => return Err(bad(acc.span)),
            }
        }
        Ok(attrs)
    }

    fn add(&self, a: &Node<cst::Add>) -> R<Eos> {
        let first = self.mult(&a.node.initial)?;
        if a.node.extended.is_empty() {
            return Ok(first);
        }
        let mut rest = Vec::new();
        for (op, m) in &a.node.extended {
            let e = self.mult(m)?;
            rest.push((*op, self.expr_of(e)?));
        }
        let mut acc = self.expr_of(first)?;
        for (op, next) in rest {
            let op = if op == cst::AddOp::Plus { BinOp::Add } else { BinOp::Sub };
            acc = self.mk(a.span, ExprKind::Binary { op, left: Box::new(acc), right: Box::new(next) });
        }
        Ok(Eos::Expr(acc, a.span))
    }

    fn mult(&self, m: &Node<cst::Mult>) -> R<Eos> {
        let first = self.unary(&m.node.initial)?;
        if m.node.extended.is_empty() {
            return Ok(first);
        }
        let mut rest = Vec::new();
        for (op, u) in &m.node.extended {
            let e = self.unary(u)?;
            let e = self.expr_of(e)?;
            match op {
                cst::MultOp::Times => rest.push(e),
                cst::MultOp::Divide => return Err(self.err(m.span, tr!("Cedar に割り算（`/`）はありません", "division (`/`) is not supported"))),
                cst::MultOp::Mod => return Err(self.err(m.span, tr!("Cedar に剰余（`%`）はありません", "remainder (`%`) is not supported"))),
            }
        }
        let first = self.expr_of(first)?;
        Ok(Eos::Expr(self.fold(m.span, BinOp::Mul, first, rest), m.span))
    }

    fn unary(&self, u: &Node<cst::Unary>) -> R<Eos> {
        let span = u.span;
        match u.node.op {
            None => self.member(&u.node.item),
            Some(cst::NegOp::Bang(n)) => {
                let mut e = self.member(&u.node.item)?;
                for _ in 0..n {
                    let x = self.expr_of(e)?;
                    e = Eos::Expr(self.mk(span, ExprKind::Not(Box::new(x))), span);
                }
                Ok(e)
            }
            Some(cst::NegOp::Dash(c)) => {
                let lit = literal_num(&u.node.item);
                let (mut e, rc) = match lit {
                    Some(n) if n == i64::MAX as u64 + 1 => (self.mk(u.node.item.span, ExprKind::Long(i64::MIN)), c - 1),
                    Some(n) if n <= i64::MAX as u64 => (self.mk(u.node.item.span, ExprKind::Long(-(n as i64))), c - 1),
                    Some(n) => return Err(self.err(span, tr!("整数 `{n}` が大きすぎます", "the integer literal `{n}` is too large"))),
                    None => {
                        let x = self.member(&u.node.item)?;
                        (self.expr_of(x)?, c)
                    }
                };
                for _ in 0..rc {
                    e = self.mk(span, ExprKind::Neg(Box::new(e)));
                }
                Ok(Eos::Expr(e, span))
            }
            Some(cst::NegOp::OverBang) => Err(self.err(span, tr!("`!` を続けて書けるのは四つまでです", "at most four `!` in a row"))),
            Some(cst::NegOp::OverDash) => Err(self.err(span, tr!("`-` を続けて書けるのは四つまでです", "at most four `-` in a row"))),
        }
    }

    fn method(&self, span: Span, recv: Expr, name: &str, args: Vec<Expr>) -> R<Expr> {
        let arity = |want: usize| -> R<()> {
            if args.len() != want {
                let n = args.len();
                return Err(self.err(span, tr!("`{name}` の引数は {want} 個です（{n} 個あります）", "`{name}` takes {want} argument(s), not {n}")));
            }
            Ok(())
        };
        match name {
            "contains" | "containsAll" | "containsAny" | "getTag" | "hasTag" => arity(1)?,
            "isEmpty" => arity(0)?,
            n if EXT_METHODS.contains(&n) => {}
            n if EXT_FUNCTIONS.contains(&n) => {
                return Err(self.err(span, tr!("`{n}` は関数です。`{n}(…)` の形で呼んでください", "`{n}` is a function; call it as `{n}(…)`")));
            }
            n => return Err(self.err(span, tr!("`{n}` というメソッドはありません", "`{n}` is not a valid method"))),
        }
        Ok(self.mk(span, ExprKind::Method { expr: Box::new(recv), name: name.to_string(), args }))
    }

    fn member(&self, m: &Node<cst::Member>) -> R<Eos> {
        let span = m.span;
        let prim = self.primary(&m.node.item)?;
        // every accessor is read first, as Cedar reads them
        let mut accs = Vec::new();
        for a in &m.node.access {
            accs.push(match &a.node {
                cst::MemAccess::Field(id) => Acc::Field(self.attr_ident(id)?),
                cst::MemAccess::Call(args) => {
                    let mut v = Vec::new();
                    for x in args {
                        v.push(self.expr(x)?);
                    }
                    Acc::Call(v)
                }
                cst::MemAccess::Index(e) => match self.expr_eos(e)? {
                    Eos::StrLit(raw, s) => Acc::Index(unescape_str(&raw).map_err(|msg| self.err(s, msg))?),
                    other => return Err(self.err(other.span(), tr!("`[…]` の中には属性の名前を文字列で書いてください", "the name of an attribute, as a string, goes inside `[…]`"))),
                },
            });
        }
        if accs.is_empty() {
            return Ok(prim);
        }
        let mut accs = accs.into_iter().peekable();
        let mut head = match prim {
            Eos::Name(name, _) => match accs.next().unwrap() {
                Acc::Call(args) => {
                    let n = name.to_string();
                    if name.path.is_empty() && (EXT_METHODS.contains(&name.id.as_str()) || BUILTIN_METHODS.contains(&name.id.as_str())) {
                        return Err(self.err(span, tr!("`{n}` はメソッドです。`x.{n}(…)` の形で呼んでください", "`{n}` is a method; call it as `x.{n}(…)`")));
                    }
                    if !name.path.is_empty() || !EXT_FUNCTIONS.contains(&name.id.as_str()) {
                        return Err(self.err(span, tr!("`{n}` という関数はありません", "`{n}` is not a function")));
                    }
                    self.mk(span, ExprKind::Call { func: name, args })
                }
                Acc::Field(f) => {
                    let n = name.to_string();
                    if matches!(accs.peek(), Some(Acc::Call(_))) {
                        return Err(self.err(span, tr!("`{n}` にはメソッドがありません（`{n}.{f}(…)`）", "`{n}` has no methods (`{n}.{f}(…)`)")));
                    }
                    return Err(self.err(span, tr!("`{n}.{f}` は読めません。`{n}` は変数でもエンティティでもありません", "`{n}.{f}` is not read: `{n}` is neither a variable nor an entity")));
                }
                Acc::Index(i) => {
                    let n = name.to_string();
                    return Err(self.err(span, tr!("`{n}[\"{i}\"]` は読めません。`{n}` は変数でもエンティティでもありません", "`{n}[\"{i}\"]` is not read: `{n}` is neither a variable nor an entity")));
                }
            },
            Eos::Var(v, vspan) => match accs.next().unwrap() {
                Acc::Call(_) => {
                    let v = v.as_str();
                    return Err(self.err(span, tr!("変数 `{v}` は関数として呼べません", "the variable `{v}` cannot be called")));
                }
                Acc::Field(f) => {
                    let var = self.mk(vspan, ExprKind::Var(v));
                    if let Some(Acc::Call(_)) = accs.peek() {
                        let Some(Acc::Call(args)) = accs.next() else { unreachable!() };
                        self.method(span, var, &f, args)?
                    } else {
                        self.mk(span, ExprKind::GetAttr { expr: Box::new(var), attr: f })
                    }
                }
                Acc::Index(i) => {
                    let var = self.mk(vspan, ExprKind::Var(v));
                    self.mk(span, ExprKind::GetAttr { expr: Box::new(var), attr: i })
                }
            },
            other => {
                let x = self.expr_of(other)?;
                self.access(span, x, &mut accs)?
            }
        };
        while accs.peek().is_some() {
            head = self.access(span, head, &mut accs)?;
        }
        Ok(Eos::Expr(head, span))
    }

    fn access(&self, span: Span, head: Expr, accs: &mut std::iter::Peekable<std::vec::IntoIter<Acc>>) -> R<Expr> {
        match accs.next().unwrap() {
            Acc::Call(_) => Err(self.err(span, tr!("式は関数として呼べません", "an expression cannot be called"))),
            Acc::Field(f) => {
                if let Some(Acc::Call(_)) = accs.peek() {
                    let Some(Acc::Call(args)) = accs.next() else { unreachable!() };
                    self.method(span, head, &f, args)
                } else {
                    Ok(self.mk(span, ExprKind::GetAttr { expr: Box::new(head), attr: f }))
                }
            }
            Acc::Index(i) => Ok(self.mk(span, ExprKind::GetAttr { expr: Box::new(head), attr: i })),
        }
    }

    fn primary(&self, p: &Node<cst::Primary>) -> R<Eos> {
        let span = p.span;
        match &p.node {
            cst::Primary::Literal(l) => match &l.node {
                cst::Literal::True => Ok(Eos::BoolLit(true, l.span)),
                cst::Literal::False => Ok(Eos::BoolLit(false, l.span)),
                cst::Literal::Num(n) => {
                    if *n > i64::MAX as u64 {
                        return Err(self.err(l.span, tr!("整数 `{n}` が大きすぎます", "the integer literal `{n}` is too large")));
                    }
                    Ok(Eos::Expr(self.mk(l.span, ExprKind::Long(*n as i64)), l.span))
                }
                cst::Literal::Str(s) => Ok(Eos::StrLit(s.node.clone(), l.span)),
            },
            cst::Primary::Ref(r) => {
                let e = self.entity_ref(r)?;
                Ok(Eos::Expr(self.mk(r.span, ExprKind::Entity(e)), r.span))
            }
            cst::Primary::Slot(s) => {
                let slot = match &s.node {
                    cst::Slot::Principal => Slot::Principal,
                    cst::Slot::Resource => Slot::Resource,
                    cst::Slot::Other(o) => return Err(self.err(s.span, tr!("`{o}` はスロットではありません（スロットは `?principal` と `?resource` です）", "`{o}` is not a slot (the slots are `?principal` and `?resource`)"))),
                };
                Ok(Eos::Expr(self.mk(s.span, ExprKind::Slot(slot)), s.span))
            }
            cst::Primary::Name(n) => {
                if n.node.path.is_empty() {
                    let var = match n.node.name.node {
                        cst::Ident::Principal => Some(Var::Principal),
                        cst::Ident::Action => Some(Var::Action),
                        cst::Ident::Resource => Some(Var::Resource),
                        cst::Ident::Context => Some(Var::Context),
                        _ => None,
                    };
                    if let Some(v) = var {
                        return Ok(Eos::Var(v, span));
                    }
                }
                Ok(Eos::Name(self.name(n)?, span))
            }
            cst::Primary::Expr(e) => {
                let x = self.expr(e)?;
                Ok(Eos::Expr(x, e.span))
            }
            cst::Primary::EList(es) => {
                let mut v = Vec::new();
                for x in es {
                    v.push(self.expr(x)?);
                }
                Ok(Eos::Expr(self.mk(span, ExprKind::Set(v)), span))
            }
            cst::Primary::RInits(inits) => {
                let mut kv: Vec<(String, Expr)> = Vec::new();
                for init in inits {
                    let key = match self.expr_eos(&init.node.0)? {
                        Eos::Var(v, _) => v.as_str().to_string(),
                        Eos::Name(n, s) => {
                            if !n.path.is_empty() {
                                let n = n.to_string();
                                return Err(self.err(s, tr!("`{n}` は属性の名前に使えません（`::` があります）", "`{n}` cannot name an attribute (it has `::`)")));
                            }
                            n.id
                        }
                        Eos::StrLit(raw, s) => unescape_str(&raw).map_err(|m| self.err(s, m))?,
                        Eos::Expr(_, s) => return Err(self.err(s, tr!("レコードのキーには名前か文字列を書いてください", "a key of a record is a name or a string"))),
                        Eos::BoolLit(b, s) => {
                            return Err(self.err(s, tr!("`{b}` は予約語なので、名前に使えません", "`{b}` is reserved and cannot be a name")));
                        }
                    };
                    let value = self.expr(&init.node.1)?;
                    if kv.iter().any(|(k, _)| *k == key) {
                        return Err(self.err(span, tr!("レコードにキー `{key}` が二度あります", "the key `{key}` is written twice in a record")));
                    }
                    kv.push((key, value));
                }
                Ok(Eos::Expr(self.mk(span, ExprKind::Record(kv)), span))
            }
        }
    }
}

enum Acc {
    Field(String),
    Call(Vec<Expr>),
    Index(String),
}

fn literal_num(m: &Node<cst::Member>) -> Option<u64> {
    if !m.node.access.is_empty() {
        return None;
    }
    match &m.node.item.node {
        cst::Primary::Literal(l) => match l.node {
            cst::Literal::Num(n) => Some(n),
            _ => None,
        },
        _ => None,
    }
}

fn is_is_relation(e: &Node<cst::Expr>) -> bool {
    match &e.node {
        cst::Expr::Or(or) => or.node.extended.is_empty() && or.node.initial.node.extended.is_empty() && matches!(or.node.initial.node.initial.node, cst::Relation::IsIn { .. }),
        _ => false,
    }
}

enum Refs {
    One(EntityUid),
    Many(Vec<EntityUid>),
}

/// Where the first slot of an expression is.
fn first_slot(e: &Expr) -> Option<(usize, usize)> {
    let mut found = None;
    walk(e, &mut |x| {
        if found.is_none() && matches!(x.kind, ExprKind::Slot(_)) {
            found = Some((x.line, x.col));
        }
    });
    found
}

/// Every expression under `e`, `e` first.
pub fn walk(e: &Expr, f: &mut impl FnMut(&Expr)) {
    f(e);
    match &e.kind {
        ExprKind::Bool(_) | ExprKind::Long(_) | ExprKind::Str(_) | ExprKind::Entity(_) | ExprKind::Var(_) | ExprKind::Slot(_) => {}
        ExprKind::Not(x) | ExprKind::Neg(x) => walk(x, f),
        ExprKind::Binary { left, right, .. } => {
            walk(left, f);
            walk(right, f);
        }
        ExprKind::GetAttr { expr, .. } | ExprKind::Has { expr, .. } | ExprKind::Like { expr, .. } => walk(expr, f),
        ExprKind::Is { expr, in_expr, .. } => {
            walk(expr, f);
            if let Some(i) = in_expr {
                walk(i, f);
            }
        }
        ExprKind::If { cond, then, els } => {
            walk(cond, f);
            walk(then, f);
            walk(els, f);
        }
        ExprKind::Set(xs) | ExprKind::Call { args: xs, .. } => xs.iter().for_each(|x| walk(x, f)),
        ExprKind::Record(kv) => kv.iter().for_each(|(_, x)| walk(x, f)),
        ExprKind::Method { expr, args, .. } => {
            walk(expr, f);
            args.iter().for_each(|x| walk(x, f));
        }
    }
}
