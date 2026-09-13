//! Parse tree. Shapes follow §1.2's fixed file structure.

use crate::diag::Span;
use crate::lex::Num;

#[derive(Debug, Clone)]
pub struct Name {
    pub text: String,
    /// §1.3: required on the public face (rule name, inputs, outputs and their types),
    /// optional elsewhere. The parser only records; E011 is a later check.
    pub ascii: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum CmpOp {
    Le,
    Ge,
    Lt,
    Gt,
}

#[derive(Debug, Clone)]
pub enum Lit {
    Num(Num),
    Word(String),
    Date(i32, u32, u32),
    Str(String),
}

#[derive(Debug, Clone)]
pub enum TypeArg {
    Word(String),
    /// `rate[step 0.1%]` — a word followed by a number.
    Scaled(String, Num),
}

#[derive(Debug, Clone)]
pub struct TypeRef {
    pub base: String,
    pub args: Vec<TypeArg>,
    /// The `?` in `区分?`. Only a `none` cell can consume it (§2.1).
    pub optional: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Range {
    /// A conjunction, e.g. `range >=1g <=40kg`.
    pub bounds: Vec<(CmpOp, Lit)>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Rounding {
    pub mode: String,
    pub grid: Num,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: Name,
    pub values: Vec<Name>,
    /// The per-value `default` mark (§11 W111). Silences a value that intentionally falls
    /// through to `-`.
    pub default_marks: Vec<bool>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct GroupDecl {
    pub name: Name,
    pub members: Vec<Name>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub name: Name,
    pub ty: TypeRef,
    pub range: Option<Range>,
    /// §11 W111: silences the unused-declaration warning for a range-guard-only input.
    pub contract_only: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct OutDecl {
    pub name: Name,
    pub ty: TypeRef,
    pub rounding: Option<Rounding>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Name(String, Span),
    Lit(Lit, Span),
    Bin(Box<Expr>, BinOp, Box<Expr>, Span),
    Call(String, Vec<Expr>, Span),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Le,
    Ge,
    Lt,
    Gt,
    Eq,
}

#[derive(Debug, Clone)]
pub struct DerivedDecl {
    pub name: Name,
    pub ty: TypeRef,
    pub expr: Expr,
    pub range: Option<Range>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct DefineDecl {
    pub name: Name,
    pub ty: TypeRef,
    pub expr: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    /// `policy unique` — DMN Unique. The default (§4).
    Unique,
    /// `policy first` — DMN First.
    TopDown,
}

#[derive(Debug, Clone)]
pub enum Cell {
    /// `-`
    DontCare,
    Lit(Lit),
    /// `北海道, 沖縄`
    Set(Vec<Lit>),
    /// `not: 北海道, 沖縄`
    Not(Vec<Lit>),
    /// `<=2000g` or `>=1000円 <20000円`
    Cmp(Vec<(CmpOp, Lit)>),
    /// `none`
    Nothing,
}

#[derive(Debug, Clone)]
pub enum OutCell {
    Lit(Lit),
    /// §3.2: a name (input, derived, define). Expressions are not allowed.
    Name(String),
}

#[derive(Debug, Clone)]
pub struct Row {
    pub cells: Vec<Cell>,
    /// Per-cell positions, kept apart from the row's span so that §11 principle 4 ("point at
    /// the column") can be met.
    pub cell_spans: Vec<Span>,
    pub outs: Vec<OutCell>,
    pub out_spans: Vec<Span>,
    pub span: Span,
    /// 1-based, as printed in diagnostics ("行3" / "row 3").
    pub index: usize,
}

#[derive(Debug, Clone)]
pub struct OutCol {
    pub name: Name,
    pub ty: Option<TypeRef>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Table {
    pub name: Option<Name>,
    pub policy: Policy,
    /// Column headers left of `→`; each names an input, derived, or boolean/enum intermediate.
    pub inputs: Vec<(String, Span)>,
    pub outputs: Vec<OutCol>,
    pub rows: Vec<Row>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Item {
    Derived(DerivedDecl),
    Define(DefineDecl),
    Table(Table),
}

#[derive(Debug, Clone)]
pub struct ResultDecl {
    pub name: String,
    pub expr: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct RuleFile {
    pub name: Name,
    pub version: String,
    pub description: Option<String>,
    pub imports: Vec<(String, Span)>,
    pub enums: Vec<EnumDecl>,
    pub groups: Vec<GroupDecl>,
    pub inputs: Vec<VarDecl>,
    pub outputs: Vec<OutDecl>,
    /// derive / define / table in source order — §5.1's define-before-use pipeline.
    pub items: Vec<Item>,
    pub result: Option<ResultDecl>,
    /// `examples` — an executable specification (§1.2), shaped like a table.
    pub examples: Option<Table>,
}
