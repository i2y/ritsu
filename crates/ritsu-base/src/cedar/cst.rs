//! The concrete syntax tree of a policy set: the nodes of Cedar's own grammar
//! (`cedar-policy-core/src/parser/cst.rs` at 4.13.0), each with the bytes it was written at.
//! The formatter walks this tree, since it keeps what the abstract one drops (parentheses, `!=`
//! as written, `is ... in`, the order and the comments), and the reader turns it into
//! [`super::Policy`].

use super::lexer::{lex, Grammar, Lines, Tok, Token};
use super::Error;
use crate::text::Text;
use crate::tr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct Node<T> {
    pub node: T,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Ident {
    Principal,
    Action,
    Resource,
    Context,
    True,
    False,
    Permit,
    Forbid,
    When,
    Unless,
    In,
    Has,
    Like,
    Is,
    If,
    Then,
    Else,
    Word(String),
}

impl Ident {
    pub fn from_word(w: &str) -> Ident {
        match w {
            "principal" => Ident::Principal,
            "action" => Ident::Action,
            "resource" => Ident::Resource,
            "context" => Ident::Context,
            "true" => Ident::True,
            "false" => Ident::False,
            "permit" => Ident::Permit,
            "forbid" => Ident::Forbid,
            "when" => Ident::When,
            "unless" => Ident::Unless,
            "in" => Ident::In,
            "has" => Ident::Has,
            "like" => Ident::Like,
            "is" => Ident::Is,
            "if" => Ident::If,
            "then" => Ident::Then,
            "else" => Ident::Else,
            w => Ident::Word(w.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Ident::Principal => "principal",
            Ident::Action => "action",
            Ident::Resource => "resource",
            Ident::Context => "context",
            Ident::True => "true",
            Ident::False => "false",
            Ident::Permit => "permit",
            Ident::Forbid => "forbid",
            Ident::When => "when",
            Ident::Unless => "unless",
            Ident::In => "in",
            Ident::Has => "has",
            Ident::Like => "like",
            Ident::Is => "is",
            Ident::If => "if",
            Ident::Then => "then",
            Ident::Else => "else",
            Ident::Word(s) => s,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Annotation {
    pub key: Node<Ident>,
    pub value: Option<Node<String>>,
}

#[derive(Clone, Debug)]
pub(crate) struct Policy {
    pub annotations: Vec<Node<Annotation>>,
    pub effect: Node<Ident>,
    pub variables: Vec<Node<VariableDef>>,
    pub conds: Vec<Node<Cond>>,
}

#[derive(Clone, Debug)]
pub(crate) struct VariableDef {
    pub variable: Node<Ident>,
    pub unused_type_name: Option<Node<Name>>,
    pub entity_type: Option<Node<Add>>,
    pub ineq: Option<(RelOp, Node<Expr>)>,
}

#[derive(Clone, Debug)]
pub(crate) struct Cond {
    pub cond: Node<Ident>,
    pub expr: Option<Node<Expr>>,
}

#[derive(Clone, Debug)]
pub(crate) enum Expr {
    Or(Box<Node<Or>>),
    If(Box<Node<Expr>>, Box<Node<Expr>>, Box<Node<Expr>>),
}

#[derive(Clone, Debug)]
pub(crate) struct Or {
    pub initial: Node<And>,
    pub extended: Vec<Node<And>>,
}

#[derive(Clone, Debug)]
pub(crate) struct And {
    pub initial: Node<Relation>,
    pub extended: Vec<Node<Relation>>,
}

/// Cedar's own shape of the node (its sizes differ; a tree lives only while a file is read).
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub(crate) enum Relation {
    Common { initial: Node<Add>, extended: Vec<(RelOp, Node<Add>)> },
    Has { target: Node<Add>, field: Node<Add> },
    Like { target: Node<Add>, pattern: Node<Add> },
    IsIn { target: Node<Add>, entity_type: Node<Add>, in_entity: Option<Node<Add>> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RelOp {
    Less,
    LessEq,
    GreaterEq,
    Greater,
    NotEq,
    Eq,
    In,
    InvalidSingleEq,
}

impl RelOp {
    pub fn as_str(self) -> &'static str {
        match self {
            RelOp::Less => "<",
            RelOp::LessEq => "<=",
            RelOp::GreaterEq => ">=",
            RelOp::Greater => ">",
            RelOp::NotEq => "!=",
            RelOp::Eq => "==",
            RelOp::In => "in",
            RelOp::InvalidSingleEq => "=",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AddOp {
    Plus,
    Minus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MultOp {
    Times,
    Divide,
    Mod,
}

#[derive(Clone, Debug)]
pub(crate) struct Add {
    pub initial: Node<Mult>,
    pub extended: Vec<(AddOp, Node<Mult>)>,
}

#[derive(Clone, Debug)]
pub(crate) struct Mult {
    pub initial: Node<Unary>,
    pub extended: Vec<(MultOp, Node<Unary>)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NegOp {
    Bang(u8),
    OverBang,
    Dash(u8),
    OverDash,
}

#[derive(Clone, Debug)]
pub(crate) struct Unary {
    pub op: Option<NegOp>,
    pub item: Node<Member>,
}

#[derive(Clone, Debug)]
pub(crate) struct Member {
    pub item: Node<Primary>,
    pub access: Vec<Node<MemAccess>>,
}

#[derive(Clone, Debug)]
pub(crate) enum MemAccess {
    Field(Node<Ident>),
    Call(Vec<Node<Expr>>),
    Index(Box<Node<Expr>>),
}

#[derive(Clone, Debug)]
pub(crate) enum Primary {
    Literal(Node<Literal>),
    Ref(Node<Ref>),
    Name(Node<Name>),
    Slot(Node<Slot>),
    Expr(Box<Node<Expr>>),
    EList(Vec<Node<Expr>>),
    RInits(Vec<Node<RecInit>>),
}

#[derive(Clone, Debug)]
pub(crate) struct Name {
    pub path: Vec<Node<Ident>>,
    pub name: Node<Ident>,
}

#[derive(Clone, Debug)]
pub(crate) enum Ref {
    Uid { path: Node<Name>, eid: Node<String> },
    /// `Type::{…}`: the grammar reads it, and no later step accepts it.
    #[allow(dead_code)]
    Ref { path: Node<Name>, rinits: Vec<Node<RefInit>> },
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct RefInit(pub Node<Ident>, pub Node<Literal>);

#[derive(Clone, Debug)]
pub(crate) struct RecInit(pub Node<Expr>, pub Node<Expr>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    Principal,
    Resource,
    Other(String),
}

impl Slot {
    pub fn as_str(&self) -> &str {
        match self {
            Slot::Principal => "?principal",
            Slot::Resource => "?resource",
            Slot::Other(s) => s,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Literal {
    True,
    False,
    Num(u64),
    /// The text between the quotes, as written.
    Str(Node<String>),
}

/// A policy set read token by token, and the tokens (the formatter needs them for comments).
pub(crate) struct Parsed {
    pub policies: Vec<Node<Policy>>,
    pub tokens: Vec<Token>,
}

pub(crate) fn parse(src: &str) -> Result<Parsed, Error> {
    let tokens = lex(src, Grammar::Policy)?;
    let lines = Lines::new(src);
    let mut p = P { t: &tokens, i: 0, lines: &lines, src, depth: 0 };
    let mut policies = Vec::new();
    while p.i < p.t.len() {
        policies.push(p.policy()?);
    }
    Ok(Parsed { policies, tokens })
}

/// The deepest nesting read: the parser is recursive.
const MAX_DEPTH: usize = 200;

struct P<'a> {
    t: &'a [Token],
    i: usize,
    lines: &'a Lines<'a>,
    src: &'a str,
    depth: usize,
}

type R<T> = Result<T, Error>;

impl<'a> P<'a> {
    fn peek(&self) -> Option<&'a Tok> {
        self.t.get(self.i).map(|t| &t.tok)
    }

    fn peek_at(&self, k: usize) -> Option<&'a Tok> {
        self.t.get(self.i + k).map(|t| &t.tok)
    }

    fn is_p(&self, p: &str) -> bool {
        matches!(self.peek(), Some(Tok::P(q)) if *q == p)
    }

    fn is_p_at(&self, k: usize, p: &str) -> bool {
        matches!(self.peek_at(k), Some(Tok::P(q)) if *q == p)
    }

    fn is_word(&self, w: &str) -> bool {
        matches!(self.peek(), Some(Tok::Ident(s)) if s == w)
    }

    fn start(&self) -> usize {
        self.t.get(self.i).map(|t| t.start).unwrap_or(self.src.len())
    }

    fn last_end(&self) -> usize {
        if self.i == 0 { 0 } else { self.t[self.i - 1].end }
    }

    fn span(&self, start: usize) -> Span {
        Span { start, end: self.last_end() }
    }

    fn err_here(&self, what: Text) -> Error {
        match self.t.get(self.i) {
            Some(t) => {
                let shown = &self.src[t.start..t.end];
                self.lines.err(t.start, tr!("ここには{}が要ります（`{shown}` があります）", "unexpected `{shown}`: {} goes here", what.ja; what.en))
            }
            None => self.lines.err(self.src.len(), tr!("ファイルが途中で終わっています（ここには{}が要ります）", "the file ends too early: {} goes here", what.ja; what.en)),
        }
    }

    fn expect_p(&mut self, p: &str) -> R<()> {
        if self.is_p(p) {
            self.i += 1;
            Ok(())
        } else {
            Err(self.err_here(Text::new(format!(" `{p}` "), format!("`{p}`"))))
        }
    }

    fn enter(&mut self) -> R<()> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.lines.err(self.start(), tr!("入れ子が深すぎます（{MAX_DEPTH} 段まで）", "nested deeper than {MAX_DEPTH} levels")));
        }
        Ok(())
    }

    /// Any identifier, keywords included (the grammar's `AnyIdent`).
    fn any_ident(&mut self) -> R<Node<Ident>> {
        match self.peek() {
            Some(Tok::Ident(w)) => {
                let start = self.start();
                self.i += 1;
                Ok(Node { node: Ident::from_word(w), span: self.span(start) })
            }
            _ => Err(self.err_here(tr!("識別子", "an identifier"))),
        }
    }

    fn string(&mut self) -> R<Node<String>> {
        match self.peek() {
            Some(Tok::Str(s)) => {
                let start = self.start();
                self.i += 1;
                Ok(Node { node: s.clone(), span: self.span(start) })
            }
            _ => Err(self.err_here(tr!("文字列", "a string"))),
        }
    }

    fn policy(&mut self) -> R<Node<Policy>> {
        let start = self.start();
        let mut annotations = Vec::new();
        while self.is_p("@") {
            let a_start = self.start();
            self.i += 1;
            let key = self.any_ident()?;
            let value = if self.is_p("(") {
                self.i += 1;
                let v = self.string()?;
                self.expect_p(")")?;
                Some(v)
            } else {
                None
            };
            annotations.push(Node { node: Annotation { key, value }, span: self.span(a_start) });
        }
        let effect = self.any_ident()?;
        self.expect_p("(")?;
        let mut variables = Vec::new();
        while !self.is_p(")") {
            variables.push(self.variable_def()?);
            if self.is_p(",") {
                self.i += 1;
            } else {
                break;
            }
        }
        self.expect_p(")")?;
        let mut conds = Vec::new();
        while !self.is_p(";") {
            let c_start = self.start();
            let cond = self.any_ident()?;
            self.expect_p("{")?;
            let expr = if self.is_p("}") { None } else { Some(self.expr()?) };
            self.expect_p("}")?;
            conds.push(Node { node: Cond { cond, expr }, span: self.span(c_start) });
        }
        self.expect_p(";")?;
        Ok(Node { node: Policy { annotations, effect, variables, conds }, span: self.span(start) })
    }

    fn variable_def(&mut self) -> R<Node<VariableDef>> {
        let start = self.start();
        let variable = self.any_ident()?;
        let unused_type_name = if self.is_p(":") {
            self.i += 1;
            Some(self.name_only()?)
        } else {
            None
        };
        let entity_type = if self.is_word("is") {
            self.i += 1;
            Some(self.add()?)
        } else {
            None
        };
        let ineq = match self.rel_op() {
            Some(op) => {
                self.i += 1;
                Some((op, self.expr()?))
            }
            None => None,
        };
        Ok(Node { node: VariableDef { variable, unused_type_name, entity_type, ineq }, span: self.span(start) })
    }

    fn rel_op(&self) -> Option<RelOp> {
        match self.peek() {
            Some(Tok::P("<")) => Some(RelOp::Less),
            Some(Tok::P("<=")) => Some(RelOp::LessEq),
            Some(Tok::P(">=")) => Some(RelOp::GreaterEq),
            Some(Tok::P(">")) => Some(RelOp::Greater),
            Some(Tok::P("!=")) => Some(RelOp::NotEq),
            Some(Tok::P("==")) => Some(RelOp::Eq),
            Some(Tok::P("=")) => Some(RelOp::InvalidSingleEq),
            Some(Tok::Ident(w)) if w == "in" => Some(RelOp::In),
            _ => None,
        }
    }

    pub(crate) fn expr(&mut self) -> R<Node<Expr>> {
        self.enter()?;
        let start = self.start();
        let r = if self.is_word("if") && !self.is_p_at(1, "::") {
            self.i += 1;
            let c = self.expr()?;
            if !self.is_word("then") {
                return Err(self.err_here(Text::new(" `then` ", "`then`")));
            }
            self.i += 1;
            let t = self.expr()?;
            if !self.is_word("else") {
                return Err(self.err_here(Text::new(" `else` ", "`else`")));
            }
            self.i += 1;
            let e = self.expr()?;
            Expr::If(Box::new(c), Box::new(t), Box::new(e))
        } else {
            Expr::Or(Box::new(self.or()?))
        };
        self.depth -= 1;
        Ok(Node { node: r, span: self.span(start) })
    }

    fn or(&mut self) -> R<Node<Or>> {
        let start = self.start();
        let initial = self.and()?;
        let mut extended = Vec::new();
        while self.is_p("||") {
            self.i += 1;
            extended.push(self.and()?);
        }
        Ok(Node { node: Or { initial, extended }, span: self.span(start) })
    }

    fn and(&mut self) -> R<Node<And>> {
        let start = self.start();
        let initial = self.relation()?;
        let mut extended = Vec::new();
        while self.is_p("&&") {
            self.i += 1;
            extended.push(self.relation()?);
        }
        Ok(Node { node: And { initial, extended }, span: self.span(start) })
    }

    fn relation(&mut self) -> R<Node<Relation>> {
        let start = self.start();
        let first = self.add()?;
        if self.is_word("has") {
            self.i += 1;
            if self.is_word("if") && !self.is_p_at(1, "::") {
                // `has if` and the fields after it: the grammar builds the field from the
                // relation's whole span
                self.i += 1;
                let mut access = Vec::new();
                while self.is_p(".") || self.is_p("(") || self.is_p("[") {
                    access.push(self.mem_access()?);
                }
                let whole = self.span(start);
                let ident = Node { node: Ident::If, span: whole };
                let name = Node { node: Name { path: vec![], name: ident }, span: whole };
                let prim = Node { node: Primary::Name(name), span: whole };
                let member = Node { node: Member { item: prim, access }, span: whole };
                let unary = Node { node: Unary { op: None, item: member }, span: whole };
                let mult = Node { node: Mult { initial: unary, extended: vec![] }, span: whole };
                let field = Node { node: Add { initial: mult, extended: vec![] }, span: whole };
                return Ok(Node { node: Relation::Has { target: first, field }, span: whole });
            }
            let field = self.add()?;
            return Ok(Node { node: Relation::Has { target: first, field }, span: self.span(start) });
        }
        if self.is_word("like") {
            self.i += 1;
            let pattern = self.add()?;
            return Ok(Node { node: Relation::Like { target: first, pattern }, span: self.span(start) });
        }
        if self.is_word("is") {
            self.i += 1;
            let entity_type = self.add()?;
            let in_entity = if self.is_word("in") {
                self.i += 1;
                Some(self.add()?)
            } else {
                None
            };
            return Ok(Node { node: Relation::IsIn { target: first, entity_type, in_entity }, span: self.span(start) });
        }
        let mut extended = Vec::new();
        while let Some(op) = self.rel_op() {
            self.i += 1;
            extended.push((op, self.add()?));
        }
        Ok(Node { node: Relation::Common { initial: first, extended }, span: self.span(start) })
    }

    fn add(&mut self) -> R<Node<Add>> {
        let start = self.start();
        let initial = self.mult()?;
        let mut extended = Vec::new();
        loop {
            let op = if self.is_p("+") {
                AddOp::Plus
            } else if self.is_p("-") {
                AddOp::Minus
            } else {
                break;
            };
            self.i += 1;
            extended.push((op, self.mult()?));
        }
        Ok(Node { node: Add { initial, extended }, span: self.span(start) })
    }

    fn mult(&mut self) -> R<Node<Mult>> {
        let start = self.start();
        let initial = self.unary()?;
        let mut extended = Vec::new();
        loop {
            let op = if self.is_p("*") {
                MultOp::Times
            } else if self.is_p("/") {
                MultOp::Divide
            } else if self.is_p("%") {
                MultOp::Mod
            } else {
                break;
            };
            self.i += 1;
            extended.push((op, self.unary()?));
        }
        Ok(Node { node: Mult { initial, extended }, span: self.span(start) })
    }

    fn unary(&mut self) -> R<Node<Unary>> {
        let start = self.start();
        let mut op = None;
        for (c, bang) in [("!", true), ("-", false)] {
            let mut n = 0u32;
            while self.is_p(c) {
                self.i += 1;
                n += 1;
            }
            if n > 0 {
                op = Some(match (bang, n) {
                    (true, n) if n <= 4 => NegOp::Bang(n as u8),
                    (true, _) => NegOp::OverBang,
                    (false, n) if n <= 4 => NegOp::Dash(n as u8),
                    (false, _) => NegOp::OverDash,
                });
                break;
            }
        }
        let item = self.member()?;
        Ok(Node { node: Unary { op, item }, span: self.span(start) })
    }

    fn member(&mut self) -> R<Node<Member>> {
        let start = self.start();
        let item = self.primary()?;
        let mut access = Vec::new();
        while self.is_p(".") || self.is_p("(") || self.is_p("[") {
            access.push(self.mem_access()?);
        }
        Ok(Node { node: Member { item, access }, span: self.span(start) })
    }

    fn mem_access(&mut self) -> R<Node<MemAccess>> {
        let start = self.start();
        let a = if self.is_p(".") {
            self.i += 1;
            MemAccess::Field(self.any_ident()?)
        } else if self.is_p("(") {
            self.i += 1;
            let args = self.expr_list(")")?;
            MemAccess::Call(args)
        } else {
            self.expect_p("[")?;
            let e = self.expr()?;
            self.expect_p("]")?;
            MemAccess::Index(Box::new(e))
        };
        Ok(Node { node: a, span: self.span(start) })
    }

    /// `Comma<Expr>` and the closing bracket: a comma may end the list.
    fn expr_list(&mut self, close: &str) -> R<Vec<Node<Expr>>> {
        let mut out = Vec::new();
        while !self.is_p(close) {
            out.push(self.expr()?);
            if self.is_p(",") {
                self.i += 1;
            } else {
                break;
            }
        }
        self.expect_p(close)?;
        Ok(out)
    }

    fn primary(&mut self) -> R<Node<Primary>> {
        self.enter()?;
        let start = self.start();
        let p = match self.peek() {
            Some(Tok::Num(digits)) => {
                let n = match digits.parse::<u64>() {
                    Ok(n) => n,
                    Err(_) => {
                        return Err(self.lines.err(start, tr!("整数 `{digits}` が大きすぎます", "the integer `{digits}` is too large")));
                    }
                };
                self.i += 1;
                Primary::Literal(Node { node: Literal::Num(n), span: self.span(start) })
            }
            Some(Tok::Str(_)) => {
                let s = self.string()?;
                Primary::Literal(Node { node: Literal::Str(s), span: self.span(start) })
            }
            Some(Tok::Slot(s)) => {
                self.i += 1;
                let slot = match s.as_str() {
                    "?principal" => Slot::Principal,
                    "?resource" => Slot::Resource,
                    _ => Slot::Other(s.clone()),
                };
                Primary::Slot(Node { node: slot, span: self.span(start) })
            }
            Some(Tok::Ident(w)) if (w == "true" || w == "false") && !self.is_p_at(1, "::") => {
                self.i += 1;
                let lit = if w == "true" { Literal::True } else { Literal::False };
                Primary::Literal(Node { node: lit, span: self.span(start) })
            }
            Some(Tok::Ident(w)) => {
                if w == "if" && !self.is_p_at(1, "::") {
                    return Err(self.err_here(tr!("式", "an expression")));
                }
                self.name_or_ref()?
            }
            Some(Tok::P("(")) => {
                self.i += 1;
                let e = self.expr()?;
                self.expect_p(")")?;
                Primary::Expr(Box::new(e))
            }
            Some(Tok::P("[")) => {
                self.i += 1;
                Primary::EList(self.expr_list("]")?)
            }
            Some(Tok::P("{")) => {
                self.i += 1;
                let mut inits = Vec::new();
                while !self.is_p("}") {
                    inits.push(self.rec_init()?);
                    if self.is_p(",") {
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                self.expect_p("}")?;
                Primary::RInits(inits)
            }
            _ => return Err(self.err_here(tr!("式", "an expression"))),
        };
        self.depth -= 1;
        Ok(Node { node: p, span: self.span(start) })
    }

    fn rec_init(&mut self) -> R<Node<RecInit>> {
        let start = self.start();
        if self.is_word("if") && self.is_p_at(1, ":") {
            self.i += 1;
            self.expect_p(":")?;
            let value = self.expr()?;
            let whole = self.span(start);
            let ident = Node { node: Ident::If, span: whole };
            let name = Node { node: Name { path: vec![], name: ident }, span: whole };
            let prim = Node { node: Primary::Name(name), span: whole };
            let member = Node { node: Member { item: prim, access: vec![] }, span: whole };
            let unary = Node { node: Unary { op: None, item: member }, span: whole };
            let mult = Node { node: Mult { initial: unary, extended: vec![] }, span: whole };
            let add = Node { node: Add { initial: mult, extended: vec![] }, span: whole };
            let rel = Node { node: Relation::Common { initial: add, extended: vec![] }, span: whole };
            let and = Node { node: And { initial: rel, extended: vec![] }, span: whole };
            let or = Node { node: Or { initial: and, extended: vec![] }, span: whole };
            let key = Node { node: Expr::Or(Box::new(or)), span: whole };
            return Ok(Node { node: RecInit(key, value), span: whole });
        }
        let key = self.expr()?;
        self.expect_p(":")?;
        let value = self.expr()?;
        Ok(Node { node: RecInit(key, value), span: self.span(start) })
    }

    /// A name standing alone (`Name`): one identifier that is not `if`, `true` or `false`, or
    /// identifiers joined by `::`.
    fn name_only(&mut self) -> R<Node<Name>> {
        let start = self.start();
        let first = self.any_ident()?;
        let mut path = Vec::new();
        let mut last = first;
        while self.is_p("::") && matches!(self.peek_at(1), Some(Tok::Ident(_))) {
            self.i += 1;
            path.push(last);
            last = self.any_ident()?;
        }
        if path.is_empty() && matches!(last.node, Ident::If | Ident::True | Ident::False) {
            self.i -= 1;
            return Err(self.err_here(tr!("名前", "a name")));
        }
        Ok(Node { node: Name { path, name: last }, span: self.span(start) })
    }

    /// A name, or an entity's reference (`Name :: "id"`, `Name :: { … }`).
    fn name_or_ref(&mut self) -> R<Primary> {
        let start = self.start();
        let name = self.name_only()?;
        if self.is_p("::") {
            match self.peek_at(1) {
                Some(Tok::Str(_)) => {
                    self.i += 1;
                    let eid = self.string()?;
                    return Ok(Primary::Ref(Node { node: Ref::Uid { path: name, eid }, span: self.span(start) }));
                }
                Some(Tok::P("{")) => {
                    self.i += 2;
                    let mut rinits = Vec::new();
                    while !self.is_p("}") {
                        let r_start = self.start();
                        let k = self.any_ident()?;
                        self.expect_p(":")?;
                        let v = self.literal()?;
                        rinits.push(Node { node: RefInit(k, v), span: self.span(r_start) });
                        if self.is_p(",") {
                            self.i += 1;
                        } else {
                            break;
                        }
                    }
                    self.expect_p("}")?;
                    return Ok(Primary::Ref(Node { node: Ref::Ref { path: name, rinits }, span: self.span(start) }));
                }
                _ => {
                    self.i += 1;
                    return Err(self.err_here(tr!("識別子か文字列", "an identifier or a string")));
                }
            }
        }
        Ok(Primary::Name(name))
    }

    fn literal(&mut self) -> R<Node<Literal>> {
        let start = self.start();
        let lit = match self.peek() {
            Some(Tok::Num(d)) => match d.parse::<u64>() {
                Ok(n) => {
                    self.i += 1;
                    Literal::Num(n)
                }
                Err(_) => return Err(self.lines.err(start, tr!("整数 `{d}` が大きすぎます", "the integer `{d}` is too large"))),
            },
            Some(Tok::Str(_)) => Literal::Str(self.string()?),
            Some(Tok::Ident(w)) if w == "true" => {
                self.i += 1;
                Literal::True
            }
            Some(Tok::Ident(w)) if w == "false" => {
                self.i += 1;
                Literal::False
            }
            _ => return Err(self.err_here(tr!("リテラル", "a literal"))),
        };
        Ok(Node { node: lit, span: self.span(start) })
    }
}
