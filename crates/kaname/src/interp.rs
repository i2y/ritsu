//! The reference interpreter (DESIGN 8): what a module means. The `examples` of `check`, `run`,
//! `vectors`, and the generated code's comparison all go through it.
//!
//! It evaluates a module that passes its check, reading what the check found of every expression
//! (its type and what its names name, `check::Checked::typed`). A value is exact ([`Value`]); every
//! value an operation makes is held to the rule of int64 (`overflow`), and a division by 0 is the
//! failure `zero` (DESIGN 5.6). The layers kaname calls are evaluated by their own languages, through
//! the ports: a rule by `Rules::eval`, a date by `Dates::eval`; a transfer is a value that is built
//! and never made (DESIGN 6.3).
//!
//! - The order (DESIGN 8): the arguments and the fields as written, from left to right, then the
//!   places they go to held to what they declare, in the order declared; the `let` lines from the top
//!   down; `and` and `or` from the left, the right not evaluated once the left decides. `any`, `all`
//!   and `first` stop at the first element that decides them.
//! - The places (DESIGN 4.2, 4.4): the arguments of a function called by its host are held to their
//!   types, ranges and counts before anything is computed (`input`); a value given to a parameter of
//!   another function of kaname, a function's answer and a field of a record of a module are held to
//!   what they declare (`range`), as an amount of a transfer is to 0 and above. Where a parameter
//!   reaches a value, a rate with a step and no range written is held to 0% to 100%, as rulec holds
//!   such an input (DESIGN 3.9), and a number or a date whose range is not written to the whole of its
//!   type (DESIGN 22.1).
//! - A failure stops the evaluation there: its kind, the module and the line, what it says, and the
//!   calls it is in. A call deeper than [`DEPTH`] stops it too: the function did not stop, which is no
//!   failure but what `diverges` allows (DESIGN 8).

use crate::ast::*;
use crate::check::Checked;
use crate::num::{self, Arith, Rat, Rounding};
use crate::suite::Suite;
use crate::ty::*;
use crate::types::Ref;
use crate::value::{Closure, Program, TransferValue, Value, num_text, range_text, wire_int};
use ritsu_base::json::Json;
use ritsu_base::text::Text;
use ritsu_ports::{DateValue, RuleError, Value as Wire, Values};
use std::cmp::Ordering;

/// How deep the calls of kaname's functions go before the evaluation stops (DESIGN 8): deep enough for
/// the recursion a module of glue code writes, and shallow enough that the generated Python, which
/// CPython stops at a thousand frames, answers every input the reference interpreter answers (stage
/// D measures it).
pub const DEPTH: usize = 200;

/// How deep a value a host gives may nest the values of records and enums that hold themselves
/// (DESIGN 28.1): counted along each path from the argument, a value of such a type one deeper. The
/// generated code holds an argument to it at the entry and compares and walks such values by
/// recursion, which CPython stops at a thousand frames.
pub const NESTED: usize = 200;

/// How deep `run` reads the JSON of its input at most (DESIGN 11.1): it reads as deep as the arguments'
/// JSON can nest with their values of records and enums that hold themselves one deeper than
/// [`NESTED`], and no deeper than this. A step of such a type that takes five levels of JSON or fewer
/// (a list, a dict, a value of a sum with fields) goes 201 deep within it, and the reading, which
/// recurses, stays inside half the stack a process's main thread is given (8 MiB) in a debug build.
pub const READ_DEPTH: usize = 1024;

/// How a value is held to what its place declares (DESIGN 4.4, 8, 28.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hold {
    /// Where a value is put: a field of a record built, a function's answer. What the place declares of
    /// the value and of its elements.
    Place,
    /// Where a function of kaname is called: what a parameter reaches too, the fields of the records
    /// and the values of the enums it holds, a rate with a step and no range held to 0% to 100%; but not
    /// inside a value of a record or an enum that holds itself, whose fields were held to what they
    /// declare where it was built or came in.
    Call,
    /// Where a host calls a function: what a parameter reaches, at every depth.
    Entry,
}

/// The kind of a failure: the five of the effects (DESIGN 4.2), and `input`, a host's argument
/// outside its function's types (DESIGN 4.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Input,
    Rule,
    Date,
    Range,
    Overflow,
    Zero,
}

impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Kind::Input => "input",
            Kind::Rule => "rule",
            Kind::Date => "date",
            Kind::Range => "range",
            Kind::Overflow => "overflow",
            Kind::Zero => "zero",
        }
    }

    pub fn of(k: FailKind) -> Kind {
        match k {
            FailKind::Rule => Kind::Rule,
            FailKind::Date => Kind::Date,
            FailKind::Range => Kind::Range,
            FailKind::Overflow => Kind::Overflow,
            FailKind::Zero => Kind::Zero,
        }
    }
}

/// Where an evaluation failed and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub kind: Kind,
    /// The module, by its name, and its file as the check was given it.
    pub module: String,
    pub file: String,
    pub line: usize,
    pub message: Text,
    /// The functions it failed in, the first called first: each its name and the line it was called
    /// from (0 for the one the host called).
    pub stack: Vec<(String, usize)>,
    /// The expression whose operation failed (a value that leaves int64, a division by 0), by its
    /// id in the module: where the search for an example (DESIGN 5.7) finds the place it is after.
    pub expr: Option<usize>,
}

/// An evaluation that did not stop: the call that went deeper than [`DEPTH`].
#[derive(Clone, Debug, PartialEq)]
pub struct Diverged {
    pub function: String,
    pub module: String,
    pub line: usize,
    pub depth: usize,
}

/// Why an evaluation ends with no answer.
#[derive(Clone, Debug, PartialEq)]
pub enum Stop {
    Failed(Box<Failure>),
    Diverged(Box<Diverged>),
}

/// A call into a layer kaname calls, or a transfer built, as `run` and `--trace` show it.
#[derive(Clone, Debug)]
pub struct Called {
    /// The module it is in, by its name, and its file as the check was given it.
    pub module: String,
    pub file: String,
    pub line: usize,
    /// As the module writes it, with what it answered: `terms.due(invoiced: 2026-03-02) = 2026-04-01`.
    pub text: String,
    pub json: Json,
}

/// A `let` and the value it names, as `run --trace` shows it.
#[derive(Clone, Debug)]
pub struct Bound {
    pub module: String,
    pub file: String,
    pub function: String,
    pub line: usize,
    pub name: String,
    pub value: Value,
    pub ty: Ty,
    /// The value as the module would write it.
    pub text: String,
}

/// One thing an evaluation went through.
#[derive(Clone, Debug)]
pub enum Event {
    Call(Called),
    Let(Box<Bound>),
}

/// What an evaluation went through, in its order: the calls into the layers kaname calls and the
/// transfers built, and with `Interp::lets`, every `let`.
#[derive(Clone, Debug, Default)]
pub struct Trace {
    pub events: Vec<Event>,
}

impl Trace {
    pub fn calls(&self) -> impl Iterator<Item = &Called> {
        self.events.iter().filter_map(|e| match e {
            Event::Call(c) => Some(c),
            Event::Let(_) => None,
        })
    }
}

/// The names in scope in a function: its parameters, its `let` lines, the parameters of a function
/// given to a builtin, the value `some` names.
#[derive(Default)]
struct Env {
    vars: Vec<(String, Value)>,
}

impl Env {
    fn get(&self, name: &str) -> Option<&Value> {
        self.vars.iter().rev().find(|(n, _)| n == name).map(|(_, v)| v)
    }
}

struct Frame {
    function: String,
    line: usize,
    /// The types the type parameters of a generic function take in this call (DESIGN 26.4): its type
    /// parameters' names, and the types, in their order. Empty for a function that takes none.
    subst: std::rc::Rc<(Vec<String>, Vec<Ty>)>,
}

/// A place a value does not keep: the place (a parameter, or a field or an element of what it
/// holds), the value as a person reads it, and the range; or the count of a list with its count.
struct Outside {
    path: String,
    shown: String,
    range: String,
    count: Option<usize>,
    /// Whether the count is of a string's code points, not of the elements of a list (DESIGN 25.1).
    chars: bool,
}

/// What an evaluation is watched for: the value each expression comes to (only those named in
/// `only`, when it says some), each with its module's file on the disk and the expression's id. The
/// search for an example reads the values that reach the places it is after (DESIGN 5.7), and the
/// test of the analysis of ranges every value (DESIGN 16.1).
#[derive(Clone, Debug, Default)]
pub struct Watch {
    pub only: Option<std::collections::BTreeSet<(std::path::PathBuf, usize)>>,
    pub seen: Vec<(std::path::PathBuf, usize, Value)>,
}

/// The interpreter over one program.
pub struct Interp<'p> {
    p: &'p Program,
    suite: &'p Suite,
    pub limit: usize,
    frames: Vec<Frame>,
    pub trace: Trace,
    /// Whether every `let` is kept in the trace too (`run --trace`).
    pub lets: bool,
    /// What the evaluation is watched for, when it is.
    pub watch: Option<Watch>,
    /// What each call into another language was given, when it is kept: the module on the disk, the
    /// call's expression, and the values on the wire by what takes them (a rule's inputs, a date's
    /// inputs, a transfer's amounts), each kept before the call is made. What ritsu-cross reads where
    /// kaname's ranges do not show a check across the languages (DESIGN 14.2, `Programs::seen`).
    pub given: Option<Vec<(std::path::PathBuf, usize, ritsu_ports::Values)>>,
}

type R<T> = Result<T, Stop>;

/// The source of an expression on its line, as written.
fn source(m: &Checked, sp: Span) -> String {
    let line = m.src.lines().nth(sp.line.saturating_sub(1)).unwrap_or("");
    let cs: Vec<char> = line.chars().collect();
    let (a, b) = (sp.col.saturating_sub(1).min(cs.len()), sp.end.saturating_sub(1).min(cs.len()));
    cs[a..b.max(a)].iter().collect::<String>().trim().to_string()
}

fn ty_raw<'m>(m: &'m Checked, e: &Expr) -> &'m Ty {
    m.typed.types.get(e.id).unwrap_or(&Ty::Error)
}

fn rf<'m>(m: &'m Checked, e: &Expr) -> &'m Ref {
    m.typed.refs.get(e.id).unwrap_or(&Ref::None)
}

/// A value read in the unit of another type: a number counted in another unit of its dimension, the
/// same through optionals and lists. The check lets a number into a place of another unit only when
/// it is a constant (DESIGN 3.2), so this is where `15lb` becomes 240 ounces.
pub fn coerce(v: Value, from: &Ty, to: &Ty) -> Result<Value, Arith> {
    match (v, from, to) {
        (Value::None, _, _) => Ok(Value::None),
        (v, Ty::Opt(f), t) => coerce(v, f, t),
        (v, f, Ty::Opt(t)) => coerce(v, f, t),
        (Value::Num(r), Ty::Num(a), Ty::Num(b)) => Ok(Value::Num(convert(r, &a.unit, &b.unit)?)),
        (Value::List(xs), Ty::List(a), Ty::List(b)) => xs.into_iter().map(|x| coerce(x, a, b)).collect::<Result<Vec<_>, _>>().map(Value::List),
        // a key read in another unit keeps its place among the others: the reading keeps the order
        (Value::Set(xs), Ty::Set(a), Ty::Set(b)) => xs.into_iter().map(|x| coerce(x, a, b)).collect::<Result<Vec<_>, _>>().map(Value::Set),
        (Value::Dict(ps), Ty::Dict(ak, av), Ty::Dict(bk, bv)) => ps.into_iter().map(|(k, v)| Ok((coerce(k, ak, bk)?, coerce(v, av, bv)?))).collect::<Result<Vec<_>, _>>().map(Value::Dict),
        (v, _, _) => Ok(v),
    }
}

/// How many numbers `numbers` makes before the evaluation stops (DESIGN 24.3): a list that long is
/// what runs out of memory, which, as an evaluation that does not stop, is outside the language's
/// meaning; the interpreter stops there as it stops a call too deep.
pub const NUMBERS: i128 = 1_000_000;

/// A number counted in `from`, counted in `to`. A number and a rate are both counted as the fraction
/// they are, so nothing changes between them; zero is zero in every unit.
pub(crate) fn convert(r: Rat, from: &ritsu_units::Unit, to: &ritsu_units::Unit) -> Result<Rat, Arith> {
    use ritsu_units::Dim;
    let dimensionless = |u: &ritsu_units::Unit| matches!(u.dim, Dim::Number | Dim::Rate);
    // a number and a rate count alike; a bare 0 is zero of anything (DESIGN 3.2)
    if dimensionless(from) && (dimensionless(to) || r.num == 0) {
        return Ok(r);
    }
    // a temperature is read with its unit's zero (30℃ is 86℉): only a constant is read in another
    // unit of it, since it has no arithmetic
    if from.compares_only() && to.compares_only() && from.offset() != to.offset() {
        return num::held(crate::ty::convert(r, from, to));
    }
    match factor(from, to) {
        Some(f) if f == Rat::int(1) => Ok(r),
        Some(f) => num::mul(r, f),
        None => Ok(r),
    }
}

/// The type two numbers are read in to be added, subtracted or compared: the one that is not a
/// constant (a constant takes the unit of what it meets), else the first. Two lists or two values
/// that may be none are read so by the numbers they hold (`[1USD] = cents`, DESIGN 23.5).
pub(crate) fn common<'t>(a: &'t Ty, b: &'t Ty) -> &'t Ty {
    fn inner(t: &Ty) -> &Ty {
        match t {
            Ty::Opt(x) | Ty::List(x) => inner(x),
            x => x,
        }
    }
    match (inner(a), inner(b)) {
        (Ty::Num(x), Ty::Num(y)) if x.konst.is_some() && y.konst.is_none() => b,
        _ => a,
    }
}

impl<'p> Interp<'p> {
    pub fn new(p: &'p Program, suite: &'p Suite) -> Interp<'p> {
        Interp { p, suite, limit: DEPTH, frames: Vec::new(), trace: Trace::default(), lets: false, watch: None, given: None }
    }

    /// The type the check gave an expression, the types the type parameters of the function evaluated
    /// now take put in (DESIGN 26.4): what orders the keys of a value of a type parameter.
    fn ty<'m>(&self, m: &'m Checked, e: &Expr) -> std::borrow::Cow<'m, Ty> {
        let t = ty_raw(m, e);
        match self.frames.last() {
            Some(f) if !f.subst.0.is_empty() && t.has_param() => std::borrow::Cow::Owned(subst(t, &f.subst.0, &f.subst.1)),
            _ => std::borrow::Cow::Borrowed(t),
        }
    }

    /// A type with the type parameters of the function evaluated now put in.
    fn sub(&self, t: &Ty) -> Ty {
        match self.frames.last() {
            Some(f) if !f.subst.0.is_empty() && t.has_param() => subst(t, &f.subst.0, &f.subst.1),
            _ => t.clone(),
        }
    }

    // The functions the evaluation recurses through (`call`, `block`, `eval` and the ones `eval`
    // hands an expression to) keep their frames small: what they compute besides evaluating what is
    // under them is done by functions of their own, which leave the stack before the next level
    // comes. A recursion of kaname's functions [`DEPTH`] deep then fits the stack of `ritsu` (DESIGN
    // 8: a debug build takes 6 to 11 KiB a level).

    // ── Failures ────────────────────────────────────────────────────────────────

    #[cold]
    #[inline(never)]
    fn failure(&self, m: &Checked, line: usize, kind: Kind, message: Text) -> Failure {
        Failure { kind, module: m.module.name.text.clone(), file: m.path.clone(), line, message, stack: self.frames.iter().map(|f| (f.function.clone(), f.line)).collect(), expr: None }
    }

    #[cold]
    #[inline(never)]
    fn fail(&self, m: &Checked, line: usize, kind: Kind, message: Text) -> Stop {
        Stop::Failed(Box::new(self.failure(m, line, kind, message)))
    }

    /// The failure of an operation that leaves int64 or divides by 0, at the expression.
    #[cold]
    #[inline(never)]
    fn arith(&self, m: &Checked, e: &Expr, a: Arith) -> Stop {
        let src = source(m, e.span);
        let mut f = match a {
            Arith::Overflow => self.failure(m, e.span.line, Kind::Overflow, tr!("`{src}` の値が int64 に収まりません", "The value of `{src}` does not fit in int64")),
            Arith::Zero => self.failure(m, e.span.line, Kind::Zero, tr!("`{src}` の割る数が 0 です", "The divisor in `{src}` is 0")),
        };
        f.expr = Some(e.id);
        Stop::Failed(Box::new(f))
    }

    #[inline(never)]
    fn coerce_at(&self, m: &Checked, e: &Expr, v: Value, from: &Ty, to: &Ty) -> R<Value> {
        coerce(v, from, to).map_err(|a| self.arith(m, e, a))
    }

    /// The value of `x` read in the type of `e`, where `x` answers for `e`.
    #[inline(never)]
    fn answer_for(&self, m: &Checked, x: &Expr, e: &Expr, v: Value) -> R<Value> {
        self.coerce_at(m, x, v, &self.ty(m, x), &self.ty(m, e))
    }

    // ── Places ──────────────────────────────────────────────────────────────────

    /// Where a value does not keep what its place declares, `what` naming the place: the range of
    /// its numbers and dates, the count of a list, each element of a list (a list of lists too, its
    /// elements to the range the outer list writes, as the generated code holds them); with `reached`,
    /// also of every field of a module's record it reaches, a rate with a step and no range held to
    /// 0% to 100% (DESIGN 3.9), a number or a date whose range is not written to the whole of its type
    /// (DESIGN 22.1). The fields of a list a rule walks are the rule's to hold.
    fn outside(&self, v: &Value, slot: &Slot, what: &str, hold: Hold) -> Option<Outside> {
        if matches!(v, Value::None) {
            return None;
        }
        let mut t = &slot.ty;
        while let Ty::Opt(x) = t {
            t = x;
        }
        // a value of a record or an enum that holds itself is not held again where kaname calls a
        // function (DESIGN 28.1)
        if hold == Hold::Call && self.p.recursive(t) {
            return None;
        }
        // each kind of value in a function of its own, kept out of this one, so that the recursion
        // through a deep value holds a small frame for each level (DESIGN 28.1)
        match (t, v) {
            (Ty::List(x), Value::List(xs)) => self.outside_list(x, xs, slot, what, hold),
            // a string's code points, to its count (DESIGN 25.1)
            (Ty::Str, Value::Str(s)) if slot.count.is_some() => Self::outside_str(s, slot, what),
            (Ty::Set(x), Value::Set(xs)) => self.outside_set(x, xs, slot, what, hold),
            (Ty::Dict(k, x), Value::Dict(ps)) => self.outside_dict(k, x, ps, slot, what, hold),
            _ => self.outside_value(v, slot, t, what, hold),
        }
    }

    /// A list: its count, then each element (a list of lists too, its elements to the range the outer
    /// list writes, as the generated code holds them).
    #[inline(never)]
    fn outside_list(&self, x: &Ty, xs: &[Value], slot: &Slot, what: &str, hold: Hold) -> Option<Outside> {
        if let Some(c) = &slot.count {
            let n = Rat::int(xs.len() as i128);
            if c.min.is_some_and(|lo| num::cmp(n, lo) == Ordering::Less) || c.max.is_some_and(|hi| num::cmp(n, hi) == Ordering::Greater) {
                return Some(Outside { path: what.to_string(), shown: String::new(), range: range_text(c, &Ty::Num(NumTy::number())), count: Some(xs.len()), chars: false });
            }
        }
        let elem = Slot { ty: x.clone(), range: slot.range.clone(), count: None };
        xs.iter().enumerate().find_map(|(i, x)| self.outside(x, &elem, &format!("{what}[{i}]"), hold))
    }

    /// A string's code points, to the count its place writes (DESIGN 25.1).
    #[inline(never)]
    fn outside_str(s: &str, slot: &Slot, what: &str) -> Option<Outside> {
        let c = slot.count.as_ref()?;
        let n = crate::text::count(s);
        let at = Rat::int(n as i128);
        if c.min.is_some_and(|lo| num::cmp(at, lo) == Ordering::Less) || c.max.is_some_and(|hi| num::cmp(at, hi) == Ordering::Greater) {
            return Some(Outside { path: what.to_string(), shown: String::new(), range: range_text(c, &Ty::Num(NumTy::number())), count: Some(n), chars: true });
        }
        None
    }

    /// A set's members, each where it travels on the wire (`s[0]`, DESIGN 24.4), to the range of the
    /// members.
    #[inline(never)]
    fn outside_set(&self, x: &Ty, xs: &[Value], slot: &Slot, what: &str, hold: Hold) -> Option<Outside> {
        if let Some(o) = self.count_outside(slot, xs.len(), what) {
            return Some(o);
        }
        let elem = Slot { ty: x.clone(), range: slot.range.clone(), count: None };
        xs.iter().enumerate().find_map(|(i, x)| self.outside(x, &elem, &format!("{what}[{i}]"), hold))
    }

    /// A dict's pairs, the key of each where it travels (`d[0][0]`) and its value (`d[0][1]`, DESIGN
    /// 24.4), the values to the range of the dict's values.
    #[inline(never)]
    fn outside_dict(&self, k: &Ty, x: &Ty, ps: &[(Value, Value)], slot: &Slot, what: &str, hold: Hold) -> Option<Outside> {
        if let Some(o) = self.count_outside(slot, ps.len(), what) {
            return Some(o);
        }
        let (key, val) = (Slot::of(k.clone()), Slot { ty: x.clone(), range: slot.range.clone(), count: None });
        ps.iter().enumerate().find_map(|(i, (a, b))| self.outside(a, &key, &format!("{what}[{i}][0]"), hold).or_else(|| self.outside(b, &val, &format!("{what}[{i}][1]"), hold)))
    }

    /// Any other value: a number or a date to its range, then, where a parameter reaches it, the fields
    /// of a module's record and of the value of a sum, and the value a result holds.
    #[inline(never)]
    fn outside_value(&self, v: &Value, slot: &Slot, t: &Ty, what: &str, hold: Hold) -> Option<Outside> {
        let reached = hold != Hold::Place;
        if let Some(o) = Self::outside_range(v, slot, t, what, reached) {
            return Some(o);
        }
        if !reached {
            return None;
        }
        if let (Ty::Record(id @ RecId::Module { .. }), Value::Record(fs)) = (t, v) {
            return self.outside_fields(self.p.fields(id), fs, what, None, hold);
        }
        // a value of a sum: the fields of the value it is, written with its name (`o.placed.on`)
        if let Value::Variant(tag, fs) = v {
            let id = match t {
                Ty::Enum(id) => Some(id.clone()),
                Ty::Record(r) => r.sum(),
                _ => None,
            };
            if let Some(o) = self.outside_fields(id.map(|i| self.p.value_fields(&i, tag)).unwrap_or_default(), fs, what, Some(tag), hold) {
                return Some(o);
            }
            // a result: the value it holds, written with `ok` or `err` (`r.ok.qty`, DESIGN 26.3)
            if let (Ty::Result(a, e), Some((_, x))) = (t, fs.first()) {
                let inner = if tag == "ok" { a } else { e };
                return self.outside(x, &Slot::of((**inner).clone()), &format!("{what}.{tag}"), hold);
            }
        }
        None
    }

    /// The fields of a record, or of the value `tag` of a sum, each to its place.
    fn outside_fields(&self, slots: Vec<(crate::ast::Name, Slot)>, fs: &[(String, Value)], what: &str, tag: Option<&str>, hold: Hold) -> Option<Outside> {
        for (f, fslot) in slots {
            if let Some((_, fv)) = fs.iter().find(|(n, _)| *n == f.text) {
                let at = match tag {
                    Some(tag) => format!("{what}.{tag}.{}", f.text),
                    None => format!("{what}.{}", f.text),
                };
                if let Some(o) = self.outside(fv, &fslot, &at, hold) {
                    return Some(o);
                }
            }
        }
        None
    }

    /// A number or a date outside its range: the range written, a rate with a step and no range 0% to
    /// 100% where a parameter reaches it (DESIGN 3.9), and there an end not written the type's: int64
    /// for a number, the days from 0001-01-01 to 9999-12-31 for a date (DESIGN 22.1).
    #[inline(never)]
    fn outside_range(v: &Value, slot: &Slot, t: &Ty, what: &str, reached: bool) -> Option<Outside> {
        let range = match (&slot.range, t) {
            (Some(r), _) => Some(r.clone()),
            (None, Ty::Num(n)) if reached && n.is_rate() && n.step.is_some() => Some(Range { min: Some(Rat::zero()), max: Some(Rat::int(1)) }),
            _ => None,
        };
        let range = if reached { filled(range.as_ref(), whole_range(t)) } else { range };
        let r = range.as_ref()?;
        let at = match v {
            Value::Num(n) => *n,
            Value::Date(d) => Rat::int(*d as i128),
            _ => return None,
        };
        if r.min.is_some_and(|lo| num::cmp(at, lo) == Ordering::Less) || r.max.is_some_and(|hi| num::cmp(at, hi) == Ordering::Greater) {
            let shown = match (v, t) {
                (Value::Num(n), Ty::Num(nt)) => num_text(*n, &nt.unit),
                (Value::Date(d), _) => crate::date::text(*d),
                _ => String::new(),
            };
            return Some(Outside { path: what.to_string(), shown, range: range_text(r, t), count: None, chars: false });
        }
        None
    }

    /// A dict's or a set's count outside the count a place declares.
    fn count_outside(&self, slot: &Slot, len: usize, what: &str) -> Option<Outside> {
        let c = slot.count.as_ref()?;
        let n = Rat::int(len as i128);
        if c.min.is_some_and(|lo| num::cmp(n, lo) == Ordering::Less) || c.max.is_some_and(|hi| num::cmp(n, hi) == Ordering::Greater) {
            return Some(Outside { path: what.to_string(), shown: String::new(), range: range_text(c, &Ty::Num(NumTy::number())), count: Some(len), chars: false });
        }
        None
    }

    /// Whether a value keeps what a place declares, as the interpreter holds it there: `reached` for a
    /// parameter of a function kaname calls (the fields of the records it reaches too, a rate with no
    /// range 0% to 100%, and not inside a value of a type that holds itself, DESIGN 28.1).
    pub fn keeps(&self, v: &Value, slot: &Slot, reached: bool) -> bool {
        self.outside(v, slot, "", if reached { Hold::Call } else { Hold::Place }).is_none()
    }

    /// The type of the first value nested too deep in an argument a host gives (DESIGN 28.1): the
    /// values of records and enums that hold themselves counted along each path from the argument
    /// (`above` of them over this one), one more than [`NESTED`] deep. Its name as it is declared;
    /// None where no value nests so deep.
    fn nested(&self, v: &Value, t: &Ty, above: usize) -> Option<String> {
        if matches!(v, Value::None) {
            return None;
        }
        let mut t = t;
        while let Ty::Opt(x) = t {
            t = x;
        }
        let here = if self.p.recursive(t) { above + 1 } else { above };
        if here > NESTED {
            return Some(match t {
                Ty::Enum(id) => id.name.clone(),
                Ty::Record(RecId::Module { name, .. } | RecId::Variant { name, .. }) => name.clone(),
                _ => String::new(),
            });
        }
        let fields = |slots: Vec<(crate::ast::Name, Slot)>, fs: &[(String, Value)]| slots.iter().find_map(|(f, s)| fs.iter().find(|(n, _)| *n == f.text).and_then(|(_, fv)| self.nested(fv, &s.ty, here)));
        match (t, v) {
            (Ty::List(x) | Ty::Set(x), Value::List(xs) | Value::Set(xs)) => xs.iter().find_map(|e| self.nested(e, x, here)),
            (Ty::Dict(k, x), Value::Dict(ps)) => ps.iter().find_map(|(a, b)| self.nested(a, k, here).or_else(|| self.nested(b, x, here))),
            (Ty::Record(id), Value::Record(fs)) => fields(self.p.fields(id), fs),
            (Ty::Result(a, e), Value::Variant(tag, fs)) => fs.first().and_then(|(_, x)| self.nested(x, if tag == "ok" { a } else { e }, here)),
            (Ty::Enum(id), Value::Variant(tag, fs)) => fields(self.p.value_fields(id, tag), fs),
            (Ty::Record(r @ RecId::Variant { .. }), Value::Variant(tag, fs)) => fields(r.sum().map(|id| self.p.value_fields(&id, tag)).unwrap_or_default(), fs),
            _ => None,
        }
    }

    /// The arguments a host gives a function, held to its parameters' types, ranges and counts and to
    /// those of every field they reach (DESIGN 4.4): the failure `input` when one is outside them.
    pub fn check_entry(&self, m: &Checked, f: usize, args: &[Value]) -> Result<(), Box<Failure>> {
        self.check_entry_with(m, f, args, &[])
    }

    /// The same, for a call of a generic function whose type parameters take `targs` (an example's,
    /// DESIGN 26.4).
    pub fn check_entry_with(&self, m: &Checked, f: usize, args: &[Value], targs: &[Ty]) -> Result<(), Box<Failure>> {
        let info = &m.decls.fns[f];
        let names = crate::names::tnames(&info.tparams);
        for (p, v) in info.params.iter().zip(args) {
            let slot = subst_slot(&p.slot, &names, targs);
            // the nesting first, so that what follows walks no deeper (DESIGN 28.1)
            if let Some(deep) = self.nested(v, &slot.ty, 0) {
                let (fun, param) = (&info.name.text, &p.name.text);
                let msg = tr!("関数 `{fun}` の引数 `{param}` は、`{deep}` の入れ子が {NESTED} 段を超えています", "The argument `{param}` of `{fun}` nests `{deep}` more than {NESTED} deep");
                return Err(Box::new(self.failure(m, info.line, Kind::Input, msg)));
            }
            if let Some(o) = self.outside(v, &slot, &p.name.text, Hold::Entry) {
                let (fun, path, shown, range) = (&info.name.text, &o.path, &o.shown, &o.range);
                let msg = match o.count {
                    Some(n) if o.chars => tr!("関数 `{fun}` の引数 `{path}` のコードポイントは {n} 個で、コードポイントの数の範囲 {range} の外です", "The argument `{path}` of `{fun}` has {n} code points, outside its count {range}"),
                    Some(n) => tr!("関数 `{fun}` の引数 `{path}` の要素は {n} 個で、要素の数の範囲 {range} の外です", "The argument `{path}` of `{fun}` has {n} elements, outside its count {range}"),
                    None => tr!("関数 `{fun}` の引数 `{path}` の {shown} は、範囲 {range} の外です", "The argument `{path}` of `{fun}` is {shown}, outside its range {range}"),
                };
                return Err(Box::new(self.failure(m, info.line, Kind::Input, msg)));
            }
        }
        Ok(())
    }

    /// The arguments of a function as a host gives them, in JSON: each by its parameter's name or
    /// alias, held to its type, then to its range (DESIGN 4.4). The failure `input` when they are not
    /// what the function takes.
    pub fn enter_json(&self, m: &Checked, f: usize, input: &Json) -> Result<Vec<Value>, Box<Failure>> {
        let info = &m.decls.fns[f];
        let fun = &info.name.text;
        let input_failure = |why: Text| Box::new(self.failure(m, info.line, Kind::Input, tr!("関数 `{fun}` の引数: {}", "An argument of `{fun}`: {}", why.ja; why.en)));
        let Some(obj) = input.as_obj() else {
            return Err(input_failure(tr!("引数は、引数の名前をキーにしたオブジェクトで渡します", "the arguments are given as an object, a key for each parameter")));
        };
        for (k, _) in obj {
            if !info.params.iter().any(|p| p.name.is(k)) {
                let all = info.params.iter().map(|p| format!("`{}`", p.name.text)).collect::<Vec<_>>().join(", ");
                return Err(input_failure(tr!("`{k}` という引数はありません（引数は {all}）", "there is no parameter `{k}` (the parameters are {all})")));
            }
        }
        let mut args = Vec::new();
        for p in &info.params {
            let given: Vec<&Json> = obj.iter().filter(|(k, _)| p.name.is(k)).map(|(_, v)| v).collect();
            let name = &p.name.text;
            let v = match given.as_slice() {
                [] if matches!(p.slot.ty, Ty::Opt(_)) => Value::None,
                [] => return Err(input_failure(tr!("`{name}` がありません", "`{name}` is missing"))),
                [x] => self.p.from_json(m, x, &p.slot.ty, name).map_err(input_failure)?,
                _ => return Err(input_failure(tr!("`{name}` が、名前と別名で二度渡されています", "`{name}` is given twice, by its name and by its alias"))),
            };
            args.push(v);
        }
        self.check_entry(m, f, &args)?;
        Ok(args)
    }

    /// An expression that names nothing of a function (an argument or the answer of an example),
    /// evaluated and read in the type `to`.
    pub fn constant(&mut self, m: &'p Checked, e: &'p Expr, to: &Ty) -> R<Value> {
        let mut env = Env::default();
        let v = self.eval(m, &mut env, e)?;
        self.coerce_at(m, e, v, &self.ty(m, e), to)
    }

    // ── Calls of kaname's functions ─────────────────────────────────────────────

    /// The function `f` of module `m` with these arguments, in its parameters' order and types, called
    /// from line `at` (0 for a host): its answer, in its result's type.
    pub fn call(&mut self, m: &'p Checked, f: usize, args: Vec<Value>, at: usize) -> R<Value> {
        self.call_with(m, f, args, at, Vec::new())
    }

    /// The same, the type parameters of a generic function taking `targs` (DESIGN 26.4).
    pub fn call_with(&mut self, m: &'p Checked, f: usize, args: Vec<Value>, at: usize, targs: Vec<Ty>) -> R<Value> {
        if self.frames.len() >= self.limit {
            return Err(self.too_deep(m, f, at));
        }
        let mut env = self.enter(m, f, args, at, targs);
        let out = self.block(m, &mut env, &m.module.fns[f].body);
        let out = self.finish(m, f, out);
        self.frames.pop();
        out
    }

    #[cold]
    #[inline(never)]
    fn too_deep(&self, m: &Checked, f: usize, at: usize) -> Stop {
        Stop::Diverged(Box::new(Diverged { function: m.decls.fns[f].name.text.clone(), module: m.module.name.text.clone(), line: at, depth: self.limit }))
    }

    #[inline(never)]
    fn enter(&mut self, m: &Checked, f: usize, args: Vec<Value>, at: usize, targs: Vec<Ty>) -> Env {
        let info = &m.decls.fns[f];
        let subst = std::rc::Rc::new((crate::names::tnames(&info.tparams), targs));
        self.frames.push(Frame { function: info.name.text.clone(), line: at, subst });
        Env { vars: info.params.iter().map(|p| p.name.text.clone()).zip(args).collect() }
    }

    /// A function's answer read in its result's type, and held to its result's range.
    #[inline(never)]
    fn finish(&self, m: &Checked, f: usize, out: R<Value>) -> R<Value> {
        let (decl, info) = (&m.module.fns[f], &m.decls.fns[f]);
        let v = self.coerce_at(m, &decl.body.answer, out?, &self.ty(m, &decl.body.answer), &self.sub(&info.ret.ty))?;
        match self.outside(&v, &info.ret, &info.name.text, Hold::Place) {
            Some(o) => {
                let (fun, shown, range) = (&info.name.text, &o.shown, &o.range);
                let msg = match o.count {
                    Some(n) if o.chars => tr!("関数 `{fun}` の答えのコードポイントは {n} 個で、コードポイントの数の範囲 {range} の外です", "The answer of `{fun}` has {n} code points, outside its count {range}"),
                    Some(n) => tr!("関数 `{fun}` の答えの要素は {n} 個で、要素の数の範囲 {range} の外です", "The answer of `{fun}` has {n} elements, outside its count {range}"),
                    None => tr!("関数 `{fun}` の答え {shown} は、範囲 {range} の外です", "The answer of `{fun}`, {shown}, is outside its range {range}"),
                };
                Err(self.fail(m, decl.body.answer.span.line, Kind::Range, msg))
            }
            None => Ok(v),
        }
    }

    fn call_fn(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr, canon: &'p std::path::Path, name: &'p str, args: &'p [Arg]) -> R<Value> {
        let vals = self.args(m, env, args)?;
        // the types a generic function's type parameters take in this call (DESIGN 26.4)
        let targs: Vec<Ty> = m.typed.inst.get(&e.id).map(|ts| ts.iter().map(|t| self.sub(t)).collect()).unwrap_or_default();
        let (callee, fi, vals) = self.place_args(m, e, canon, name, args, vals, &targs)?;
        self.call_with(callee, fi, vals, e.span.line, targs)
    }

    /// The arguments of a call, evaluated from the left.
    fn args(&mut self, m: &'p Checked, env: &mut Env, args: &'p [Arg]) -> R<Vec<Value>> {
        let mut out = Vec::with_capacity(args.len());
        for a in args {
            out.push(self.eval(m, env, a.expr())?);
        }
        Ok(out)
    }

    /// The arguments of a call of a function of kaname, in its parameters' order and types, held to
    /// what they declare (DESIGN 4.4: a call of one function by another is not the entry, and what it
    /// gives is held as a place is).
    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    fn place_args(&self, m: &'p Checked, e: &Expr, canon: &std::path::Path, name: &str, args: &[Arg], vals: Vec<Value>, targs: &[Ty]) -> R<(&'p Checked, usize, Vec<Value>)> {
        let callee: &'p Checked = self.p.module(canon);
        let Some(fi) = callee.decls.fns.iter().position(|f| f.name.text == name) else {
            return Err(self.fail(m, e.span.line, Kind::Range, tr!("関数 `{name}` がありません", "There is no function `{name}`")));
        };
        let info = &callee.decls.fns[fi];
        let names = crate::names::tnames(&info.tparams);
        let slots: Vec<Slot> = info.params.iter().map(|p| subst_slot(&p.slot, &names, targs)).collect();
        let mut given: Vec<Value> = vec![Value::None; info.params.len()];
        for (a, v) in args.iter().zip(vals) {
            let Arg::Named(n, _, x) = a else { continue };
            if let Some(pi) = info.params.iter().position(|p| p.name.is(n)) {
                given[pi] = self.coerce_at(m, x, v, &self.ty(m, x), &slots[pi].ty)?;
            }
        }
        for ((p, slot), v) in info.params.iter().zip(&slots).zip(&given) {
            if let Some(o) = self.outside(v, slot, &p.name.text, Hold::Call) {
                let (fun, path, shown, range) = (&info.name.text, &o.path, &o.shown, &o.range);
                let msg = match o.count {
                    Some(n) if o.chars => tr!("関数 `{fun}` に渡す引数 `{path}` のコードポイントは {n} 個で、コードポイントの数の範囲 {range} の外です", "The argument `{path}` given to `{fun}` has {n} code points, outside its count {range}"),
                    Some(n) => tr!("関数 `{fun}` に渡す引数 `{path}` の要素は {n} 個で、要素の数の範囲 {range} の外です", "The argument `{path}` given to `{fun}` has {n} elements, outside its count {range}"),
                    None => tr!("関数 `{fun}` に渡す引数 `{path}` の {shown} は、範囲 {range} の外です", "The argument `{path}` given to `{fun}` is {shown}, outside its range {range}"),
                };
                return Err(self.fail(m, e.span.line, Kind::Range, msg));
            }
        }
        Ok((callee, fi, given))
    }

    // ── Blocks ──────────────────────────────────────────────────────────────────

    /// A block's answer, in the type of its answer expression.
    fn block(&mut self, m: &'p Checked, env: &mut Env, b: &'p Block) -> R<Value> {
        let mark = env.vars.len();
        for l in &b.lets {
            let v = self.eval(m, env, &l.value)?;
            self.bind(m, env, l, v)?;
        }
        let out = self.eval(m, env, &b.answer);
        env.vars.truncate(mark);
        out
    }

    /// A `let`'s value, read in the type it gives its name, named.
    #[inline(never)]
    fn bind(&mut self, m: &Checked, env: &mut Env, l: &Let, v: Value) -> R<()> {
        let bound = m.typed.bound.get(&l.value.id).cloned().unwrap_or_else(|| self.ty(m, &l.value).into_owned());
        let v = self.coerce_at(m, &l.value, v, &self.ty(m, &l.value), &bound)?;
        if self.lets {
            let function = self.frames.last().map(|f| f.function.clone()).unwrap_or_default();
            let text = self.p.show(m, &v, &bound);
            self.trace.events.push(Event::Let(Box::new(Bound { module: m.module.name.text.clone(), file: m.path.clone(), function, line: l.span.line, name: l.name.clone(), value: v.clone(), ty: bound, text })));
        }
        env.vars.push((l.name.clone(), v));
        Ok(())
    }

    /// A block that answers for the expression `e`: its answer read in `e`'s type.
    fn branch(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr, b: &'p Block) -> R<Value> {
        let v = self.block(m, env, b)?;
        self.answer_for(m, &b.answer, e, v)
    }

    // ── Expressions ─────────────────────────────────────────────────────────────

    /// The value of an expression, in the unit of the type the check gave it.
    fn eval(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr) -> R<Value> {
        let out = self.eval_kind(m, env, e);
        if self.watch.is_some()
            && let Ok(v) = &out
        {
            self.observe(m, e, v);
        }
        out
    }

    /// A value an expression came to, kept where the evaluation is watched for it.
    #[cold]
    #[inline(never)]
    fn observe(&mut self, m: &Checked, e: &Expr, v: &Value) {
        if let Some(w) = &mut self.watch
            && w.only.as_ref().is_none_or(|o| o.contains(&(m.canon.clone(), e.id)))
        {
            w.seen.push((m.canon.clone(), e.id, v.clone()));
        }
    }

    fn eval_kind(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr) -> R<Value> {
        match &e.kind {
            ExprKind::Field(base, _, _) => self.field(m, env, e, base),
            ExprKind::Call(callee, args) => self.call_expr(m, env, e, callee, args),
            ExprKind::Record(_, base, inits) => self.record(m, env, e, base.as_deref(), inits),
            ExprKind::List(xs) => self.list_expr(m, env, e, xs),
            ExprKind::Dict(pairs) => self.dict_expr(m, env, e, pairs),
            ExprKind::Neg(x) | ExprKind::Not(x) | ExprKind::In(x, _, _) => {
                let v = self.eval(m, env, x)?;
                self.unary(m, e, x, v)
            }
            ExprKind::Bin(a, op, b) => self.binary(m, env, e, a, *op, b),
            ExprKind::If(c, t, f) => {
                let go = matches!(self.eval(m, env, c)?, Value::Bool(true));
                self.branch(m, env, e, if go { t } else { f })
            }
            ExprKind::Match(s, arms) => self.matching(m, env, e, s, arms),
            ExprKind::Block(b) => self.branch(m, env, e, b),
            _ => self.leaf(m, env, e),
        }
    }

    /// A literal, a name, or a function written where nothing takes it.
    #[inline(never)]
    fn leaf(&self, m: &Checked, env: &Env, e: &Expr) -> R<Value> {
        match &e.kind {
            ExprKind::Num(n) => {
                let v = if n.unit == "%" { n.value.div(Rat::int(100)) } else { n.value };
                num::held(Some(v)).map(Value::Num).map_err(|a| self.arith(m, e, a))
            }
            ExprKind::Date(d) => Ok(Value::Date(*d)),
            ExprKind::Str(s) => Ok(Value::Str(s.clone())),
            ExprKind::Bool(b) => Ok(Value::Bool(*b)),
            ExprKind::Var(w) => match rf(m, e) {
                Ref::EnumValue(_, v) => Ok(Value::Enum(v.clone())),
                _ => env.get(w).cloned().ok_or_else(|| self.fail(m, e.span.line, Kind::Range, tr!("`{w}` に値がありません", "`{w}` has no value"))),
            },
            // a lambda made a value, with the values of the names in scope (DESIGN 27.3)
            ExprKind::Lambda(..) => {
                let (function, subst) = match self.frames.last() {
                    Some(f) => (f.function.clone(), f.subst.clone()),
                    None => (String::new(), std::rc::Rc::new((Vec::new(), Vec::new()))),
                };
                Ok(Value::Closure(std::rc::Rc::new(Closure { module: m.canon.clone(), expr: e.id, env: env.vars.clone(), function, subst })))
            }
            _ => Ok(Value::None),
        }
    }

    fn field(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr, base: &'p Expr) -> R<Value> {
        match rf(m, e) {
            Ref::EnumValue(_, v) => Ok(Value::Enum(v.clone())),
            Ref::Field(name) => {
                let b = self.eval(m, env, base)?;
                Ok(b.field(name).cloned().unwrap_or(Value::None))
            }
            _ => Ok(Value::None),
        }
    }

    /// `-x`, `not x`, `x in USDc`, with the value of `x`.
    #[inline(never)]
    fn unary(&self, m: &Checked, e: &Expr, x: &Expr, v: Value) -> R<Value> {
        match (&e.kind, v) {
            (ExprKind::Neg(_), Value::Num(r)) => num::neg(r).map(Value::Num).map_err(|a| self.arith(m, e, a)),
            (ExprKind::Not(_), Value::Bool(b)) => Ok(Value::Bool(!b)),
            (ExprKind::In(..), v) => self.coerce_at(m, e, v, &self.ty(m, x), &self.ty(m, e)),
            (_, v) => Ok(v),
        }
    }

    fn list_expr(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr, xs: &'p [Expr]) -> R<Value> {
        let mut out = Vec::with_capacity(xs.len());
        for x in xs {
            let v = self.eval(m, env, x)?;
            out.push(self.element(m, e, x, v)?);
        }
        Ok(Value::List(out))
    }

    /// A dict written out (DESIGN 24.2): each key, then its value, from the left, each read in the
    /// dict's type of keys or of values as it comes, as an element of a list is; the later of two
    /// pairs of one key is the one kept.
    fn dict_expr(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr, pairs: &'p [(Expr, Expr)]) -> R<Value> {
        let (kt, vt) = dict_types(&self.ty(m, e));
        let mut out = Vec::with_capacity(pairs.len());
        for (k, v) in pairs {
            let a = self.eval(m, env, k)?;
            let a = self.part(m, k, a, &kt)?;
            let b = self.eval(m, env, v)?;
            let b = self.part(m, v, b, &vt)?;
            out.push((a, b));
        }
        Ok(Value::Dict(self.p.dict_of(&kt, out)))
    }

    /// A key or a value of a dict written out, read in the dict's type of keys or of values.
    #[inline(never)]
    fn part(&self, m: &Checked, x: &Expr, v: Value, to: &Ty) -> R<Value> {
        if to.is_error() {
            return Ok(v);
        }
        self.coerce_at(m, x, v, &self.ty(m, x), to)
    }

    /// An element of a list written out, read in the type of the list's elements.
    #[inline(never)]
    fn element(&self, m: &Checked, e: &Expr, x: &Expr, v: Value) -> R<Value> {
        match &*self.ty(m, e) {
            Ty::List(t) => self.coerce_at(m, x, v, &self.ty(m, x), t),
            _ => Ok(v),
        }
    }

    fn binary(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr, a: &'p Expr, op: BinOp, b: &'p Expr) -> R<Value> {
        if matches!(op, BinOp::And | BinOp::Or) {
            let left = self.eval(m, env, a)?;
            // the right is not evaluated once the left decides (DESIGN 8)
            if matches!(left, Value::Bool(l) if l == (op == BinOp::Or)) {
                return Ok(left);
            }
            return self.eval(m, env, b);
        }
        let x = self.eval(m, env, a)?;
        let y = self.eval(m, env, b)?;
        self.combine(m, e, a, op, b, x, y)
    }

    /// A comparison or an arithmetic operation, with the values of its two sides.
    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    fn combine(&self, m: &Checked, e: &Expr, a: &Expr, op: BinOp, b: &Expr, x: Value, y: Value) -> R<Value> {
        let (ta_c, tb_c) = (self.ty(m, a), self.ty(m, b));
        let (ta, tb): (&Ty, &Ty) = (&ta_c, &tb_c);
        if op.is_comparison() {
            let order = match (&x, &y) {
                (Value::Num(_), Value::Num(_)) => {
                    let t = common(ta, tb);
                    let (p, q) = (self.coerce_at(m, a, x, ta, t)?, self.coerce_at(m, b, y, tb, t)?);
                    num::cmp(p.num().unwrap_or(Rat::zero()), q.num().unwrap_or(Rat::zero()))
                }
                (Value::Date(p), Value::Date(q)) => p.cmp(q),
                // strings by their code points: the order of the bytes of UTF-8 (DESIGN 25.1)
                (Value::Str(p), Value::Str(q)) if !matches!(op, BinOp::Eq | BinOp::Ne) => p.cmp(q),
                (Value::Enum(p), Value::Enum(q)) if !matches!(op, BinOp::Eq | BinOp::Ne) => self.rank(ta, p).cmp(&self.rank(ta, q)),
                // the values of a type parameter held to keys: the order of keys (DESIGN 26.2)
                (x, y) if !matches!(op, BinOp::Eq | BinOp::Ne) && matches!(ty_raw(m, a), Ty::Param(_)) => self.p.key_cmp(ta, x, y),
                // two values of one type, whole (DESIGN 23.5): the numbers a list or a value that may
                // be none holds read in one unit, then every part the same
                _ => {
                    let t = common(ta, tb);
                    let (p, q) = (self.coerce_at(m, a, x, ta, t)?, self.coerce_at(m, b, y, tb, t)?);
                    let same = p == q;
                    return Ok(Value::Bool(if op == BinOp::Ne { !same } else { same }));
                }
            };
            return Ok(Value::Bool(match op {
                BinOp::Eq => order == Ordering::Equal,
                BinOp::Ne => order != Ordering::Equal,
                BinOp::Lt => order == Ordering::Less,
                BinOp::Le => order != Ordering::Greater,
                BinOp::Gt => order == Ordering::Greater,
                _ => order != Ordering::Less,
            }));
        }
        match (x, y) {
            (Value::Date(p), Value::Date(q)) => Ok(Value::Num(Rat::int((p - q) as i128))),
            (Value::Num(p), Value::Num(q)) => {
                let r = match op {
                    BinOp::Add | BinOp::Sub => {
                        let t_c = self.ty(m, e);
        let t: &Ty = &t_c;
                        let p = convert(p, &unit_of(ta), &unit_of(t)).map_err(|x| self.arith(m, a, x))?;
                        let q = convert(q, &unit_of(tb), &unit_of(t)).map_err(|x| self.arith(m, b, x))?;
                        if op == BinOp::Add { num::add(p, q) } else { num::sub(p, q) }
                    }
                    BinOp::Mul => num::mul(p, q),
                    _ => {
                        // two amounts of one dimension are read in one unit before one is divided by
                        // the other; anything over a number or a rate keeps its own
                        let (p, q) = if has_unit(ta) && has_unit(tb) {
                            let t = common(ta, tb);
                            (convert(p, &unit_of(ta), &unit_of(t)).map_err(|x| self.arith(m, a, x))?, convert(q, &unit_of(tb), &unit_of(t)).map_err(|x| self.arith(m, b, x))?)
                        } else {
                            (p, q)
                        };
                        num::div(p, q)
                    }
                };
                r.map(Value::Num).map_err(|x| self.arith(m, e, x))
            }
            (x, _) => Ok(x),
        }
    }

    /// The place of a value in the order of its enum.
    fn rank(&self, t: &Ty, v: &str) -> usize {
        match t.present() {
            Ty::Enum(id) => self.p.values(id).0.iter().position(|x| x.text == v).unwrap_or(0),
            _ => 0,
        }
    }

    fn matching(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr, s: &'p Expr, arms: &'p [Arm]) -> R<Value> {
        let v = self.eval(m, env, s)?;
        let (k, bind) = self.pick(m, e, s, arms, &v)?;
        let mark = env.vars.len();
        if let Some(x) = bind {
            // `ok x` and `err e` name the value the `ok` or the `err` holds (DESIGN 26.3)
            let named = match (ty_raw(m, s), &v) {
                (Ty::Result(..), Value::Variant(_, fs)) => fs.first().map(|(_, x)| x.clone()).unwrap_or(Value::None),
                _ => v,
            };
            env.vars.push((x.to_string(), named));
        }
        let out = self.branch(m, env, e, &arms[k].body);
        env.vars.truncate(mark);
        out
    }

    /// The arm of a `match` that takes the value, and the name `some x` or a value with fields
    /// (`placed p`) gives it there: the first arm, from the top, whose patterns take it (DESIGN 23.6).
    #[inline(never)]
    fn pick<'a>(&self, m: &Checked, e: &Expr, s: &Expr, arms: &'a [Arm], v: &Value) -> R<(usize, Option<&'a str>)> {
        let st_c = self.ty(m, s);
        let st: &Ty = &st_c;
        for (k, arm) in arms.iter().enumerate() {
            for p in &arm.patterns {
                let hit = match (p, v) {
                    (Pattern::Else(_), _) => true,
                    (Pattern::None(_), Value::None) => true,
                    (Pattern::Some(x, _), w) if !matches!(w, Value::None) => return Ok((k, Some(x.as_str()))),
                    (Pattern::Bool(b, _), Value::Bool(x)) => b == x,
                    (Pattern::Value(path, bind, _), Value::Enum(x) | Value::Variant(x, _)) => {
                        let word = &path[path.len() - 1].0;
                        let hit = match st.present() {
                            Ty::Enum(id) => self.p.values(id).0.iter().any(|n| n.is(word) && n.text == *x),
                            Ty::Result(..) => word == x,
                            _ => false,
                        };
                        if hit && let Some((b, _)) = bind {
                            return Ok((k, Some(b.as_str())));
                        }
                        hit
                    }
                    (Pattern::Lit(x), w) => literal(m, x, st).is_some_and(|l| l == *w),
                    _ => false,
                };
                if hit {
                    return Ok((k, None));
                }
            }
        }
        Err(self.fail(m, e.span.line, Kind::Range, tr!("`match` のどの分岐も値を受けません", "No arm of the `match` takes the value")))
    }

    fn record(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr, base: Option<&'p Expr>, inits: &'p [FieldInit]) -> R<Value> {
        // the value the fields not written are taken from, before the fields (DESIGN 23.6)
        let from = match base {
            Some(b) => Some(self.eval(m, env, b)?),
            None => None,
        };
        let mut vals = Vec::with_capacity(inits.len());
        for fi in inits {
            vals.push(self.eval(m, env, &fi.value)?);
        }
        self.make_record(m, e, from, inits, vals)
    }

    /// A record, or a value of an enum that has fields, built from the values of its fields as
    /// written: in its fields' order and types, a field of a module's record or value held to what it
    /// declares (a list a rule walks is held to the rule's ranges by the rule), a field left out the
    /// field of the value it is made from (`..x`), else `none`.
    #[inline(never)]
    fn make_record(&self, m: &Checked, e: &Expr, from: Option<Value>, inits: &[FieldInit], vals: Vec<Value>) -> R<Value> {
        let Ref::Record(id) = rf(m, e) else { return Ok(Value::None) };
        let fields = self.p.fields(id);
        let mut given: Vec<Value> = fields.iter().map(|(f, _)| from.as_ref().and_then(|x| x.field(&f.text)).cloned().unwrap_or(Value::None)).collect();
        let mut lines: Vec<usize> = vec![e.span.line; fields.len()];
        for (fi, v) in inits.iter().zip(vals) {
            let Some(k) = fields.iter().position(|(n, _)| n.is(&fi.name)) else { continue };
            given[k] = self.coerce_at(m, &fi.value, v, &self.ty(m, &fi.value), &fields[k].1.ty)?;
            lines[k] = fi.span.line;
        }
        // the fields are held to what they declare once every one is worked out, in their order
        if matches!(id, RecId::Module { .. } | RecId::Variant { .. }) {
            for (k, (f, slot)) in fields.iter().enumerate() {
                if let Some(o) = self.outside(&given[k], slot, &f.text, Hold::Place) {
                    let (rec, path, shown, range) = (self.p.show_ty(m, &Ty::Record(id.clone())), &o.path, &o.shown, &o.range);
                    let msg = match o.count {
                        Some(n) if o.chars => tr!("{rec} のフィールド `{path}` に置くコードポイントは {n} 個で、コードポイントの数の範囲 {range} の外です", "The field `{path}` of {rec} is given {n} code points, outside its count {range}"),
                        Some(n) => tr!("{rec} のフィールド `{path}` に置く要素は {n} 個で、要素の数の範囲 {range} の外です", "The field `{path}` of {rec} is given {n} elements, outside its count {range}"),
                        None => tr!("{rec} のフィールド `{path}` に置く {shown} は、範囲 {range} の外です", "The field `{path}` of {rec} is given {shown}, outside its range {range}"),
                    };
                    return Err(self.fail(m, lines[k], Kind::Range, msg));
                }
            }
        }
        let fs: Vec<(String, Value)> = fields.iter().zip(given).map(|((n, _), v)| (n.text.clone(), v)).collect();
        Ok(match id {
            RecId::Variant { value, .. } => Value::Variant(value.clone(), fs),
            _ => Value::Record(fs),
        })
    }

    // ── Calls ───────────────────────────────────────────────────────────────────

    fn call_expr(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr, callee: &'p Expr, args: &'p [Arg]) -> R<Value> {
        // a function value called: what it is, then its arguments from the left, each read in its
        // parameter's type, then the call (DESIGN 27.3)
        if let Ref::CallValue = rf(m, e) {
            let f = self.eval(m, env, callee)?;
            let vals = self.args(m, env, args)?;
            let ps = match ty_raw(m, callee) {
                Ty::Fn(ps, _, _) => ps.clone(),
                _ => Vec::new(),
            };
            let mut given = Vec::with_capacity(vals.len());
            for ((a, v), p) in args.iter().zip(vals).zip(ps.iter().map(|p| self.sub(p))) {
                given.push(self.coerce_at(m, a.expr(), v, &self.ty(m, a.expr()), &p)?);
            }
            return self.call_value(m, e, f, given);
        }
        match rf(m, callee) {
            Ref::Fn(canon, name) => self.call_fn(m, env, e, canon, name, args),
            Ref::Rule(canon) => {
                let vals = self.args(m, env, args)?;
                self.call_rule(m, e, callee, canon, args, vals)
            }
            Ref::Date(canon, name) => {
                let vals = self.args(m, env, args)?;
                self.call_date(m, e, callee, canon, name, args, vals)
            }
            Ref::Transfer(id) => {
                let vals = self.args(m, env, args)?;
                self.transfer(m, e, id, args, vals)
            }
            Ref::Builtin(name) => {
                let xs: Vec<&'p Expr> = args.iter().map(Arg::expr).collect();
                self.builtin(m, env, e, name, xs)
            }
            // a builtin written after a value: the value is its first argument (DESIGN 21.4)
            Ref::Method(name) => {
                let ExprKind::Field(base, _, _) = &callee.kind else { return Ok(Value::None) };
                let mut xs: Vec<&'p Expr> = vec![&**base];
                xs.extend(args.iter().map(Arg::expr));
                self.builtin(m, env, e, name, xs)
            }
            _ => Ok(Value::None),
        }
    }

    /// A value as it goes into a rule: on the wire, rulec's.
    fn to_wire(&self, v: &Value, t: &Ty) -> Option<Wire> {
        Some(match (v, t) {
            (Value::None, _) => Wire::None,
            (v, Ty::Opt(x)) => self.to_wire(v, x)?,
            (Value::Num(r), Ty::Num(n)) => Wire::Int(wire_int(*r, n)?),
            (Value::Bool(b), _) => Wire::Bool(*b),
            (Value::Str(s), _) => Wire::Str(s.clone()),
            (Value::Date(d), _) => Wire::Date(crate::date::text(*d)),
            (Value::Enum(x), _) => Wire::Enum(x.clone()),
            (Value::List(xs), Ty::List(x)) => {
                let Ty::Record(id) = &**x else { return None };
                let fields = self.p.fields(id);
                Wire::List(
                    xs.iter()
                        .map(|el| {
                            fields
                                .iter()
                                .map(|(f, s)| Some((f.text.clone(), self.to_wire(el.field(&f.text).unwrap_or(&Value::None), &s.ty)?)))
                                .collect::<Option<Vec<_>>>()
                        })
                        .collect::<Option<Vec<_>>>()?,
                )
            }
            _ => return None,
        })
    }

    /// A value as it comes out of a rule.
    fn off_wire(&self, w: &Wire, t: &Ty) -> Option<Value> {
        Some(match (w, t) {
            (Wire::None, _) => Value::None,
            (w, Ty::Opt(x)) => self.off_wire(w, x)?,
            (Wire::Int(i), Ty::Num(n)) => Value::Num(if n.is_rate() { Rat::int(*i).mul(n.step?) } else { Rat::int(*i) }),
            (Wire::Bool(b), _) => Value::Bool(*b),
            (Wire::Str(s), _) => Value::Str(s.clone()),
            (Wire::Date(s), _) => Value::Date(crate::date::parse(s)?),
            (Wire::Enum(x), _) => Value::Enum(x.clone()),
            _ => return None,
        })
    }

    /// A rule called with the values of its arguments: its outputs, or the failure `rule` when it
    /// refuses them (DESIGN 6.1).
    #[inline(never)]
    fn call_rule(&mut self, m: &Checked, e: &Expr, callee: &Expr, canon: &std::path::Path, args: &[Arg], vals: Vec<Value>) -> R<Value> {
        let p = self.p;
        let Some(facts) = p.rules.get(canon) else { return Ok(Value::None) };
        let name = source(m, callee.span);
        // what the call gives each input, and each list the rule walks, by the rule's names
        let mut places: Vec<(String, Ty)> = facts.inputs.iter().map(|c| (c.name.clone(), crate::names::column_slot(c, canon).ty)).collect();
        for (seq, _, _) in &facts.elements {
            places.push((seq.clone(), Ty::List(Box::new(Ty::Record(RecId::Element(canon.to_path_buf(), seq.clone()))))));
        }
        let mut given: Vec<Value> = vec![Value::None; places.len()];
        let mut shown_args = Vec::new();
        for (a, v) in args.iter().zip(vals) {
            let Arg::Named(n, _, x) = a else { continue };
            let k = places
                .iter()
                .position(|(q, _)| q == n)
                .or_else(|| facts.inputs.iter().position(|c| c.alias == *n))
                .or_else(|| facts.elements.iter().position(|(_, al, _)| al == n).map(|i| facts.inputs.len() + i));
            if let Some(k) = k {
                let v = self.coerce_at(m, x, v, &self.ty(m, x), &places[k].1)?;
                shown_args.push(format!("{n}: {}", p.show(m, &v, &places[k].1)));
                given[k] = v;
            }
        }
        let mut wire: Values = Vec::new();
        for ((pn, pt), v) in places.iter().zip(&given) {
            match self.to_wire(v, pt) {
                Some(w) => wire.push((pn.clone(), w)),
                None => {
                    let shown = p.show(m, v, pt);
                    return Err(self.fail(m, e.span.line, Kind::Rule, tr!("規則 `{name}` の入力 `{pn}` に渡す {shown} は、その型の値ではありません", "{shown}, given to the input `{pn}` of the rule `{name}`, is no value of its type")));
                }
            }
        }
        let call = format!("{name}({})", shown_args.join(", "));
        let in_json = Json::obj(wire.iter().map(|(n, w)| (n.clone(), wire_json(w))));
        if let Some(g) = &mut self.given {
            g.push((m.canon.clone(), e.id, wire.clone()));
        }
        let Some(rules) = self.suite.rules.as_ref() else {
            return Err(self.fail(m, e.span.line, Kind::Rule, tr!("規則 `{name}` を呼べません（rulec がつながっていません）", "The rule `{name}` cannot be called: rulec is not joined")));
        };
        let out_ty = Ty::Record(RecId::Outputs(canon.to_path_buf()));
        let head = |kind: &str| vec![("line".to_string(), Json::int(e.span.line as i128)), ("module".to_string(), Json::str(m.module.name.text.clone())), (kind.to_string(), Json::str(name.clone()))];
        match rules.eval(&p.file(canon), &wire) {
            Ok(outs) => {
                let mut fs = Vec::new();
                for c in &facts.outputs {
                    let t = crate::names::column_slot(c, canon).ty;
                    let w = outs.iter().find(|(n, _)| *n == c.name).map(|(_, w)| w.clone()).unwrap_or(Wire::None);
                    let v = self.off_wire(&w, &t).unwrap_or(Value::None);
                    let v = num_fit(v).map_err(|a| self.arith(m, e, a))?;
                    fs.push((c.name.clone(), v));
                }
                let v = Value::Record(fs);
                let shown = p.show(m, &v, &out_ty);
                let out_json = Json::obj(outs.iter().map(|(n, w)| (n.clone(), wire_json(w))));
                let mut j = head("rule");
                j.extend([("in".to_string(), in_json), ("out".to_string(), out_json)]);
                self.called(m, e, format!("{call} = {shown}"), Json::Obj(j));
                Ok(v)
            }
            Err(err) => {
                let why = match err {
                    RuleError::Input(t) | RuleError::Contradiction(t) => t,
                    RuleError::Unread(said) => said.first().map(|s| s.message.clone()).unwrap_or_default(),
                };
                let mut j = head("rule");
                j.extend([("in".to_string(), in_json), ("refused".to_string(), Json::str(why.en.clone()))]);
                self.called(m, e, format!("{call}: {}", why.en), Json::Obj(j));
                // the reason is the trace's: the message is what the code of every target says (DESIGN 10.4)
                Err(self.fail(m, e.span.line, Kind::Rule, tr!("規則 `{name}` が入力を受け付けません", "The rule `{name}` refuses its inputs")))
            }
        }
    }

    /// A date of a dates file worked out for the values of its arguments: its day, or the failure
    /// `date` when koyomi finds none (DESIGN 6.2). The inputs the date does not read are given a
    /// value of their range, the lowest, then the highest should another date the file works out
    /// first stop there.
    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    fn call_date(&mut self, m: &Checked, e: &Expr, callee: &Expr, canon: &std::path::Path, date: &str, args: &[Arg], vals: Vec<Value>) -> R<Value> {
        let p = self.p;
        let Some(facts) = p.dates.get(canon) else { return Ok(Value::None) };
        let name = source(m, callee.span);
        let mut given: Vec<(String, i64)> = Vec::new();
        let mut shown_args = Vec::new();
        for (a, v) in args.iter().zip(vals) {
            let Arg::Named(n, _, x) = a else { continue };
            let Some(input) = facts.inputs.iter().find(|i| i.name == *n || i.alias == *n) else { continue };
            let place = match input.kind {
                ritsu_ports::DateKind::Date => Ty::Date,
                ritsu_ports::DateKind::Int => Ty::Num(NumTy::number()),
            };
            let v = self.coerce_at(m, x, v, &self.ty(m, x), &place)?;
            shown_args.push(format!("{n}: {}", p.show(m, &v, &place)));
            let at = match v {
                Value::Date(d) => d,
                Value::Num(r) => i64::try_from(r.num.div_euclid(r.den)).unwrap_or(i64::MAX),
                _ => 0,
            };
            given.push((input.name.clone(), at));
        }
        let call = format!("{name}({})", shown_args.join(", "));
        let in_json = Json::obj(given.iter().map(|(n, at)| {
            let date = facts.inputs.iter().any(|i| i.name == *n && i.kind == ritsu_ports::DateKind::Date);
            (n.clone(), if date { Json::str(crate::date::text(*at)) } else { Json::int(*at as i128) })
        }));
        if let Some(g) = &mut self.given {
            let on_wire = given.iter().map(|(n, at)| {
                let date = facts.inputs.iter().any(|i| i.name == *n && i.kind == ritsu_ports::DateKind::Date);
                (n.clone(), if date { Wire::Date(crate::date::text(*at)) } else { Wire::Int(*at as i128) })
            });
            g.push((m.canon.clone(), e.id, on_wire.collect()));
        }
        let Some(dates) = self.suite.dates.as_ref() else {
            return Err(self.fail(m, e.span.line, Kind::Date, tr!("日付 `{name}` を求められません（koyomi がつながっていません）", "The date `{name}` cannot be worked out: koyomi is not joined")));
        };
        let mut answer: Result<i64, Text> = Err(Text::default());
        for lowest in [true, false] {
            let inputs: Vec<(String, i64)> = facts
                .inputs
                .iter()
                .map(|i| given.iter().find(|(n, _)| *n == i.name).cloned().unwrap_or_else(|| (i.name.clone(), if lowest { i.min } else { i.max })))
                .collect();
            answer = match dates.eval(&p.file(canon), &inputs) {
                Ok(vals) => match vals.into_iter().find(|(n, _)| n == date).map(|(_, v)| v) {
                    Some(DateValue::Day(d)) => Ok(d),
                    Some(DateValue::Stopped(t)) => Err(t),
                    None => Err(tr!("日付 `{date}` がありません", "there is no date `{date}`")),
                },
                Err(said) => Err(said.first().map(|s| s.message.clone()).unwrap_or_default()),
            };
            if answer.is_ok() || facts.inputs.len() == given.len() {
                break;
            }
        }
        let head = vec![("line".to_string(), Json::int(e.span.line as i128)), ("module".to_string(), Json::str(m.module.name.text.clone())), ("date".to_string(), Json::str(name.clone())), ("in".to_string(), in_json)];
        match answer {
            Ok(d) => {
                let mut j = head;
                j.push(("out".to_string(), Json::str(crate::date::text(d))));
                self.called(m, e, format!("{call} = {}", crate::date::text(d)), Json::Obj(j));
                Ok(Value::Date(d))
            }
            Err(why) => {
                let mut j = head;
                j.push(("stopped".to_string(), Json::str(why.en.clone())));
                self.called(m, e, format!("{call}: {}", why.en), Json::Obj(j));
                Err(self.fail(m, e.span.line, Kind::Date, tr!("日付 `{name}` が求まりません", "The date `{name}` cannot be worked out")))
            }
        }
    }

    /// A transfer built from the values of its arguments (DESIGN 6.3): an amount below 0 is the
    /// failure `range`, as chobo takes none.
    #[inline(never)]
    fn transfer(&mut self, m: &Checked, e: &Expr, id: &TransferId, args: &[Arg], vals: Vec<Value>) -> R<Value> {
        let p = self.p;
        let Some((_, params)) = p.transfer(id) else { return Ok(Value::None) };
        let mut given: Vec<Option<Value>> = vec![None; params.len()];
        // the amounts, on the wire, as they are given: kept for the checks across the languages
        let mut amounts: Values = Vec::new();
        for (a, v) in args.iter().zip(vals) {
            let Arg::Named(n, _, x) = a else { continue };
            let Some(k) = params.iter().position(|(q, _)| q == n) else { continue };
            let v = self.coerce_at(m, x, v, &self.ty(m, x), &params[k].1)?;
            if let Value::Num(r) = v {
                amounts.push((n.clone(), Wire::Int(r.num.div_euclid(r.den))));
            }
            if let Value::Num(r) = v
                && r.num < 0
            {
                if let Some(g) = &mut self.given {
                    g.push((m.canon.clone(), e.id, amounts));
                }
                let shown = num_text(r, &unit_of(&params[k].1));
                let t = format!("{}.{}", id.transfer, id.op);
                return Err(self.fail(m, x.span.line, Kind::Range, tr!("振替 `{t}` の額 `{n}` に、負の値 {shown} を渡しています", "The amount `{n}` of the transfer `{t}` is given {shown}, which is below 0")));
            }
            given[k] = Some(v);
        }
        if let Some(g) = &mut self.given {
            g.push((m.canon.clone(), e.id, amounts));
        }
        let args: Vec<(String, Value)> = params.iter().zip(given).filter_map(|((n, _), v)| v.map(|v| (n.clone(), v))).collect();
        let v = Value::Transfer(Box::new(TransferValue { id: id.clone(), args }));
        let t = Ty::Transfer(id.clone());
        let shown = p.show(m, &v, &t);
        let json = Json::obj([("line", Json::int(e.span.line as i128)), ("module", Json::str(m.module.name.text.clone())), ("transfer", p.to_json(&v, &t))]);
        self.called(m, e, shown, json);
        Ok(v)
    }

    fn called(&mut self, m: &Checked, e: &Expr, text: String, json: Json) {
        self.trace.events.push(Event::Call(Called { module: m.module.name.text.clone(), file: m.path.clone(), line: e.span.line, text, json }));
    }

    /// A function value called at `at` with the values of its parameters (DESIGN 27.3): a frame of the
    /// function the lambda was made in, one more call deep, the lambda's body evaluated with the names
    /// it caught and its parameters. Its answer, in the type of the lambda's answer.
    #[inline(never)]
    fn call_value(&mut self, m: &'p Checked, at: &Expr, f: Value, vals: Vec<Value>) -> R<Value> {
        let Value::Closure(c) = f else {
            return Err(self.fail(m, at.span.line, Kind::Range, tr!("関数の値ではないものを呼んでいます", "What is called is no function value")));
        };
        if self.frames.len() >= self.limit {
            return Err(Stop::Diverged(Box::new(Diverged { function: c.function.clone(), module: m.module.name.text.clone(), line: at.span.line, depth: self.limit })));
        }
        let lm: &'p Checked = self.p.module(&c.module);
        let Some((_, lam)) = lm.lambda(c.expr) else { return Ok(Value::None) };
        let ExprKind::Lambda(ps, body) = &lam.kind else { return Ok(Value::None) };
        self.frames.push(Frame { function: c.function.clone(), line: at.span.line, subst: c.subst.clone() });
        let mut inner = Env { vars: c.env.clone() };
        for ((p, _), v) in ps.iter().zip(vals) {
            inner.vars.push((p.clone(), v));
        }
        let out = self.eval(lm, &mut inner, body);
        let out = out.and_then(|v| {
            let answer = match self.ty(lm, lam).into_owned() {
                Ty::Fn(_, r, _) => *r,
                t => t,
            };
            self.coerce_at(lm, body, v, &self.ty(lm, body), &answer)
        });
        self.frames.pop();
        out
    }

    // ── Builtins ────────────────────────────────────────────────────────────────

    /// A function given to a builtin, applied: its answer, in the type of its body. A function value
    /// given to a builtin is read as the lambda that calls it (`wrapped.rs`).
    fn apply(&mut self, m: &'p Checked, env: &mut Env, lam: &'p Expr, vals: Vec<Value>) -> R<Value> {
        let ExprKind::Lambda(ps, body) = &lam.kind else { return Ok(Value::None) };
        let mark = env.vars.len();
        for ((p, _), v) in ps.iter().zip(vals) {
            env.vars.push((p.clone(), v));
        }
        let out = self.eval(m, env, body);
        env.vars.truncate(mark);
        out
    }

    /// The list a builtin walks.
    fn list(&mut self, m: &'p Checked, env: &mut Env, x: &'p Expr) -> R<Vec<Value>> {
        Ok(match self.eval(m, env, x)? {
            Value::List(xs) => xs,
            _ => Vec::new(),
        })
    }

    /// A function given to a builtin, applied to each value in turn: what it answers for each, read in
    /// the type `to`, until `stop` says the answers so far decide the builtin.
    fn each(&mut self, m: &'p Checked, env: &mut Env, xs: &[Value], lam: &'p Expr, to: Option<&Ty>, stop: fn(&Value) -> bool) -> R<Vec<Value>> {
        let mut out = Vec::with_capacity(xs.len());
        for x in xs {
            let v = self.apply(m, env, lam, vec![x.clone()])?;
            let v = match to {
                Some(t) => self.coerce_at(m, lam, v, &self.ty(m, lam), t)?,
                None => v,
            };
            let done = stop(&v);
            out.push(v);
            if done {
                break;
            }
        }
        Ok(out)
    }

    fn builtin(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr, name: &str, xs: Vec<&'p Expr>) -> R<Value> {
        match (name, xs.as_slice()) {
            ("reduce", [list, first, f]) => self.reduce(m, env, e, list, first, f),
            (
                "count" | "sum" | "any" | "all" | "map" | "first" | "last" | "max_of" | "min_of" | "running_totals" | "sort_by" | "sort_by_desc" | "max_by" | "min_by" | "flat_map" | "filter_map" | "group_by",
                [list, f],
            ) => self.walk(m, env, e, name, list, f),
            ("filter", [x, f]) => self.filter(m, env, e, x, f),
            ("to_dict", [list, kf, vf]) => self.to_dict(m, env, e, list, kf, vf),
            ("map_values", [d, f]) => self.map_values(m, env, e, d, f),
            ("merge_with", [d, o, f]) => self.merge_with(m, env, e, d, o, f),
            _ => {
                let mut vals = Vec::with_capacity(xs.len());
                for x in &xs {
                    vals.push(self.eval(m, env, x)?);
                }
                self.scalar(m, e, name, &xs, vals)
            }
        }
    }

    /// A builtin that applies a function to each element of a list: from the first, or, for `last`,
    /// from the last (DESIGN 24.3).
    fn walk(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr, name: &str, list: &'p Expr, lam: &'p Expr) -> R<Value> {
        let to = self.lambda_place(m, e, name);
        let stop: fn(&Value) -> bool = match name {
            "any" | "first" | "last" => |v| matches!(v, Value::Bool(true)),
            "all" => |v| matches!(v, Value::Bool(false)),
            _ => |_| false,
        };
        let mut xs = self.list(m, env, list)?;
        if name == "last" {
            xs.reverse();
        }
        let answers = self.each(m, env, &xs, lam, to.as_ref(), stop)?;
        self.fold(m, e, name, xs, answers)
    }

    /// `filter` of a list, or of a dict, whose pairs its function is given as entries in the order of
    /// their keys (DESIGN 24.3).
    fn filter(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr, x: &'p Expr, lam: &'p Expr) -> R<Value> {
        let yes = |v: &Value| matches!(v, Value::Bool(true));
        match self.eval(m, env, x)? {
            Value::Dict(ps) => {
                let entries: Vec<Value> = ps.iter().map(|(k, v)| entry(k.clone(), v.clone())).collect();
                let answers = self.each(m, env, &entries, lam, None, |_| false)?;
                Ok(Value::Dict(ps.into_iter().zip(answers).filter(|(_, a)| yes(a)).map(|(p, _)| p).collect()))
            }
            Value::List(xs) => {
                let answers = self.each(m, env, &xs, lam, None, |_| false)?;
                self.fold(m, e, "filter", xs, answers)
            }
            _ => Ok(Value::List(Vec::new())),
        }
    }

    /// `xs.to_dict(x => key, x => value)`: for each element, its key, then its value; the later of two
    /// elements of one key wins (DESIGN 24.3).
    #[allow(clippy::too_many_arguments)]
    fn to_dict(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr, list: &'p Expr, kf: &'p Expr, vf: &'p Expr) -> R<Value> {
        let xs = self.list(m, env, list)?;
        let (kt, vt) = dict_types(&self.ty(m, e));
        let mut pairs = Vec::with_capacity(xs.len());
        for x in xs {
            let k = self.apply(m, env, kf, vec![x.clone()])?;
            let k = self.coerce_at(m, kf, k, &self.ty(m, kf), &kt)?;
            let v = self.apply(m, env, vf, vec![x])?;
            let v = self.coerce_at(m, vf, v, &self.ty(m, vf), &vt)?;
            pairs.push((k, v));
        }
        Ok(Value::Dict(self.p.dict_of(&kt, pairs)))
    }

    /// `d.map_values(v => …)`: the function given each value, in the order of the keys.
    fn map_values(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr, d: &'p Expr, lam: &'p Expr) -> R<Value> {
        let ps = match self.eval(m, env, d)? {
            Value::Dict(ps) => ps,
            _ => Vec::new(),
        };
        let (_, vt) = dict_types(&self.ty(m, e));
        let mut out = Vec::with_capacity(ps.len());
        for (k, v) in ps {
            let u = self.apply(m, env, lam, vec![v])?;
            out.push((k, self.coerce_at(m, lam, u, &self.ty(m, lam), &vt)?));
        }
        Ok(Value::Dict(out))
    }

    /// `d.merge_with(o, (a, b) => …)`: the pairs of both, a key of both given to the function with
    /// `d`'s value and `o`'s, in the order of the keys (DESIGN 24.3).
    #[allow(clippy::too_many_arguments)]
    fn merge_with(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr, d: &'p Expr, o: &'p Expr, lam: &'p Expr) -> R<Value> {
        let me = self.ty(m, e).into_owned();
        let a = self.eval(m, env, d)?;
        let a = self.coerce_at(m, d, a, &self.ty(m, d), &me)?;
        let b = self.eval(m, env, o)?;
        let b = self.coerce_at(m, o, b, &self.ty(m, o), &me)?;
        let (Value::Dict(a), Value::Dict(b)) = (a, b) else { return Ok(Value::Dict(Vec::new())) };
        let (kt, vt) = dict_types(&me);
        let mut out = a.clone();
        for (k, w) in b {
            let v = match a.iter().find(|(x, _)| self.p.key_cmp(&kt, x, &k) == Ordering::Equal) {
                Some((_, v)) => {
                    let r = self.apply(m, env, lam, vec![v.clone(), w])?;
                    self.coerce_at(m, lam, r, &self.ty(m, lam), &vt)?
                }
                None => w,
            };
            out = self.p.dict_of(&kt, out.into_iter().chain([(k, v)]).collect());
        }
        Ok(Value::Dict(out))
    }

    /// `reduce(xs, first, (acc, x) => …)`: the running value is read in the type of the first value,
    /// which the function given takes it as, and the answer in the type of `reduce`.
    fn reduce(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr, list: &'p Expr, first: &'p Expr, lam: &'p Expr) -> R<Value> {
        let xs = self.list(m, env, list)?;
        let mut acc = self.eval(m, env, first)?;
        for x in xs {
            let v = self.apply(m, env, lam, vec![acc, x])?;
            acc = self.answer_for(m, lam, first, v)?;
        }
        self.answer_for(m, first, e, acc)
    }

    /// The type the answers of the function given to a builtin are read in, where it sums them or
    /// keeps them.
    #[inline(never)]
    fn lambda_place(&self, m: &Checked, e: &Expr, name: &str) -> Option<Ty> {
        let me_c = self.ty(m, e);
        let me: &Ty = &me_c;
        match name {
            "sum" => Some(me.clone()),
            "max_of" | "min_of" => Some(me.present().clone()),
            "map" => match me {
                Ty::List(t) => Some((**t).clone()),
                _ => None,
            },
            // the lists `flat_map` joins, and the values `filter_map` keeps
            "flat_map" => Some(me.clone()),
            "filter_map" => match me {
                Ty::List(t) => Some(Ty::Opt(t.clone())),
                _ => None,
            },
            "running_totals" => match me {
                Ty::List(t) => match &**t {
                    Ty::Record(RecId::Running(_, w)) => Some((**w).clone()),
                    _ => None,
                },
                _ => None,
            },
            // the keys a dict is made of
            "group_by" => Some(dict_types(me).0),
            _ => None,
        }
    }

    /// A builtin that walks a list, from the list and what the function given answered for its
    /// elements (as many as it was applied to).
    #[inline(never)]
    fn fold(&self, m: &Checked, e: &Expr, name: &str, list: Vec<Value>, answers: Vec<Value>) -> R<Value> {
        let yes = |v: &Value| matches!(v, Value::Bool(true));
        match name {
            "count" => Ok(Value::Num(Rat::int(answers.iter().filter(|v| yes(v)).count() as i128))),
            "any" => Ok(Value::Bool(answers.iter().any(yes))),
            "all" => Ok(Value::Bool(answers.iter().all(yes))),
            "map" => Ok(Value::List(answers)),
            "filter" => Ok(Value::List(list.into_iter().zip(answers).filter(|(_, a)| yes(a)).map(|(x, _)| x).collect())),
            // `last` walks the list from its end
            "first" | "last" => Ok(list.into_iter().zip(answers).find(|(_, a)| yes(a)).map(|(x, _)| x).unwrap_or(Value::None)),
            "sum" => {
                let mut acc = Rat::zero();
                for v in &answers {
                    acc = num::add(acc, v.num().unwrap_or(Rat::zero())).map_err(|a| self.arith(m, e, a))?;
                }
                Ok(Value::Num(acc))
            }
            "max_of" | "min_of" => {
                let t = self.ty(m, e).present().clone();
                let mut best: Option<Value> = None;
                for v in answers {
                    let better = match (&best, &v) {
                        (None, _) => true,
                        (Some(Value::Num(b)), Value::Num(n)) => num::cmp(*n, *b) == if name == "max_of" { Ordering::Greater } else { Ordering::Less },
                        (Some(Value::Date(b)), Value::Date(n)) => n.cmp(b) == if name == "max_of" { Ordering::Greater } else { Ordering::Less },
                        (Some(Value::Enum(b)), Value::Enum(n)) => self.rank(&t, n).cmp(&self.rank(&t, b)) == if name == "max_of" { Ordering::Greater } else { Ordering::Less },
                        _ => false,
                    };
                    if better {
                        best = Some(v);
                    }
                }
                Ok(best.unwrap_or(Value::None))
            }
            "running_totals" => {
                let weights: Vec<Rat> = answers.iter().map(|v| v.num().unwrap_or(Rat::zero())).collect();
                let mut total = Rat::zero();
                for w in &weights {
                    total = num::add(total, *w).map_err(|a| self.arith(m, e, a))?;
                }
                let mut before = Rat::zero();
                let mut out = Vec::with_capacity(list.len());
                for (x, w) in list.into_iter().zip(weights) {
                    let upto = num::add(before, w).map_err(|a| self.arith(m, e, a))?;
                    out.push(Value::Record(vec![("item".into(), x), ("before".into(), Value::Num(before)), ("upto".into(), Value::Num(upto)), ("total".into(), Value::Num(total))]));
                    before = upto;
                }
                Ok(Value::List(out))
            }
            // in the order of the keys the function answers, each two of one key in the order of the
            // list (DESIGN 24.3)
            "sort_by" | "sort_by_desc" => {
                let kt = self.key_ty(m, e);
                let mut pairs: Vec<(Value, Value)> = answers.into_iter().zip(list).collect();
                if name == "sort_by" {
                    pairs.sort_by(|(a, _), (b, _)| self.p.key_cmp(&kt, a, b));
                } else {
                    pairs.sort_by(|(a, _), (b, _)| self.p.key_cmp(&kt, b, a));
                }
                Ok(Value::List(pairs.into_iter().map(|(_, x)| x).collect()))
            }
            // the first element of the greatest key, or of the least
            "max_by" | "min_by" => {
                let kt = self.key_ty(m, e);
                let want = if name == "max_by" { Ordering::Greater } else { Ordering::Less };
                let mut best: Option<(Value, Value)> = None;
                for (k, x) in answers.into_iter().zip(list) {
                    let better = match &best {
                        None => true,
                        Some((b, _)) => self.p.key_cmp(&kt, &k, b) == want,
                    };
                    if better {
                        best = Some((k, x));
                    }
                }
                Ok(best.map(|(_, x)| x).unwrap_or(Value::None))
            }
            "flat_map" => Ok(Value::List(
                answers
                    .into_iter()
                    .flat_map(|a| match a {
                        Value::List(xs) => xs,
                        _ => Vec::new(),
                    })
                    .collect(),
            )),
            "filter_map" => Ok(Value::List(answers.into_iter().filter(|a| !matches!(a, Value::None)).collect())),
            // the elements of each key, in the order of the list, the keys in their order
            "group_by" => {
                let (kt, _) = dict_types(&self.ty(m, e));
                let mut groups: Vec<(Value, Value)> = Vec::new();
                for (k, x) in answers.into_iter().zip(list) {
                    match groups.binary_search_by(|(g, _)| self.p.key_cmp(&kt, g, &k)) {
                        Ok(i) => {
                            if let Value::List(xs) = &mut groups[i].1 {
                                xs.push(x);
                            }
                        }
                        Err(i) => groups.insert(i, (k, Value::List(vec![x]))),
                    }
                }
                Ok(Value::Dict(groups))
            }
            _ => Ok(Value::None),
        }
    }

    /// The type of the keys the function given to `sort_by` and its kind answer: the function's.
    fn key_ty(&self, m: &Checked, e: &Expr) -> Ty {
        let ExprKind::Call(callee, args) = &e.kind else { return Ty::Error };
        let lam = match (rf(m, callee), args.as_slice()) {
            (Ref::Method(_), [f]) | (Ref::Builtin(_), [_, f]) => f.expr(),
            _ => return Ty::Error,
        };
        self.ty(m, lam).into_owned()
    }

    /// A builtin that takes no function, from the values of its arguments.
    #[inline(never)]
    fn scalar(&self, m: &Checked, e: &Expr, name: &str, xs: &[&Expr], vals: Vec<Value>) -> R<Value> {
        let me_c = self.ty(m, e);
        let me: &Ty = &me_c;
        // the numbers `mod`, `isqrt`, `allocate` and the roundings take: a value that is no number is
        // one the check of types lets no module give them, and the evaluation stops there rather than
        // read it as 0 (DESIGN 8; the model stops there too, DESIGN 15.3)
        let n = |i: usize| match vals.get(i).and_then(Value::num) {
            Some(r) => r,
            None => panic!("kaname: `{name}` is given {:?}, which is no number, and the check of types lets no module do so", vals.get(i)),
        };
        // a whole number a builtin takes (`at`, `take`, `numbers`), past what fits in a list where it
        // is past it
        let whole = |i: usize| -> i128 {
            let r = n(i);
            r.num.div_euclid(r.den)
        };
        match name {
            "min" | "max" => {
                let mut it = vals.into_iter();
                let (a, b) = (it.next().unwrap_or(Value::None), it.next().unwrap_or(Value::None));
                let (a, b) = (self.coerce_at(m, xs[0], a, &self.ty(m, xs[0]), me)?, self.coerce_at(m, xs[1], b, &self.ty(m, xs[1]), me)?);
                let order = match (&a, &b) {
                    (Value::Num(p), Value::Num(q)) => num::cmp(*p, *q),
                    (Value::Date(p), Value::Date(q)) => p.cmp(q),
                    (Value::Str(p), Value::Str(q)) => p.cmp(q),
                    (Value::Enum(p), Value::Enum(q)) => self.rank(me, p).cmp(&self.rank(me, q)),
                    _ => Ordering::Equal,
                };
                let first = if name == "min" { order != Ordering::Greater } else { order != Ordering::Less };
                Ok(if first { a } else { b })
            }
            // the remainder of the whole part of `x` by `n`, from 0 to |n| − 1 (Euclid's, DESIGN 24.3);
            // `zero` where `n` is 0
            "mod" => {
                let d = n(1);
                if d.num == 0 {
                    return Err(self.arith(m, e, crate::num::Arith::Zero));
                }
                Ok(Value::Num(num::modulo(n(0), d.num.div_euclid(d.den))))
            }
            "isqrt" => match num::isqrt(n(0)) {
                Some(v) => Ok(Value::Num(v)),
                None => {
                    let shown = n(0).to_string();
                    Err(self.fail(m, e.span.line, Kind::Range, tr!("`isqrt` に負の値 {shown} を渡しています", "`isqrt` is given {shown}, which is below 0")))
                }
            },
            "allocate" => {
                let (tc_c, ts_c) = (self.ty(m, xs[1]), self.ty(m, xs[2]));
                let (tc, ts): (&Ty, &Ty) = (&tc_c, &ts_c);
                let both = common(tc, ts);
                let c = convert(n(1), &unit_of(tc), &unit_of(both)).map_err(|a| self.arith(m, xs[1], a))?;
                let s = convert(n(2), &unit_of(ts), &unit_of(both)).map_err(|a| self.arith(m, xs[2], a))?;
                let v = num::allocate(n(0), c, s).map_err(|a| self.arith(m, e, a))?;
                convert(v, &unit_of(&self.ty(m, xs[0])), &unit_of(me)).map(Value::Num).map_err(|a| self.arith(m, e, a))
            }
            "up" | "down" | "half_up" | "half_down" | "half_even" => {
                let tx_c = self.ty(m, xs[0]);
        let tx: &Ty = &tx_c;
                let g = convert(n(1), &unit_of(&self.ty(m, xs[1])), &unit_of(tx)).map_err(|a| self.arith(m, xs[1], a))?;
                let way = Rounding::parse(name).unwrap_or(Rounding::Down);
                let v = num::round(n(0), way, g).map_err(|a| self.arith(m, e, a))?;
                convert(v, &unit_of(tx), &unit_of(me)).map(Value::Num).map_err(|a| self.arith(m, e, a))
            }
            "count" => Ok(Value::Num(Rat::int(match vals.first() {
                Some(Value::List(xs)) | Some(Value::Set(xs)) => xs.len() as i128,
                Some(Value::Dict(ps)) => ps.len() as i128,
                Some(Value::Str(s)) => crate::text::count(s) as i128,
                _ => 0,
            }))),
            "starts_with" => Ok(Value::Bool(matches!((vals.first(), vals.get(1)), (Some(Value::Str(s)), Some(Value::Str(p))) if s.as_bytes().starts_with(p.as_bytes())))),
            // lists one after another, or strings
            "concat" if matches!(me, Ty::List(_) | Ty::Empty) => {
                let mut out = Vec::new();
                for (x, v) in xs.iter().zip(vals) {
                    if let Value::List(ys) = self.coerce_at(m, x, v, &self.ty(m, x), me)? {
                        out.extend(ys);
                    }
                }
                Ok(Value::List(out))
            }
            "concat" => Ok(Value::Str(
                vals.iter()
                    .filter_map(|v| match v {
                        Value::Str(s) => Some(s.as_str()),
                        _ => None,
                    })
                    .collect(),
            )),
            "text" => {
                let t_c = self.ty(m, xs[0]);
        let t: &Ty = &t_c;
                Ok(Value::Str(match (vals.first().unwrap_or(&Value::None), t) {
                    (Value::Num(r), Ty::Num(nt)) => num_text(*r, &nt.unit),
                    (Value::Num(r), _) => r.to_string(),
                    (Value::Str(s), _) => s.clone(),
                    (Value::Bool(b), _) => b.to_string(),
                    (Value::Date(d), _) => crate::date::text(*d),
                    (Value::Enum(x), _) => x.clone(),
                    (v, t) => self.p.show(m, v, t),
                }))
            }
            // `numbers(a, b)`: a to b, both in; none where b is below a. The count is held to int64,
            // and a list past `NUMBERS` stops the evaluation (DESIGN 24.3)
            "numbers" => {
                let (a, b) = (whole(0), whole(1));
                if b < a {
                    return Ok(Value::List(Vec::new()));
                }
                let count = b - a + 1;
                if count > i64::MAX as i128 {
                    return Err(self.arith(m, e, crate::num::Arith::Overflow));
                }
                if count > NUMBERS {
                    let function = self.frames.last().map(|f| f.function.clone()).unwrap_or_default();
                    return Err(Stop::Diverged(Box::new(Diverged { function, module: m.module.name.text.clone(), line: e.span.line, depth: self.frames.len() })));
                }
                Ok(Value::List((a..=b).map(|i| Value::Num(Rat::int(i))).collect()))
            }
            // `ok(x)` and `err(e)`: a value of a `result`, the value it holds read in its type (DESIGN 26.3)
            "ok" | "err" => {
                let (a, er) = match me.present() {
                    Ty::Result(a, er) => ((**a).clone(), (**er).clone()),
                    _ => (Ty::Error, Ty::Error),
                };
                let to = if name == "ok" { a } else { er };
                let v = self.coerce_at(m, xs[0], vals.into_iter().next().unwrap_or(Value::None), &self.ty(m, xs[0]), &to)?;
                Ok(Value::Variant(name.to_string(), vec![(if name == "ok" { "value" } else { "error" }.to_string(), v)]))
            }
            // `char(n)`: the code point `n`, a constant the check holds to a scalar value (DESIGN 25.2)
            "char" => {
                let c = u32::try_from(whole(0)).ok().and_then(char::from_u32);
                match c {
                    Some(c) => Ok(Value::Str(c.to_string())),
                    None => panic!("kaname: `char` is given {:?}, which is no scalar value, and the check of types lets no module do so", vals.first()),
                }
            }
            // the builtins of a string (DESIGN 25.2), and `to_text` of a list of code points
            _ if matches!(vals.first(), Some(Value::Str(_))) || name == "to_text" => self.string(m, e, name, vals),
            // the builtins of a dict, a set and a list that take no function (DESIGN 24.3)
            _ => self.collection(m, e, name, xs, vals),
        }
    }

    /// The builtins of a string, from the values of their arguments, the string first (DESIGN 25.2):
    /// each counts, cuts and finds in code points, the steps of `text.rs`. A padding past
    /// `text::LONGEST` stops the evaluation, as a list of more numbers does (DESIGN 24.3).
    #[inline(never)]
    fn string(&self, m: &Checked, e: &Expr, name: &str, vals: Vec<Value>) -> R<Value> {
        use crate::text;
        let s = |i: usize| -> &str {
            match vals.get(i) {
                Some(Value::Str(x)) => x.as_str(),
                other => panic!("kaname: `{name}` is given {other:?} where a string goes, and the check of types lets no module do so"),
            }
        };
        let whole = |i: usize| -> i128 {
            match vals.get(i).and_then(Value::num) {
                Some(r) => r.num.div_euclid(r.den),
                None => panic!("kaname: `{name}` is given {:?} where a whole number goes, and the check of types lets no module do so", vals.get(i)),
            }
        };
        let strs = |xs: Vec<String>| Value::List(xs.into_iter().map(Value::Str).collect());
        let num = |n: i128| Value::Num(Rat::int(n));
        Ok(match name {
            "chars" => strs(s(0).chars().map(String::from).collect()),
            "code_points" => Value::List(s(0).chars().map(|c| num(c as i128)).collect()),
            "to_text" => {
                let ns: Vec<i128> = match vals.first() {
                    Some(Value::List(xs)) => xs.iter().map(|x| x.num().map(|r| r.num.div_euclid(r.den)).unwrap_or(-1)).collect(),
                    _ => Vec::new(),
                };
                text::of_code_points(&ns).map(Value::Str).unwrap_or(Value::None)
            }
            "slice" => Value::Str(text::slice(s(0), whole(1), whole(2))),
            "take" => Value::Str(text::take(s(0), whole(1))),
            "drop" => Value::Str(text::drop(s(0), whole(1))),
            "index_of" => text::index_of(s(0), s(1)).map(|i| num(i as i128)).unwrap_or(Value::None),
            "contains" => Value::Bool(s(0).contains(s(1))),
            "ends_with" => Value::Bool(s(0).ends_with(s(1))),
            "split" => strs(text::split(s(0), s(1))),
            "words" => strs(text::words(s(0))),
            "replace" => Value::Str(text::replace(s(0), s(1), s(2))),
            "trim" => Value::Str(text::trim(s(0))),
            "trim_start" => Value::Str(text::trim_start(s(0))),
            "trim_end" => Value::Str(text::trim_end(s(0))),
            "lower_ascii" => Value::Str(text::lower_ascii(s(0))),
            "upper_ascii" => Value::Str(text::upper_ascii(s(0))),
            "pad_start" | "pad_end" => {
                let width = whole(1);
                if width > text::LONGEST && !s(2).is_empty() && width > text::count(s(0)) as i128 {
                    let function = self.frames.last().map(|f| f.function.clone()).unwrap_or_default();
                    return Err(Stop::Diverged(Box::new(Diverged { function, module: m.module.name.text.clone(), line: e.span.line, depth: self.frames.len() })));
                }
                Value::Str(text::pad(s(0), width, s(2), name == "pad_end"))
            }
            "parse_number" => text::parse_number(s(0)).map(num).unwrap_or(Value::None),
            "parse_date" => text::parse_date(s(0)).map(Value::Date).unwrap_or(Value::None),
            _ => panic!("kaname: `{name}` is no builtin of a string"),
        })
    }

    /// The builtins of a dict, a set and a list that take no function: from the values of their
    /// arguments, the dict, the set or the list first.
    #[inline(never)]
    fn collection(&self, m: &Checked, e: &Expr, name: &str, xs: &[&Expr], vals: Vec<Value>) -> R<Value> {
        let me_c = self.ty(m, e);
        let me: &Ty = &me_c;
        let t0 = self.ty(m, xs[0]).into_owned();
        let mut it = vals.into_iter();
        let first = it.next().unwrap_or(Value::None);
        let rest: Vec<Value> = it.collect();
        // the i-th argument after the first, read in the type `to`
        let arg = |i: usize, to: &Ty| -> R<Value> { self.coerce_at(m, xs[i + 1], rest.get(i).cloned().unwrap_or(Value::None), &self.ty(m, xs[i + 1]), to) };
        let at = |i: usize| -> i128 {
            match rest.get(i) {
                Some(Value::Num(r)) => r.num.div_euclid(r.den),
                _ => 0,
            }
        };
        let same = |kt: &Ty, a: &Value, b: &Value| self.p.key_cmp(kt, a, b) == Ordering::Equal;
        match first {
            Value::Dict(ps) => {
                let (kt, vt) = dict_types(&t0);
                match name {
                    "get" => {
                        let k = arg(0, &kt)?;
                        Ok(ps.into_iter().find(|(x, _)| same(&kt, x, &k)).map(|(_, v)| v).unwrap_or(Value::None))
                    }
                    "has" => {
                        let k = arg(0, &kt)?;
                        Ok(Value::Bool(ps.iter().any(|(x, _)| same(&kt, x, &k))))
                    }
                    "put" => {
                        let (k, v) = (arg(0, &kt)?, arg(1, &vt)?);
                        Ok(Value::Dict(self.p.dict_of(&kt, ps.into_iter().chain([(k, v)]).collect())))
                    }
                    "remove" => {
                        let k = arg(0, &kt)?;
                        Ok(Value::Dict(ps.into_iter().filter(|(x, _)| !same(&kt, x, &k)).collect()))
                    }
                    "keys" => Ok(Value::List(ps.into_iter().map(|(k, _)| k).collect())),
                    "values" => Ok(Value::List(ps.into_iter().map(|(_, v)| v).collect())),
                    "entries" => Ok(Value::List(ps.into_iter().map(|(k, v)| entry(k, v)).collect())),
                    // the pairs of both, `o`'s value for a key of both
                    "merge" => {
                        let o = match arg(0, &t0)? {
                            Value::Dict(o) => o,
                            _ => Vec::new(),
                        };
                        Ok(Value::Dict(self.p.dict_of(&kt, ps.into_iter().chain(o).collect())))
                    }
                    _ => Ok(Value::Dict(ps)),
                }
            }
            Value::Set(ms) => {
                let et = match &t0 {
                    Ty::Set(x) => (**x).clone(),
                    _ => Ty::Error,
                };
                let other = |i: usize| -> R<Vec<Value>> {
                    Ok(match arg(i, &t0)? {
                        Value::Set(o) => o,
                        _ => Vec::new(),
                    })
                };
                match name {
                    "has" => {
                        let x = arg(0, &et)?;
                        Ok(Value::Bool(ms.iter().any(|y| same(&et, y, &x))))
                    }
                    "add" => {
                        let x = arg(0, &et)?;
                        Ok(Value::Set(self.p.set_of(&et, ms.into_iter().chain([x]).collect())))
                    }
                    "remove" => {
                        let x = arg(0, &et)?;
                        Ok(Value::Set(ms.into_iter().filter(|y| !same(&et, y, &x)).collect()))
                    }
                    "union" => Ok(Value::Set(self.p.set_of(&et, ms.into_iter().chain(other(0)?).collect()))),
                    "intersect" => {
                        let o = other(0)?;
                        Ok(Value::Set(ms.into_iter().filter(|y| o.iter().any(|z| same(&et, y, z))).collect()))
                    }
                    "difference" => {
                        let o = other(0)?;
                        Ok(Value::Set(ms.into_iter().filter(|y| !o.iter().any(|z| same(&et, y, z))).collect()))
                    }
                    "to_list" => Ok(Value::List(ms)),
                    _ => Ok(Value::Set(ms)),
                }
            }
            Value::List(xs_) => {
                let et = match me {
                    Ty::List(x) => (**x).clone(),
                    Ty::Opt(x) => (**x).clone(),
                    Ty::Set(x) => (**x).clone(),
                    _ => match &t0 {
                        Ty::List(x) => (**x).clone(),
                        _ => Ty::Error,
                    },
                };
                // a whole number between 0 and the count, where one past either end is cut to it
                let len = xs_.len() as i128;
                let cut = |i: i128| i.clamp(0, len) as usize;
                match name {
                    "first" => Ok(xs_.into_iter().next().unwrap_or(Value::None)),
                    "last" => Ok(xs_.into_iter().next_back().unwrap_or(Value::None)),
                    "append" => {
                        let x = arg(0, &et)?;
                        let mut out = xs_;
                        out.push(x);
                        Ok(Value::List(out))
                    }
                    "at" => {
                        let i = at(0);
                        Ok(if (0..len).contains(&i) { xs_[i as usize].clone() } else { Value::None })
                    }
                    "slice" => {
                        let (a, b) = (cut(at(0)), cut(at(1)));
                        Ok(Value::List(if a < b { xs_[a..b].to_vec() } else { Vec::new() }))
                    }
                    "take" => Ok(Value::List(xs_[..cut(at(0))].to_vec())),
                    "drop" => Ok(Value::List(xs_[cut(at(0))..].to_vec())),
                    "reverse" => Ok(Value::List(xs_.into_iter().rev().collect())),
                    "sort" => {
                        let mut out = xs_;
                        out.sort_by(|a, b| self.p.key_cmp(&et, a, b));
                        Ok(Value::List(out))
                    }
                    // the first of each key, in the order of the list
                    "distinct" => {
                        let mut out: Vec<Value> = Vec::new();
                        for x in xs_ {
                            if !out.iter().any(|y| same(&et, y, &x)) {
                                out.push(x);
                            }
                        }
                        Ok(Value::List(out))
                    }
                    "contains" => {
                        let x = arg(0, &et)?;
                        Ok(Value::Bool(xs_.contains(&x)))
                    }
                    "flatten" => Ok(Value::List(
                        xs_.into_iter()
                            .flat_map(|x| match x {
                                Value::List(ys) => ys,
                                _ => Vec::new(),
                            })
                            .collect(),
                    )),
                    "indexed" => Ok(Value::List(xs_.into_iter().enumerate().map(|(i, x)| Value::Record(vec![("index".into(), Value::Num(Rat::int(i as i128))), ("item".into(), x)])).collect())),
                    "to_set" => Ok(Value::Set(self.p.set_of(&et, xs_))),
                    "join" => {
                        let sep = match rest.first() {
                            Some(Value::Str(s)) => s.clone(),
                            _ => String::new(),
                        };
                        let parts: Vec<&str> = xs_
                            .iter()
                            .filter_map(|x| match x {
                                Value::Str(s) => Some(s.as_str()),
                                _ => None,
                            })
                            .collect();
                        Ok(Value::Str(parts.join(&sep)))
                    }
                    _ => Ok(Value::List(xs_)),
                }
            }
            v => Ok(v),
        }
    }
}

/// An entry of a dict: a record of its key and its value (DESIGN 24.3).
fn entry(k: Value, v: Value) -> Value {
    Value::Record(vec![("key".into(), k), ("value".into(), v)])
}

/// The type of a dict's keys and of its values.
fn dict_types(t: &Ty) -> (Ty, Ty) {
    match t.present() {
        Ty::Dict(k, v) => ((**k).clone(), (**v).clone()),
        _ => (Ty::Error, Ty::Error),
    }
}

/// A number that came in from a rule, held to the rule of int64.
fn num_fit(v: Value) -> Result<Value, Arith> {
    match v {
        Value::Num(r) => num::held(Some(r)).map(Value::Num),
        v => Ok(v),
    }
}

/// The value of a literal of an arm (a number, a minus before one, a date, a string), read in the
/// type the `match` splits: what the check found it is (its type's constant); None for anything else.
pub fn literal(m: &Checked, x: &Expr, to: &Ty) -> Option<Value> {
    match &x.kind {
        ExprKind::Num(_) | ExprKind::Neg(_) => {
            let n = ty_raw(m, x).num()?.clone();
            let k = n.konst?;
            convert(k, &n.unit, &unit_of(to)).ok().map(Value::Num)
        }
        ExprKind::Date(d) => Some(Value::Date(*d)),
        ExprKind::Str(s) => Some(Value::Str(s.clone())),
        _ => None,
    }
}

/// The unit of a numeric type; a number for anything else.
pub(crate) fn unit_of(t: &Ty) -> ritsu_units::Unit {
    match t.present() {
        Ty::Num(n) => n.unit.clone(),
        _ => ritsu_units::Unit::number(),
    }
}

pub(crate) fn has_unit(t: &Ty) -> bool {
    t.present().num().is_some_and(NumTy::has_unit)
}

/// A value on rulec's wire, as JSON.
pub fn wire_json(w: &Wire) -> Json {
    match w {
        Wire::Bool(b) => Json::Bool(*b),
        Wire::Int(i) => Json::Int(*i),
        Wire::Str(s) | Wire::Date(s) | Wire::Enum(s) => Json::str(s.clone()),
        Wire::List(xs) => Json::arr(xs.iter().map(|el| Json::obj(el.iter().map(|(n, v)| (n.clone(), wire_json(v)))))),
        Wire::None => Json::Null,
    }
}

/// The function of a module a word names, by its name or its alias: its index.
pub fn function(m: &Checked, word: &str) -> Option<usize> {
    m.decls.fns.iter().position(|f| f.name.is(word))
}
