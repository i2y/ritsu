//! What a `.kn` says, as the parser read it (DESIGN 2). Nothing here is resolved: a name is the word
//! as written, a type is the word or the unit as written, a literal is the number with the unit
//! written after it. `names.rs` reads the files the module uses and says what each name names;
//! `types.rs` gives every expression its type.
//!
//! Every expression has an id, from 0 in the order the parser made them, unique in its module:
//! what the checks find of an expression (its type, what a name in it names) is kept by the id, for
//! the stages that come after the check (the reference interpreter, the analysis of ranges, the
//! generated code).

use crate::lex::Num;

/// Where something is written: the line, and the columns it starts at and ends before (1-based,
/// counting characters). A span that runs over lines ends on its first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub line: usize,
    pub col: usize,
    pub end: usize,
}

impl Span {
    /// From the start of `self` to the end of `other`, when `other` ends on the same line.
    pub fn to(self, other: Span) -> Span {
        if other.line == self.line { Span { end: other.end.max(self.end), ..self } } else { self }
    }
}

/// A declared name and the ASCII alias written after it: `請求日(invoiced)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name {
    pub text: String,
    pub alias: Option<(String, Span)>,
    pub span: Span,
}

impl Name {
    /// Whether a word names this: by the name, or by the alias.
    pub fn is(&self, word: &str) -> bool {
        self.text == word || self.alias.as_ref().is_some_and(|(a, _)| a == word)
    }

    /// What the generated code calls it: the alias, or the name when none is written (which, in a
    /// module that passes its check, is ASCII).
    pub fn ascii(&self) -> &str {
        self.alias.as_ref().map(|(a, _)| a.as_str()).unwrap_or(&self.text)
    }
}

/// A `.kn` file: one module (DESIGN 2.1).
#[derive(Clone, Debug)]
pub struct Module {
    /// `module late_payment v1`, `module 支払いの遅れ(late_payment_ja) v1`.
    pub name: Name,
    /// `v1`, as written: a mark for people.
    pub version: String,
    pub description: Option<(String, Span)>,
    pub uses: Vec<Use>,
    pub enums: Vec<EnumDecl>,
    pub records: Vec<RecordDecl>,
    pub fns: Vec<FnDecl>,
    pub examples: Vec<Example>,
    /// How many expressions the parser made: the ids are below it.
    pub exprs: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UseKind {
    Rule,
    Dates,
    Book,
    Module,
}

impl UseKind {
    pub fn word(self) -> &'static str {
        match self {
            UseKind::Rule => "rule",
            UseKind::Dates => "dates",
            UseKind::Book => "book",
            UseKind::Module => "module",
        }
    }

    /// The extension of the files a `use` of this kind reads.
    pub fn extension(self) -> &'static str {
        match self {
            UseKind::Rule => "rule",
            UseKind::Dates => "cal",
            UseKind::Book => "book",
            UseKind::Module => "kn",
        }
    }
}

/// `use rule charges from "rules/late_payment_charges.rule"`.
#[derive(Clone, Debug)]
pub struct Use {
    pub kind: UseKind,
    pub name: String,
    pub name_span: Span,
    /// As written: from the module's directory, `/` between the parts.
    pub path: String,
    pub path_span: Span,
    pub line: usize,
}

/// `enum tier = basic < silver < gold`.
#[derive(Clone, Debug)]
pub struct EnumDecl {
    pub name: Name,
    /// Written with `<`: its values are in order.
    pub ordered: bool,
    pub values: Vec<EnumValueDecl>,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct EnumValueDecl {
    pub name: Name,
    /// The display name, `gold "Gold"`.
    pub label: Option<String>,
}

/// `record Invoice` and its fields.
#[derive(Clone, Debug)]
pub struct RecordDecl {
    pub name: Name,
    pub fields: Vec<FieldDecl>,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct FieldDecl {
    pub name: Name,
    pub ty: TypeRef,
    pub bounds: Bounds,
    pub line: usize,
}

/// A type as written (DESIGN 3.1).
#[derive(Clone, Debug, PartialEq)]
pub struct TypeRef {
    pub kind: TypeKind,
    /// `T?`
    pub optional: bool,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TypeKind {
    Number,
    Bool,
    Str,
    Date,
    /// `rate`, `rate[step 0.01%]`.
    Rate(Option<Num>),
    /// `money[GBP]`, `money[USD, incl_tax]`, `mass[oz]`: the word, the unit and the tax as written.
    Unit {
        dim: String,
        unit: String,
        tax: Option<String>,
        unit_span: Span,
    },
    /// `list[Line]`
    List(Box<TypeRef>),
    /// `running[Line, money[USDc]]`
    Running(Box<TypeRef>, Box<TypeRef>),
    /// `Invoice`, `charges.band`, `ledger.capture.do`, `pricing.Quote`.
    Named(Vec<(String, Span)>),
}

/// `range >=1 <=1500` and `count >=1 <=30`, as written after a type.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bounds {
    pub range: Option<(Vec<Bound>, Span)>,
    pub count: Option<(Vec<Bound>, Span)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmp {
    Lt,
    Le,
    Gt,
    Ge,
}

impl Cmp {
    pub fn shown(self) -> &'static str {
        match self {
            Cmp::Lt => "<",
            Cmp::Le => "<=",
            Cmp::Gt => ">",
            Cmp::Ge => ">=",
        }
    }
}

/// One end of a range: `>=1GBPc`, `<=2027-12-31`.
#[derive(Clone, Debug, PartialEq)]
pub struct Bound {
    pub op: Cmp,
    pub value: BoundValue,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BoundValue {
    /// A number with its unit, and whether a minus is written before it.
    Num(Num, bool),
    /// A day number.
    Date(i64),
}

/// The effect written at the end of a function's head (DESIGN 4).
#[derive(Clone, Debug, PartialEq)]
pub struct Effect {
    pub total: bool,
    pub fails: Vec<(FailKind, Span)>,
    pub diverges: bool,
    pub span: Span,
}

/// A kind of failure (DESIGN 4.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FailKind {
    Rule,
    Date,
    Range,
    Overflow,
    Zero,
}

impl FailKind {
    pub fn word(self) -> &'static str {
        match self {
            FailKind::Rule => "rule",
            FailKind::Date => "date",
            FailKind::Range => "range",
            FailKind::Overflow => "overflow",
            FailKind::Zero => "zero",
        }
    }

    pub fn parse(w: &str) -> Option<FailKind> {
        match w {
            "rule" => Some(FailKind::Rule),
            "date" => Some(FailKind::Date),
            "range" => Some(FailKind::Range),
            "overflow" => Some(FailKind::Overflow),
            "zero" => Some(FailKind::Zero),
            _ => None,
        }
    }
}

/// `fn claim(invoice: Invoice, …) -> Claim? total` and its body.
#[derive(Clone, Debug)]
pub struct FnDecl {
    pub name: Name,
    pub params: Vec<Param>,
    pub ret: TypeRef,
    pub ret_bounds: Bounds,
    pub effect: Option<Effect>,
    pub description: Option<(String, Span)>,
    pub body: Block,
    pub line: usize,
    /// The last line of its body, from 1.
    pub last: usize,
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: Name,
    pub ty: TypeRef,
    pub bounds: Bounds,
}

/// The lines `let` names, then the expression that answers (DESIGN 2.3, `block`).
#[derive(Clone, Debug)]
pub struct Block {
    pub lets: Vec<Let>,
    pub answer: Expr,
}

/// `let late = paid_on - due`, `let x : money[GBP] = …`.
#[derive(Clone, Debug)]
pub struct Let {
    pub name: String,
    pub span: Span,
    pub ty: Option<TypeRef>,
    pub value: Expr,
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub id: usize,
    pub span: Span,
    pub kind: ExprKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

impl BinOp {
    pub fn shown(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Eq => "=",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::And => "and",
            BinOp::Or => "or",
        }
    }

    pub fn is_comparison(self) -> bool {
        matches!(self, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge)
    }
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    /// `1200JPY`, `3%`, `2`: the magnitude and the unit as written.
    Num(Num),
    /// A day number.
    Date(i64),
    Str(String),
    Bool(bool),
    None,
    /// A word: a local, a parameter, an enum's value.
    Var(String),
    /// `invoice.amount`, `charges.band`: what is before the dot, and the word after it.
    Field(Box<Expr>, String, Span),
    /// `charges(debt: …)`, `terms.due(invoiced: …)`, `sum(xs, x => …)`: what is called, as written
    /// (a word, or words with dots between them), and the arguments.
    Call(Box<Expr>, Vec<Arg>),
    /// `Claim { due: …, … }`, `shipping.lines { … }`: the type's words and the fields.
    Record(Vec<(String, Span)>, Vec<FieldInit>),
    /// `[a, b]`
    List(Vec<Expr>),
    /// `-x`
    Neg(Box<Expr>),
    /// `not x`
    Not(Box<Expr>),
    Bin(Box<Expr>, BinOp, Box<Expr>),
    /// `fee in USDc`: the value, and the unit as written.
    In(Box<Expr>, String, Span),
    /// `if … then … else …` on one line, or the block form with `else if`.
    If(Box<Expr>, Box<Block>, Box<Block>),
    Match(Box<Expr>, Vec<Arm>),
    /// `x => …`, `(acc, x) => …`: only as an argument of a builtin (DESIGN 3.7).
    Lambda(Vec<(String, Span)>, Box<Expr>),
    /// The block indented under `let x =`, when it names values of its own.
    Block(Box<Block>),
}

#[derive(Clone, Debug)]
pub struct FieldInit {
    pub name: String,
    pub span: Span,
    pub value: Expr,
}

#[derive(Clone, Debug)]
pub enum Arg {
    /// `debt: invoice.amount`
    Named(String, Span, Expr),
    Pos(Expr),
}

impl Arg {
    pub fn expr(&self) -> &Expr {
        match self {
            Arg::Named(_, _, e) | Arg::Pos(e) => e,
        }
    }
}

/// One arm of a `match`: its patterns, and the block it answers with.
#[derive(Clone, Debug)]
pub struct Arm {
    pub patterns: Vec<Pattern>,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Pattern {
    None(Span),
    /// `some x`: the name the value is given in the arm.
    Some(String, Span),
    Bool(bool, Span),
    /// A value of an enum, by its name or its alias: `gold`, `tier.gold`.
    Value(Vec<(String, Span)>, Span),
}

impl Pattern {
    pub fn span(&self) -> Span {
        match self {
            Pattern::None(s) | Pattern::Some(_, s) | Pattern::Bool(_, s) | Pattern::Value(_, s) => *s,
        }
    }
}

/// One line of `examples`: a call, and the answer it gives or the failure it ends in.
#[derive(Clone, Debug)]
pub struct Example {
    pub call: Expr,
    pub expect: Expect,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub enum Expect {
    Value(Expr),
    Fails(FailKind, Span),
}

impl Expr {
    /// The words of a name with dots between them (`ledger.capture.do`), when the expression is
    /// one; None for anything else.
    pub fn path(&self) -> Option<Vec<(String, Span)>> {
        match &self.kind {
            ExprKind::Var(w) => Some(vec![(w.clone(), self.span)]),
            ExprKind::Field(base, w, sp) => {
                let mut p = base.path()?;
                p.push((w.clone(), *sp));
                Some(p)
            }
            _ => None,
        }
    }

    /// Every expression under this one, this one first, in the order written.
    pub fn walk<'a>(&'a self, out: &mut Vec<&'a Expr>) {
        out.push(self);
        match &self.kind {
            ExprKind::Num(_) | ExprKind::Date(_) | ExprKind::Str(_) | ExprKind::Bool(_) | ExprKind::None | ExprKind::Var(_) => {}
            ExprKind::Field(b, _, _) | ExprKind::Neg(b) | ExprKind::Not(b) | ExprKind::In(b, _, _) | ExprKind::Lambda(_, b) => b.walk(out),
            ExprKind::Call(c, args) => {
                c.walk(out);
                for a in args {
                    a.expr().walk(out);
                }
            }
            ExprKind::Record(_, fs) => {
                for f in fs {
                    f.value.walk(out);
                }
            }
            ExprKind::List(xs) => {
                for x in xs {
                    x.walk(out);
                }
            }
            ExprKind::Bin(a, _, b) => {
                a.walk(out);
                b.walk(out);
            }
            ExprKind::If(c, t, e) => {
                c.walk(out);
                t.walk(out);
                e.walk(out);
            }
            ExprKind::Match(s, arms) => {
                s.walk(out);
                for a in arms {
                    a.body.walk(out);
                }
            }
            ExprKind::Block(b) => b.walk(out),
        }
    }
}

impl Block {
    pub fn walk<'a>(&'a self, out: &mut Vec<&'a Expr>) {
        for l in &self.lets {
            l.value.walk(out);
        }
        self.answer.walk(out);
    }
}
