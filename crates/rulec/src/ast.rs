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

impl CmpOp {
    /// How it is written, in the source and in everything that quotes the source back.
    pub fn word(self) -> &'static str {
        match self {
            CmpOp::Le => "<=",
            CmpOp::Ge => ">=",
            CmpOp::Lt => "<",
            CmpOp::Gt => ">",
        }
    }
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

/// Which kind of file an imported enum's values are declared in (§15.59, §15.60).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnumSource {
    /// `.proto`: the enum is named, and the value names carry its name as a prefix.
    Proto,
    /// JSON Schema, OpenAPI included: the enum is reached by a JSON Pointer, and the values
    /// are the strings that go on the wire.
    JsonSchema,
}

impl EnumSource {
    /// The word written after `import`.
    pub fn word(self) -> &'static str {
        match self {
            EnumSource::Proto => crate::kw::PROTO,
            EnumSource::JsonSchema => crate::kw::JSONSCHEMA,
        }
    }
}

/// `import <kind> "<file>" <selector> -> <enum of this rule>` (§15.59, §15.60). The rule keeps
/// the words; the file keeps the set, and the check holds them together.
#[derive(Debug, Clone)]
pub struct EnumImport {
    pub kind: EnumSource,
    /// The path as written, followed from the directory of the `.rule`.
    pub file: String,
    /// What names the enum inside that file: its name in a `.proto`, a JSON Pointer in a
    /// schema.
    pub source: String,
    /// The enum declared here that it has to agree with.
    pub target: Name,
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

impl Expr {
    /// Where the expression was written. Every variant carries one, so a caller that needs
    /// the source line does not have to match on the shape.
    pub fn span(&self) -> &Span {
        match self {
            Expr::Name(_, s) | Expr::Lit(_, s) | Expr::Bin(_, _, _, s) | Expr::Call(_, _, s) => s,
        }
    }
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
    pub cite: Option<Cite>,
}

#[derive(Debug, Clone)]
pub struct DefineDecl {
    pub name: Name,
    pub ty: TypeRef,
    pub expr: Expr,
    pub span: Span,
    pub cite: Option<Cite>,
}

/// `@<source> <fragment>, …` at the end of a table, clause, derive or define line, or of a
/// row: which part of which document the definition transcribes (§15.68). The
/// fragment is named as the document names it (`第91条`, `第20条第2項`, `別表第一`).
#[derive(Debug, Clone)]
pub struct Cite {
    pub source: String,
    pub fragments: Vec<String>,
    pub span: Span,
}

/// What kind of document a `source` is.
#[derive(Debug, Clone)]
pub enum SourceKind {
    /// A law in a statute database, by the id that database gives it, read as of a date. Its
    /// fragments can be fetched one by one, so each cited one is pinned on its own.
    Law { db: LawDb, id: String, asof: String },
    /// A file beside the rule that has no addressable fragments; pinned whole. `url` is where
    /// the copy came from, when there is such a place: it lets `source fetch` bring it again
    /// and `source outdated` ask whether it has moved on (§15.76). A document handed over by a
    /// person has none, so it stays optional.
    File { path: String, url: Option<String>, hash: Option<String> },
}

/// Which statute database a `law` source reads from.
///
/// The word is written after `law` (`law ecfr "29 CFR 1910" asof 2026-01-01`) and left out
/// for e-Gov, which is where every rule written before there was a choice reads its laws.
/// What differs between them is the shape of the id, the shape of a fragment, where a copy
/// is fetched from and how `outdated` asks whether the text has moved on — not what a copy
/// is or what it is held to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LawDb {
    /// e-Gov, the Japanese government's statute database.
    Egov,
    /// The Electronic Code of Federal Regulations: the US federal regulations as in force on
    /// a date, a section at a time.
    Ecfr,
}

impl LawDb {
    /// The word as it is written after `law`, or `None` for anything else — which is how the
    /// parser tells a database word from the id that may stand in its place.
    pub fn parse(w: &str) -> Option<Self> {
        match w {
            "egov" => Some(Self::Egov),
            "ecfr" => Some(Self::Ecfr),
            _ => None,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Self::Egov => "egov",
            Self::Ecfr => "ecfr",
        }
    }
}

/// One pinned fragment under a `source … = law` line: `  第91条 sha256:77aa00bb11cc22dd`.
#[derive(Debug, Clone)]
pub struct Pin {
    pub fragment: String,
    pub hash: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct SourceDecl {
    pub name: Name,
    pub kind: SourceKind,
    pub pins: Vec<Pin>,
    pub span: Span,
    /// The rule file the declaration was written in, when it came in through an `apply`:
    /// its copies sit beside that file, and its pins were checked there (§15.69).
    pub base: Option<String>,
}

/// What a callee input is bound to: a name of this rule, or a literal.
#[derive(Debug, Clone)]
pub enum BindValue {
    Name(String),
    Lit(Lit),
}

/// `<callee input> = <value> [with <value> -> <value>, …]` under an `apply`.
#[derive(Debug, Clone)]
pub struct Binding {
    pub input: String,
    pub value: BindValue,
    /// For an enum: which value of this rule's enum stands for which value of the callee's.
    /// A value spelled the same on both sides needs no entry.
    pub map: Vec<(String, String)>,
    pub span: Span,
}

/// `<callee output> -> <name>` under an `apply`: what the callee's answer is called here.
#[derive(Debug, Clone)]
pub struct OutBinding {
    pub output: String,
    pub name: Name,
    pub span: Span,
}

/// `apply <name> = "<path>" sha256:…` with its bindings (§15.69). After
/// expansion the callee's definitions sit in `items` from `at` on, `count` of them, named
/// `<name>:<their name>`.
#[derive(Debug, Clone)]
pub struct ApplyDecl {
    pub name: Name,
    pub path: String,
    pub hash: Option<String>,
    pub bindings: Vec<Binding>,
    pub excepts: Vec<(String, Span)>,
    pub outputs: Vec<OutBinding>,
    pub cite: Option<Cite>,
    pub span: Span,
    /// Where in `items` the expansion goes: the index the next item had when the `apply`
    /// was read.
    pub at: usize,
    /// How many items the expansion put there.
    pub count: usize,
    /// The names of the definitions the expansion added for the callee's outputs.
    pub defines: Vec<String>,
    /// The callee as read, and where: what the page renders the applied tables from.
    pub callee_src: String,
    pub callee_path: String,
    /// What the expansion learned about the callee, for the checks that need this rule's
    /// types: its inputs with their types and ranges, its constraints, its outputs.
    pub callee: Option<Callee>,
}

/// The callee's contract as its own check established it (§15.69).
#[derive(Debug, Clone)]
pub struct Callee {
    pub rule: Name,
    pub inputs: Vec<CalleeInput>,
    pub constraints: Vec<Constraint>,
    /// Enum name → its values, for the mapping of a bound enum input.
    pub enums: Vec<(String, Vec<String>)>,
}

/// One input of the callee, with its type resolved by the callee's own check.
#[derive(Debug, Clone)]
pub struct CalleeInput {
    pub name: String,
    pub ty: crate::types::Ty,
    pub range: Option<Range>,
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
    /// `starts_with "ABC"` or `starts_with "ABC", "XY"` — the one test a `string` column
    /// takes (§15.101). A finite set of prefixes cuts the strings into finitely many
    /// classes, which is what §6.2's compression needs.
    Prefix(Vec<String>),
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
    /// 1-based, as printed in diagnostics ("行3" / "row 3"). The position the row was written
    /// at, which is what the trace reports whatever order the rows are evaluated in.
    pub index: usize,
    /// The row's own citation, when it was taken from somewhere other than its table.
    pub cite: Option<Cite>,
    /// The label written before the first `|` (`r1 | … |`). It names the row where an
    /// `overrides` line, a trace, or a later version needs to.
    pub label: Option<Name>,
    /// The table this row was written in, when the row sits in a merged definition set (a
    /// table built from several tables that define the same output). `None` as parsed.
    pub origin: Option<String>,
}

/// One target of an `overrides` line: a table (all of its rows) or one labelled row of it
/// (`表:ラベル`).
#[derive(Debug, Clone)]
pub struct OverrideRef {
    pub table: String,
    pub row: Option<String>,
    pub span: Span,
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
    /// The definitions every row of this table takes precedence over (the `overrides` line
    /// after `policy`). Each target is declared above this table.
    pub overrides: Vec<OverrideRef>,
    /// Written as a `clause`: one row, its columns those the `when` line names, its output
    /// the `then` value. Everything downstream treats it as the one-row table it is; only the
    /// page and the wording of a diagnostic call it a clause.
    pub clause: bool,
    /// The citation on the `table` or `clause` line.
    pub cite: Option<Cite>,
    /// The `apply` this table came in through, when it did. Its rows are alive in the
    /// callee's own file, so the ones this rule never reaches are listed, not E102.
    pub applied: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Item {
    Derived(DerivedDecl),
    Define(DefineDecl),
    Table(Table),
    Agg(AggDecl),
}

/// Which summary of the walk this is (§15.58, §15.100).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggKind {
    /// `count 一致数(hits) over 納入先 where 判定 = 一致` — how many elements pass one test.
    Count,
    /// `sum 合計(total) over 明細 of 金額` — the total of one column over the elements.
    Sum,
}

/// `count 一致数(hits) over 納入先 where 判定 = 一致  range >=0 <=100`, or
/// `sum 合計(total) over 明細 of 金額  range >=0円 <=1000000円` — one summary of the
/// sequence the rule walks (§15.58, §15.100).
///
/// A summary is the walk's account of itself rather than its answer: it is an ordinary
/// value from then on, so the table that turns it into a class is checked like any other
/// table. The declared range is what the completeness proof quantifies over, and it is
/// also what the entry guard holds the walk to — a sequence that leaves it is refused at
/// the door, the way a number outside its range is.
#[derive(Debug, Clone)]
pub struct AggDecl {
    pub kind: AggKind,
    pub name: Name,
    /// The sequence it runs over.
    pub over: String,
    /// The column of one element it reads: a field, or the output of a per-element table.
    pub column: Name,
    /// `count` only: the value that column must take. `None` is `true`, which is why a
    /// bool field needs no `= <value>`.
    pub value: Option<Name>,
    pub range: Option<Range>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ResultDecl {
    pub name: String,
    pub expr: Expr,
    pub span: Span,
}

/// `elements 運賃行(fee_rows)` — the fields of one element of the sequence the rule walks.
///
/// An element is a row of inputs, so the fields are declared exactly like `inputs` and a
/// table may use them as columns. The sequence itself is what the caller passes; how many
/// there are is decided at run time, and nothing in the checks depends on that number
/// (§15.56).
#[derive(Debug, Clone)]
pub struct ElementsDecl {
    pub name: Name,
    pub fields: Vec<VarDecl>,
    pub span: Span,
}

/// What one verdict does to the walk (§15.56).
#[derive(Debug, Clone)]
pub enum Arm {
    /// Leave this element and look at the next.
    Next,
    /// End the walk. With an expression, that is the answer; without one, the answer is
    /// whatever `exhausted` says.
    Stop(Option<Expr>),
    /// Take this element's value. `unique` makes a second taking element an error at run
    /// time; `first` keeps the first and ignores the rest.
    Take { expr: Expr, unique: bool },
    /// Hold this element's value, replacing what is held when the key is larger.
    KeepMax { expr: Expr, key: Expr },
}

/// `fold 採用 over 運賃行` — how a column of verdicts becomes one answer.
///
/// The table decides one element at a time and its verdict column is a **finite** enum, so
/// the walk is a reduction of a string over a finite alphabet: a small automaton, which is
/// why adding it does not cost the checks their decidability (§15.56).
#[derive(Debug, Clone)]
pub struct FoldDecl {
    /// The table output column the walk reads, one verdict per element.
    pub verdict: String,
    /// The sequence it walks.
    pub over: String,
    /// One arm per value of the verdict's enum, in source order.
    pub arms: Vec<(Name, Arm, Span)>,
    /// The answer when there are no elements at all. Declaring it is not optional.
    pub empty: Option<Expr>,
    /// The answer when the walk reached the end. Declaring it is not optional.
    pub exhausted: Option<Expr>,
    pub span: Span,
}

/// `constraint 全条件一致数 <= 会社名一致数` — a relation between two inputs that the caller
/// guarantees. Nothing computes with it: it says which combinations exist, so the checks do
/// not demand rows for the ones that do not, and the entry guard refuses them (§15.55).
#[derive(Debug, Clone)]
pub struct Constraint {
    pub left: String,
    pub op: CmpOp,
    pub right: String,
    pub span: Span,
}

/// One named list of elements (§15.56). The columns are the fields of `elements` and every
/// cell is a literal: this is a value, not a pattern, so nothing here narrows a range.
#[derive(Debug, Clone)]
pub struct SeqDecl {
    pub name: Name,
    pub cols: Vec<(String, Span)>,
    pub rows: Vec<Row>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct RuleFile {
    pub name: Name,
    pub version: String,
    pub description: Option<String>,
    pub imports: Vec<(String, Span)>,
    /// The enums whose value set is declared outside this file (§15.59, §15.60).
    pub enum_imports: Vec<EnumImport>,
    /// The documents the rule transcribes, with the pinned digests of what it cites (§15.68).
    pub sources: Vec<SourceDecl>,
    /// The rules applied with their inputs bound (§15.69), in the order written.
    pub applies: Vec<ApplyDecl>,
    pub enums: Vec<EnumDecl>,
    pub groups: Vec<GroupDecl>,
    pub inputs: Vec<VarDecl>,
    pub outputs: Vec<OutDecl>,
    /// derive / define / table in source order — §5.1's define-before-use pipeline.
    pub items: Vec<Item>,
    pub result: Option<ResultDecl>,
    /// `examples` — an executable specification (§1.2), shaped like a table.
    pub examples: Option<Table>,
    /// The relations between inputs that always hold (§15.55).
    pub constraints: Vec<Constraint>,
    /// The fields of one element of the sequence, when the rule walks one (§15.56).
    pub elements: Option<ElementsDecl>,
    /// Named lists of elements. A cell of `examples` names one, which is how a case for a
    /// walk is written: a row of cells has no room for a sequence (§15.56).
    pub sequences: Vec<SeqDecl>,
    /// How the verdicts of the per-element table reduce to one answer (§15.56).
    pub fold: Option<FoldDecl>,
}
