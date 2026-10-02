//! What a `.cal` says, as the parser read it (DESIGN 1). Names are not resolved here; that is
//! `resolve.rs`.

use crate::date::{Day, Missing};

/// Where something was written: 1-based line and column (columns count characters).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Calendar,
    Dates,
}

/// A name and its ASCII alias, `受領日(received)`.
#[derive(Clone, Debug)]
pub struct Name {
    pub text: String,
    pub alias: Option<(String, Span)>,
    pub span: Span,
}

impl Name {
    /// The alias, or the name itself when it is already ASCII of the alias form.
    pub fn ascii(&self) -> Option<&str> {
        match &self.alias {
            Some((a, _)) => Some(a),
            None if is_alias(&self.text) => Some(&self.text),
            None => None,
        }
    }
}

/// `[a-z][a-z0-9_]*`: the form an alias takes, because it becomes an identifier in five
/// languages (DESIGN 1.2).
pub fn is_alias(s: &str) -> bool {
    let mut cs = s.chars();
    matches!(cs.next(), Some('a'..='z')) && cs.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[derive(Clone, Debug)]
pub struct File {
    pub kind: Kind,
    /// The path as it is shown in diagnostics.
    pub path: String,
    pub src: String,
    pub name: Name,
    pub version: String,
    pub description: Option<String>,
    /// `offset <text>`: the text as written, checked by the calendar (E107).
    pub offset: Option<(String, Span)>,
    pub use_calendar: Option<(String, Span)>,
    pub sources: Vec<SourceDecl>,
    pub rules: Vec<CalRule>,
    pub inputs: Vec<Input>,
    pub dates: Vec<DateDecl>,
    pub claims: Vec<Claim>,
    pub examples: Option<Examples>,
}

/// `@民法 第141条, 第143条`.
#[derive(Clone, Debug)]
pub struct Cite {
    pub source: String,
    pub fragments: Vec<(String, Span)>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct SourceDecl {
    pub name: String,
    pub span: Span,
    pub kind: SourceKind,
}

#[derive(Clone, Debug)]
pub enum SourceKind {
    /// A table of holidays: `source 祝日 = file "data/syukujitsu.csv" url "…" sha256:…`.
    File {
        path: String,
        url: Option<String>,
        pin: Option<String>,
        format: Option<(Format, Span)>,
        covers: Option<(Covers, Span)>,
    },
    /// A law on e-Gov as of a date, with the digest of each article it pins.
    Law { id: String, asof: Day, pins: Vec<LawPin> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Format {
    Csv { shift_jis: bool },
    GovUk { division: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Covers {
    Range(Day, Day),
    ListedYears,
}

#[derive(Clone, Debug)]
pub struct LawPin {
    pub fragment: String,
    pub pin: Option<String>,
    pub span: Span,
}

/// A `closed` or `open` line of a calendar.
#[derive(Clone, Debug)]
pub struct CalRule {
    pub what: RuleKind,
    pub span: Span,
    pub cite: Option<Cite>,
}

#[derive(Clone, Debug)]
pub enum RuleKind {
    /// `closed weekly sat, sun` (Monday is 0).
    Weekly(Vec<u32>),
    /// `closed 祝日`: the days of a table.
    Table(String, Span),
    /// `closed every 12-29..01-03 "年末年始"`: month and day, both ends included.
    Every { from: (u32, u32), to: (u32, u32), name: Option<String> },
    /// `closed 2026-08-13..2026-08-15 "夏季休業"`.
    Days { from: Day, to: Day, name: Option<String> },
    /// `open 2026-12-28 "臨時営業"`.
    Open { from: Day, to: Day, name: Option<String> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Date,
    Int,
}

#[derive(Clone, Debug)]
pub struct Input {
    pub name: Name,
    pub ty: Ty,
    pub span: Span,
    /// `range >=… <=…`: each end as written, None when missing.
    pub lo: Option<(Lit, Span)>,
    pub hi: Option<(Lit, Span)>,
    /// Where `range` is, or where it should have been.
    pub range_span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lit {
    Date(Day),
    Int(i64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sign {
    Plus,
    Minus,
}

impl Sign {
    pub fn apply(self, n: i64) -> i64 {
        match self {
            Sign::Plus => n,
            Sign::Minus => -n,
        }
    }
}

/// A number in an operation: written out, or the name of an integer input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arg {
    Lit(i64),
    Name(String, Span),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conv {
    Following,
    Preceding,
    ModifiedFollowing,
    ModifiedPreceding,
}

/// One operation (DESIGN 1.6). `else` is None where none was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    Days(Sign, Arg),
    BusinessDays(Sign, Arg),
    Months(Sign, Arg, Option<(Missing, Span)>),
    Years(Sign, Arg, Option<(Missing, Span)>),
    /// `day N of month ±k`.
    DayOfMonth(Arg, Sign, Arg, Option<(Missing, Span)>),
    StartOfMonth(Sign, Arg),
    EndOfMonth(Sign, Arg),
    CloseDay(Arg, Option<(Missing, Span)>),
    CloseEndOfMonth,
    Roll(Conv),
    IfClosed(Box<Op>),
}

/// An operation and the line it is on.
#[derive(Clone, Debug)]
pub struct OpLine {
    pub op: Op,
    pub span: Span,
    /// The operation as written, without the citation and the comment.
    pub text: String,
    pub cite: Option<Cite>,
    /// `else …` after an operation that never lands on a missing day (W201).
    pub stray_else: Option<(Missing, Span)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum At {
    /// Minutes after midnight.
    Time(u32),
    /// The end of the day: midnight at the start of the next.
    EndOfDay,
}

#[derive(Clone, Debug)]
pub struct DateDecl {
    pub name: Name,
    pub span: Span,
    pub start: (String, Span),
    pub cite: Option<Cite>,
    pub ops: Vec<OpLine>,
    pub at: Option<(At, Span, Option<Cite>)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmp {
    Eq,
    Lt,
    Le,
    Gt,
    Ge,
}

impl Cmp {
    pub fn symbol(self) -> &'static str {
        match self {
            Cmp::Eq => "=",
            Cmp::Lt => "<",
            Cmp::Le => "<=",
            Cmp::Gt => ">",
            Cmp::Ge => ">=",
        }
    }

    pub fn holds(self, a: Day, b: Day) -> bool {
        match self {
            Cmp::Eq => a == b,
            Cmp::Lt => a < b,
            Cmp::Le => a <= b,
            Cmp::Gt => a > b,
            Cmp::Ge => a >= b,
        }
    }
}

/// `支払日`, or `受領日 + 60 days`, on one side of a claim.
#[derive(Clone, Debug)]
pub struct DateRef {
    pub name: String,
    pub span: Span,
    /// `± n [business] days`; the bool says business days.
    pub offset: Option<(Sign, Arg, bool)>,
}

#[derive(Clone, Debug)]
pub enum ClaimKind {
    IsOpen(DateRef),
    Monotonic(String, Span),
    Compare(DateRef, Cmp, DateRef),
}

#[derive(Clone, Debug)]
pub struct Claim {
    pub name: String,
    pub span: Span,
    pub kind: ClaimKind,
    /// The claim as written after the colon.
    pub text: String,
    pub cite: Option<Cite>,
}

#[derive(Clone, Debug)]
pub struct Examples {
    pub span: Span,
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
}

#[derive(Clone, Debug)]
pub struct Column {
    pub name: String,
    /// `-> 支払日`: a date the row gives, rather than an input.
    pub output: bool,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub span: Span,
    pub cells: Vec<(Lit, Span)>,
}
