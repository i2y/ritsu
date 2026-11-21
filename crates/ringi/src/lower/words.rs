//! The words of the lowered files (DESIGN 6.4): the names a lowered file declares, kept clear of the
//! words rulec and koyomi reserve and of each other; the literals, written as rulec and koyomi read
//! them; and a condition of the form as the rows of a table, the side where it holds and the side
//! where it does not, which ringi writes so that rulec's check of completeness covers both.

use crate::ast::{CmpOp, Expr, ExprKind, Field, Spec, Test, Titled, TypeExpr, Word};
use crate::names::Scope;
use ritsu_units::{Dim, Rat, Unit};
use std::collections::BTreeSet;

/// The words rulec does not take as a name (its `kw::RESERVED`; a test holds the copy to it).
pub const RULEC_RESERVED: &[&str] = &[
    "rule", "description", "import", "enum", "group", "inputs", "elements", "outputs", "derive", "define", "constraint", "table", "fold", "count", "sum", "of", "starts_with", "where", "sequence", "policy",
    "overrides", "clause", "when", "then", "always", "source", "apply", "except", "result", "examples", "range", "round", "contract_only", "default", "not", "none", "true", "false", "min", "max", "up", "down",
    "half_up", "half_even", "half_down", "allocate", "next", "stop", "with", "take_unique", "take_first", "keep_max", "by", "empty", "exhausted", "held", "over", "machine", "scenario", "carry", "initial",
    "final", "never", "after", "once",
];

/// The words rulec does not take as a value of an enum (its `kw::VALUE_RESERVED`).
pub const RULEC_VALUE_RESERVED: &[&str] = &["not", "none", "true", "false", "starts_with", "default", "after", "empty", "exhausted", "by", "with"];

/// The words rulec reads as a type where a type is written (its `kw` of §2.1): an input typed with
/// an enum of that name would be read as the type, so no enum is named with one.
pub const RULEC_TYPES: &[&str] = &["money", "mass", "length", "area", "volume", "duration", "temperature", "sound", "rate", "number", "bool", "string", "date", "incl_tax", "excl_tax"];

/// The words koyomi does not take as a name (its `kw::RESERVED`).
pub const KOYOMI_RESERVED: &[&str] = &[
    "calendar", "dates", "description", "offset", "source", "closed", "open", "use", "inputs", "date", "claims", "examples", "file", "url", "law", "asof", "sha256", "format", "covers", "csv", "govuk",
    "utf8", "shift_jis", "listed", "years", "weekly", "every", "mon", "tue", "wed", "thu", "fri", "sat", "sun", "int", "range", "day", "days", "business", "month", "months", "year", "of", "start", "end",
    "close", "roll", "following", "preceding", "modified", "if", "else", "at", "end_of_month", "start_of_next_month", "reject", "is", "monotonic",
];

/// The words kaname does not take as a name (its `kw::RESERVED`): the glue declares none of them.
/// A field of a record is declared under another name with the word as its ASCII name
/// (`match_value(match)`), which the generated code and the JSON take; kaname reads it after a dot
/// and in a record's braces as it is (`form.match`, `Form { match: … }`).
pub const KANAME_RESERVED: &[&str] = &["let", "try", "if", "then", "else", "match", "some", "none", "true", "false", "and", "or", "not", "in"];

/// The names of the helpers of kaname's generated code (its `kw::HELPERS`), which no name of the
/// generated code can be (kaname's E007), not even an ASCII name in brackets: ringi's E002.
pub const KANAME_HELPERS: &[&str] = &["kaname", "KanameError", "Q", "Date", "Rat", "glue"];

/// The types the code rulec generates for a rule declares, or names by their names alone, in the
/// Pascal case an enum's type is written in (`option` is `Option`): its input, its output, the row
/// that fired, a row of its elements (`Element`, and `<Name>Element` where it walks more than one),
/// its errors, and the standard types of the languages it is generated in that it reads (Rust's
/// `Option` and `Result`, TypeScript's `Object`, Python's `NamedTuple`). An enum of a lowered rule
/// named so would stand for one of them in that code, which then does not compile, so it is named
/// otherwise (`option_value`).
pub const RULEC_MADE_TYPES: &[&str] = &[
    "Input", "Output", "Fired", "Element", "RuleError", "RuleInputError", "RuleContradictionError", "NoValue", "NamedTuple", "Object", "Array", "Map", "Set", "Number", "String", "Boolean", "BigInt", "Symbol",
    "Math", "Error", "Record", "Readonly", "Date", "Promise", "Option", "Result", "Vec", "Box", "Some", "None", "Ok", "Err",
];

/// A name as the code generated for Go writes it, in Pascal case: `a_b` and `aB` are both `AB`, one
/// name there (rulec's W121), and a name another is the same as there is not free.
pub fn spelled(n: &str) -> String {
    ritsu_emit::ident::pascal(n)
}

/// Whether koyomi takes a name as it is, as the name of a file, an input or a date: `[a-z][a-z0-9_]*`
/// (its E010), with no two `_` in a row, which rustc warns of in the code generated for Rust
/// (koyomi's E016 when the code is generated). A name in capitals, one that starts with `_`, or one
/// with `__` in it, needs another there.
pub fn koyomi_takes(n: &str) -> bool {
    let mut cs = n.chars();
    cs.next().is_some_and(|c| c.is_ascii_lowercase()) && cs.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') && !n.contains("__")
}

/// The name koyomi takes for one it does not take as it is: the name in small letters, with `n`
/// before it where it starts with `_`, and two `_` or more in a row as one (`firstDay` is
/// `firstday`, `_x` is `n_x`, `x__y` is `x_y`).
pub fn for_koyomi(n: &str) -> String {
    if koyomi_takes(n) {
        return n.to_string();
    }
    let l = n.to_ascii_lowercase();
    let mut l = if l.starts_with(|c: char| c.is_ascii_lowercase()) { l } else { format!("n{l}") };
    while l.contains("__") {
        l = l.replace("__", "_");
    }
    l
}

/// The name of a lowered file of koyomi's (`dates <name> v1`): the name as it is where koyomi takes
/// it, else with the name it takes in brackets after it (`fooBar_terms(foobar_terms)`), which the
/// code generated from the file is named by. A request or a calendar named in capitals is lowered so.
pub fn koyomi_file(n: &str) -> String {
    if koyomi_takes(n) { n.to_string() } else { format!("{n}({})", for_koyomi(n)) }
}

/// The names one lowered file declares: each name asked for, as it is when it is free, else with
/// a word after it (`next` is `next_value`), so that no two are the same, none is the same as
/// another in Pascal case (`a_b` and `aB`, one name in the code generated for Go), and none is a
/// word the language reserves. A name given in place of another is none of the names the request
/// declares (`avoiding`), so that what a language says of it is said of the one asked for, and of
/// no other.
#[derive(Default)]
pub struct Names {
    taken: BTreeSet<String>,
    /// The names taken, as [`spelled`] writes them.
    spelled: BTreeSet<String>,
    reserved: &'static [&'static [&'static str]],
    /// Whether a name is reserved too where its Pascal case is a type rulec's code makes or reads
    /// ([`RULEC_MADE_TYPES`], or one that ends in `Element`): the names of enums.
    types: bool,
    /// Whether a name is taken as koyomi takes it ([`for_koyomi`]): the names of a file of koyomi's.
    koyomi: bool,
    /// Whether a name that starts with `_` is given another, with `n` before it (`_t` is `n_t`):
    /// the names of a file of rulec's, whose code generated for Python holds a rule's outputs and an
    /// element's fields in a `NamedTuple`, which takes no field whose name starts with `_`.
    lead: bool,
    avoid: BTreeSet<String>,
    renamed: Vec<(String, String)>,
}

impl Names {
    fn of(reserved: &'static [&'static [&'static str]]) -> Names {
        Names { taken: BTreeSet::new(), spelled: BTreeSet::new(), reserved, types: false, koyomi: false, lead: false, avoid: BTreeSet::new(), renamed: Vec::new() }
    }

    pub fn rulec() -> Names {
        Names { lead: true, ..Names::of(&[RULEC_RESERVED]) }
    }

    /// The names of a file of koyomi's: in small letters, as koyomi takes them (`firstDay` is
    /// `firstday`).
    pub fn koyomi() -> Names {
        Names { koyomi: true, ..Names::of(&[KOYOMI_RESERVED]) }
    }

    pub fn kaname() -> Names {
        Names::of(&[KANAME_RESERVED])
    }

    /// The names of the enums of a lowered rule: no word rulec reserves, no type, and none whose
    /// Pascal case is a type rulec's code makes or reads ([`RULEC_MADE_TYPES`]).
    pub fn enums() -> Names {
        Names { types: true, lead: true, ..Names::of(&[RULEC_RESERVED, RULEC_TYPES]) }
    }

    /// The values of one enum of a lowered rule: none of the words rulec reads otherwise in a value's
    /// place.
    pub fn values() -> Names {
        Names { lead: true, ..Names::of(&[RULEC_VALUE_RESERVED]) }
    }

    /// The same, with no name given in place of another being one of `names`.
    pub fn avoiding(mut self, names: &BTreeSet<String>) -> Names {
        self.avoid.extend(names.iter().cloned());
        self
    }

    fn reserves(&self, n: &str) -> bool {
        if self.reserved.iter().any(|l| l.contains(&n)) {
            return true;
        }
        let p = spelled(n);
        self.types && (RULEC_MADE_TYPES.contains(&p.as_str()) || p.ends_with("Element"))
    }

    /// Whether a name is taken, or another taken is the same in Pascal case.
    fn is_taken(&self, n: &str) -> bool {
        self.taken.contains(n) || self.spelled.contains(&spelled(n))
    }

    /// A free name for `want`, taken from now on.
    pub fn take(&mut self, want: &str) -> String {
        let base = match () {
            _ if self.koyomi => for_koyomi(want),
            _ if self.lead && want.starts_with('_') => format!("n{want}"),
            _ => want.to_string(),
        };
        let mut n = base.clone();
        let mut i = 1;
        while self.reserves(&n) || self.is_taken(&n) || (n != want && self.avoid.contains(&n)) {
            n = if i == 1 { format!("{base}_value") } else { format!("{base}_value{i}") };
            i += 1;
        }
        self.keep(&n);
        if n != want {
            self.renamed.push((n.clone(), want.to_string()));
        }
        n
    }

    /// `want` taken as it is (a name the file must declare as it is: a name another file reads it by).
    pub fn keep(&mut self, want: &str) {
        self.taken.insert(want.to_string());
        self.spelled.insert(spelled(want));
    }

    /// Each name given that is not the one asked for (`days_value`), with the one asked for.
    pub fn renamed(&self) -> Vec<(String, String)> {
        self.renamed.clone()
    }

    /// Whether a name is declared.
    pub fn has(&self, n: &str) -> bool {
        self.taken.contains(n)
    }
}

/// The values of an enum a lowered rule writes for the ways to choose an approver, or for the values
/// of its stages' outputs (`wants`: a way's ASCII name, or a stage's value, `a_and_b`), each with the
/// value written for it. A value is the name itself, unless rulec reserves it as a value or a name, a
/// declaration of the rule has the same name (an output cell would read two ways, rulec's E075), a
/// value written as it is before it is the same in Pascal case, or it starts with `_` (an enum of
/// Python's takes no member named `_x_`, and none named `__x`); else the name with `_approver` after
/// it (`after_approver`), or, for one that starts with `_`, with `n` before it (`n_mgr`), and a
/// number after that where that is not free (`after_approver2` for the way `after` beside a way
/// named `after_approver`). A name given in place of another is
/// none of the names the request declares (`request`), none of the rule's and none of the other
/// values, so that each value stands for one way, and the request's name of it is read back from it.
pub fn approver_values(wants: &[String], declared: &Names, request: &BTreeSet<String>) -> Vec<(String, String)> {
    let mut seen: Vec<&String> = Vec::new();
    for w in wants {
        if !seen.contains(&w) {
            seen.push(w);
        }
    }
    let mut values = Names::default();
    // the values written as they are, first: a name given in place of another is none of them
    let mut kept: Vec<&String> = Vec::new();
    for w in &seen {
        if !w.starts_with('_') && !RULEC_VALUE_RESERVED.contains(&w.as_str()) && !RULEC_RESERVED.contains(&w.as_str()) && !declared.has(w) && !values.is_taken(w) {
            values.keep(w);
            kept.push(w);
        }
    }
    let mut out = Vec::new();
    for w in seen {
        if kept.contains(&w) {
            out.push((w.clone(), w.clone()));
            continue;
        }
        let first = if w.starts_with('_') { format!("n{w}") } else { format!("{w}_approver") };
        let mut n = first.clone();
        let mut i = 2;
        while RULEC_RESERVED.contains(&n.as_str()) || declared.has(&n) || request.contains(&n) || values.is_taken(&n) {
            n = format!("{first}{i}");
            i += 1;
        }
        values.keep(&n);
        out.push((w.clone(), n));
    }
    out
}

/// An integer with `_` between each three figures when it has more than four: `30_000`, `1000`.
pub fn grouped(n: i128) -> String {
    let digits = n.unsigned_abs().to_string();
    let sign = if n < 0 { "-" } else { "" };
    if digits.len() <= 4 {
        return format!("{sign}{digits}");
    }
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push('_');
        }
        out.push(c);
    }
    format!("{sign}{out}")
}

/// A number in a unit as rulec writes it: `30_000JPY`, `3%`, `5`.
pub fn number(v: Rat, unit: &Unit) -> String {
    let body = |r: Rat| if r.is_int() { grouped(r.num) } else { crate::lex::decimal(r) };
    match &unit.dim {
        Dim::Rate => format!("{}%", body(v.mul(Rat::int(100)))),
        Dim::Number => body(v),
        _ => format!("{}{}", body(v), unit.unit),
    }
}

/// The smallest step a value of the unit takes: one of the unit (one yen), or a rate's step.
pub fn step(unit: &Unit) -> Rat {
    match (&unit.dim, unit.step) {
        (_, Some(s)) => s,
        (Dim::Rate, None) => Rat::new(1, 100),
        _ => Rat::int(1),
    }
}

/// A constant of a range, a cell or an example: a number, or a day.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Val {
    Num(Rat),
    Day(i64),
}

impl Val {
    pub fn rat(self) -> Rat {
        match self {
            Val::Num(r) => r,
            Val::Day(d) => Rat::int(d as i128),
        }
    }
}

/// The value of a number as written: a rate as the share it is (`3%` is 3/100, as [`number`]
/// writes it back), any other number as it is.
pub fn literal(n: &crate::lex::Num) -> Rat {
    if n.unit == "%" { n.value.div(Rat::int(100)) } else { n.value }
}

/// The constant an expression is: a number, a negative number, a day.
pub fn constant(e: &Expr) -> Option<Val> {
    match &e.kind {
        ExprKind::Num(n) => Some(Val::Num(literal(n))),
        ExprKind::Date(d) => Some(Val::Day(*d)),
        ExprKind::Neg(x) => match constant(x)? {
            Val::Num(r) => Some(Val::Num(Rat::int(0).sub(r))),
            Val::Day(_) => None,
        },
        _ => None,
    }
}

/// A constant counted in the unit of a field of the type `ty`: a number written in another unit of
/// the field's dimension, converted (`500g` for a field of `kg` is 0.5, `100銭` for one of `JPY` is
/// 1), as the form's check reads the ends of a range ([`crate::form`]); any other constant as
/// [`constant`] reads it. What is written back in the field's unit ([`written`]) is then the same
/// amount.
pub fn constant_in(e: &Expr, ty: &TypeExpr) -> Option<Val> {
    let TypeExpr::Num(to) = ty else { return constant(e) };
    let (n, neg) = match &e.kind {
        ExprKind::Num(n) => (n, false),
        ExprKind::Neg(x) => match &x.kind {
            ExprKind::Num(n) => (n, true),
            _ => return constant(e),
        },
        _ => return constant(e),
    };
    let v = if neg { Rat::int(0).sub(literal(n)) } else { literal(n) };
    let from = crate::types::lit_unit(n);
    if from.dim != to.dim || n.unit.is_empty() || n.unit == "%" {
        return Some(Val::Num(v));
    }
    Some(Val::Num(v.mul(from.factor()).add(from.offset()).sub(to.offset()).div(to.factor())))
}

/// A constant written in the type of a field: a number in the field's unit, or a date.
pub fn written(v: Val, ty: &TypeExpr) -> String {
    match (v, ty) {
        (Val::Day(d), _) => crate::lex::date_text(d),
        (Val::Num(r), TypeExpr::Num(u)) => number(r, u),
        (Val::Num(r), _) => number(r, &Unit { dim: Dim::Number, unit: String::new(), tax: None, step: None }),
    }
}

/// The ends of a field's range as written: `>=1JPY <=10_000_000JPY`. Empty when none is written.
pub fn range_text(f: &Field) -> String {
    let Some(b) = &f.range else { return String::new() };
    let mut parts = Vec::new();
    if let Some(lo) = &b.lo
        && let Some(v) = constant_in(&lo.value, &f.ty)
    {
        parts.push(format!("{}{}", if lo.inclusive { ">=" } else { ">" }, written(v, &f.ty)));
    }
    if let Some(hi) = &b.hi
        && let Some(v) = constant_in(&hi.value, &f.ty)
    {
        parts.push(format!("{}{}", if hi.inclusive { "<=" } else { "<" }, written(v, &f.ty)));
    }
    parts.join(" ")
}

/// The ends of an interval of values, both in: what a range comes to with its marks read (`>1JPY`
/// is `>=2JPY` for an amount in whole yen).
pub fn ends(f: &Field) -> (Option<Rat>, Option<Rat>) {
    let st = match &f.ty {
        TypeExpr::Num(u) => step(u),
        _ => Rat::int(1),
    };
    let Some(b) = &f.range else { return (None, None) };
    let lo = b.lo.as_ref().and_then(|l| constant_in(&l.value, &f.ty).map(|v| if l.inclusive { v.rat() } else { v.rat().add(st) }));
    let hi = b.hi.as_ref().and_then(|h| constant_in(&h.value, &f.ty).map(|v| if h.inclusive { v.rat() } else { v.rat().sub(st) }));
    (lo, hi)
}

/// The value a stage's cell or line names, as the lowered rule writes it, and how it reads: one way
/// to choose (`manager`), several joined (`dept_head_and_accounting`, `dept_head and accounting`),
/// a quorum (`at_least_60_percent_of_board`), a stage that circulates or carries a note. Names are
/// the ASCII names of the ways to choose and the flows. None for an empty cell, a rule's output
/// (its values are the rule's) and a shared route.
pub fn spec_value(s: &Scope, spec: &Spec) -> Option<(String, String)> {
    let who = |w: &Word| ascii_of_who(s, &w.text);
    let shown = spec.shown();
    let v = match spec {
        // `a and b` and `b and a` are the same approvers: named in the order `who` declares them
        Spec::Who(ws) => {
            let all = s.who_all();
            let mut names: Vec<(usize, String)> = ws.iter().map(|w| (all.iter().position(|x| x.who.name.text == w.text || x.who.name.ascii() == w.text).unwrap_or(usize::MAX), who(w))).collect();
            names.sort();
            names.into_iter().map(|(_, n)| n).collect::<Vec<_>>().join("_and_")
        }
        Spec::Quorum(crate::ast::Quorum::All, w, _) => format!("all_of_{}", who(w)),
        Spec::Quorum(crate::ast::Quorum::AtLeast(n), w, _) => format!("at_least_{n}_of_{}", who(w)),
        Spec::Quorum(crate::ast::Quorum::Percent(n), w, _) => format!("at_least_{n}_percent_of_{}", who(w)),
        Spec::Circulate(w) => format!("circulate_to_{}", who(w)),
        Spec::Note(w) => format!("note_from_{}", alias_ascii(s, &w.text)),
        Spec::Empty | Spec::RuleOutput(_) | Spec::Route(_) => return None,
    };
    Some((v, shown))
}

/// The ASCII name of a way to choose an approver, by its name as written (the file's own or a shared
/// route's); the name itself when it is not declared.
pub fn ascii_of_who(s: &Scope, name: &str) -> String {
    s.who_all().iter().find(|w| w.who.name.text == name || w.who.name.ascii() == name).map(|w| w.who.name.ascii().to_string()).unwrap_or_else(|| name.to_string())
}

/// The ASCII name of a `use` line's alias.
pub fn alias_ascii(s: &Scope, name: &str) -> String {
    s.file.uses.iter().filter_map(|u| u.alias.as_ref()).find(|a| a.text == name).map(|a| a.ascii().to_string()).unwrap_or_else(|| name.to_string())
}

/// The ASCII name of a value of a choice, by its name as written: the value the JSON carries.
pub fn choice_ascii(f: &Field, name: &str) -> String {
    f.options.iter().find(|o| o.name.text == name).map(|o| o.name.ascii().to_string()).unwrap_or_else(|| name.to_string())
}

/// Every ASCII name the request declares: its fields and their rows' fields, the values of its
/// choices, its stages, its ways to choose an approver, its tables and their columns and the aliases
/// of its `use` lines. A name the lowered files give in place of another is none of them.
pub fn declared(s: &Scope) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for f in s.fields() {
        out.insert(f.name.ascii().to_string());
        out.extend(f.options.iter().map(|o| o.name.ascii().to_string()));
        for c in &f.children {
            out.insert(c.name.ascii().to_string());
            out.extend(c.options.iter().map(|o| o.name.ascii().to_string()));
        }
    }
    out.extend(s.stages().unwrap_or_default().iter().map(|n| n.ascii().to_string()));
    out.extend(s.who_all().iter().map(|w| w.who.name.ascii().to_string()));
    for t in &s.file.tables {
        out.insert(t.name.ascii().to_string());
        out.extend(t.inputs.iter().chain(&t.outputs).map(|c| c.ascii().to_string()));
    }
    out.extend(s.file.uses.iter().filter_map(|u| u.alias.as_ref()).map(|a| a.ascii().to_string()));
    out
}

/// The values of a choice as the lowered files write them, each with its ASCII name: the ASCII name,
/// unless rulec reads the word in a value's place as something else (`after`, `by`, `empty`), when
/// `_value` follows it (DESIGN 6.4).
pub fn lowered_values(s: &Scope, f: &Field) -> Vec<(String, String)> {
    let mut names = Names::values().avoiding(&declared(s));
    f.options.iter().map(|o| (o.name.ascii().to_string(), names.take(o.name.ascii()))).collect()
}

/// A value of a choice as the lowered files write it, by its name as written or its ASCII name.
pub fn choice_value(s: &Scope, f: &Field, name: &str) -> String {
    let ascii = choice_ascii(f, name);
    lowered_values(s, f).into_iter().find(|(a, _)| *a == ascii).map(|(_, l)| l).unwrap_or(ascii)
}

/// The value of a choice a lowered file writes `lowered` for: its name as written. None for none of
/// them.
pub fn choice_written<'f>(s: &Scope, f: &'f Field, lowered: &str) -> Option<&'f Titled> {
    let ascii = lowered_values(s, f).into_iter().find(|(_, l)| l == lowered).map(|(a, _)| a)?;
    f.options.iter().find(|o| o.name.ascii() == ascii)
}

/// Each value of a choice the lowered files write in place of its ASCII name, with that name: what a
/// lowered file's messages are said back in.
pub fn values_renamed(s: &Scope) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for f in s.fields().iter().filter(|f| matches!(f.ty, TypeExpr::Choice)) {
        out.extend(lowered_values(s, f).into_iter().filter(|(a, l)| a != l).map(|(a, l)| (l, a)));
    }
    out
}

/// What a column of a table of conditions reads.
#[derive(Clone, Debug, PartialEq)]
pub enum ColKind {
    /// A number or a date, between its ends (None where it is open), on its step, written in its
    /// type.
    Ordered { lo: Option<Rat>, hi: Option<Rat>, step: Rat, ty: TypeExpr },
    /// One of these values.
    Values(Vec<String>),
    Bool,
}

/// A column of a table of conditions: the input's name in the lowered rule, and what it reads.
/// `opt`: the value may not be there (`T?`, DESIGN 4.9), which is a value of its own, `none`.
#[derive(Clone, Debug, PartialEq)]
pub struct Col {
    pub name: String,
    pub kind: ColKind,
    pub opt: bool,
}

/// One cut of an ordered column: `> c` (strict) or `>= c`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cut {
    at: Rat,
    strict: bool,
}

/// A cell of a row: the values of its column it takes.
#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
    /// From `lo` to `hi`: each end None where it is the column's own, else the value and whether
    /// it is in.
    Between(Option<(Rat, bool)>, Option<(Rat, bool)>),
    Values(Vec<String>),
    Bool(bool),
    Any,
    /// No value (`none`), in a column whose value may not be there.
    None,
    /// No value, or a value other than these (`not: a, b`), in a column whose value may not be
    /// there: rulec's `not:` takes `none` too.
    NotValues(Vec<String>),
}

/// An atom of a condition, read against the columns. A value that is not there makes every atom
/// false but [`Atom::Absent`] (DESIGN 4.9).
#[derive(Clone, Debug)]
pub enum Atom {
    /// `col op value`.
    Cmp(usize, CmpOp, Rat),
    /// `col is v1, v2` (negated with `not`).
    In(usize, Vec<String>),
    /// A bool column is true.
    True(usize),
    /// The column has no value (`is none`).
    Absent(usize),
}

/// A condition over columns, as the lowered rule reads it.
#[derive(Clone, Debug)]
pub enum Cond {
    Atom(Atom),
    And(Box<Cond>, Box<Cond>),
    Or(Box<Cond>, Box<Cond>),
    Not(Box<Cond>),
    Const(bool),
}

impl Cond {
    /// Whether the condition holds on a row of cells, each with a value inside it (every atom has
    /// one answer on a whole cell, so the value stands for the cell).
    fn eval(&self, row: &[(Cell, Option<Rat>)]) -> bool {
        match self {
            Cond::Const(b) => *b,
            Cond::And(a, b) => a.eval(row) && b.eval(row),
            Cond::Or(a, b) => a.eval(row) || b.eval(row),
            Cond::Not(a) => !a.eval(row),
            Cond::Atom(Atom::True(c)) => matches!(row[*c].0, Cell::Bool(true)),
            Cond::Atom(Atom::Absent(c)) => matches!(row[*c].0, Cell::None),
            Cond::Atom(Atom::In(c, vs)) => match &row[*c].0 {
                Cell::Values(x) => vs.contains(&x[0]),
                _ => false,
            },
            Cond::Atom(Atom::Cmp(c, op, v)) => {
                use std::cmp::Ordering::*;
                let Some(x) = row[*c].1 else { return false };
                let o = x.cmp_to(*v);
                match op {
                    CmpOp::Lt => o == Less,
                    CmpOp::Le => o != Greater,
                    CmpOp::Gt => o == Greater,
                    CmpOp::Ge => o != Less,
                    CmpOp::Is => o == Equal,
                    CmpOp::IsNot => o != Equal,
                }
            }
        }
    }

    /// The atoms, each column's cuts and values.
    fn atoms<'c>(&'c self, out: &mut Vec<&'c Atom>) {
        match self {
            Cond::Atom(a) => out.push(a),
            Cond::And(a, b) | Cond::Or(a, b) => {
                a.atoms(out);
                b.atoms(out);
            }
            Cond::Not(a) => a.atoms(out),
            Cond::Const(_) => {}
        }
    }
}

/// The cells of each column the atoms cut it into: the intervals between an ordered column's cuts,
/// each value of a column of values, and true and false; and `none` where the value may not be
/// there.
fn cells_of(cols: &[Col], cond: &Cond) -> Vec<Vec<(Cell, Option<Rat>)>> {
    let mut atoms = Vec::new();
    cond.atoms(&mut atoms);
    cols.iter()
        .enumerate()
        .map(|(i, c)| {
            let mut cells = cells_of_kind(i, c, &atoms);
            if c.opt {
                cells.push((Cell::None, None));
            }
            cells
        })
        .collect()
}

/// The cells of the values a column takes (no `none`).
fn cells_of_kind(i: usize, c: &Col, atoms: &[&Atom]) -> Vec<(Cell, Option<Rat>)> {
    match &c.kind {
        ColKind::Bool => vec![(Cell::Bool(true), None), (Cell::Bool(false), None)],
        ColKind::Values(vs) => vs.iter().map(|v| (Cell::Values(vec![v.clone()]), None)).collect(),
        ColKind::Ordered { lo, hi, step, .. } => {
            let mut cuts: Vec<Cut> = atoms
                .iter()
                .filter_map(|a| match a {
                    Atom::Cmp(j, op, v) if *j == i => Some(match op {
                        CmpOp::Gt | CmpOp::Le => Cut { at: *v, strict: true },
                        _ => Cut { at: *v, strict: false },
                    }),
                    _ => None,
                })
                // a cut at or past an end of the column cuts nothing off
                .filter(|c| lo.is_none_or(|l| c.at.cmp_to(l) == std::cmp::Ordering::Greater || (c.at.cmp_to(l) == std::cmp::Ordering::Equal && c.strict)))
                .filter(|c| hi.is_none_or(|h| c.at.cmp_to(h) == std::cmp::Ordering::Less || (c.at.cmp_to(h) == std::cmp::Ordering::Equal && !c.strict)))
                .collect();
            cuts.sort_by(|a, b| a.at.cmp_to(b.at).then((!a.strict).cmp(&!b.strict).reverse()));
            cuts.dedup();
            // between the cuts; `> c` cuts after c (c is in the cell below), `>= c` before it
            let inside = |below: Option<(Rat, bool)>, top: Option<(Rat, bool)>| match (below, top) {
                (Some((x, inc)), _) => Some(if inc { x } else { x.add(*step) }),
                (None, Some((y, inc))) => Some(if inc { y } else { y.sub(*step) }),
                (None, None) => Some(lo.or(*hi).unwrap_or(Rat::int(0))),
            };
            let mut out = Vec::new();
            let mut below: Option<(Rat, bool)> = None;
            for c in &cuts {
                let top = Some((c.at, c.strict));
                out.push((Cell::Between(below, top), inside(below, top)));
                below = Some((c.at, !c.strict));
            }
            out.push((Cell::Between(below, None), inside(below, None)));
            out
        }
    }
}

/// The rows of a table that decides the condition: each a cell for every column, and whether the
/// condition holds there. The rows take every input once (rulec's check of completeness and overlap
/// sees both sides), and rows that differ in one column only are joined into one.
pub fn rows(cols: &[Col], cond: &Cond) -> Vec<(Vec<Cell>, bool)> {
    let cells = cells_of(cols, cond);
    let mut rows: Vec<(Vec<Cell>, bool)> = Vec::new();
    let mut idx = vec![0usize; cols.len()];
    loop {
        let row: Vec<(Cell, Option<Rat>)> = idx.iter().enumerate().map(|(c, i)| cells[c][*i].clone()).collect();
        let holds = cond.eval(&row);
        rows.push((row.into_iter().map(|(c, _)| c).collect(), holds));
        // the next combination
        let mut c = cols.len();
        loop {
            if c == 0 {
                return join(cols, rows);
            }
            c -= 1;
            idx[c] += 1;
            if idx[c] < cells[c].len() {
                break;
            }
            idx[c] = 0;
        }
        if cols.is_empty() {
            return join(cols, rows);
        }
    }
}

/// Rows that differ in one column only, with the same answer, joined: adjacent intervals into one,
/// values into a list; a cell that takes its whole column is `-`.
fn join(cols: &[Col], mut rows: Vec<(Vec<Cell>, bool)>) -> Vec<(Vec<Cell>, bool)> {
    loop {
        let mut joined = false;
        'outer: for c in (0..cols.len()).rev() {
            for i in 0..rows.len() {
                for j in i + 1..rows.len() {
                    if rows[i].1 != rows[j].1 || (0..cols.len()).any(|k| k != c && rows[i].0[k] != rows[j].0[k]) {
                        continue;
                    }
                    if let Some(u) = union(&cols[c], &rows[i].0[c], &rows[j].0[c]) {
                        rows[i].0[c] = u;
                        rows.remove(j);
                        joined = true;
                        break 'outer;
                    }
                }
            }
        }
        if !joined {
            break;
        }
    }
    // a cell that takes every value is `-`; where the value may not be there, only a cell that takes
    // `none` too (`-` takes it in rulec)
    for (cells, _) in rows.iter_mut() {
        for (c, cell) in cells.iter_mut().enumerate() {
            let whole = !cols[c].opt
                && match (&cols[c].kind, &*cell) {
                    (ColKind::Values(all), Cell::Values(vs)) => vs.len() == all.len(),
                    (_, Cell::Between(None, None)) => true,
                    _ => false,
                };
            if whole {
                *cell = Cell::Any;
            }
        }
    }
    rows
}

/// Two cells of the column `col` as one, where one cell can write both.
fn union(col: &Col, a: &Cell, b: &Cell) -> Option<Cell> {
    let all = match &col.kind {
        ColKind::Values(all) => all.as_slice(),
        _ => &[],
    };
    // the values of the column other than these, in its order
    let rest = |vs: &[String]| all.iter().filter(|v| !vs.contains(v)).cloned().collect::<Vec<_>>();
    match (a, b) {
        (Cell::Values(x), Cell::Values(y)) => Some(Cell::Values(x.iter().chain(y).cloned().collect())),
        (Cell::Between(alo, Some(ahi)), Cell::Between(Some(blo), bhi)) if ahi.0.cmp_to(blo.0) == std::cmp::Ordering::Equal && ahi.1 != blo.1 => Some(Cell::Between(*alo, *bhi)),
        (Cell::Between(Some(alo), ahi), Cell::Between(blo, Some(bhi))) if bhi.0.cmp_to(alo.0) == std::cmp::Ordering::Equal && bhi.1 != alo.1 => Some(Cell::Between(*blo, *ahi)),
        // no value, beside values: `not:` the others, or every value and none (`-`)
        (Cell::Values(x), Cell::None) | (Cell::None, Cell::Values(x)) if col.opt && !all.is_empty() => {
            let r = rest(x);
            Some(if r.is_empty() { Cell::Any } else { Cell::NotValues(r) })
        }
        (Cell::NotValues(r), Cell::Values(x)) | (Cell::Values(x), Cell::NotValues(r)) if col.opt => {
            let r: Vec<String> = r.iter().filter(|v| !x.contains(v)).cloned().collect();
            Some(if r.is_empty() { Cell::Any } else { Cell::NotValues(r) })
        }
        (Cell::Between(None, None), Cell::None) | (Cell::None, Cell::Between(None, None)) if col.opt => Some(Cell::Any),
        _ => None,
    }
}

/// A cell as the lowered table writes it, in the column's type.
pub fn cell_text(col: &Col, cell: &Cell) -> String {
    match cell {
        Cell::Any => "-".into(),
        Cell::None => "none".into(),
        Cell::Bool(b) => b.to_string(),
        Cell::NotValues(rest) => {
            let all = match &col.kind {
                ColKind::Values(all) => all.clone(),
                _ => rest.clone(),
            };
            format!("not: {}", all.iter().filter(|v| rest.contains(v)).cloned().collect::<Vec<_>>().join(", "))
        }
        // every value the column takes, where `-` would take `none` too
        Cell::Between(None, None) if col.opt => {
            let whole = match &col.kind {
                ColKind::Ordered { lo, hi, .. } => Cell::Between(lo.map(|l| (l, true)), hi.map(|h| (h, true))),
                _ => Cell::Any,
            };
            if whole == Cell::Between(None, None) { "-".into() } else { cell_text(col, &whole) }
        }
        Cell::Values(vs) => {
            if let ColKind::Values(all) = &col.kind {
                let rest: Vec<&String> = all.iter().filter(|v| !vs.contains(v)).collect();
                // `not:` takes `none` too, so a column whose value may not be there lists its values
                if vs.len() > 1 && rest.len() == 1 && !col.opt {
                    return format!("not: {}", rest[0]);
                }
                // in the column's order
                return all.iter().filter(|v| vs.contains(v)).cloned().collect::<Vec<_>>().join(", ");
            }
            vs.join(", ")
        }
        Cell::Between(lo, hi) => {
            let ty = match &col.kind {
                ColKind::Ordered { ty, .. } => ty.clone(),
                _ => TypeExpr::Num(Unit { dim: Dim::Number, unit: String::new(), tax: None, step: None }),
            };
            let day = matches!(ty, TypeExpr::Date);
            let w = |r: Rat| written(if day { Val::Day(r.num as i64) } else { Val::Num(r) }, &ty);
            let mut parts = Vec::new();
            if let Some((x, inc)) = lo {
                parts.push(format!("{}{}", if *inc { ">=" } else { ">" }, w(*x)));
            }
            if let Some((y, inc)) = hi {
                parts.push(format!("{}{}", if *inc { "<=" } else { "<" }, w(*y)));
            }
            parts.join(" ")
        }
    }
}

/// A test of a route's cell, as the lowered table writes it: names of choices in ASCII, numbers in
/// the column's unit.
pub fn test_text(s: &Scope, column: Option<&Field>, t: &Test) -> String {
    match t {
        Test::Any => "-".into(),
        Test::Values { values, not } => {
            let vs: Vec<String> = values.iter().map(|v| value_text(s, column, v)).collect();
            format!("{}{}", if *not { "not: " } else { "" }, vs.join(", "))
        }
        Test::Compare(cmps) => cmps
            .iter()
            .map(|(op, e)| {
                let v = match (column.map_or_else(|| constant(e), |f| constant_in(e, &f.ty)), column) {
                    (Some(v), Some(f)) => written(v, &f.ty),
                    (Some(v), None) => written(v, &TypeExpr::Num(Unit { dim: Dim::Number, unit: String::new(), tax: None, step: None })),
                    (None, _) => value_text(s, column, e),
                };
                format!("{}{v}", op.sign())
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// A value written in a cell or an example: a choice's value in ASCII, a number in the column's
/// unit, a date, `none`, `true` or `false`.
pub fn value_text(s: &Scope, column: Option<&Field>, e: &Expr) -> String {
    let _ = s;
    match &e.kind {
        ExprKind::None => "none".into(),
        ExprKind::Bool(b) => b.to_string(),
        ExprKind::Path(p) if p.len() == 1 => match column {
            Some(f) if matches!(f.ty, TypeExpr::Choice) => choice_value(s, f, &p[0].text),
            _ => p[0].text.clone(),
        },
        ExprKind::Str(t) => format!("\"{t}\""),
        _ => match (column.map_or_else(|| constant(e), |f| constant_in(e, &f.ty)), column) {
            (Some(v), Some(f)) => written(v, &f.ty),
            (Some(v), None) => written(v, &TypeExpr::Num(Unit { dim: Dim::Number, unit: String::new(), tax: None, step: None })),
            (None, _) => String::new(),
        },
    }
}
