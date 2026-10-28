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
    /// `range from koyomi "<file>" date <name>`: the days a date of a koyomi file comes to over
    /// the whole range of its inputs, in place of bounds (ritsu's DESIGN 7.5, §15.174). The rule
    /// keeps the naming; koyomi keeps the set, and the check reads it through ritsu's port.
    pub days: Option<DaysFrom>,
    pub span: Span,
}

/// Where the days of `range from koyomi "<file>" date <name>` come from: the file as written
/// (from the directory of the `.rule`), and the date of it.
#[derive(Debug, Clone)]
pub struct DaysFrom {
    pub file: String,
    pub date: String,
    pub span: Span,
    /// The file the line was written in, when it is not the one being checked: the rule a
    /// contribution's check is about, or the contribution an input of a merged rule came from
    /// (§15.235). The path is followed from its directory.
    pub base: Option<String>,
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
    /// The display name written after a value (`persian_immortal "Persian Immortal"`), per
    /// value, with where it is written (§15.211). It is for people and the UI: no check, no
    /// wire and no decision of the generated code reads it.
    pub labels: Vec<Option<(String, Span)>>,
    /// The values are written with `<` between them, so they are ordered as written
    /// (§15.210): a cell may compare against one, and `max`, `min` and the four comparisons
    /// take two of them.
    pub ordered: bool,
    /// The `open` mark at the end of the line, where it is written: a contribution may add values
    /// to the enum with `extend` (§15.228). Without it, `extend` is E067.
    pub open: Option<Span>,
    /// Where the enum is declared, when the rule reads it from an `enums` file with `import
    /// enum … from` (§15.212). `None` for an enum the rule declares itself.
    pub from: Option<EnumOwner>,
    /// The values a contribution added with `extend`, each with the contribution that added it
    /// (§15.228). Empty for an enum nothing extended. The values themselves are in `values`, in
    /// their place in the order.
    pub added: Vec<Added>,
    pub span: Span,
}

/// A value of an enum a contribution added (§15.228): which value, which contribution added it
/// (its name and its file as the run shows it), and where the `extend` line is.
#[derive(Debug, Clone)]
pub struct Added {
    pub value: String,
    pub by: String,
    pub file: String,
    pub span: Span,
}

impl EnumDecl {
    /// The display name of the value at `i`, when it has one.
    pub fn label(&self, i: usize) -> Option<&str> {
        self.labels.get(i).and_then(|l| l.as_ref()).map(|(s, _)| s.as_str())
    }

    /// Whether any value has a display name: the enums that get a label function (§15.211).
    pub fn has_labels(&self) -> bool {
        self.labels.iter().any(|l| l.is_some())
    }
}

/// The `enums` file an enum is read from, and its name there (§15.212).
#[derive(Debug, Clone)]
pub struct EnumOwner {
    /// The path as the `import enum` line writes it, from the directory of the rule.
    pub file: String,
    /// The path as rulec reached it: the rule's directory joined with `file`.
    pub path: String,
    /// The name of the `enums` file (its heading), whose alias names the module the
    /// generated code reads the type from.
    pub module: Name,
    /// The enum's name in that file.
    pub name: Name,
}

/// `import enum era from "content.rule"` or `import enum era from "content.rule" -> period`
/// (§15.212): an enum read whole — values, order, display names, `default` marks and its
/// groups — from an `enums` file, rather than written again.
#[derive(Debug, Clone)]
pub struct EnumFrom {
    /// The enum's name in the `enums` file.
    pub name: Name,
    /// The path as written, followed from the directory of the rule.
    pub file: String,
    /// The name it has in this rule, when `->` renames it.
    pub local: Option<Name>,
    pub span: Span,
}

/// Which heading a `.rule` file starts with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// `rule <name> v<N>`: one decision, one generated function.
    Rule,
    /// `enums <name> v<N>`: enums and groups that rules read with `import enum … from`, and
    /// no function (§15.212).
    Enums,
    /// `contribution <name> v<N>`: what one file adds to another rule's open places — terms of
    /// its open values, values of its open enums, tables and clauses for its open outputs, lines
    /// of its open prerequisites (§15.227). It has no function of its own: it is merged into the
    /// rule it names with `to` before anything else reads it.
    Contribution,
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
    /// The file the line was written in, when it is not the one being checked: the rule a
    /// contribution's check is about (§15.235). The path is followed from its directory.
    pub base: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GroupDecl {
    pub name: Name,
    pub members: Vec<Name>,
    pub span: Span,
    /// The `enums` file the group came from with its enum (§15.212), as the `import enum` line
    /// writes the path. `None` for a group the rule declares itself.
    pub from: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub name: Name,
    pub ty: TypeRef,
    pub range: Option<Range>,
    /// §11 W111: silences the unused-declaration warning for a range-guard-only input.
    pub contract_only: bool,
    /// Where the caller's object holds this input, when it says (§15.125). It changes no
    /// check of the table: what comes out of a projection is a scalar input like any other,
    /// and the region analysis never sees that it was projected.
    ///
    /// On a field of an element taken from a contract (`elements lines from order.lines`), it is
    /// the path **from the element** (§15.223): `root` is its first step and `path` the rest, and
    /// the kind is always [`ProjKind::Field`]. [`Projection::steps`] gives the whole of it.
    pub from: Option<Projection>,
    /// The `random` mark at the end of the line, where it is written (§15.224): the caller draws
    /// the value uniformly from what the declaration lets in, independently of the other inputs,
    /// within what the `constraint` lines leave. The generated function is the same; what the mark
    /// adds is the claims of `chances` and the function that counts the answers.
    pub random: Option<Span>,
    pub span: Span,
}

/// `from order.shipping.prefecture`, the three that walk one collection to a truth value or a
/// count, and the three that walk it to a number (§15.223).
///
/// The language stops here on purpose: **one collection, `any` / `all` / `count` / `sum` /
/// `max` / `min`, and a unary test on a field of an element**. A join, a nested quantifier or a
/// path inside a cell would put the caller's object model inside the checks, and the cell
/// language is where this tool's boundary is (§0).
#[derive(Debug, Clone)]
pub struct Projection {
    /// The `shape` the path starts at. On a field of an element, the first step from the
    /// element (see [`VarDecl::from`]).
    pub root: Name,
    /// The field names after the root, in order. For a walk, the path of the collection.
    pub path: Vec<Name>,
    pub kind: ProjKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ProjKind {
    /// `order.shipping.prefecture` — the value at the path.
    Field,
    /// `any order.lines where category = chilled` — true when some element passes.
    Any(Name, Cell),
    /// `all order.lines where category = chilled` — true when every element does.
    All(Name, Cell),
    /// `count order.lines [where category = chilled]` — how many pass.
    Count(Option<(Name, Cell)>),
    /// `sum order.lines of amount [where …]` — the total of one field over the elements that
    /// pass, 0 when none does (§15.223).
    Sum(Name, Option<(Name, Cell)>),
    /// `max order.lines of amount [where …] empty 0円` — the largest value of one field over the
    /// elements that pass, and the `empty` value, with where it is written, when none does
    /// (§15.223). Leaving `empty` out is E074.
    Max(Name, Option<(Name, Cell)>, Option<(Lit, Span)>),
    /// `min order.lines of due [where …] empty 2026-12-31` — the smallest, the same way.
    Min(Name, Option<(Name, Cell)>, Option<(Lit, Span)>),
}

impl ProjKind {
    /// How it is written back, for a message and for `fmt`.
    pub fn word(&self) -> &'static str {
        match self {
            ProjKind::Field => "",
            ProjKind::Any(..) => crate::kw::ANY,
            ProjKind::All(..) => crate::kw::ALL,
            ProjKind::Count(_) => crate::kw::COUNT,
            ProjKind::Sum(..) => crate::kw::SUM,
            ProjKind::Max(..) => crate::kw::MAX,
            ProjKind::Min(..) => crate::kw::MIN,
        }
    }

    /// The test on an element, when there is one.
    pub fn test(&self) -> Option<(&Name, &Cell)> {
        match self {
            ProjKind::Any(n, c) | ProjKind::All(n, c) => Some((n, c)),
            ProjKind::Count(Some((n, c))) | ProjKind::Sum(_, Some((n, c))) => Some((n, c)),
            ProjKind::Max(_, Some((n, c)), _) | ProjKind::Min(_, Some((n, c)), _) => Some((n, c)),
            _ => None,
        }
    }

    /// The field of an element `sum`, `max` and `min` read, after `of` (§15.223).
    pub fn of(&self) -> Option<&Name> {
        match self {
            ProjKind::Sum(n, _) | ProjKind::Max(n, ..) | ProjKind::Min(n, ..) => Some(n),
            _ => None,
        }
    }

    /// The value `max` and `min` give when no element passes, and where it is written.
    pub fn empty(&self) -> Option<&(Lit, Span)> {
        match self {
            ProjKind::Max(_, _, e) | ProjKind::Min(_, _, e) => e.as_ref(),
            _ => None,
        }
    }

    /// `max` or `min`: a walk that picks one value of a field rather than adding them up.
    pub fn extreme(&self) -> bool {
        matches!(self, ProjKind::Max(..) | ProjKind::Min(..))
    }

    /// `sum`, `max` or `min`: a walk to a number read from a field of the elements (§15.223).
    pub fn numeric(&self) -> bool {
        self.of().is_some()
    }
}

impl Projection {
    /// The clause written back as it is read, for the inventory and for the reader's
    /// page. One spelling, so the two cannot disagree.
    pub fn text(&self) -> String {
        let path = self.steps().join(".");
        let mut out = match self.kind.word() {
            "" => path,
            w => format!("{w} {path}"),
        };
        if let Some(of) = self.kind.of() {
            out = format!("{out} {} {}", crate::kw::OF, of.text);
        }
        if let Some((fd, c)) = self.kind.test() {
            out = format!("{out} {} {} {}", crate::kw::WHERE, fd.text, cell_text(c));
        }
        if let Some((l, _)) = self.kind.empty() {
            out = format!("{out} {} {}", crate::kw::EMPTY, lit_text(l));
        }
        out
    }

    /// Every step written, the root first: `order.lines` as `["order", "lines"]`, and on a field
    /// of an element the whole path from the element (§15.223).
    pub fn steps(&self) -> Vec<String> {
        std::iter::once(self.root.text.clone()).chain(self.path.iter().map(|n| n.text.clone())).collect()
    }

    /// The steps after the root: the path inside the shape.
    pub fn path_steps(&self) -> Vec<String> {
        self.path.iter().map(|n| n.text.clone()).collect()
    }
}

/// A comparison on an ordered enum as it is written: `>=medieval <industrial` (§15.210).
pub fn ranked_text(ops: &[(CmpOp, String)]) -> String {
    ops.iter().map(|(o, v)| format!("{}{v}", o.word())).collect::<Vec<_>>().join(" ")
}

/// Whether value `v` of an enum whose values are `order` passes every comparison of `ops`
/// (§15.210). A value the enum does not have passes none: its place in the order is unknown.
pub fn ranked_holds(order: &[String], ops: &[(CmpOp, String)], v: &str) -> bool {
    let Some(x) = order.iter().position(|o| o == v) else { return false };
    ops.iter().all(|(op, b)| {
        let Some(y) = order.iter().position(|o| o == b) else { return false };
        match op {
            CmpOp::Lt => x < y,
            CmpOp::Le => x <= y,
            CmpOp::Gt => x > y,
            CmpOp::Ge => x >= y,
        }
    })
}

/// A literal written back as it was written: `16`, `3000円`, `true`, `"A"`.
pub fn lit_text(l: &Lit) -> String {
    match l {
        Lit::Num(n) => n.raw.clone(),
        Lit::Word(w) => w.clone(),
        Lit::Str(s) => format!("{s:?}"),
        Lit::Date(y, m, d) => format!("{y:04}-{m:02}-{d:02}"),
    }
}

/// A cell written back, for the one place outside the generator that has to quote one.
pub fn cell_text(c: &Cell) -> String {
    let lit = lit_text;
    match c {
        Cell::DontCare => "-".into(),
        Cell::Nothing => crate::kw::NONE.into(),
        Cell::Lit(l) => format!("= {}", lit(l)),
        Cell::Set(ls) => format!("= {}", ls.iter().map(lit).collect::<Vec<_>>().join(", ")),
        Cell::Not(ls) => format!("{}: {}", crate::kw::NOT, ls.iter().map(lit).collect::<Vec<_>>().join(", ")),
        Cell::Cmp(ops) => ops.iter().map(|(o, l)| format!("{}{}", o.word(), lit(l))).collect::<Vec<_>>().join(" "),
        Cell::Ranked { ops, .. } => ranked_text(ops),
        Cell::Prefix(ps) => {
            format!("{} {}", crate::kw::STARTS_WITH, ps.iter().map(|p| format!("{p:?}")).collect::<Vec<_>>().join(", "))
        }
    }
}

/// `shape order(order) = jsonschema "order.json" "#/$defs/Order"` — the contract the caller's
/// object is already described by (§15.125).
///
/// It is read on every `check`, unpinned, the way `import proto` is (§15.59): a contract that
/// moved is a change nobody has read, and the paths are what say so.
#[derive(Debug, Clone)]
pub struct ShapeDecl {
    pub name: Name,
    pub source: EnumSource,
    /// The contract file, relative to the rule.
    pub file: String,
    /// A JSON Pointer for a schema, a message name for a `.proto`.
    pub at: String,
    pub span: Span,
    /// The file the line was written in, when it is not the one being checked: the rule a
    /// contribution's check is about (§15.235). The path is followed from its directory.
    pub base: Option<String>,
}

impl ShapeDecl {
    /// The file the contract is read from: `file` followed from the directory of the rule that
    /// wrote the line — `rule_path`, the file being checked, unless the line came from another.
    pub fn contract_path(&self, rule_path: &str) -> std::path::PathBuf {
        let base = self.base.as_deref().unwrap_or(rule_path);
        std::path::Path::new(base).parent().unwrap_or(std::path::Path::new(".")).join(&self.file)
    }
}

impl EnumImport {
    /// The file the enum is read from, followed from the directory of the rule that wrote the line.
    pub fn file_path(&self, rule_path: &str) -> std::path::PathBuf {
        let base = self.base.as_deref().unwrap_or(rule_path);
        std::path::Path::new(base).parent().unwrap_or(std::path::Path::new(".")).join(&self.file)
    }
}

impl DaysFrom {
    /// The koyomi file the days are read from, followed from the directory of the file that wrote
    /// the line.
    pub fn file_path(&self, rule_path: &str) -> std::path::PathBuf {
        let base = self.base.as_deref().unwrap_or(rule_path);
        std::path::Path::new(base).parent().unwrap_or(std::path::Path::new("")).join(&self.file)
    }
}

#[derive(Debug, Clone)]
pub struct OutDecl {
    pub name: Name,
    pub ty: TypeRef,
    pub rounding: Option<Rounding>,
    pub span: Span,
    /// The range written on the line (§15.268): held to the values the output takes, then the
    /// range other languages read it in.
    pub range: Option<Range>,
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
pub use ritsu_base::sources::LawDb;

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
    /// `with <callee's value> -> <this rule's value>, …` (§15.230): the values of an enum the callee
    /// declares, read as this rule's. A value spelled the same on both sides needs no entry.
    pub map: Vec<(String, String)>,
    pub span: Span,
}

/// `apply <name> = "<path>" sha256:…` with its bindings (§15.69). After
/// expansion the callee's definitions sit in `items` from `at` on, `count` of them, named
/// `<name>:<their name>`.
#[derive(Debug, Clone)]
pub struct ApplyDecl {
    pub name: Name,
    pub path: String,
    /// `sha256:<16 hex>`: the callee's bytes, as pins were written before `meaning:` (§15.69).
    pub hash: Option<String>,
    /// `meaning:<16 hex>`: the callee's rows and declarations with its comments, `description`
    /// and citations left out (§15.230). A comment does not move it; a row does.
    pub meaning: Option<String>,
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
    /// The fields of the callee's elements, each with the field of this rule's sequence it reads
    /// (§15.230): held to the same checks of type and range an input's binding is.
    pub fields: Vec<(String, CalleeInput)>,
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
    /// The values the `range` of an input of an ordered enum lets in, in the callee's order and
    /// spelling (`Checked::enum_ranges`, §15.210): what E043 holds the values passed to (§15.252).
    /// None for an input with no such range, and for a field of an element, whose enum takes none
    /// (E069).
    pub enum_range: Option<Vec<String>>,
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
    /// A comparison on a column of an ordered enum — `<medieval`, `>=medieval <industrial`
    /// (§15.210) — with the values it takes. The parser reads it as a `Cmp`; once the column
    /// is known to be an ordered enum, `types::read_words` gives it the values the comparison
    /// covers, in the enum's order. Whatever asks which values a cell takes reads `values`,
    /// as it reads a set; the generated code, the certificate and the messages write `ops`,
    /// the row as it was written.
    Ranked { ops: Vec<(CmpOp, String)>, values: Vec<String> },
}

#[derive(Debug, Clone)]
pub enum OutCell {
    Lit(Lit),
    /// §3.2: a name (input, derived, define). Expressions are not allowed.
    Name(String),
    /// `none`: the row answers that there is no value. Only a column of `T?` takes it
    /// (§15.249); anywhere else it is E103.
    Nothing,
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
    /// The `open` mark after the name on the `table` or `clause` line: a contribution may add
    /// tables and clauses that define the same output (§15.229).
    pub open: Option<Span>,
    /// The contribution this table or clause came in from, when it did (§15.229): its name and
    /// its file as the run shows it. Its rows are written there.
    pub contributed: Option<(String, String)>,
}

#[derive(Debug, Clone)]
pub enum Item {
    Derived(DerivedDecl),
    Define(DefineDecl),
    Table(Table),
    Agg(AggDecl),
    /// A `fold`, where it is written (§15.219): its answer is a value of the rule from there on,
    /// like a summary's.
    Fold(FoldDecl),
}

/// Which summary of the walk this is (§15.58, §15.100, §15.219).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggKind {
    /// `count 一致数(hits) over 納入先 where 判定 = 一致` — how many elements pass one test.
    Count,
    /// `sum 合計(total) over 明細 of 金額` — the total of one column over the elements.
    Sum,
    /// `max 最高額(top) over 明細 of 金額 empty 0円` — the largest value of one column over the
    /// elements a `where` lets in, or the `empty` value when there are none (§15.219).
    Max,
    /// `min 最も早い期日(earliest) over 作業 of 期日 empty 2026-12-31` — the smallest, the same way.
    Min,
}

impl AggKind {
    /// The word the line starts with.
    pub fn word(self) -> &'static str {
        match self {
            AggKind::Count => crate::kw::COUNT,
            AggKind::Sum => crate::kw::SUM,
            AggKind::Max => crate::kw::MAX,
            AggKind::Min => crate::kw::MIN,
        }
    }

    /// `max` or `min`: a summary that picks one value of its column rather than adding them up.
    pub fn extreme(self) -> bool {
        matches!(self, AggKind::Max | AggKind::Min)
    }
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
    /// `max` and `min` only: `where <column> [= <value>]`, the elements whose value is looked at
    /// — a column of one element, tested the way `count` tests one (§15.219). `None` looks at
    /// every element.
    pub filter: Option<(Name, Option<Name>)>,
    /// `max` and `min` only: `empty <value>`, the answer when no element is looked at, and where
    /// it is written (§15.219). Required (E022).
    pub empty: Option<(Lit, Span)>,
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
    /// `elements lines from order.lines`: the collection of the caller's object the sequence is
    /// taken from, each field then saying with its own `from` where it stands in an element
    /// (§15.223). The kind is [`ProjKind::Field`]. Like an input's, it changes no check: the
    /// elements are elements like any others.
    pub from: Option<Projection>,
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
    /// `take_weighted <value> by <weight>` (§15.226): add this element's weight to the running
    /// total, and take its value when the drawn value (`draw -> <input>`) is below that total for
    /// the first time. Each element is taken with the chance its weight is of the whole.
    TakeWeighted { expr: Expr, weight: Expr },
}

/// `-> 最良の補正(best_aura) : rate` after a `fold` heading: the name its answer is bound to, and
/// the type written for a name the rule does not declare elsewhere (§15.219).
#[derive(Debug, Clone)]
pub struct FoldAnswer {
    pub name: Name,
    pub ty: Option<TypeRef>,
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
    /// `-> <name> [: <type>]`: where the answer goes. Without it, the rule's first output, as
    /// every fold did before a rule could have more than one (§15.219).
    pub answer: Option<FoldAnswer>,
    /// One arm per value of the verdict's enum, in source order.
    pub arms: Vec<(Name, Arm, Span)>,
    /// The answer when there are no elements at all. Declaring it is not optional.
    pub empty: Option<Expr>,
    /// The answer when the walk reached the end. Declaring it is not optional.
    pub exhausted: Option<Expr>,
    /// `draw -> <random input>` (§15.226): the drawn value a `take_weighted` arm compares the
    /// running total of the weights with, by its name and where it is written.
    pub draw: Option<Name>,
    pub span: Span,
}

impl FoldDecl {
    /// Whether an arm picks by weight (§15.226).
    pub fn weighted(&self) -> bool {
        self.arms.iter().any(|(_, a, _)| matches!(a, Arm::TakeWeighted { .. }))
    }
}

/// A claimed chance (§15.224): an exact rational — written `5/14`, `0`, `1` or `35%` — or
/// comparisons with such values (`<=1/100`, `>=3/4`, `>=1/4 <=1/2`), all of which the chance has to
/// pass.
#[derive(Debug, Clone)]
pub enum ChanceTest {
    Is(crate::num::Rat),
    Cmp(Vec<(CmpOp, crate::num::Rat)>),
}

impl ChanceTest {
    /// Whether the chance `p` passes it.
    pub fn holds(&self, p: crate::num::Rat) -> bool {
        use std::cmp::Ordering;
        match self {
            ChanceTest::Is(v) => p.cmp_to(*v) == Ordering::Equal,
            ChanceTest::Cmp(cs) => cs.iter().all(|(op, v)| {
                let o = p.cmp_to(*v);
                match op {
                    CmpOp::Lt => o == Ordering::Less,
                    CmpOp::Le => o != Ordering::Greater,
                    CmpOp::Gt => o == Ordering::Greater,
                    CmpOp::Ge => o != Ordering::Less,
                }
            }),
        }
    }

    /// The values it is written with, in order.
    pub fn values(&self) -> Vec<crate::num::Rat> {
        match self {
            ChanceTest::Is(v) => vec![*v],
            ChanceTest::Cmp(cs) => cs.iter().map(|(_, v)| *v).collect(),
        }
    }
}

/// One row of a `chances` table (§15.224): the cells of the inputs that are not random (and the
/// sequences, for a rule that walks), the cells of the outputs the claim is about, and the chance.
#[derive(Debug, Clone)]
pub struct ChanceRow {
    pub cells: Vec<Cell>,
    pub cell_spans: Vec<Span>,
    pub outs: Vec<Cell>,
    pub out_spans: Vec<Span>,
    /// The claimed chance, as read; `None` when the cell could not be read (an error was said).
    pub chance: Option<ChanceTest>,
    /// The chance as it was written, for the page and the messages.
    pub chance_text: String,
    pub chance_span: Span,
    pub span: Span,
    /// 1-based, as the diagnostics print it.
    pub index: usize,
}

/// `chances` and the table under it (§15.224): claims about how likely each answer is when the
/// `random` inputs are drawn. Every input that is not random has a column; the outputs the claims
/// are about have one each after `->`; the last column, `chance`, holds the claim. A row says: for
/// **every** combination of the inputs that are not random its cells let in, drawing the random
/// inputs uniformly, the chance that the outputs pass the row's output cells is the value written
/// (or passes the comparisons written).
#[derive(Debug, Clone)]
pub struct ChancesDecl {
    pub inputs: Vec<(String, Span)>,
    pub outputs: Vec<(String, Span)>,
    pub rows: Vec<ChanceRow>,
    /// The `chances` line.
    pub span: Span,
}

/// `constraint 全条件一致数 <= 会社名一致数` — a relation between two inputs that the caller
/// guarantees. Nothing computes with it: it says which combinations exist, so the checks do
/// not demand rows for the ones that do not, and the entry guard refuses them (§15.55).
///
/// One side may add several inputs up: `constraint roll < to_defender + to_attacker`,
/// `constraint discount + points <= subtotal` (§15.216). Each side is the list of the inputs it
/// adds, one name for a side that is not a sum, and at most one side has more than one.
#[derive(Debug, Clone)]
pub struct Constraint {
    pub left: Vec<String>,
    pub op: CmpOp,
    pub right: Vec<String>,
    pub span: Span,
}

impl Constraint {
    /// Every input the constraint names, the left side's first.
    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.left.iter().chain(self.right.iter())
    }

    /// The two inputs of a constraint between two inputs: the shape every constraint had before
    /// a side could be a sum, and the one the reasoning that chains `a <= b` and `b <= c` reads.
    /// `None` for a constraint with a sum on a side.
    pub fn pair(&self) -> Option<(&String, &String)> {
        match (self.left.as_slice(), self.right.as_slice()) {
            ([a], [b]) => Some((a, b)),
            _ => None,
        }
    }

    /// Whether a side adds several inputs up (§15.216).
    pub fn is_sum(&self) -> bool {
        self.left.len() > 1 || self.right.len() > 1
    }

    /// One side as it is written: `to_defender + to_attacker`.
    pub fn side_text(names: &[String]) -> String {
        names.join(" + ")
    }

    /// The constraint as it is written, with the operator in its ASCII spelling:
    /// `discount + points <= subtotal`.
    pub fn said(&self) -> String {
        format!("{} {} {}", Self::side_text(&self.left), self.op.word(), Self::side_text(&self.right))
    }
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

/// `never 出荷済, 配達済 after 取消` (§15.148): no sequence of calls reaches one of the first
/// states once the case has been in one of the second.
#[derive(Debug, Clone)]
pub struct NeverDecl {
    pub states: Vec<Name>,
    pub after: Vec<Name>,
    pub span: Span,
}

/// `once 返金額 >0円` (§15.148): in any sequence of calls, at most one call answers an output
/// the cell accepts.
#[derive(Debug, Clone)]
pub struct OnceDecl {
    pub output: Name,
    pub cell: Cell,
    /// Where the cell was written.
    pub cell_span: Span,
    pub span: Span,
}

/// `always 残り日数 >=1 <=7 when 猶予中, 督促中` (§15.217): in every state a case can reach — every
/// one whose state is on the `when` list, when there is one — each carried value the line
/// names is one its cell accepts.
#[derive(Debug, Clone)]
pub struct AlwaysDecl {
    /// `<carried input> <cell>`, joined by `and`: the name, the cell, and where the cell is.
    pub tests: Vec<(Name, Cell, Span)>,
    /// The states the line speaks of; empty for all of them.
    pub when: Vec<Name>,
    pub span: Span,
}

/// `machine 注文(order) over 遷移` and the lines under it (§15.148).
///
/// The rule is one step of a state machine. `carry` names the output that comes back as an
/// input on the next call, and that is the whole of what is new about the meaning: the
/// generated function still takes everything as an argument and keeps nothing, and the state
/// lives where it always did, with the host (§5.4). What the section adds is claims about
/// every sequence of calls, and those are decidable for the same reason a table is: the state
/// is a finite enum, and the tables cut every other input into finitely many classes.
///
/// The first `carry` line is the state. Further lines carry a number with a range, a boolean
/// or another enum along with it (§15.217): what a case is in is then the state together with
/// those values, which stay finitely many because the number has a range.
#[derive(Debug, Clone)]
pub struct MachineDecl {
    pub name: Name,
    /// The table that decides the carried output: its rows are the transitions.
    pub over: Option<Name>,
    /// `carry <input> -> <output>`, with where the line is: the first line, the state.
    pub carry: Option<(Name, Name, Span)>,
    /// The `carry` lines after the first (§15.217), in the order written.
    pub carries: Vec<(Name, Name, Span)>,
    pub initial: Option<(Name, Span)>,
    /// `initial <carried input> <value>`: where each further carried value starts (§15.217).
    pub initials: Vec<(Name, Lit, Span)>,
    pub finals: Vec<Name>,
    /// Where the `final` line is, when there is one.
    pub final_span: Option<Span>,
    pub nevers: Vec<NeverDecl>,
    pub onces: Vec<OnceDecl>,
    /// The `always` lines (§15.217).
    pub alwayses: Vec<AlwaysDecl>,
    /// `held <input>, …`: the inputs a case holds from its first call to its last, and where
    /// the line is (§15.149).
    pub held: Vec<Name>,
    pub held_span: Option<Span>,
    pub span: Span,
}

impl MachineDecl {
    /// The carried input and output, when the `carry` line was read.
    pub fn carried(&self) -> Option<(&str, &str)> {
        self.carry.as_ref().map(|(i, o, _)| (i.text.as_str(), o.text.as_str()))
    }

    /// The further carried inputs and outputs (§15.217), in the order written.
    pub fn more(&self) -> Vec<(&str, &str)> {
        self.carries.iter().map(|(i, o, _)| (i.text.as_str(), o.text.as_str())).collect()
    }
}

/// `scenario 取消のあとの入金(late_pay)` and its table (§15.148): a sequence of calls from the
/// machine's initial state, one row per call, with what each call answers. Shaped like
/// `examples`, except that the carried input has no column: it is what the call before
/// answered.
#[derive(Debug, Clone)]
pub struct ScenarioDecl {
    pub name: Name,
    pub table: Table,
    pub span: Span,
}

/// `prerequisites research over tech [open]` and the lines under it (§15.222): which values of an
/// enum a case has to hold before it can take another. A finite graph over the enum's values,
/// checked whole — no cycle, every value reached from `start`, no prerequisite another already
/// implies, and a table's order kept — and generated as two functions, the prerequisites of a
/// value and whether it can be taken now.
#[derive(Debug, Clone)]
pub struct PrereqDecl {
    /// The block's name, whose alias names the generated functions (`research_available`).
    pub name: Name,
    /// The enum whose values it is about, when the heading names one.
    pub over: Option<Name>,
    /// The `open` mark after the enum: a contribution may add lines for the values it adds. It is
    /// syntax only here; what it opens is the contributions' (§15.222).
    pub open: Option<Span>,
    /// `start <value>, …`: the values that need nothing, and where the line is.
    pub start: Vec<Name>,
    pub start_span: Option<Span>,
    /// One line per value that needs something, in the order written.
    pub lines: Vec<RequiresLine>,
    /// `ordered by <table>`: the table whose answer for a value is never below its prerequisites',
    /// and where the line is.
    pub ordered_by: Option<(Name, Span)>,
    pub span: Span,
    /// The last line of the block, so that what quotes it whole (an item of ritsu's index, the
    /// page) knows where it ends.
    pub end_line: usize,
}

impl PrereqDecl {
    /// The name of the enum it is about, or `""` when the heading names none.
    pub fn over_name(&self) -> &str {
        self.over.as_ref().map(|n| n.text.as_str()).unwrap_or("")
    }

    /// The line of `value`, when it has one.
    pub fn line_of(&self, value: &str) -> Option<&RequiresLine> {
        self.lines.iter().find(|l| l.value.text == value)
    }
}

/// `<value> requires <prerequisites>` under `prerequisites` (§15.222).
#[derive(Debug, Clone)]
pub struct RequiresLine {
    pub value: Name,
    pub needs: Needs,
    pub span: Span,
}

/// What a value of a `prerequisites` block needs (§15.222).
#[derive(Debug, Clone)]
pub enum Needs {
    /// Every group, each by one of its values: `a, b or c` is `[[a], [b, c]]`.
    Groups(Vec<Vec<Name>>),
    /// `requires nothing "<reason>"`: a value no case can take, and why.
    Nothing { reason: String, span: Span },
}

/// How the terms of an `open` value are put together (§15.227). None of the four depends on the
/// order of the terms, which is why contributions need no order to load in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combine {
    Sum,
    Product,
    Max,
    Min,
}

impl Combine {
    pub fn word(self) -> &'static str {
        match self {
            Combine::Sum => crate::kw::SUM,
            Combine::Product => crate::kw::PRODUCT,
            Combine::Max => crate::kw::MAX,
            Combine::Min => crate::kw::MIN,
        }
    }

    pub fn parse(w: &str) -> Option<Combine> {
        match w {
            crate::kw::SUM => Some(Combine::Sum),
            crate::kw::PRODUCT => Some(Combine::Product),
            crate::kw::MAX => Some(Combine::Max),
            crate::kw::MIN => Some(Combine::Min),
            _ => None,
        }
    }
}

/// One term of an `open` value: a name, and the contribution it came from when it did.
#[derive(Debug, Clone)]
pub struct Term {
    pub name: Name,
    /// The contribution's name and its file as the run shows it; `None` for a term the rule
    /// itself writes under the `open` line.
    pub from: Option<(String, String)>,
}

/// `open stackable : money[USD] combine sum range >=0USD <=100USD` and the terms under it
/// (§15.227): a value the rule computes from its terms and the terms contributions add, put
/// together one way. The declared range is the promise: what every contribution comes to
/// together stays inside it (E131), and the tables below are checked over the whole of it, so the
/// rule's own check holds whatever the contributions are.
#[derive(Debug, Clone)]
pub struct OpenDecl {
    pub name: Name,
    pub ty: TypeRef,
    /// `combine <how>` and where the word is. `None` when the line has none (E068).
    pub combine: Option<(Combine, Span)>,
    /// `empty <value>`: the value the terms start from. `sum` starts at 0 and `product` at 1 when
    /// none is written; `max` and `min` need one (E068).
    pub empty: Option<(Lit, Span)>,
    pub range: Option<Range>,
    pub terms: Vec<Term>,
    /// Where in `items` it sits: the index the next item had when the line was read. The value
    /// is a `define` there once the contributions are in (`link::lower`).
    pub at: usize,
    pub span: Span,
}

/// `to "<file>" signature:<digest>` or `after "<file>" signature:<digest>` in a contribution
/// (§15.227, §15.230): the file it adds to or comes after, and the pinned shape of it.
#[derive(Debug, Clone)]
pub struct Target {
    pub file: String,
    pub signature: Option<String>,
    pub span: Span,
}

/// `extend <enum> with <value>(<alias>) ["<label>"] [after <value>] [default]` (§15.228).
#[derive(Debug, Clone)]
pub struct ExtendDecl {
    pub enum_name: Name,
    pub value: Name,
    pub label: Option<(String, Span)>,
    /// `after <value>`: where the value goes in an ordered enum.
    pub after: Option<Name>,
    /// `default`: the value may fall to the rows that do not name it (§15.228, E033).
    pub default: bool,
    pub span: Span,
}

/// `into <open value>` and the terms under it (§15.227): the names this contribution adds to an
/// open value of the file it adds to.
#[derive(Debug, Clone)]
pub struct IntoDecl {
    pub target: Name,
    pub terms: Vec<Name>,
    pub span: Span,
}

/// What a `contribution` file says besides its items (§15.227).
#[derive(Debug, Clone)]
pub struct ContribHead {
    pub to: Option<Target>,
    pub afters: Vec<Target>,
    pub extends: Vec<ExtendDecl>,
    pub intos: Vec<IntoDecl>,
    /// The lines this file adds to an open `prerequisites` block of the file it adds to: the
    /// block's name and the `requires` lines.
    pub prereq_lines: Vec<(Name, Vec<RequiresLine>)>,
}

/// One contribution merged into a rule (§15.231), as the generated header, `rulec api`, the page and
/// the certificate say it: its name, its file as the run shows it, the SHA-256 of the file, the
/// shape it pinned of what it adds to, the inputs it added and the file it adds to.
#[derive(Debug, Clone)]
pub struct Merged {
    pub name: Name,
    pub version: String,
    pub description: Option<String>,
    pub file: String,
    pub sha256: String,
    pub signature: Option<String>,
    pub to: String,
    pub inputs: Vec<String>,
    /// The source of the file, for what reads its cells back (the certificate, the page).
    pub src: String,
    /// The `apply` the contribution reaches this rule through, when it was merged into a rule this
    /// one applies (§15.237): the apply's name, `a:b` for an apply inside an applied rule. Such a
    /// contribution is part of the function as the callee's rows are, and adds no input to it.
    pub via: Option<String>,
}

impl Merged {
    /// The contribution's name as this rule shows it: `<apply>:<name>` for one that came in through
    /// an `apply`, as the names the apply brings in are written (§15.237).
    pub fn shown_name(&self) -> String {
        match &self.via {
            Some(a) => format!("{a}:{}", self.name.text),
            None => self.name.text.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuleFile {
    /// `rule`, `enums` (§15.212) or `contribution` (§15.227).
    pub kind: FileKind,
    pub name: Name,
    pub version: String,
    pub description: Option<String>,
    pub imports: Vec<(String, Span)>,
    /// The enums whose value set is declared outside this file (§15.59, §15.60).
    pub enum_imports: Vec<EnumImport>,
    /// The enums read whole from an `enums` file (§15.212), in the order written. Once read
    /// they sit in `enums` like any other, with `from` saying where they came from.
    pub enum_froms: Vec<EnumFrom>,
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
    /// `examples` — an executable specification (§1.2), shaped like a table. A rule may
    /// have more than one section, each with a header of its own, and every one runs: a
    /// second section used to replace the first without a word (§15.149).
    pub examples: Vec<Table>,
    /// The relations between inputs that always hold (§15.55).
    pub constraints: Vec<Constraint>,
    /// The shapes of the caller's objects that inputs are projected from (§15.125).
    pub shapes: Vec<ShapeDecl>,
    /// The sequences the rule walks, each with the fields of one element (§15.56), in the order
    /// written. A rule may walk more than one (§15.219).
    pub elements: Vec<ElementsDecl>,
    /// Named lists of elements. A cell of `examples` names one, which is how a case for a
    /// walk is written: a row of cells has no room for a sequence (§15.56).
    pub sequences: Vec<SeqDecl>,
    /// The state machine the rule is one step of (§15.148).
    pub machine: Option<MachineDecl>,
    /// Sequences of calls with what each one answers (§15.148).
    pub scenarios: Vec<ScenarioDecl>,
    /// Which values of an enum need which before them (§15.222), in the order written.
    pub prerequisites: Vec<PrereqDecl>,
    /// The claims about the chances of the answers when the `random` inputs are drawn (§15.224), one
    /// per `chances` section, in the order written.
    pub chances: Vec<ChancesDecl>,
    /// The values a contribution may add terms to (§15.227), in the order written. Each is a
    /// `define` in `items` once `link::lower` has run; this keeps what the `define` came from.
    pub opens: Vec<OpenDecl>,
    /// What a `contribution` file says besides its items (§15.227). `None` for a rule or an
    /// `enums` file.
    pub contrib: Option<ContribHead>,
    /// The contributions merged into this rule, in the order they were merged (§15.231).
    pub contributions: Vec<Merged>,
}

impl FoldDecl {
    /// The name the fold's answer is bound to: the one after `->`, else the rule's first output.
    pub fn answer_name<'a>(&'a self, f: &'a RuleFile) -> Option<&'a str> {
        match &self.answer {
            Some(a) => Some(a.name.text.as_str()),
            None => f.outputs.first().map(|o| o.name.text.as_str()),
        }
    }

    /// The values the fold can answer with (§15.219): `empty`, `exhausted` unless it is `held`, a
    /// `stop with`, and what an arm takes or keeps. `held` is one of the values taken or kept, or
    /// `empty` when none was, so the answer is always one of these.
    pub fn answer_values(&self) -> Vec<&Expr> {
        let mut out: Vec<&Expr> = Vec::new();
        for e in self.empty.iter().chain(self.exhausted.iter()) {
            if !matches!(e, Expr::Name(n, _) if n == crate::kw::HELD) {
                out.push(e);
            }
        }
        for (_, arm, _) in &self.arms {
            match arm {
                Arm::Stop(Some(x)) => out.push(x),
                Arm::Take { expr, .. } | Arm::KeepMax { expr, .. } | Arm::TakeWeighted { expr, .. } => out.push(expr),
                _ => {}
            }
        }
        out
    }
}

impl RuleFile {
    /// The `fold`s, in the order written (§15.219).
    pub fn folds(&self) -> impl Iterator<Item = &FoldDecl> {
        self.items.iter().filter_map(|it| if let Item::Fold(d) = it { Some(d) } else { None })
    }

    /// The `count`, `sum`, `max` and `min` lines, in the order written.
    pub fn aggs(&self) -> impl Iterator<Item = &AggDecl> {
        self.items.iter().filter_map(|it| if let Item::Agg(d) = it { Some(d) } else { None })
    }

    /// Whether the rule walks a sequence: it has a `fold`, or a summary over one.
    pub fn walks(&self) -> bool {
        self.items.iter().any(|it| matches!(it, Item::Agg(_) | Item::Fold(_)))
    }

    /// The sequence declared under this name.
    pub fn sequence_named(&self, name: &str) -> Option<&ElementsDecl> {
        self.elements.iter().find(|e| e.name.text == name)
    }

    /// The fields of every element the rule walks, the first sequence's first.
    pub fn element_fields(&self) -> impl Iterator<Item = &VarDecl> {
        self.elements.iter().flat_map(|e| e.fields.iter())
    }

    /// The sequence one field belongs to.
    pub fn sequence_of_field(&self, field: &str) -> Option<&ElementsDecl> {
        self.elements.iter().find(|e| e.fields.iter().any(|fd| fd.name.text == field))
    }

    /// The inputs marked `random` (§15.224), in the order declared.
    pub fn random_inputs(&self) -> impl Iterator<Item = &VarDecl> {
        self.inputs.iter().filter(|i| i.random.is_some())
    }

    /// Whether the rule has an input marked `random`.
    pub fn has_random(&self) -> bool {
        self.inputs.iter().any(|i| i.random.is_some())
    }

    /// Whether `name` is an input marked `random`.
    pub fn is_random(&self, name: &str) -> bool {
        self.inputs.iter().any(|i| i.random.is_some() && i.name.text == name)
    }

    /// The `open` value of this name (§15.227), when it is one.
    pub fn open_value(&self, name: &str) -> Option<&OpenDecl> {
        self.opens.iter().find(|o| o.name.text == name)
    }
}

impl Lit {
    /// The literal as the rule writes it: `500円`, `2026-04-01`, `gold`, `"text"`.
    pub fn src(&self) -> String {
        match self {
            Lit::Num(n) => n.raw.clone(),
            Lit::Word(w) => w.clone(),
            Lit::Str(s) => format!("\"{s}\""),
            Lit::Date(y, m, d) => format!("{y:04}-{m:02}-{d:02}"),
        }
    }
}
