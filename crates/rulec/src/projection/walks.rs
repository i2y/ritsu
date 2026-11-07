//! What the reads of one collection can say together (§15.281).
//!
//! `any`, `all`, `count`, `sum`, `max` and `min` each walk a collection on their own, and the
//! check of a table reads what comes out as columns that have nothing to do with each other:
//! it does not know that `has_tool = any lines where category = tool` and
//! `all_parts = all lines where category = part` cannot both be true. §3.3 keeps it that way on
//! purpose, because the targets that do not read `from` take whatever a caller worked out by
//! hand. This module is where the definitions are read after all, for what they say beside the
//! table: a `constraint` the contract does not keep (E123), a gap no definition can reach (the
//! note on E101), and a row none can (W133).
//!
//! **The question is linear.** The elements of a collection fall into classes by the values the
//! tests written over it tell apart, and each class has a whole number of elements. `any` is that
//! the classes that pass hold at least one, `all` that the ones that fail hold none, `count` adds
//! up the classes that pass, and `sum` adds up what each class's values can come to. On an empty
//! collection that gives `all` true, `any` false and `count` 0 with nothing further said. `max` and
//! `min` are the one read that is not linear, and they are opened into cases: no element passes
//! and the read is its `empty` value, or the extreme lies in this part of the field's values and
//! nothing that passes lies beyond it. All of it goes to `Relation::find` together with what the
//! contract says across its fields, in the same whole numbers E123 already asks about, so no new
//! solver is added.
//!
//! **Each answer says only what it has shown.** [`Possible::No`] comes from a system that is
//! wider than the truth, never narrower: a form this does not read adds nothing to it. And
//! [`Possible::Yes`] comes from elements that were built and then evaluated, not from a point of
//! the system, because the system is wider. Whatever is left is [`Possible::Unknown`], and a
//! caller says nothing on it. The tests in `tests/from_relations.rs` hold both directions to every
//! small collection there is.
//!
//! **A question is asked in three passes.** Elimination grows quickly with the names in a system,
//! and a system past its limit answers nothing. So the first pass has only the rows that touch what
//! the question names, and it decides almost everything; a `No` there holds of the whole system
//! too, since the whole has more rows. When that pass ends with neither answer, the second has
//! every row. What is left then is tried among small collections, when the question names one walk,
//! which is how a `sum` beside a `max` is found: those two are not linear together. A search that
//! finds nothing proves nothing, so it can only say `Yes`.

use super::{Across, Contract, Ints, proto_ints, proto_strs, read, wire_constraint};
use crate::ast::{AggDecl, AggKind, Cell, CmpOp, EnumSource, Item, Lit, ProjKind, RuleFile, ShapeDecl, VarDecl};
use crate::diag::{Diag, FixKind, WVal};
use crate::fourier::{Ineq, Lin};
use crate::num::Rat;
use crate::relation::{Extra, Found, Proof, Relation, Term};
use crate::types::{Checked, Ty};
use std::collections::{BTreeMap, BTreeSet};

/// The classes one collection is read in before it is called unreadable: the product of what its
/// fields are cut into.
const CLASSES: usize = 256;
/// The cases `max` and `min` open a question into.
const WAYS: usize = 256;
/// The elements an example may hold. Past it the example is not one a person reads anyway.
const MOST: i128 = 4096;
/// How far a search among small collections goes: the elements of a collection, the different
/// elements it picks them from, and the collections it tries for one question.
const SEARCH_ELEMENTS: usize = 4;
const SEARCH_KINDS: usize = 64;
const SEARCH_TRIES: usize = 4096;

/// What a search for a combination found.
#[derive(Debug, Clone)]
pub enum Possible {
    /// It occurs, and this is a collection that gives it: built and evaluated.
    Yes(Example),
    /// It does not, whichever elements there are: a case of the contract, or a case of a `max` or
    /// a `min`, fails each way. The proofs are what a certificate would carry.
    No(#[allow(dead_code)] Vec<Proof>),
    /// Neither could be shown.
    Unknown,
}

/// What a combination asks of a value that is not a number. A number is asked with an inequality
/// over its name, at the scale it travels at.
#[derive(Debug, Clone)]
pub enum Ask {
    Truth(bool),
    Words(Vec<String>),
}

/// A request that gives a combination.
#[derive(Debug, Clone, Default)]
pub struct Example {
    /// The number each name read comes to, at the scale it travels at.
    pub values: BTreeMap<String, Rat>,
    /// What each `any` and `all` comes to.
    pub truths: BTreeMap<String, bool>,
    /// The fields read directly, as a message writes them, and their values.
    pub plain: Vec<(String, String)>,
    /// What each collection the question named holds.
    pub walks: Vec<Shown>,
}

/// One collection of an example.
#[derive(Debug, Clone)]
pub struct Shown {
    pub path: String,
    /// How many elements of each kind: their count, and the fields the question reads.
    pub elements: Vec<(i128, Vec<(String, String)>)>,
}

impl Example {
    /// The request in words: `` `order.lines` holding 1 element with `category` = "service" ``.
    pub fn request(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        for w in &self.walks {
            let p = &w.path;
            if w.elements.is_empty() {
                parts.push(tr!("`{p}` が空", "an empty `{p}`"));
                continue;
            }
            let kinds: Vec<String> = w
                .elements
                .iter()
                .map(|(n, fields)| {
                    let what = fields.iter().map(|(k, v)| tr!("`{k}` が {v}", "`{k}` = {v}")).collect::<Vec<_>>();
                    let element = if *n == 1 { "element" } else { "elements" };
                    if what.is_empty() {
                        tr!("要素が {n} 件", "{n} {element}")
                    } else {
                        let what_ja = what.join("、");
                        let what_en = what.join(" and ");
                        tr!("{what_ja} の要素が {n} 件", "{n} {element} with {what_en}")
                    }
                })
                .collect();
            let kinds_ja = kinds.join(" と ");
            let kinds_en = kinds.join(" and ");
            parts.push(tr!("`{p}` に {kinds_ja}", "`{p}` holding {kinds_en}"));
        }
        for (p, v) in &self.plain {
            parts.push(tr!("`{p}` が {v}", "`{p}` at {v}"));
        }
        if crate::i18n::ja() {
            parts.join("、")
        } else {
            match parts.split_last() {
                Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
                _ => parts.join(""),
            }
        }
    }
}

/// What a field of an element can hold, as the check can read it.
#[derive(Debug, Clone)]
enum Dom {
    Truth,
    /// Strings: the ones listed, or any string at all. `numbers` gives the number of each value of
    /// a `.proto` enum, which is what a CEL expression reads it as.
    Words { listed: Option<Vec<String>>, numbers: Vec<(String, i128)> },
    /// Whole numbers, or a date as its day number.
    Whole(Ints),
    /// A number with a fraction, between two ends with whether each is left out.
    Frac { lo: Option<(Rat, bool)>, hi: Option<(Rat, bool)> },
    /// Something this does not read.
    Wide,
}

/// One value of a field.
#[derive(Debug, Clone, PartialEq)]
enum Val {
    Truth(bool),
    Word(String),
    Num(Rat),
}

/// The test of a `where`, on one value.
#[derive(Debug, Clone)]
enum Test {
    /// One of these, or with `false` none of them.
    Among(Vec<Val>, bool),
    /// All of these comparisons.
    Cmp(Vec<(CmpOp, Rat)>),
}

/// A number a test compares with, as the generated code writes it: the digits of the cell. A
/// literal with a unit or a multiplier is not read.
fn number_of(n: &crate::lex::Num) -> Option<Rat> {
    if n.unit.is_some() || n.mult != 1 {
        return None;
    }
    let den = 10i128.checked_pow(u32::try_from(n.frac.len()).ok()?)?;
    let whole: i128 = format!("{}{}", n.digits, n.frac).parse().ok()?;
    Some(Rat::new(if n.neg { -whole } else { whole }, den))
}

fn val_of(l: &Lit) -> Option<(Val, bool)> {
    Some(match l {
        Lit::Word(w) if w == crate::kw::TRUE => (Val::Truth(true), false),
        Lit::Word(w) if w == crate::kw::FALSE => (Val::Truth(false), false),
        Lit::Word(w) | Lit::Str(w) => (Val::Word(w.clone()), false),
        Lit::Num(n) => (Val::Num(number_of(n)?), false),
        Lit::Date(y, m, d) => (Val::Num(crate::types::date_ord(*y, *m, *d)), true),
    })
}

impl Test {
    /// The test a cell writes, and whether it compares dates. `None` for a cell this does not
    /// read: `starts_with`, `none`, and a cell that mixes dates with anything else.
    fn of_cell(cell: &Cell) -> Option<(Test, bool)> {
        let among = |ls: &[Lit], yes: bool| -> Option<(Test, bool)> {
            let vs = ls.iter().map(val_of).collect::<Option<Vec<_>>>()?;
            let dates = vs.iter().filter(|(_, d)| *d).count();
            if dates != 0 && dates != vs.len() {
                return None;
            }
            Some((Test::Among(vs.into_iter().map(|(v, _)| v).collect(), yes), dates != 0))
        };
        match cell {
            Cell::DontCare => Some((Test::Among(Vec::new(), false), false)),
            Cell::Lit(l) => among(std::slice::from_ref(l), true),
            Cell::Set(ls) => among(ls, true),
            Cell::Not(ls) => among(ls, false),
            Cell::Cmp(ops) => {
                let mut out = Vec::new();
                let mut dates = 0;
                for (o, l) in ops {
                    let (v, d) = val_of(l)?;
                    let Val::Num(n) = v else { return None };
                    dates += usize::from(d);
                    out.push((*o, n));
                }
                if dates != 0 && dates != out.len() {
                    return None;
                }
                Some((Test::Cmp(out), dates != 0))
            }
            _ => None,
        }
    }

    fn holds(&self, v: &Val) -> bool {
        match self {
            Test::Among(vs, yes) => vs.contains(v) == *yes,
            Test::Cmp(ops) => {
                let Val::Num(x) = v else { return false };
                ops.iter().all(|(o, k)| {
                    let c = x.cmp_to(*k);
                    match o {
                        CmpOp::Le => c.is_le(),
                        CmpOp::Lt => c.is_lt(),
                        CmpOp::Ge => c.is_ge(),
                        CmpOp::Gt => c.is_gt(),
                    }
                })
            }
        }
    }

    /// The numbers it compares with.
    fn numbers(&self) -> Vec<Rat> {
        match self {
            Test::Among(vs, _) => vs.iter().filter_map(|v| if let Val::Num(n) = v { Some(*n) } else { None }).collect(),
            Test::Cmp(ops) => ops.iter().map(|(_, k)| *k).collect(),
        }
    }

    /// Whether it can be asked of a value of this kind.
    fn fits(&self, dom: &Dom) -> bool {
        match (self, dom) {
            (_, Dom::Wide) => false,
            (Test::Cmp(_), Dom::Whole(_) | Dom::Frac { .. }) => true,
            (Test::Cmp(_), _) => false,
            (Test::Among(vs, _), Dom::Truth) => vs.iter().all(|v| matches!(v, Val::Truth(_))),
            (Test::Among(vs, _), Dom::Words { .. }) => vs.iter().all(|v| matches!(v, Val::Word(_))),
            (Test::Among(vs, _), Dom::Whole(_) | Dom::Frac { .. }) => vs.iter().all(|v| matches!(v, Val::Num(_))),
        }
    }
}

/// One stretch of a field's values that every test on the field answers the same way.
#[derive(Debug, Clone)]
struct Part {
    /// What each test on the field answers here.
    truth: Vec<bool>,
    /// The values in it, where they are few.
    vals: Vec<Val>,
    /// A string no test names is in it, as well.
    other: bool,
    /// Its ends, on a whole number or a date.
    lo: Option<Rat>,
    hi: Option<Rat>,
    sample: Val,
}

/// A field of an element that some read looks at.
#[derive(Debug, Clone)]
struct Field {
    /// How a message writes it: `category`.
    name: String,
    /// The path from the element, when a contract gives one that can be asked of its condition.
    path: Option<Vec<String>>,
    dom: Dom,
    date: bool,
    /// Values here cannot be shown to pass: the rules on the field are not read, or a table works
    /// them out. Elements with such values take part in a `No`, and never in a `Yes`.
    unsure: bool,
    /// The tests on it. `None` where the test was not read.
    tests: Vec<Option<Test>>,
    /// Read by `sum`, `max` or `min`.
    of: bool,
    /// Read by `sum`.
    summed: bool,
    parts: Vec<Part>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Any,
    All,
    Count,
    Sum,
    Max,
    Min,
}

/// One read of a collection: an input, or a summary.
#[derive(Debug, Clone)]
struct Read {
    name: String,
    kind: Kind,
    /// The field and the test on it.
    test: Option<(usize, usize)>,
    of: Option<usize>,
    /// The value `max` and `min` give when no element passes.
    empty: Option<Rat>,
    /// Whether each class passes.
    pass: Vec<bool>,
    readable: bool,
    /// The definition as it is written.
    said: String,
    /// Read through `from`, rather than summed up from a sequence.
    from: bool,
}

/// One collection, or one sequence with no contract.
#[derive(Debug, Clone)]
struct Walk {
    shown: String,
    /// The name its length goes by.
    size: String,
    cap: Option<i128>,
    /// What the contract says across the fields of one element.
    each: Relation,
    /// Whether its fields cut into few enough classes to read it at all.
    fine: bool,
    fields: Vec<Field>,
    /// The classes, each one part of every field.
    classes: Vec<Vec<usize>>,
    /// For each field that is summed, the classes no read can tell apart, put together: what one
    /// total stands for.
    cells: Vec<Vec<Vec<usize>>>,
    reads: Vec<Read>,
}

impl Walk {
    fn n(wi: usize, ci: usize) -> String {
        format!("\u{1}n:{wi}:{ci}")
    }

    fn s(wi: usize, fi: usize, cell: usize) -> String {
        format!("\u{1}s:{wi}:{fi}:{cell}")
    }

    /// The elements of the classes `which` picks, as a linear form.
    fn count_of(&self, wi: usize, which: &dyn Fn(usize) -> bool) -> Lin {
        let mut l = Lin::con(Rat::zero());
        for ci in 0..self.classes.len() {
            if which(ci) {
                l = l.plus(&Lin::var(&Self::n(wi, ci)));
            }
        }
        l
    }

    /// The one value a lot's elements can hold, where they can hold only one.
    fn cell_value(&self, fi: usize, cell: usize) -> Option<Rat> {
        let p = self.cells[fi][cell].first().and_then(|ci| self.fields[fi].parts.get(self.classes[*ci][fi]))?;
        p.lo.zip(p.hi).filter(|(l, h)| l == h).map(|(l, _)| l)
    }

    /// The total of one lot: a name of its own, unless its elements can hold only one value, when
    /// it is that many times the value and elimination is left a name fewer to go through.
    fn cell_total(&self, wi: usize, fi: usize, cell: usize) -> Lin {
        match self.cell_value(fi, cell) {
            Some(v) => self.count_of(wi, &|ci| self.cells[fi][cell].contains(&ci)).scale(v),
            None => Lin::var(&Self::s(wi, fi, cell)),
        }
    }

    /// The total of field `fi` over the lots `which` picks, as a linear form.
    fn total_of(&self, wi: usize, fi: usize, which: &dyn Fn(usize) -> bool) -> Lin {
        let mut l = Lin::con(Rat::zero());
        for cell in 0..self.cells[fi].len() {
            if which(cell) {
                l = l.plus(&self.cell_total(wi, fi, cell));
            }
        }
        l
    }
}

/// What an inequality of the base is there for. A question that names nothing of it can be asked
/// without it first: the system is only wider without it, so a "no" still holds and a "yes" still
/// has to be built, and elimination has far less to go through.
#[derive(Debug, Clone)]
enum Need {
    Always,
    /// The classes of a collection.
    Walk(usize),
    /// The lots of a field that is summed.
    Field(usize, usize),
    /// The door's range on a read.
    Read(String),
    /// The door's range on a `max` or a `min`: a name of its own, which no question that does not
    /// name it has any use for, whatever else the base holds.
    Extreme(String),
}

/// One contract, with the collections read from it, or one sequence with no contract.
struct Group {
    shape: Option<String>,
    ac: Option<Across>,
    rel: Relation,
    walks: Vec<Walk>,
    /// A date read directly: nothing in a contract says anything about it, so it can be any day
    /// its range lets in.
    free: BTreeMap<String, String>,
    /// What holds whatever is asked, and what each of it is there for.
    base: Vec<(Need, Ineq)>,
    /// Each `count` and `sum`, as what its classes come to.
    defs: BTreeMap<String, Lin>,
    unread: Vec<String>,
}

impl Group {
    fn name(&self, t: &Term) -> String {
        match &self.ac {
            Some(ac) => ac.name(t),
            None => format!("\u{1}{}", t.word()),
        }
    }

    fn read(&self, name: &str) -> Option<(usize, &Read)> {
        self.walks.iter().enumerate().find_map(|(wi, w)| w.reads.iter().find(|r| r.name == name).map(|r| (wi, r)))
    }

    /// The base without what a question that names `named` has no use for.
    fn base_for(&self, named: &BTreeSet<String>) -> Vec<Ineq> {
        let terms = self.rel.terms();
        let walk_asked = |w: &Walk| {
            named.contains(&w.size) || w.reads.iter().any(|r| named.contains(&r.name)) || terms.iter().any(|t| self.name(t) == w.size)
        };
        self.base
            .iter()
            .filter(|(need, _)| match need {
                Need::Always => true,
                Need::Walk(wi) => walk_asked(&self.walks[*wi]),
                Need::Field(wi, fi) => summed_by_name(&self.walks[*wi], *fi, named),
                Need::Read(n) | Need::Extreme(n) => named.contains(n),
            })
            .map(|(_, q)| q.clone())
            .collect()
    }

    /// The whole base, for a question that names `named`.
    fn base_whole(&self, named: &BTreeSet<String>) -> Vec<Ineq> {
        self.base.iter().filter(|(need, _)| !matches!(need, Need::Extreme(n) if !named.contains(n))).map(|(_, q)| q.clone()).collect()
    }

    fn knows(&self, name: &str) -> bool {
        self.read(name).is_some() || self.free.contains_key(name) || self.ac.as_ref().is_some_and(|a| a.term(name).is_some())
    }
}

/// What the rule's collections say, read once and asked many times.
pub struct Walks<'a> {
    groups: Vec<Group>,
    f: &'a RuleFile,
    c: &'a Checked,
}

/// The contract a `shape` names, when it can be read: the file's own errors are `check`'s to say.
fn contract_of(d: &ShapeDecl, rule_path: &str) -> Option<Contract> {
    let followed = d.contract_path(rule_path);
    if crate::vfs::refused(&followed).is_some() {
        return None;
    }
    let p = followed.ok()?;
    let text = ritsu_base::fs::read_to_string(&p).ok()?;
    read(d, &text).ok()
}

/// A collection being read, before its fields are cut.
struct Plan {
    group: String,
    steps: Vec<String>,
    shown: String,
    cap: Option<i128>,
    element: Option<Contract>,
    fields: Vec<Field>,
    reads: Vec<Read>,
}

/// What a field comes to from its contract: what it can hold, whether that is known of it, its
/// enum's values and the rules on it that were not read.
type FieldRead = (Dom, bool, Option<Vec<String>>, Vec<String>);

impl Plan {
    fn field(&mut self, name: &str, make: &dyn Fn(Option<&Contract>) -> FieldRead, unread: &mut Vec<String>) -> usize {
        if let Some(i) = self.fields.iter().position(|x| x.name == name) {
            return i;
        }
        let (dom, unsure, path, more) = make(self.element.as_ref());
        for u in more {
            if !unread.contains(&u) {
                unread.push(u);
            }
        }
        self.fields.push(Field { name: name.to_string(), path, dom, date: false, unsure, tests: Vec::new(), of: false, summed: false, parts: Vec::new() });
        self.fields.len() - 1
    }
}

/// One end of a number with a fraction: the value, and whether it is left out.
type End = Option<(Rat, bool)>;

/// The ends of a number with a fraction in a JSON Schema, over what certainly applies.
fn fractions(con: &Contract, node: &crate::json::Json) -> (End, End) {
    let mut lo: End = None;
    let mut hi: End = None;
    let raise = |b: &mut Option<(Rat, bool)>, v: Rat, strict: bool| match b {
        Some((x, s)) => {
            let o = v.cmp_to(*x);
            if o.is_gt() || (o.is_eq() && strict && !*s) {
                *b = Some((v, strict));
            }
        }
        None => *b = Some((v, strict)),
    };
    let lower = |b: &mut Option<(Rat, bool)>, v: Rat, strict: bool| match b {
        Some((x, s)) => {
            let o = v.cmp_to(*x);
            if o.is_lt() || (o.is_eq() && strict && !*s) {
                *b = Some((v, strict));
            }
        }
        None => *b = Some((v, strict)),
    };
    for x in con.applying(node) {
        let flag = |k: &str| matches!(super::get(x, k), Some(crate::json::Json::Bool(true)));
        if let Some(m) = super::get(x, "minimum").and_then(super::rat_of) {
            raise(&mut lo, m, flag("exclusiveMinimum"));
        }
        if let Some(m) = super::get(x, "maximum").and_then(super::rat_of) {
            lower(&mut hi, m, flag("exclusiveMaximum"));
        }
        if let Some(m) = super::get(x, "exclusiveMinimum").and_then(super::rat_of) {
            raise(&mut lo, m, true);
        }
        if let Some(m) = super::get(x, "exclusiveMaximum").and_then(super::rat_of) {
            lower(&mut hi, m, true);
        }
    }
    (lo, hi)
}

/// What the element's contract says one field can hold: the values, whether they can be shown to
/// pass, the path to ask its condition about, and the rules on it that were not read.
fn from_contract(element: Option<&Contract>, field: &str) -> (Dom, bool, Option<Vec<String>>, Vec<String>) {
    let Some(econ) = element else { return (Dom::Wide, true, None, Vec::new()) };
    let path = vec![field.to_string()];
    match econ.kind {
        EnumSource::JsonSchema => {
            let Some(node) = econ.schema_node(&path) else { return (Dom::Wide, true, None, Vec::new()) };
            let (dom, strings) = match econ.kind_at(node) {
                super::At::Bool => (Dom::Truth, false),
                super::At::Int => (Dom::Whole(econ.ints_at(node)), false),
                super::At::Frac => {
                    let (lo, hi) = fractions(econ, node);
                    (Dom::Frac { lo, hi }, false)
                }
                super::At::Str => (Dom::Words { listed: econ.strs_at(node), numbers: Vec::new() }, true),
                _ => (Dom::Wide, false),
            };
            let unread = econ.unread_at(node, strings);
            (dom, false, Some(path), unread)
        }
        EnumSource::Proto => {
            let Some(fd) = econ.root_message().and_then(|m| m.fields.iter().find(|x| x.name == field)).cloned() else {
                return (Dom::Wide, true, None, Vec::new());
            };
            if fd.repeated {
                return (Dom::Wide, true, None, Vec::new());
            }
            let absent = fd.optional && !fd.rules.required;
            let mut unread = fd.rules.unread.clone();
            if econ.cel_unread(&fd, &path) {
                unread.push("cel".into());
            }
            let (dom, unsure) = match crate::proto::scalar(&fd.ty) {
                Some(crate::proto::Scalar::Int) => match proto_ints(&fd) {
                    Some(s) => (Dom::Whole(if absent { s.union(&Ints::points(&[0])) } else { s }), false),
                    None => (Dom::Wide, true),
                },
                Some(crate::proto::Scalar::Str) => {
                    let mut listed = proto_strs(&fd);
                    if let (Some(l), true) = (listed.as_mut(), absent)
                        && !l.iter().any(String::is_empty)
                    {
                        l.push(String::new());
                    }
                    (Dom::Words { listed, numbers: Vec::new() }, false)
                }
                Some(crate::proto::Scalar::Bool) => (Dom::Truth, false),
                // The rules on a `double` are not read at all, so a value cannot be shown to pass.
                Some(crate::proto::Scalar::Frac) => (Dom::Frac { lo: None, hi: None }, true),
                None => match econ.enum_of(&fd.ty) {
                    Some(e) => {
                        let r = &fd.rules.enum_values;
                        let keep = |n: i128| {
                            r.konst.is_none_or(|k| k == n)
                                && (r.in_.is_empty() || r.in_.contains(&n))
                                && !r.not_in.contains(&n)
                                && (n != 0 || !fd.rules.required || fd.optional)
                                || (n == 0 && absent)
                        };
                        let numbers: Vec<(String, i128)> = e.values.iter().map(|v| (v.name.clone(), i128::from(v.number))).filter(|(_, n)| keep(*n)).collect();
                        (Dom::Words { listed: Some(numbers.iter().map(|(v, _)| v.clone()).collect()), numbers }, false)
                    }
                    None => (Dom::Wide, true),
                },
            };
            (dom, unsure, Some(path), unread)
        }
    }
}

/// What the rule's own declaration of a value lets it hold. `None` for what cannot be read: an
/// optional value, a value on no whole step of how it travels.
fn declared(name: &str, c: &Checked) -> Option<Dom> {
    let ty = c.ty_of(name)?;
    Some(match &ty {
        Ty::Bool => Dom::Truth,
        Ty::Enum(e) => {
            let all = c.enum_ranges.get(name).or_else(|| c.enums.get(e))?;
            Dom::Words { listed: Some(all.clone()), numbers: Vec::new() }
        }
        Ty::Str => Dom::Words { listed: None, numbers: Vec::new() },
        Ty::Date => {
            let (lo, hi) = c.ranges.get(name).copied().unwrap_or((None, None));
            Dom::Whole(Ints::between(lo.map(|v| v.num / v.den), hi.map(|v| v.num / v.den)))
        }
        Ty::Number | Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => {
            let sc = c.wire_scale(name);
            if c.value_step(name).checked_mul(Rat::int(sc)).is_none_or(|s| s != Rat::int(1)) {
                return None;
            }
            let (lo, hi) = c.ranges.get(name).copied().unwrap_or((None, None));
            let at = |v: Rat, up: bool| {
                let s = v.mul(Rat::int(sc));
                if s.is_int() {
                    s.num
                } else if up {
                    s.num.div_euclid(s.den) + 1
                } else {
                    s.num.div_euclid(s.den)
                }
            };
            Dom::Whole(Ints::between(lo.map(|v| at(v, true)), hi.map(|v| at(v, false))))
        }
        _ => return None,
    })
}

/// A field of a sequence, as far as both the contract and its declaration let it go.
fn narrowed(dom: Dom, decl: Option<Dom>) -> Dom {
    match (dom, decl) {
        (Dom::Wide, _) | (_, None) => Dom::Wide,
        (Dom::Truth, Some(Dom::Truth)) => Dom::Truth,
        (Dom::Whole(a), Some(Dom::Whole(b))) => Dom::Whole(a.intersect(&b)),
        (Dom::Words { listed: a, numbers }, Some(Dom::Words { listed: b, .. })) => {
            let listed = match (a, b) {
                (Some(x), Some(y)) => Some(x.into_iter().filter(|v| y.contains(v)).collect()),
                (x, y) => x.or(y),
            };
            Dom::Words { listed, numbers }
        }
        _ => Dom::Wide,
    }
}

/// The value of a literal at the scale a value of `ty` travels at.
fn wire_of(l: &Lit, ty: &Ty, scale: i128) -> Option<Rat> {
    match l {
        Lit::Num(n) => crate::types::lit_value_in_pub(n, ty).map(|v| v.mul(Rat::int(scale))),
        Lit::Date(y, m, d) => Some(crate::types::date_ord(*y, *m, *d)),
        _ => None,
    }
}

/// A summary written back, for the note that says what it is.
fn agg_text(d: &AggDecl) -> String {
    let mut out = format!("{} {} {}", d.kind.word(), crate::kw::OVER, d.over);
    match d.kind {
        AggKind::Count => {
            out = format!("{out} {} {}", crate::kw::WHERE, d.column.text);
            if let Some(v) = &d.value {
                out = format!("{out} = {}", v.text);
            }
        }
        _ => {
            out = format!("{out} {} {}", crate::kw::OF, d.column.text);
            if let Some((w, v)) = &d.filter {
                out = format!("{out} {} {}", crate::kw::WHERE, w.text);
                if let Some(v) = v {
                    out = format!("{out} = {}", v.text);
                }
            }
            if let Some((l, _)) = &d.empty {
                out = format!("{out} {} {}", crate::kw::EMPTY, crate::ast::lit_text(l));
            }
        }
    }
    out
}

/// Whether a table of one element works this name out: a column the sequence does not carry.
fn worked_out(f: &RuleFile, name: &str) -> bool {
    f.items.iter().any(|it| match it {
        Item::Table(t) => t.outputs.iter().any(|o| o.name.text == name),
        Item::Derived(d) => d.name.text == name,
        Item::Define(d) => d.name.text == name,
        _ => false,
    })
}

fn cut_field(fd: &mut Field) -> bool {
    // A date travels as a string the reader turns into its day number, so a field that a date
    // test or a `max` of dates reads holds days, and every test on it has to compare days too.
    if fd.date {
        fd.dom = match &fd.dom {
            Dom::Words { listed: None, .. } => Dom::Whole(Ints::between(Some(crate::types::date_ord(1, 1, 1).num), Some(crate::types::date_ord(9999, 12, 31).num))),
            Dom::Whole(s) => Dom::Whole(s.clone()),
            _ => Dom::Wide,
        };
    }
    let tests: Vec<Option<Test>> = fd.tests.clone();
    let truth = |v: &Val| -> Vec<bool> { tests.iter().map(|t| t.as_ref().is_some_and(|t| t.holds(v))).collect() };
    let mut parts: Vec<Part> = Vec::new();
    let merge = |parts: &mut Vec<Part>, p: Part, keep_apart: bool| {
        if !keep_apart
            && let Some(q) = parts.iter_mut().find(|q| q.truth == p.truth)
        {
            q.vals.extend(p.vals);
            q.other |= p.other;
            q.lo = match (q.lo, p.lo) {
                (Some(a), Some(b)) => Some(if a.cmp_to(b).is_le() { a } else { b }),
                _ => None,
            };
            q.hi = match (q.hi, p.hi) {
                (Some(a), Some(b)) => Some(if a.cmp_to(b).is_ge() { a } else { b }),
                _ => None,
            };
            return;
        }
        parts.push(p);
    };
    match &fd.dom {
        Dom::Wide => {
            parts.push(Part { truth: vec![false; tests.len()], vals: Vec::new(), other: true, lo: None, hi: None, sample: Val::Word(String::new()) });
        }
        Dom::Truth => {
            for b in [true, false] {
                let v = Val::Truth(b);
                merge(&mut parts, Part { truth: truth(&v), vals: vec![v.clone()], other: false, lo: None, hi: None, sample: v }, false);
            }
        }
        Dom::Words { listed, .. } => {
            let mut words: Vec<String> = Vec::new();
            match listed {
                Some(l) => words.extend(l.iter().cloned()),
                None => {
                    for t in tests.iter().flatten() {
                        if let Test::Among(vs, _) = t {
                            for v in vs {
                                if let Val::Word(w) = v
                                    && !words.contains(w)
                                {
                                    words.push(w.clone());
                                }
                            }
                        }
                    }
                }
            }
            for w in &words {
                let v = Val::Word(w.clone());
                merge(&mut parts, Part { truth: truth(&v), vals: vec![v.clone()], other: false, lo: None, hi: None, sample: v }, false);
            }
            if listed.is_none() {
                // A string none of the tests names: one more kind of value, or the one there is.
                let mut other = String::from("x");
                while words.contains(&other) {
                    other.push('x');
                }
                let v = Val::Word(other);
                merge(&mut parts, Part { truth: truth(&v), vals: Vec::new(), other: true, lo: None, hi: None, sample: v }, false);
            }
        }
        Dom::Whole(s) => {
            // The first value of each stretch the tests tell apart.
            let mut starts: BTreeSet<i128> = BTreeSet::new();
            for k in tests.iter().flatten().flat_map(Test::numbers) {
                if k.is_int() {
                    starts.insert(k.num);
                    starts.insert(k.num + 1);
                } else {
                    starts.insert(k.num.div_euclid(k.den) + 1);
                }
            }
            let mut ends: Vec<(Option<i128>, Option<i128>)> = Vec::new();
            let mut last = None;
            for b in &starts {
                ends.push((last, Some(*b - 1)));
                last = Some(*b);
            }
            ends.push((last, None));
            for (a, b) in ends {
                for (lo, hi) in s.intersect(&Ints::between(a, b)).0 {
                    let x = match (lo, hi) {
                        (Some(l), _) if l >= 0 => l,
                        (_, Some(h)) if h <= 0 => h,
                        _ => 0,
                    };
                    let v = Val::Num(Rat::int(x));
                    let part = Part { truth: truth(&v), vals: Vec::new(), other: false, lo: lo.map(Rat::int), hi: hi.map(Rat::int), sample: v };
                    merge(&mut parts, part, fd.of);
                }
            }
        }
        Dom::Frac { lo, hi } => {
            // Between two neighbouring boundaries every test answers the same, so these cover
            // every answer there is.
            let mut bounds: Vec<Rat> = tests.iter().flatten().flat_map(Test::numbers).collect();
            bounds.extend(lo.iter().chain(hi.iter()).map(|(k, _)| *k));
            bounds.sort_by(|a, b| a.cmp_to(*b));
            bounds.dedup();
            let mut tries: Vec<Rat> = Vec::new();
            match (bounds.first(), bounds.last()) {
                (Some(a), Some(b)) => {
                    tries.push(a.sub(Rat::int(1)));
                    tries.extend(bounds.iter().copied());
                    tries.extend(bounds.windows(2).map(|w| w[0].add(w[1]).mul(Rat::new(1, 2))));
                    tries.push(b.add(Rat::int(1)));
                }
                _ => tries.push(Rat::zero()),
            }
            let inside = |v: Rat| {
                lo.is_none_or(|(k, s)| if s { v.cmp_to(k).is_gt() } else { v.cmp_to(k).is_ge() })
                    && hi.is_none_or(|(k, s)| if s { v.cmp_to(k).is_lt() } else { v.cmp_to(k).is_le() })
            };
            for x in tries.into_iter().filter(|v| inside(*v)) {
                let v = Val::Num(x);
                merge(&mut parts, Part { truth: truth(&v), vals: Vec::new(), other: false, lo: None, hi: None, sample: v }, false);
            }
        }
    }
    // `sum`, `max` and `min` compare stretches by where they lie.
    if fd.of {
        parts.sort_by(|a, b| match (&a.lo, &b.lo) {
            (Some(x), Some(y)) => x.cmp_to(*y),
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });
    }
    let fine = parts.len() <= CLASSES;
    fd.parts = parts;
    fine
}

/// Whether the element's condition rules a class of values out.
fn class_refused(w: &Walk, class: &[usize]) -> bool {
    if w.each.is_trivial() {
        return false;
    }
    let name = |t: &Term| format!("\u{1}{}", t.word());
    let mut x = Extra::default();
    for (fi, fd) in w.fields.iter().enumerate() {
        let (Some(path), Some(p)) = (&fd.path, fd.parts.get(class[fi])) else { continue };
        let t = Term::Field(path.clone());
        let var = name(&t);
        match &fd.dom {
            Dom::Truth => {
                if let [Val::Truth(b)] = p.vals.as_slice() {
                    x.bools.push((t, *b));
                }
            }
            Dom::Words { numbers, .. } if !numbers.is_empty() => {
                if let [Val::Word(v)] = p.vals.as_slice()
                    && let Some((_, n)) = numbers.iter().find(|(m, _)| m == v)
                {
                    let d = Lin::var(&var).plus(&Lin::con(Rat::int(-*n)));
                    x.ineqs.push(d.clone().le(false));
                    x.ineqs.push(d.ge(false));
                }
            }
            Dom::Words { .. } => {
                if !p.other {
                    let vs = p.vals.iter().filter_map(|v| if let Val::Word(w) = v { Some(w.clone()) } else { None }).collect();
                    x.strs.push((t, vs));
                }
            }
            Dom::Whole(_) if !fd.date => {
                if let Some(l) = p.lo {
                    x.ineqs.push(Lin::var(&var).plus(&Lin::con(l.mul(Rat::int(-1)))).ge(false));
                }
                if let Some(h) = p.hi {
                    x.ineqs.push(Lin::var(&var).plus(&Lin::con(h.mul(Rat::int(-1)))).le(false));
                }
            }
            _ => {}
        }
    }
    matches!(w.each.find(&x, &name), Found::None(_))
}

/// Whether one element with these values is one the element's condition lets through.
fn element_passes(w: &Walk, values: &[Val]) -> bool {
    if w.each.is_trivial() {
        return true;
    }
    let name = |t: &Term| format!("\u{1}{}", t.word());
    let mut x = Extra::default();
    for (fi, fd) in w.fields.iter().enumerate() {
        let (Some(path), Some(v)) = (&fd.path, values.get(fi)) else { continue };
        let t = Term::Field(path.clone());
        let var = name(&t);
        let exactly = |k: Rat| {
            let d = Lin::var(&var).plus(&Lin::con(k.mul(Rat::int(-1))));
            [d.clone().le(false), d.ge(false)]
        };
        match (&fd.dom, v) {
            (Dom::Truth, Val::Truth(b)) => x.bools.push((t, *b)),
            (Dom::Words { numbers, .. }, Val::Word(s)) if !numbers.is_empty() => {
                if let Some((_, n)) = numbers.iter().find(|(m, _)| m == s) {
                    x.ineqs.extend(exactly(Rat::int(*n)));
                }
            }
            (Dom::Words { .. }, Val::Word(s)) => x.strs.push((t, vec![s.clone()])),
            (Dom::Whole(_), Val::Num(k)) if !fd.date => x.ineqs.extend(exactly(*k)),
            _ => {}
        }
    }
    !matches!(w.each.find(&x, &name), Found::None(_))
}

/// `a = b`, as the two inequalities it is.
fn equal(a: Lin, b: &Lin) -> [Ineq; 2] {
    let d = a.plus(&b.clone().scale(Rat::int(-1)));
    [d.clone().le(false), d.ge(false)]
}

fn at_least(l: Lin, k: Rat) -> Ineq {
    l.plus(&Lin::con(k.mul(Rat::int(-1)))).ge(false)
}

fn at_most(l: Lin, k: Rat) -> Ineq {
    l.plus(&Lin::con(k.mul(Rat::int(-1)))).le(false)
}

/// The names of an inequality's terms.
fn ineq_names(q: &Ineq) -> impl Iterator<Item = &String> {
    q.terms.iter().filter(|(_, c)| c.num != 0).map(|(n, _)| n)
}

/// A value as a message writes it.
fn shown_val(v: &Val, date: bool) -> String {
    match v {
        Val::Truth(b) => b.to_string(),
        Val::Word(w) => crate::json::quote(w),
        Val::Num(n) if date => {
            let (y, m, d) = crate::types::ord_to_date(*n);
            format!("\"{y:04}-{m:02}-{d:02}\"")
        }
        Val::Num(n) => {
            if n.is_int() {
                n.num.to_string()
            } else {
                n.to_string()
            }
        }
    }
}

impl<'a> Walks<'a> {
    /// Every collection the rule reads, by the contract it is read from, and every sequence that
    /// has none.
    pub fn of(f: &'a RuleFile, c: &'a Checked, rule_path: &str) -> Walks<'a> {
        let contracts: BTreeMap<String, Contract> = f.shapes.iter().filter_map(|d| contract_of(d, rule_path).map(|con| (d.name.text.clone(), con))).collect();
        let mut plans: Vec<Plan> = Vec::new();
        let mut unread: BTreeMap<String, Vec<String>> = BTreeMap::new();
        fn plan_at<'p>(plans: &'p mut Vec<Plan>, group: &str, steps: &[String], shown: String, cap: Option<i128>, element: Option<Contract>) -> &'p mut Plan {
            let i = match plans.iter().position(|p| p.group == group && p.steps == steps) {
                Some(i) => i,
                None => {
                    plans.push(Plan { group: group.to_string(), steps: steps.to_vec(), shown, cap: None, element, fields: Vec::new(), reads: Vec::new() });
                    plans.len() - 1
                }
            };
            let p = &mut plans[i];
            p.cap = match (p.cap, cap) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
            p
        }
        // The reads `from` an object.
        for i in &f.inputs {
            let Some(pr) = &i.from else { continue };
            if matches!(pr.kind, ProjKind::Field) {
                continue;
            }
            let Some(con) = contracts.get(&pr.root.text) else { continue };
            let steps = pr.path_steps();
            let shown = super::full_path(pr);
            let name = &i.name.text;
            let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
            let kind = match pr.kind {
                ProjKind::Any(..) => Kind::Any,
                ProjKind::All(..) => Kind::All,
                ProjKind::Count(_) => Kind::Count,
                ProjKind::Sum(..) => Kind::Sum,
                ProjKind::Max(..) => Kind::Max,
                _ => Kind::Min,
            };
            let plan = plan_at(&mut plans, &pr.root.text, &steps, shown, None, con.element(&steps));
            let more = unread.entry(pr.root.text.clone()).or_default();
            let mut readable = true;
            let test = pr.kind.test().map(|(fd, cell)| {
                let fi = plan.field(&fd.text, &|e| from_contract(e, &fd.text), more);
                let read = Test::of_cell(cell);
                readable &= read.is_some();
                if read.as_ref().is_some_and(|(_, d)| *d) {
                    plan.fields[fi].date = true;
                }
                plan.fields[fi].tests.push(read.map(|(t, _)| t));
                (fi, plan.fields[fi].tests.len() - 1)
            });
            let of = pr.kind.of().map(|n| {
                let fi = plan.field(&n.text, &|e| from_contract(e, &n.text), more);
                plan.fields[fi].of = true;
                plan.fields[fi].summed |= kind == Kind::Sum;
                plan.fields[fi].date |= ty == Ty::Date;
                fi
            });
            let empty = pr.kind.empty().and_then(|(l, _)| wire_of(l, &ty, c.wire_scale(name)));
            plan.reads.push(Read { name: name.clone(), kind, test, of, empty, pass: Vec::new(), readable, said: pr.text(), from: true });
        }
        // The summaries of a sequence.
        for d in f.aggs() {
            let Some(el) = f.elements.iter().find(|e| e.name.text == d.over) else { continue };
            let cap = crate::door::count_cap(f, c, &d.over);
            let source = el.from.as_ref().and_then(|sp| contracts.get(&sp.root.text).map(|con| (sp, con)));
            let (group, steps, shown, element) = match source {
                Some((sp, con)) => {
                    let steps = sp.path_steps();
                    let e = con.element(&steps);
                    (sp.root.text.clone(), steps, super::full_path(sp), e)
                }
                None => (format!("\u{1}{}", d.over), Vec::new(), d.over.clone(), None),
            };
            let plan = plan_at(&mut plans, &group, &steps, shown, cap, element);
            let more = unread.entry(group.clone()).or_default();
            let column = |plan: &mut Plan, col: &str, more: &mut Vec<String>| -> usize {
                let carried = el.fields.iter().find(|fd| fd.name.text == col);
                match carried {
                    Some(fd) => field_of_sequence(plan, fd, c, more),
                    // A column a table works out for each element: its enum, or truth value, is
                    // all that is known of it, and a number is not on any known step.
                    None => plan.field(
                        col,
                        &|_| {
                            let dom = c.ty_of(col).filter(|t| matches!(t, Ty::Bool | Ty::Enum(_))).and_then(|_| declared(col, c)).filter(|_| worked_out(f, col));
                            (dom.unwrap_or(Dom::Wide), true, None, Vec::new())
                        },
                        more,
                    ),
                }
            };
            let test_of = |w: Option<&crate::ast::Name>| -> Test {
                match w {
                    None => Test::Among(vec![Val::Truth(true)], true),
                    Some(n) if n.text == crate::kw::TRUE => Test::Among(vec![Val::Truth(true)], true),
                    Some(n) if n.text == crate::kw::FALSE => Test::Among(vec![Val::Truth(false)], true),
                    Some(n) => Test::Among(vec![Val::Word(n.text.clone())], true),
                }
            };
            let (kind, test, of) = match d.kind {
                AggKind::Count => {
                    let fi = column(plan, &d.column.text, more);
                    plan.fields[fi].tests.push(Some(test_of(d.value.as_ref())));
                    (Kind::Count, Some((fi, plan.fields[fi].tests.len() - 1)), None)
                }
                AggKind::Sum | AggKind::Max | AggKind::Min => {
                    let fi = column(plan, &d.column.text, more);
                    plan.fields[fi].of = true;
                    plan.fields[fi].summed |= d.kind == AggKind::Sum;
                    let kind = match d.kind {
                        AggKind::Sum => Kind::Sum,
                        AggKind::Max => Kind::Max,
                        _ => Kind::Min,
                    };
                    let test = d.filter.as_ref().map(|(w, v)| {
                        let wi = column(plan, &w.text, more);
                        plan.fields[wi].tests.push(Some(test_of(v.as_ref())));
                        (wi, plan.fields[wi].tests.len() - 1)
                    });
                    (kind, test, Some(fi))
                }
            };
            let name = &d.name.text;
            let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
            let scaled = of.is_none() || c.wire_scale(name) == c.wire_scale(&d.column.text);
            if let Some(fi) = of {
                plan.fields[fi].date |= ty == Ty::Date;
            }
            let empty = d.empty.as_ref().and_then(|(l, _)| wire_of(l, &ty, c.wire_scale(name)));
            plan.reads.push(Read { name: name.clone(), kind, test, of, empty, pass: Vec::new(), readable: scaled, said: agg_text(d), from: false });
        }
        // Each contract across its fields, its collections now known.
        let mut groups: Vec<Group> = Vec::new();
        let keys: Vec<String> = {
            let mut out: Vec<String> = Vec::new();
            for p in &plans {
                if !out.contains(&p.group) {
                    out.push(p.group.clone());
                }
            }
            out
        };
        for key in keys {
            let mine: Vec<Plan> = {
                let (a, b): (Vec<Plan>, Vec<Plan>) = std::mem::take(&mut plans).into_iter().partition(|p| p.group == key);
                plans = b;
                a
            };
            let con = contracts.get(&key);
            let walked: Vec<Vec<String>> = mine.iter().map(|p| p.steps.clone()).collect();
            let ac = con.map(|k| k.across_reading(f, c, &key, &walked));
            let rel = ac.as_ref().map_or_else(Relation::any, |a| a.rel.clone());
            let mut walks: Vec<Walk> = Vec::new();
            for plan in mine {
                let size = match &ac {
                    Some(a) => a.name(&Term::Size(plan.steps.clone())),
                    None => format!("\u{1}size:{}", plan.shown),
                };
                walks.push(finish(plan, size));
            }
            let mut free = BTreeMap::new();
            if con.is_some() {
                for i in &f.inputs {
                    let Some(pr) = &i.from else { continue };
                    if pr.root.text == key && matches!(pr.kind, ProjKind::Field) && c.ty_of(&i.name.text) == Some(Ty::Date) {
                        free.insert(i.name.text.clone(), super::full_path(pr));
                    }
                }
            }
            let mut group = Group {
                shape: con.map(|_| key.clone()),
                ac,
                rel,
                walks,
                free,
                base: Vec::new(),
                defs: BTreeMap::new(),
                unread: unread.remove(&key).unwrap_or_default(),
            };
            base_of(&mut group, c);
            groups.push(group);
        }
        Walks { groups, f, c }
    }

    /// Whether a name is one the walks know about: a read, or a value read beside them.
    pub fn knows(&self, name: &str) -> bool {
        self.groups.iter().any(|g| g.knows(name))
    }

    /// Whether it is a read of a collection.
    pub fn walks_for(&self, name: &str) -> bool {
        self.groups.iter().any(|g| g.read(name).is_some())
    }

    /// Whether anything at all was read.
    pub fn is_empty(&self) -> bool {
        self.groups.iter().all(|g| g.walks.is_empty())
    }

    /// The `constraint`s every name of which is known here, as they hold at the door.
    pub fn constraints(&self) -> Vec<Ineq> {
        let mut out = Vec::new();
        for (i, k) in self.f.constraints.iter().enumerate() {
            if k.names().all(|n| self.knows(n)) {
                out.push(wire_constraint(k, i, self.c, false));
            }
        }
        out
    }

    /// Whether some request gives this combination: the one question everything here asks.
    ///
    /// `asks` says what a truth value or an enum has to be, and `ineqs` what numbers have to be,
    /// over the names of the inputs and summaries at the scale they travel at. A name nothing
    /// here knows is the caller's to leave out. An inequality between two contracts, or a contract
    /// and a sequence, is left out as well, which only widens; a `Yes` is then held to it.
    pub fn ask(&self, asks: &[(String, Ask)], ineqs: &[Ineq]) -> Possible {
        let mut out = Example::default();
        let mut unknown = false;
        let mut said = false;
        for g in &self.groups {
            let a: Vec<&(String, Ask)> = asks.iter().filter(|(n, _)| g.knows(n)).collect();
            let q: Vec<&Ineq> = ineqs.iter().filter(|q| ineq_names(q).all(|n| g.knows(n) || n.starts_with('\u{1}'))).filter(|q| ineq_names(q).any(|n| g.knows(n))).collect();
            if a.is_empty() && q.is_empty() {
                continue;
            }
            said = true;
            match ask_group(g, self.c, &a, &q) {
                Possible::No(p) => return Possible::No(p),
                Possible::Unknown => unknown = true,
                Possible::Yes(ex) => {
                    out.values.extend(ex.values);
                    out.truths.extend(ex.truths);
                    out.plain.extend(ex.plain);
                    out.walks.extend(ex.walks);
                }
            }
        }
        if unknown || !said {
            return Possible::Unknown;
        }
        if ineqs.iter().any(|q| q.holds_at(&out.values) != Some(true)) {
            return Possible::Unknown;
        }
        Possible::Yes(out)
    }

    /// Why a combination that does not occur is still asked for: what its reads are, and who can
    /// reach it. `names` are the columns the combination asks about.
    pub fn never_notes(&self, names: &[String]) -> Vec<String> {
        let reads: Vec<&Read> = names.iter().filter_map(|n| self.groups.iter().find_map(|g| g.read(n).map(|(_, r)| r))).collect();
        let mut out = Vec::new();
        if reads.is_empty() {
            return out;
        }
        let said = reads.iter().map(|r| tr!("`{}` は `{}`", "`{}` is `{}`", r.name, r.said)).collect::<Vec<_>>();
        let said = if crate::i18n::ja() { said.join("、") } else { said.join(", ") };
        out.push(tr!("この組み合わせは、定義からは起こりません（{said}）。", "This combination never comes out of the definitions ({said})."));
        if reads.iter().any(|r| r.from) {
            let targets = without_from();
            out.push(tr!(
                "それでも行を求めるのは、`from` を読まない呼び出しには届くからです。{targets} の生成コードは `from` を読まず、呼ぶ側が計算した値をそのまま受け取ります。",
                "A row is still asked for because a call that does not read through `from` reaches it: the code generated for {targets} does not read `from`, and takes whatever the caller worked out."
            ));
        }
        if reads.iter().any(|r| !r.from) {
            out.push(tr!(
                "表の検査は集計を一つの列として読み、定義までは読まないので、行を求めます。",
                "The check of a table reads a summary as one column and not its definition, so it still asks for a row."
            ));
        }
        out
    }

    /// E123 for a `constraint` that names a read of a collection: `None` when it is not one, when
    /// the contract keeps it, or when that could not be shown either way.
    pub(super) fn broken(&self, shape: &str, idx: usize, k: &crate::ast::Constraint, rule_path: &str) -> Option<Diag> {
        let g = self.groups.iter().find(|g| g.shape.as_deref() == Some(shape))?;
        let names: Vec<&String> = k.names().collect();
        if !names.iter().any(|n| g.read(n).is_some_and(|(_, r)| r.from)) || !names.iter().all(|n| g.knows(n)) {
            return None;
        }
        let q = wire_constraint(k, idx, self.c, true);
        let Possible::Yes(ex) = self.ask(&[], std::slice::from_ref(&q)) else { return None };
        // Each place the constraint's values come from, once.
        let mut places: Vec<String> = Vec::new();
        for n in &names {
            let p = match g.read(n) {
                Some((wi, _)) => g.walks[wi].shown.clone(),
                None => self.f.inputs.iter().find(|i| i.name.text == **n).and_then(|i| i.from.as_ref()).map(super::full_path).unwrap_or_default(),
            };
            if !places.contains(&p) {
                places.push(p);
            }
        }
        let all = if crate::i18n::ja() {
            places.join("` と `")
        } else {
            match places.split_last() {
                Some((last, rest)) if !rest.is_empty() => format!("{}` and `{last}", rest.join("`, `")),
                _ => places.join(""),
            }
        };
        let rel = k.said();
        let mut d = Diag::error(
            "E123",
            tr!("契約は `{all}` について、`constraint {rel}` を破る値も通します", "The contract lets `{all}` through with values that break `constraint {rel}`"),
        )
        .at(tr!("{rule_path}:{} 制約", "{rule_path}:{} constraint", k.span.line))
        .mark(k.span.clone(), tr!("契約はこの関係を約束していません", "the contract does not promise this"));
        let date = |n: &str| self.c.ty_of(n) == Some(Ty::Date);
        let mut values: Vec<String> = Vec::new();
        for n in &names {
            let v = ex.values.get(*n).copied().unwrap_or(Rat::zero());
            if date(n) {
                let (y, m, dd) = crate::types::ord_to_date(v);
                let s = format!("{y:04}-{m:02}-{dd:02}");
                d = d.win(n.as_str(), WVal::Str(s.clone()));
                values.push(tr!("`{n}` が {s}", "`{n}` = {s}"));
            } else {
                let w = v.num / v.den;
                d = d.win(n.as_str(), WVal::Int(w));
                values.push(tr!("`{n}` が {w}", "`{n}` = {w}"));
            }
        }
        let values_ja = values.join("、");
        let values_en = values.join(" and ");
        let at = ex.request();
        // A space only after a number or a name: `1 件のリクエスト`, `limit` が 3 のリクエスト.
        let at_ja = if at.ends_with(|c: char| c.is_ascii()) { format!("{at} ") } else { at.clone() };
        d = d.note(tr!(
            "たとえば {at_ja}のリクエストは、契約の検証を通り、{values_ja} になって `constraint` を満たしません。",
            "A request with {at}, for one, passes the contract's validation and gives {values_en}, which breaks the `constraint`."
        ));
        d = d.note(super::refused_note());
        d = d.fix_kind(FixKind::None).note(tr!(
            "ヒント: `constraint` が間違っているなら直してください。それでも合っているなら、`constraint` を外して、その組み合わせのときの答えを表で決めてください。契約の側で約束する書き方は、ここでは読めません。",
            "hint: if the `constraint` is wrong, correct it. If it is right, take it off and decide in the tables what the rule answers for that combination. A contract cannot promise this in a form read here."
        ));
        if g.ac.as_ref().is_some_and(|a| a.unread) || !g.unread.is_empty() {
            d = d.note(tr!(
                "契約には読めなかった規則があり、ここでは無いものとして扱っています。",
                "The contract has rules that could not be read, and they are treated here as not there."
            ));
        }
        Some(d)
    }
}

/// The targets that do not read `from`, as a person reads them.
fn without_from() -> String {
    let names: Vec<&str> = crate::backend::ALL.iter().filter(|b| !b.projects).map(|b| b.name).collect();
    if crate::i18n::ja() {
        names.join("・")
    } else {
        match names.split_last() {
            Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
            _ => names.join(""),
        }
    }
}

/// A field a sequence carries, read as a path from the element when the sequence comes from a
/// contract. A path of several steps is not read: what is on the way to it may be left out.
fn field_of_sequence(plan: &mut Plan, fd: &VarDecl, c: &Checked, more: &mut Vec<String>) -> usize {
    let name = fd.name.text.clone();
    let path = fd.from.as_ref().map(|p| p.steps());
    let key = match &path {
        Some(p) if p.len() == 1 => p[0].clone(),
        _ => name.clone(),
    };
    plan.field(
        &key,
        &|e| match (&path, e) {
            (Some(p), Some(_)) if p.len() == 1 => {
                let (dom, unsure, q, un) = from_contract(e, &p[0]);
                (narrowed(dom, declared(&name, c)), unsure, q, un)
            }
            (Some(_), _) => (Dom::Wide, true, None, Vec::new()),
            (None, _) => (declared(&name, c).unwrap_or(Dom::Wide), false, None, Vec::new()),
        },
        more,
    )
}

/// One collection, its fields cut and its classes laid out.
fn finish(plan: Plan, size: String) -> Walk {
    let each = match &plan.element {
        Some(econ) => {
            let reads: Vec<Term> = plan.fields.iter().filter_map(|fd| fd.path.clone().map(Term::Field)).collect();
            let cond = match econ.kind {
                EnumSource::JsonSchema => econ.schema_condition(&reads),
                EnumSource::Proto => econ.proto_condition(&reads),
            };
            Relation::of(&cond)
        }
        None => Relation::any(),
    };
    let mut fields = plan.fields;
    let mut fine = true;
    for fd in &mut fields {
        fine &= cut_field(fd);
    }
    let mut count = 1usize;
    for fd in &fields {
        count = count.saturating_mul(fd.parts.len());
    }
    fine &= count <= CLASSES;
    let mut w = Walk { shown: plan.shown, size, cap: plan.cap, each, fine, fields, classes: Vec::new(), cells: Vec::new(), reads: plan.reads };
    if fine {
        let mut all: Vec<Vec<usize>> = vec![Vec::new()];
        for fd in &w.fields {
            let mut next = Vec::with_capacity(all.len() * fd.parts.len());
            for a in &all {
                for p in 0..fd.parts.len() {
                    let mut x = a.clone();
                    x.push(p);
                    next.push(x);
                }
            }
            all = next;
        }
        let kept: Vec<Vec<usize>> = all.into_iter().filter(|cl| !class_refused(&w, cl)).collect();
        w.classes = kept;
    }
    let classes = w.classes.clone();
    let fields = w.fields.clone();
    for r in &mut w.reads {
        r.readable &= fine;
        if let Some((fi, ti)) = r.test {
            let fd = &fields[fi];
            match &fd.tests[ti] {
                Some(t) if t.fits(&fd.dom) => {}
                _ => r.readable = false,
            }
            r.pass = classes.iter().map(|cl| fd.parts[cl[fi]].truth[ti]).collect();
        } else {
            r.pass = vec![true; classes.len()];
        }
        if let Some(fi) = r.of
            && !matches!(fields[fi].dom, Dom::Whole(_))
        {
            r.readable = false;
        }
    }
    // A summed field's classes, put together where no read can tell them apart: one total stands
    // for each such lot.
    let mut cells: Vec<Vec<Vec<usize>>> = vec![Vec::new(); fields.len()];
    for (fi, fd) in fields.iter().enumerate() {
        if !fd.summed {
            continue;
        }
        let mut keys: Vec<(usize, Vec<bool>)> = Vec::new();
        for (ci, cl) in classes.iter().enumerate() {
            let key = (cl[fi], w.reads.iter().filter(|r| r.kind == Kind::Sum && r.of == Some(fi)).map(|r| r.pass[ci]).collect::<Vec<bool>>());
            match keys.iter().position(|k| *k == key) {
                Some(i) => cells[fi][i].push(ci),
                None => {
                    keys.push(key);
                    cells[fi].push(vec![ci]);
                }
            }
        }
    }
    w.cells = cells;
    w
}

/// What holds of a contract's collections whatever is asked: every class holds a whole number of
/// elements, a total stays inside what its elements can come to, and the door takes each `count`
/// and `sum` only inside its range. A `count` and a `sum` are not unknowns of the system: each
/// stands for what its classes come to, which is what `defs` keeps, and the elimination is left
/// fewer names to go through.
fn base_of(g: &mut Group, c: &Checked) {
    let mut out: Vec<(Need, Ineq)> = Vec::new();
    let mut defs: BTreeMap<String, Lin> = BTreeMap::new();
    if let Some(ac) = &g.ac {
        out.extend(super::wire_ranges(ac, c).into_iter().map(|q| (Need::Always, q)));
    }
    for name in g.free.keys() {
        let (lo, hi) = c.ranges.get(name).copied().unwrap_or((None, None));
        if let Some(l) = lo {
            out.push((Need::Always, at_least(Lin::var(name), l)));
        }
        if let Some(h) = hi {
            out.push((Need::Always, at_most(Lin::var(name), h)));
        }
    }
    let terms = g.rel.terms();
    let named_in_rel = |g_ac: &Option<Across>, n: &str| terms.iter().any(|t| g_ac.as_ref().is_some_and(|a| a.name(t) == n));
    for (wi, w) in g.walks.iter().enumerate() {
        // Nothing is known of a collection whose fields would not cut, not even its length.
        if !w.fine {
            continue;
        }
        for ci in 0..w.classes.len() {
            out.push((Need::Walk(wi), at_least(Lin::var(&Walk::n(wi, ci)), Rat::zero())));
        }
        let length = w.count_of(wi, &|_| true);
        if named_in_rel(&g.ac, &w.size) {
            out.extend(equal(Lin::var(&w.size), &length).map(|q| (Need::Walk(wi), q)));
        } else {
            defs.insert(w.size.clone(), length.clone());
        }
        if let Some(k) = w.cap {
            out.push((Need::Walk(wi), at_most(length, Rat::int(k))));
        }
        for (fi, cells) in w.cells.iter().enumerate() {
            for (cell, classes) in cells.iter().enumerate() {
                if w.cell_value(fi, cell).is_some() {
                    continue;
                }
                let Some(p) = classes.first().and_then(|ci| w.fields[fi].parts.get(w.classes[*ci][fi])) else { continue };
                let (s, n) = (Lin::var(&Walk::s(wi, fi, cell)), w.count_of(wi, &|ci| classes.contains(&ci)));
                if let Some(l) = p.lo {
                    out.push((Need::Field(wi, fi), s.clone().plus(&n.clone().scale(l.mul(Rat::int(-1)))).ge(false)));
                }
                if let Some(h) = p.hi {
                    out.push((Need::Field(wi, fi), s.plus(&n.scale(h.mul(Rat::int(-1)))).le(false)));
                }
            }
        }
        for r in w.reads.iter().filter(|r| r.readable) {
            let form = match r.kind {
                Kind::Count => w.count_of(wi, &|ci| r.pass[ci]),
                Kind::Sum => {
                    let Some(fi) = r.of else { continue };
                    w.total_of(wi, fi, &|cell| w.cells[fi][cell].first().is_some_and(|ci| r.pass[*ci]))
                }
                Kind::Max | Kind::Min => {
                    // The door's range on an extreme, which is a name of its own.
                    let sc = Rat::int(c.wire_scale(&r.name));
                    let (lo, hi) = c.ranges.get(&r.name).copied().unwrap_or((None, None));
                    if let Some(l) = lo {
                        out.push((Need::Extreme(r.name.clone()), at_least(Lin::var(&r.name), l.mul(sc))));
                    }
                    if let Some(h) = hi {
                        out.push((Need::Extreme(r.name.clone()), at_most(Lin::var(&r.name), h.mul(sc))));
                    }
                    continue;
                }
                _ => continue,
            };
            let sc = Rat::int(c.wire_scale(&r.name));
            let (lo, hi) = c.ranges.get(&r.name).copied().unwrap_or((None, None));
            // A count is never below nothing, so a range from 0 says nothing more.
            if let Some(l) = lo.filter(|l| r.kind != Kind::Count || l.cmp_to(Rat::zero()).is_gt()) {
                out.push((Need::Read(r.name.clone()), at_least(form.clone(), l.mul(sc))));
            }
            if let Some(h) = hi {
                out.push((Need::Read(r.name.clone()), at_most(form.clone(), h.mul(sc))));
            }
            if *r.name != w.size || !named_in_rel(&g.ac, &w.size) {
                defs.insert(r.name.clone(), form);
            } else {
                out.extend(equal(Lin::var(&r.name), &form).map(|q| (Need::Always, q)));
            }
        }
    }
    g.base = out;
    g.defs = defs;
}

/// Whether a read the question names sums this field: the lots of a field nothing named sums need
/// no total.
fn summed_by_name(w: &Walk, fi: usize, named: &BTreeSet<String>) -> bool {
    w.reads.iter().any(|r| r.kind == Kind::Sum && r.of == Some(fi) && named.contains(&r.name))
}

/// An inequality with each name that stands for what classes come to written out as that.
fn written_out(q: &Ineq, defs: &BTreeMap<String, Lin>) -> Ineq {
    let mut out = Lin::con(q.k);
    for (n, k) in &q.terms {
        match defs.get(n) {
            Some(d) => out = out.plus(&d.clone().scale(*k)),
            None => out = out.plus(&Lin::var(n).scale(*k)),
        }
    }
    out.terms.retain(|_, k| k.num != 0);
    Ineq { terms: out.terms, k: out.k, strict: q.strict, origin: q.origin.clone() }
}

/// Each name that stands for what classes come to, at a point of the system.
fn with_defs(mut at: BTreeMap<String, Rat>, defs: &BTreeMap<String, Lin>) -> BTreeMap<String, Rat> {
    for (name, form) in defs {
        // A name the system did not go through says nothing here, and the read is worked out on
        // the elements in any case.
        let v = form.terms.iter().try_fold(form.k, |v, (n, k)| if k.num == 0 { Some(v) } else { at.get(n).map(|x| v.add(k.mul(*x))) });
        if let Some(v) = v {
            at.insert(name.clone(), v);
        }
    }
    at
}

/// The ways a `max` or a `min` can come out: no element passes and it is its `empty` value, or
/// the extreme lies in one part of the field's values and nothing that passes lies beyond that.
/// A field that is summed as well holds that one value and the rest inside the part.
fn ways_of(w: &Walk, wi: usize, r: &Read, summed: bool) -> Vec<(bool, Vec<Ineq>)> {
    let Some(fi) = r.of else { return Vec::new() };
    let fd = &w.fields[fi];
    let v = Lin::var(&r.name);
    let passing = w.count_of(wi, &|ci| r.pass[ci]);
    let mut out: Vec<(bool, Vec<Ineq>)> = Vec::new();
    let mut none = vec![at_most(passing, Rat::zero())];
    if let Some(e) = r.empty {
        none.extend(equal(v.clone(), &Lin::con(e)));
    }
    out.push((true, none));
    let parts = fd.parts.len();
    for pi in 0..parts {
        let of_part = |ci: usize, p: usize| w.classes[ci][fi] == p;
        if !(0..w.classes.len()).any(|ci| r.pass[ci] && of_part(ci, pi)) {
            continue;
        }
        let p = &fd.parts[pi];
        let mut way = vec![at_least(w.count_of(wi, &|ci| r.pass[ci] && of_part(ci, pi)), Rat::int(1))];
        if let Some(l) = p.lo {
            way.push(at_least(v.clone(), l));
        }
        if let Some(h) = p.hi {
            way.push(at_most(v.clone(), h));
        }
        for q in 0..parts {
            let beyond = if r.kind == Kind::Max { q > pi } else { q < pi };
            if beyond {
                way.push(at_most(w.count_of(wi, &|ci| r.pass[ci] && of_part(ci, q)), Rat::zero()));
            }
        }
        if summed {
            let total = w.total_of(wi, fi, &|cell| w.cells[fi][cell].first().is_some_and(|ci| of_part(*ci, pi)));
            let many = w.count_of(wi, &|ci| of_part(ci, pi));
            // total >= v + (many - 1) * lo, and total <= v + (many - 1) * hi
            let rest = |k: Rat| v.clone().plus(&many.clone().plus(&Lin::con(Rat::int(-1))).scale(k));
            if let Some(l) = p.lo {
                way.push(total.clone().plus(&rest(l).scale(Rat::int(-1))).ge(false));
            }
            if let Some(h) = p.hi {
                way.push(total.clone().plus(&rest(h).scale(Rat::int(-1))).le(false));
            }
        }
        out.push((false, way));
    }
    out
}

/// One question of one contract, or one sequence.
fn ask_group(g: &Group, c: &Checked, asks: &[&(String, Ask)], ineqs: &[&Ineq]) -> Possible {
    let mut x = Extra::default();
    let mut question: Vec<Ineq> = Vec::new();
    // A read asked about that is not read: the question is wider than what was asked, so nothing
    // found in it can be claimed as an example.
    let mut blind = false;
    let mut named: BTreeSet<String> = BTreeSet::new();
    for pair in asks {
        let (name, ask) = (&pair.0, &pair.1);
        named.insert(name.clone());
        if let Some(t) = g.ac.as_ref().and_then(|a| a.term(name)) {
            match ask {
                Ask::Truth(b) => x.bools.push((t.clone(), *b)),
                Ask::Words(vs) => x.strs.push((t.clone(), vs.clone())),
            }
            continue;
        }
        let Some((wi, r)) = g.read(name) else { continue };
        if !r.readable {
            blind = true;
            continue;
        }
        let w = &g.walks[wi];
        let yes = w.count_of(wi, &|ci| r.pass[ci]);
        let no = w.count_of(wi, &|ci| !r.pass[ci]);
        match (r.kind, ask) {
            (Kind::Any, Ask::Truth(true)) => question.push(at_least(yes, Rat::int(1))),
            (Kind::Any, Ask::Truth(false)) => question.push(at_most(yes, Rat::zero())),
            (Kind::All, Ask::Truth(true)) => question.push(at_most(no, Rat::zero())),
            (Kind::All, Ask::Truth(false)) => question.push(at_least(no, Rat::int(1))),
            _ => blind = true,
        }
    }
    for q in ineqs {
        question.push(written_out(q, &g.defs));
        for n in ineq_names(q) {
            named.insert(n.clone());
        }
    }
    let mut extremes: Vec<(usize, &Read)> = Vec::new();
    for n in &named {
        if let Some((wi, r)) = g.read(n) {
            if !r.readable {
                blind = true;
            } else if matches!(r.kind, Kind::Max | Kind::Min) {
                extremes.push((wi, r));
            }
        }
    }
    let parts = |r: &Read, wi: usize| r.of.map_or(0, |fi| g.walks[wi].fields[fi].parts.len()) + 1;
    if extremes.iter().map(|(wi, r)| parts(r, *wi)).fold(1usize, usize::saturating_mul) > WAYS {
        extremes.clear();
        blind = true;
    }
    let q = Question { g, c, named: &named, x: &x, blind, asks, ineqs, asked: &question, extremes: &extremes };
    let narrow = g.base_for(&named);
    let all = g.base_whole(&named);
    if narrow.len() < all.len() {
        match q.pass(&narrow, false) {
            Possible::Unknown => {}
            decided => return decided,
        }
    }
    match q.pass(&all, true) {
        Possible::Unknown => q.searched().map_or(Possible::Unknown, Possible::Yes),
        decided => decided,
    }
}

/// A question, ready to be put to a base.
struct Question<'q> {
    g: &'q Group,
    c: &'q Checked,
    named: &'q BTreeSet<String>,
    x: &'q Extra,
    blind: bool,
    asks: &'q [&'q (String, Ask)],
    ineqs: &'q [&'q Ineq],
    /// The question, written out over the classes.
    asked: &'q [Ineq],
    extremes: &'q [(usize, &'q Read)],
}

impl Question<'_> {
    /// The question over a base. `whole` is whether the base is all of it: what `sum` reads the
    /// question does not name come to is then part of the system, and not only worked out on the
    /// elements afterwards.
    fn pass(&self, base: &[Ineq], whole: bool) -> Possible {
        let (g, c, named, x, blind) = (self.g, self.c, self.named, self.x, self.blind);
        let extremes = self.extremes;
        let summed = |wi: usize, r: &Read| r.of.is_some_and(|fi| if whole { g.walks[wi].fields[fi].summed } else { summed_by_name(&g.walks[wi], fi, named) });
        let ways: Vec<Vec<(bool, Vec<Ineq>)>> = extremes.iter().map(|(wi, r)| ways_of(&g.walks[*wi], *wi, r, summed(*wi, r))).collect();
        let mut sys: Vec<Ineq> = base.to_vec();
        sys.extend(self.asked.iter().cloned());
        let unsure = classes_unsure(g, named);
        let name = |t: &Term| g.name(t);
        let extra = |ineqs: Vec<Ineq>| Extra { ineqs: tidied(ineqs), strs: x.strs.clone(), bools: x.bools.clone() };
        let mut proofs = Vec::new();
        let mut undecided = false;
        let mut pick = vec![0usize; ways.len()];
        loop {
            let mut one = sys.clone();
            for (k, &i) in pick.iter().enumerate() {
                one.extend(ways[k][i].1.iter().cloned());
            }
            if !ways.is_empty() {
                one.extend(between_extremes(g, extremes, &ways, &pick));
            }
            // An example is made only of values that can be claimed, where there are others.
            if !blind && !unsure.is_empty() {
                let mut claimed = one.clone();
                claimed.extend(unsure.iter().map(|n| at_most(Lin::var(n), Rat::zero())));
                if let Found::Point(at, strs, truth) = g.rel.find(&extra(claimed.clone()), &name)
                    && let Some(ex) = realize(g, c, named, &claimed, x, at, strs, truth, whole)
                {
                    return Possible::Yes(ex);
                }
            }
            match g.rel.find(&extra(one.clone()), &name) {
                Found::None(p) => proofs.extend(p),
                Found::Unknown => undecided = true,
                Found::Point(at, strs, truth) => {
                    if !blind
                        && unsure.is_empty()
                        && let Some(ex) = realize(g, c, named, &one, x, at, strs, truth, whole)
                    {
                        return Possible::Yes(ex);
                    }
                    undecided = true;
                }
            }
            // The next case.
            let mut carried = true;
            for k in 0..pick.len() {
                pick[k] += 1;
                if pick[k] < ways[k].len() {
                    carried = false;
                    break;
                }
                pick[k] = 0;
            }
            if carried {
                break;
            }
        }
        if undecided { Possible::Unknown } else { Possible::No(proofs) }
    }

    /// Small collections, tried one by one. The system is wider than the truth: it holds a total
    /// and a `max` over one field apart, and an extreme is one element while the system counts
    /// elements, so its point may stand for nothing where a request does exist. `max > min` needs
    /// two elements and the system is content with one. What comes out is built and worked out
    /// like any other example, and finding nothing here says nothing.
    fn searched(&self) -> Option<Example> {
        let (g, named) = (self.g, self.named);
        if self.blind || !classes_unsure(g, named).is_empty() {
            return None;
        }
        // One collection; the rest of the question is what the contract says beside it.
        let asked: Vec<usize> = (0..g.walks.len()).filter(|wi| named.contains(&g.walks[*wi].size) || g.walks[*wi].reads.iter().any(|r| named.contains(&r.name))).collect();
        let [wi] = asked.as_slice() else { return None };
        let w = &g.walks[*wi];
        if !w.fine {
            return None;
        }
        // The numbers the question and the reads mention, and their neighbours, and the smallest
        // there are for a field with no bound.
        let mut numbers: Vec<Rat> = w.reads.iter().filter(|r| named.contains(&r.name)).filter_map(|r| r.empty).collect();
        numbers.extend([0, 1, -1].map(Rat::int));
        for q in self.ineqs {
            let mut terms = q.terms.iter().filter(|(_, c)| c.num != 0);
            if let (Some((_, c)), None) = (terms.next(), terms.next()) {
                numbers.push(q.k.mul(Rat::int(-1)).div(*c));
            }
        }
        for v in numbers.clone() {
            numbers.push(v.add(Rat::int(1)));
            numbers.push(v.sub(Rat::int(1)));
        }
        let kinds = kinds_of(w, &numbers);
        let mut tries = SEARCH_TRIES;
        for size in 0..=SEARCH_ELEMENTS {
            if let Some(ex) = self.try_collections(*wi, &kinds, size, 0, &mut Vec::new(), &mut tries) {
                return Some(ex);
            }
        }
        None
    }

    /// Every collection of `left` more elements, picked from `kinds` from `from` on.
    fn try_collections(&self, wi: usize, kinds: &[Vec<Val>], left: usize, from: usize, have: &mut Vec<Vec<Val>>, tries: &mut usize) -> Option<Example> {
        if left == 0 {
            if *tries == 0 {
                return None;
            }
            *tries -= 1;
            return self.try_collection(wi, have);
        }
        for i in from..kinds.len() {
            have.push(kinds[i].clone());
            let found = self.try_collections(wi, kinds, left - 1, i, have, tries);
            have.pop();
            if found.is_some() {
                return found;
            }
        }
        None
    }

    /// Whether these elements answer the question, and the rest of the request with them.
    fn try_collection(&self, wi: usize, elements: &[Vec<Val>]) -> Option<Example> {
        let g = self.g;
        let w = &g.walks[wi];
        let length = Rat::int(elements.len() as i128);
        if w.cap.is_some_and(|k| Rat::int(k).cmp_to(length).is_lt()) {
            return None;
        }
        let mut at: BTreeMap<String, Rat> = BTreeMap::from([(w.size.clone(), length)]);
        for r in w.reads.iter().filter(|r| self.named.contains(&r.name)) {
            match came_to(w, r, elements)? {
                Came::Number(v) => {
                    at.insert(r.name.clone(), v);
                }
                Came::Truth(b) => {
                    if self.asks.iter().any(|p| p.0 == r.name && !matches!(p.1, Ask::Truth(want) if want == b)) {
                        return None;
                    }
                }
            }
        }
        if self.ineqs.iter().any(|q| q.holds_at(&at) == Some(false)) {
            return None;
        }
        let mut sys: Vec<Ineq> = g.base.iter().filter(|(need, _)| matches!(need, Need::Always)).map(|(_, q)| q.clone()).collect();
        sys.extend(self.ineqs.iter().map(|q| (*q).clone()));
        for (n, v) in &at {
            sys.extend(equal(Lin::var(n), &Lin::con(*v)));
        }
        let extra = Extra { ineqs: tidied(sys), strs: self.x.strs.clone(), bools: self.x.bools.clone() };
        let Found::Point(mut point, strs, truth) = g.rel.find(&extra, &|t: &Term| g.name(t)) else { return None };
        point.extend(at);
        build(g, self.c, self.named, &point, &strs, &truth, Some((wi, elements)))
    }
}

/// The different elements a search picks from: each class, with the numbers a read of a field
/// could want from it.
fn kinds_of(w: &Walk, numbers: &[Rat]) -> Vec<Vec<Val>> {
    let mut out: Vec<Vec<Val>> = Vec::new();
    for cl in &w.classes {
        let mut here: Vec<Vec<Val>> = vec![Vec::new()];
        for (fi, fd) in w.fields.iter().enumerate() {
            let p = &fd.parts[cl[fi]];
            let mut vals: Vec<Val> = vec![p.sample.clone()];
            if fd.of && matches!(fd.dom, Dom::Whole(_)) {
                let inside = |v: &Rat| v.is_int() && p.lo.is_none_or(|l| l.cmp_to(*v).is_le()) && p.hi.is_none_or(|h| h.cmp_to(*v).is_ge());
                for v in p.lo.iter().chain(p.hi.iter()).chain(numbers.iter()).filter(|v| inside(v)) {
                    let v = Val::Num(*v);
                    if !vals.contains(&v) {
                        vals.push(v);
                    }
                }
            }
            let mut next = Vec::with_capacity(here.len() * vals.len());
            for e in &here {
                for v in &vals {
                    let mut x = e.clone();
                    x.push(v.clone());
                    next.push(x);
                }
            }
            here = next;
        }
        out.extend(here.into_iter().filter(|e| element_passes(w, e)));
    }
    out.truncate(SEARCH_KINDS);
    out
}

/// A system without the terms a coefficient of zero left in it, and without the inequalities it
/// says twice. Elimination pairs what it is given, so these are what would take it past its limit.
fn tidied(sys: Vec<Ineq>) -> Vec<Ineq> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    sys.into_iter()
        .map(|mut q| {
            q.terms.retain(|_, k| k.num != 0);
            q
        })
        .filter(|q| seen.insert(format!("{:?}|{:?}|{}", q.terms, q.k, q.strict)))
        .collect()
}

/// The classes of the collections whose values a named read looks at and cannot show to pass, by
/// the name of their count.
fn classes_unsure(g: &Group, named: &BTreeSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    for (wi, w) in g.walks.iter().enumerate() {
        let leans = w.reads.iter().any(|r| named.contains(&r.name) && uses_unsure(w, r));
        if leans {
            out.extend((0..w.classes.len()).map(|ci| Walk::n(wi, ci)));
        }
    }
    out
}

/// Whether a read looks at a field whose values cannot be shown to pass.
fn uses_unsure(w: &Walk, r: &Read) -> bool {
    r.test.is_some_and(|(fi, _)| w.fields[fi].unsure) || r.of.is_some_and(|fi| w.fields[fi].unsure)
}

/// What two extremes of one field say of each other: when every element the second looks at is
/// one the first looks at too, the second's value is among the first's.
fn between_extremes(g: &Group, extremes: &[(usize, &Read)], ways: &[Vec<(bool, Vec<Ineq>)>], pick: &[usize]) -> Vec<Ineq> {
    let mut out = Vec::new();
    for (a, (wa, ra)) in extremes.iter().enumerate() {
        for (b, (wb, rb)) in extremes.iter().enumerate() {
            if a == b || wa != wb || ra.of != rb.of || ways[b][pick[b]].0 {
                continue;
            }
            let classes = g.walks[*wa].classes.len();
            if !(0..classes).all(|ci| !rb.pass[ci] || ra.pass[ci]) {
                continue;
            }
            let d = Lin::var(&rb.name).plus(&Lin::var(&ra.name).scale(Rat::int(-1)));
            out.push(if ra.kind == Kind::Max { d.le(false) } else { d.ge(false) });
        }
    }
    out
}

/// The elements a point of the system stands for, if there are any: fewer where fewer do, since a
/// person reads the example. A point can stand for nothing, because the system is wider than the
/// truth; then the counts are held where they are and what `max` and `min` need of a total, which
/// is no longer a product of two unknowns, is added and the system asked again.
#[allow(clippy::too_many_arguments)]
fn realize(g: &Group, c: &Checked, named: &BTreeSet<String>, sys: &[Ineq], x: &Extra, at: BTreeMap<String, Rat>, strs: BTreeMap<Term, String>, truth: BTreeMap<Term, bool>, whole: bool) -> Option<Example> {
    let name = |t: &Term| g.name(t);
    let again = |ineqs: Vec<Ineq>| g.rel.find(&Extra { ineqs: tidied(ineqs), strs: x.strs.clone(), bools: x.bools.clone() }, &name);
    let first: Vec<Ineq> = sys.to_vec();
    // Fewer elements: each class at nothing, or at one, where the question still holds.
    let mut sys: Vec<Ineq> = sys.to_vec();
    let (mut at, mut strs, mut truth) = (at, strs, truth);
    for (wi, w) in g.walks.iter().enumerate() {
        for ci in 0..w.classes.len() {
            let n = Walk::n(wi, ci);
            let was = at.get(&n).copied().unwrap_or(Rat::zero());
            if was.cmp_to(Rat::int(1)).is_le() {
                sys.push(at_most(Lin::var(&n), was));
                continue;
            }
            for k in [0, 1] {
                let mut less = sys.clone();
                less.push(at_most(Lin::var(&n), Rat::int(k)));
                if let Found::Point(a, s, t) = again(less.clone()) {
                    sys = less;
                    at = a;
                    strs = s;
                    truth = t;
                    break;
                }
            }
        }
    }
    if let Some(ex) = build(g, c, named, &with_defs(at.clone(), &g.defs), &strs, &truth, None) {
        return Some(ex);
    }
    // A total with no elements under it: a field with no bound lets the system say so, since "no
    // elements, so no total" is not linear. Each such lot gets an element.
    let mut filled = first;
    let mut empty_lot = false;
    for (wi, w) in g.walks.iter().enumerate() {
        for (fi, cells) in w.cells.iter().enumerate() {
            for (cell, classes) in cells.iter().enumerate() {
                let total = at.get(&Walk::s(wi, fi, cell)).copied().unwrap_or(Rat::zero());
                let held = classes.iter().any(|ci| at.get(&Walk::n(wi, *ci)).is_some_and(|n| n.num != 0));
                if total.num != 0 && !held {
                    filled.push(at_least(w.count_of(wi, &|ci| classes.contains(&ci)), Rat::int(1)));
                    empty_lot = true;
                }
            }
        }
    }
    if empty_lot
        && let Found::Point(a, s, t) = again(filled)
        && let Some(ex) = build(g, c, named, &with_defs(a, &g.defs), &s, &t, None)
    {
        return Some(ex);
    }
    let mut fixed = sys.clone();
    for (wi, w) in g.walks.iter().enumerate() {
        for ci in 0..w.classes.len() {
            let n = Walk::n(wi, ci);
            let k = at.get(&n).copied().unwrap_or(Rat::zero());
            fixed.extend(equal(Lin::var(&n), &Lin::con(k)));
        }
        for r in w.reads.iter().filter(|r| r.readable && named.contains(&r.name) && matches!(r.kind, Kind::Max | Kind::Min)) {
            let Some(fi) = r.of else { continue };
            if !whole && !summed_by_name(w, fi, named) {
                continue;
            }
            for (cell, classes) in w.cells[fi].iter().enumerate() {
                if !classes.iter().all(|ci| r.pass[*ci]) {
                    continue;
                }
                let mut k = Rat::zero();
                for ci in classes {
                    k = k.add(at.get(&Walk::n(wi, *ci)).copied().unwrap_or(Rat::zero()));
                }
                if k.num == 0 {
                    continue;
                }
                let d = w.cell_total(wi, fi, cell).plus(&Lin::var(&r.name).scale(k.mul(Rat::int(-1))));
                fixed.push(if r.kind == Kind::Max { d.le(false) } else { d.ge(false) });
            }
        }
    }
    match again(fixed) {
        Found::Point(a, s, t) => build(g, c, named, &with_defs(a, &g.defs), &s, &t, None),
        _ => None,
    }
}

/// What a read comes to on some elements.
enum Came {
    Number(Rat),
    Truth(bool),
}

fn came_to(w: &Walk, r: &Read, elements: &[Vec<Val>]) -> Option<Came> {
    let passing: Vec<&Vec<Val>> = elements.iter().filter(|e| r.test.is_none_or(|(fi, ti)| w.fields[fi].tests[ti].as_ref().is_some_and(|t| t.holds(&e[fi])))).collect();
    let of_num = |e: &Vec<Val>, fi: usize| if let Val::Num(k) = &e[fi] { Some(*k) } else { None };
    Some(match r.kind {
        Kind::Any => Came::Truth(!passing.is_empty()),
        Kind::All => Came::Truth(passing.len() == elements.len()),
        Kind::Count => Came::Number(Rat::int(passing.len() as i128)),
        Kind::Sum => {
            let fi = r.of?;
            let mut t = Rat::zero();
            for e in &passing {
                t = t.checked_add(of_num(e, fi)?)?;
            }
            Came::Number(t)
        }
        Kind::Max | Kind::Min => {
            let fi = r.of?;
            let mut best: Option<Rat> = None;
            for e in &passing {
                let v = of_num(e, fi)?;
                let better = best.is_none_or(|b| if r.kind == Kind::Max { v.cmp_to(b).is_gt() } else { v.cmp_to(b).is_lt() });
                if better {
                    best = Some(v);
                }
            }
            Came::Number(best.or(r.empty)?)
        }
    })
}

/// The elements a point stands for, or the elements `given` for one collection, and what every
/// read comes to on them: evaluated, and refused where it is not what the point says.
fn build(g: &Group, c: &Checked, named: &BTreeSet<String>, at: &BTreeMap<String, Rat>, strs: &BTreeMap<Term, String>, truth: &BTreeMap<Term, bool>, given: Option<(usize, &[Vec<Val>])>) -> Option<Example> {
    let mut ex = Example::default();
    // The plain values the question named.
    if let Some(ac) = &g.ac {
        for n in named {
            let Some(t) = ac.term(n) else { continue };
            let path = t.path();
            let shape = g.shape.clone().unwrap_or_default();
            let place = match t {
                Term::Size(_) => None,
                Term::Field(_) => Some(format!("{shape}.{}", path.join("."))),
            };
            if let Some(v) = at.get(n) {
                ex.values.insert(n.clone(), *v);
                if let Some(p) = place {
                    ex.plain.push((p, shown_val(&Val::Num(*v), false)));
                }
            } else if let (Some(s), Some(p)) = (strs.get(t), &place) {
                ex.plain.push((p.clone(), crate::json::quote(s)));
            } else if let (Some(b), Some(p)) = (truth.get(t), &place) {
                ex.plain.push((p.clone(), b.to_string()));
            }
        }
    }
    for (n, place) in &g.free {
        if named.contains(n) {
            let v = *at.get(n)?;
            ex.values.insert(n.clone(), v);
            ex.plain.push((place.clone(), shown_val(&Val::Num(v), true)));
        }
    }
    for (wi, w) in g.walks.iter().enumerate() {
        if !w.reads.iter().any(|r| named.contains(&r.name)) && !named.contains(&w.size) {
            continue;
        }
        if !w.fine {
            return None;
        }
        let elements = match given {
            Some((i, es)) if i == wi => es.to_vec(),
            _ => elements_of(w, wi, named, at)?,
        };
        // Every read comes to what the point says, on the elements themselves.
        for r in &w.reads {
            if !r.readable || (uses_unsure(w, r) && !named.contains(&r.name)) {
                continue;
            }
            match came_to(w, r, &elements)? {
                Came::Number(v) => {
                    if (matches!(r.kind, Kind::Count | Kind::Sum) || named.contains(&r.name)) && at.get(&r.name).is_some_and(|k| *k != v) {
                        return None;
                    }
                    let sc = Rat::int(c.wire_scale(&r.name));
                    let (lo, hi) = c.ranges.get(&r.name).copied().unwrap_or((None, None));
                    if lo.is_some_and(|l| v.cmp_to(l.mul(sc)).is_lt()) || hi.is_some_and(|h| v.cmp_to(h.mul(sc)).is_gt()) {
                        return None;
                    }
                    ex.values.insert(r.name.clone(), v);
                }
                Came::Truth(holds) => {
                    ex.truths.insert(r.name.clone(), holds);
                }
            }
        }
        if elements.iter().any(|e| !element_passes(w, e)) {
            return None;
        }
        if at.get(&w.size).is_some_and(|k| *k != Rat::int(elements.len() as i128)) {
            return None;
        }
        // Only what the question reads is shown.
        let shown_fields: Vec<usize> = (0..w.fields.len())
            .filter(|fi| w.reads.iter().any(|r| named.contains(&r.name) && (r.test.is_some_and(|(f, _)| f == *fi) || r.of == Some(*fi))))
            .collect();
        let mut kinds: Vec<(i128, Vec<(String, String)>)> = Vec::new();
        for e in &elements {
            let key: Vec<(String, String)> = shown_fields.iter().map(|fi| (w.fields[*fi].name.clone(), shown_val(&e[*fi], w.fields[*fi].date))).collect();
            match kinds.iter_mut().find(|(_, k)| *k == key) {
                Some((n, _)) => *n += 1,
                None => kinds.push((1, key)),
            }
        }
        ex.walks.push(Shown { path: w.shown.clone(), elements: kinds });
    }
    // What was asked of an `any` or an `all` is what the elements give.
    Some(ex)
}

/// The elements of one collection at a point: each field of each element given a value, the
/// fields `sum`, `max` and `min` read so that they come to what the point says.
fn elements_of(w: &Walk, wi: usize, named: &BTreeSet<String>, at: &BTreeMap<String, Rat>) -> Option<Vec<Vec<Val>>> {
    let mut how_many: Vec<i128> = Vec::with_capacity(w.classes.len());
    let mut total = 0i128;
    for ci in 0..w.classes.len() {
        let n = at.get(&Walk::n(wi, ci)).copied().unwrap_or(Rat::zero());
        if !n.is_int() || n.num < 0 {
            return None;
        }
        how_many.push(n.num);
        total = total.checked_add(n.num)?;
    }
    if total > MOST {
        return None;
    }
    let mut elements: Vec<Vec<Val>> = Vec::with_capacity(total as usize);
    let mut class_of: Vec<usize> = Vec::with_capacity(total as usize);
    for (ci, cl) in w.classes.iter().enumerate() {
        for _ in 0..how_many[ci] {
            elements.push(w.fields.iter().enumerate().map(|(fi, fd)| fd.parts[cl[fi]].sample.clone()).collect());
            class_of.push(ci);
        }
    }
    for (fi, fd) in w.fields.iter().enumerate() {
        if !fd.of || !matches!(fd.dom, Dom::Whole(_)) {
            continue;
        }
        let mut value: Vec<Option<Rat>> = vec![None; elements.len()];
        // How far each class's values can go: its part, and every extreme that looks at it.
        let mut lo: Vec<Option<Rat>> = w.classes.iter().map(|cl| fd.parts[cl[fi]].lo).collect();
        let mut hi: Vec<Option<Rat>> = w.classes.iter().map(|cl| fd.parts[cl[fi]].hi).collect();
        let extremes: Vec<&Read> = w.reads.iter().filter(|r| r.readable && r.of == Some(fi) && named.contains(&r.name) && matches!(r.kind, Kind::Max | Kind::Min)).collect();
        for r in &extremes {
            let v = *at.get(&r.name)?;
            for ci in 0..w.classes.len() {
                if !r.pass[ci] {
                    continue;
                }
                if r.kind == Kind::Max {
                    hi[ci] = Some(hi[ci].map_or(v, |h| if h.cmp_to(v).is_le() { h } else { v }));
                } else {
                    lo[ci] = Some(lo[ci].map_or(v, |l| if l.cmp_to(v).is_ge() { l } else { v }));
                }
            }
        }
        // Each extreme that found an element has one element at its value.
        for r in &extremes {
            if !(0..w.classes.len()).any(|ci| r.pass[ci] && how_many[ci] > 0) {
                continue;
            }
            let v = *at.get(&r.name)?;
            let at_class = |e: usize| class_of[e];
            if (0..elements.len()).any(|e| r.pass[at_class(e)] && value[e] == Some(v)) {
                continue;
            }
            let fits = |ci: usize| r.pass[ci] && lo[ci].is_none_or(|l| l.cmp_to(v).is_le()) && hi[ci].is_none_or(|h| h.cmp_to(v).is_ge());
            let slot = (0..elements.len()).find(|e| value[*e].is_none() && fits(at_class(*e)))?;
            value[slot] = Some(v);
        }
        let start = |ci: usize| -> Option<Rat> {
            Some(match (lo[ci], hi[ci]) {
                (Some(l), Some(h)) if l.cmp_to(h).is_gt() => return None,
                (Some(l), _) => l,
                (None, Some(h)) => h,
                (None, None) => Rat::zero(),
            })
        };
        if fd.summed {
            for (cell, classes) in w.cells[fi].iter().enumerate() {
                // A lot the system holds no total for: one value, or one nothing asks about.
                let Some(mut left) = at.get(&Walk::s(wi, fi, cell)).copied() else { continue };
                let slots: Vec<usize> = (0..elements.len()).filter(|e| classes.contains(&class_of[*e])).collect();
                let free: Vec<usize> = slots.iter().copied().filter(|e| value[*e].is_none()).collect();
                for e in &slots {
                    if let Some(v) = value[*e] {
                        left = left.checked_sub(v)?;
                    }
                }
                for e in &free {
                    let v = start(class_of[*e])?;
                    value[*e] = Some(v);
                    left = left.checked_sub(v)?;
                }
                for e in &free {
                    if left.num == 0 {
                        break;
                    }
                    let (v, ci) = (value[*e]?, class_of[*e]);
                    let by = if left.num > 0 {
                        match hi[ci].map(|h| h.sub(v)) {
                            Some(room) if room.cmp_to(left).is_lt() => room,
                            _ => left,
                        }
                    } else {
                        match lo[ci].map(|l| l.sub(v)) {
                            Some(room) if room.cmp_to(left).is_gt() => room,
                            _ => left,
                        }
                    };
                    value[*e] = Some(v.add(by));
                    left = left.sub(by);
                }
                if left.num != 0 {
                    return None;
                }
            }
        }
        for e in 0..elements.len() {
            let v = match value[e] {
                Some(v) => v,
                None => start(class_of[e])?,
            };
            elements[e][fi] = Val::Num(v);
        }
    }
    Some(elements)
}

/// W133: rows that only a combination the definitions never give reaches.
pub fn impossible_rows(f: &RuleFile, c: &Checked, rule_path: &str, skip: &BTreeSet<(String, usize)>) -> Vec<Diag> {
    let mut out = Vec::new();
    if !f.inputs.iter().any(|i| i.from.as_ref().is_some_and(|p| !matches!(p.kind, ProjKind::Field))) && f.aggs().next().is_none() {
        return out;
    }
    let walks = Walks::of(f, c, rule_path);
    if walks.is_empty() {
        return out;
    }
    let held = walks.constraints();
    for it in &f.items {
        let Item::Table(t) = it else { continue };
        if t.applied.is_some() {
            continue;
        }
        let tname = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        for (ri, row) in t.rows.iter().enumerate() {
            if skip.contains(&(tname.clone(), row.index)) {
                continue;
            }
            let mut asks: Vec<(String, Ask)> = Vec::new();
            let mut ineqs: Vec<Ineq> = held.clone();
            let mut read: Vec<String> = Vec::new();
            for (ci, (col, _)) in t.inputs.iter().enumerate() {
                if !walks.knows(col) {
                    continue;
                }
                let Some(cell) = row.cells.get(ci) else { continue };
                if matches!(cell, Cell::DontCare) {
                    continue;
                }
                let ty = c.ty_of(col).unwrap_or(Ty::Unknown);
                let took = match &ty {
                    Ty::Bool => match truth_of(cell) {
                        Some(b) => {
                            asks.push((col.clone(), Ask::Truth(b)));
                            true
                        }
                        None => false,
                    },
                    Ty::Enum(e) => {
                        let all = c.enums.get(e).cloned().unwrap_or_default();
                        match super::cell_values(cell, c, &all) {
                            Some(vs) => {
                                asks.push((col.clone(), Ask::Words(vs)));
                                true
                            }
                            None => false,
                        }
                    }
                    _ => match super::cell_ineqs(cell, &ty, c.wire_scale(col), col, ri) {
                        Some(qs) => {
                            ineqs.extend(qs);
                            true
                        }
                        None => false,
                    },
                };
                if took && walks.walks_for(col) {
                    read.push(col.clone());
                }
            }
            if read.is_empty() || !matches!(walks.ask(&asks, &ineqs), Possible::No(_)) {
                continue;
            }
            let (kind, rn) = if t.clause {
                (tr!("節", "clause"), tr!("節 {tname}", "clause {tname}"))
            } else {
                let base = tr!("行{}", "row {}", row.index);
                (
                    tr!("表", "table"),
                    match &row.label {
                        Some(l) => tr!("{base}（{}）", "{base} ({})", l.text),
                        None => base,
                    },
                )
            };
            let mut d = Diag::warning(
                "W133",
                tr!("{rn} は、定義からは起こらない組み合わせでしか当たりません", "{rn} is reached only by a combination the definitions never give"),
            )
            .at(format!("{rule_path}:{} {kind} {tname}", row.span.line))
            .table(tname.clone())
            .row(row.index)
            .rowref(tname.clone(), row.index)
            .key(format!("W133\u{1}{tname}\u{1}{}", crate::region::row_key(row)))
            .fix_kind(FixKind::None)
            .mark(row.span.clone(), tr!("{} の定義からは、この組み合わせになりません", "the definitions of {} never give this combination", read.join(", ")));
            let notes = walks.never_notes(&read);
            for n in notes.iter().take(1) {
                d = d.note(n.clone());
            }
            let from = read.iter().any(|n| walks.groups.iter().any(|g| g.read(n).is_some_and(|(_, r)| r.from)));
            let summed = read.iter().any(|n| walks.groups.iter().any(|g| g.read(n).is_some_and(|(_, r)| !r.from)));
            if from {
                let targets = without_from();
                d = d.note(tr!(
                    "`from` を読む呼び出しは、この行に当たりません。{targets} の生成コードは `from` を読まないので、呼ぶ側が計算した値だけがここに届きます。",
                    "A call that reads through `from` never reaches this row. The code generated for {targets} does not read `from`, so only values a caller worked out reach it."
                ));
            }
            if summed {
                d = d.note(tr!(
                    "どの生成先でも、集計の値は並びから計算するので、この行に当たる呼び出しはありません。",
                    "Every target works a summary out from the sequence, so no call reaches this row."
                ));
            }
            d = d.note(tr!(
                "ヒント: この行を消しても E101 が出ないなら、消してください。E101 が出るなら、表の完全性にこの行が要るので、残してください。",
                "hint: if E101 does not come back without this row, delete it. If it does, the table needs the row to be complete, so keep it."
            ));
            out.push(d);
        }
    }
    out
}

/// The truth value a cell on a boolean column asks for, where it asks for one.
fn truth_of(cell: &Cell) -> Option<bool> {
    let word = |l: &Lit| match l {
        Lit::Word(w) if w == crate::kw::TRUE => Some(true),
        Lit::Word(w) if w == crate::kw::FALSE => Some(false),
        _ => None,
    };
    match cell {
        Cell::Lit(l) => word(l),
        Cell::Set(ls) if ls.len() == 1 => word(&ls[0]),
        Cell::Not(ls) if ls.len() == 1 => word(&ls[0]).map(|b| !b),
        _ => None,
    }
}
