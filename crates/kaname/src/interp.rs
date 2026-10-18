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
//!   what they declare (`range`), as an amount of a transfer is to 0 and above. A rate with a step and
//!   no range written, where a parameter reaches it, is held to 0% to 100%, as rulec holds such an
//!   input (DESIGN 3.9).
//! - A failure stops the evaluation there: its kind, the module and the line, what it says, and the
//!   calls it is in. A call deeper than [`DEPTH`] stops it too: the function did not stop, which is no
//!   failure but what `diverges` allows (DESIGN 8).

use crate::ast::*;
use crate::check::Checked;
use crate::num::{self, Arith, Rat, Rounding};
use crate::suite::Suite;
use crate::ty::*;
use crate::types::Ref;
use crate::value::{Program, TransferValue, Value, num_text, range_text, wire_int};
use ritsu_base::json::Json;
use ritsu_base::text::Text;
use ritsu_ports::{DateValue, RuleError, Value as Wire, Values};
use std::cmp::Ordering;

/// How deep the calls of kaname's functions go before the evaluation stops (DESIGN 8): deep enough for
/// the recursion a module of glue code writes, and shallow enough that the generated Python, which
/// CPython stops at a thousand frames, answers every input the reference interpreter answers (stage
/// D measures it).
pub const DEPTH: usize = 200;

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
}

/// A place a value does not keep: the place (a parameter, or a field or an element of what it
/// holds), the value as a person reads it, and the range; or the count of a list with its count.
struct Outside {
    path: String,
    shown: String,
    range: String,
    count: Option<usize>,
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

fn ty<'m>(m: &'m Checked, e: &Expr) -> &'m Ty {
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
        (v, _, _) => Ok(v),
    }
}

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
/// constant (a constant takes the unit of what it meets), else the first.
pub(crate) fn common<'t>(a: &'t Ty, b: &'t Ty) -> &'t Ty {
    match (a, b) {
        (Ty::Num(x), Ty::Num(y)) if x.konst.is_some() && y.konst.is_none() => b,
        _ => a,
    }
}

impl<'p> Interp<'p> {
    pub fn new(p: &'p Program, suite: &'p Suite) -> Interp<'p> {
        Interp { p, suite, limit: DEPTH, frames: Vec::new(), trace: Trace::default(), lets: false, watch: None, given: None }
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
        self.coerce_at(m, x, v, ty(m, x), ty(m, e))
    }

    // ── Places ──────────────────────────────────────────────────────────────────

    /// Where a value does not keep what its place declares, `what` naming the place: the range of
    /// its numbers and dates, the count of a list; with `reached`, also of every field of a module's
    /// record it reaches, a rate with a step and no range held to 0% to 100% (DESIGN 3.9). The
    /// fields of a list a rule walks are the rule's to hold.
    fn outside(&self, v: &Value, slot: &Slot, what: &str, reached: bool) -> Option<Outside> {
        if matches!(v, Value::None) {
            return None;
        }
        let inner_ty = |t: &Ty| -> Ty {
            let mut t = t.clone();
            while let Ty::Opt(x) = t {
                t = *x;
            }
            t
        };
        let t = inner_ty(&slot.ty);
        let (elems, elem_ty): (Vec<(String, &Value)>, Ty) = match (&t, v) {
            (Ty::List(x), Value::List(xs)) => {
                if let Some(c) = &slot.count {
                    let n = Rat::int(xs.len() as i128);
                    if c.min.is_some_and(|lo| num::cmp(n, lo) == Ordering::Less) || c.max.is_some_and(|hi| num::cmp(n, hi) == Ordering::Greater) {
                        return Some(Outside { path: what.to_string(), shown: String::new(), range: range_text(c, &Ty::Num(NumTy::number())), count: Some(xs.len()) });
                    }
                }
                (xs.iter().enumerate().map(|(i, x)| (format!("{what}[{i}]"), x)).collect(), inner_ty(x))
            }
            _ => (vec![(what.to_string(), v)], t.clone()),
        };
        let range = match (&slot.range, &elem_ty) {
            (Some(r), _) => Some(r.clone()),
            (None, Ty::Num(n)) if reached && n.is_rate() && n.step.is_some() => Some(Range { min: Some(Rat::zero()), max: Some(Rat::int(1)) }),
            _ => None,
        };
        for (path, x) in elems {
            if matches!(x, Value::None) {
                continue;
            }
            if let Some(r) = &range {
                let at = match x {
                    Value::Num(n) => Some(*n),
                    Value::Date(d) => Some(Rat::int(*d as i128)),
                    _ => None,
                };
                if let Some(at) = at
                    && (r.min.is_some_and(|lo| num::cmp(at, lo) == Ordering::Less) || r.max.is_some_and(|hi| num::cmp(at, hi) == Ordering::Greater))
                {
                    let shown = match (x, &elem_ty) {
                        (Value::Num(n), Ty::Num(nt)) => num_text(*n, &nt.unit),
                        (Value::Date(d), _) => crate::date::text(*d),
                        _ => String::new(),
                    };
                    return Some(Outside { path, shown, range: range_text(r, &elem_ty), count: None });
                }
            }
            if reached
                && let (Ty::Record(id @ RecId::Module { .. }), Value::Record(fs)) = (&elem_ty, x)
            {
                for (f, fslot) in self.p.fields(id) {
                    if let Some((_, fv)) = fs.iter().find(|(n, _)| *n == f.text)
                        && let Some(o) = self.outside(fv, &fslot, &format!("{path}.{}", f.text), true)
                    {
                        return Some(o);
                    }
                }
            }
        }
        None
    }

    /// Whether a value keeps what a place declares, as the interpreter holds it there: `reached` for a
    /// parameter (the fields of the records it reaches too, a rate with no range 0% to 100%).
    pub fn keeps(&self, v: &Value, slot: &Slot, reached: bool) -> bool {
        self.outside(v, slot, "", reached).is_none()
    }

    /// The arguments a host gives a function, held to its parameters' types, ranges and counts and to
    /// those of every field they reach (DESIGN 4.4): the failure `input` when one is outside them.
    pub fn check_entry(&self, m: &Checked, f: usize, args: &[Value]) -> Result<(), Box<Failure>> {
        let info = &m.decls.fns[f];
        for (p, v) in info.params.iter().zip(args) {
            if let Some(o) = self.outside(v, &p.slot, &p.name.text, true) {
                let (fun, path, shown, range) = (&info.name.text, &o.path, &o.shown, &o.range);
                let msg = match o.count {
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
        self.coerce_at(m, e, v, ty(m, e), to)
    }

    // ── Calls of kaname's functions ─────────────────────────────────────────────

    /// The function `f` of module `m` with these arguments, in its parameters' order and types, called
    /// from line `at` (0 for a host): its answer, in its result's type.
    pub fn call(&mut self, m: &'p Checked, f: usize, args: Vec<Value>, at: usize) -> R<Value> {
        if self.frames.len() >= self.limit {
            return Err(self.too_deep(m, f, at));
        }
        let mut env = self.enter(m, f, args, at);
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
    fn enter(&mut self, m: &Checked, f: usize, args: Vec<Value>, at: usize) -> Env {
        let info = &m.decls.fns[f];
        self.frames.push(Frame { function: info.name.text.clone(), line: at });
        Env { vars: info.params.iter().map(|p| p.name.text.clone()).zip(args).collect() }
    }

    /// A function's answer read in its result's type, and held to its result's range.
    #[inline(never)]
    fn finish(&self, m: &Checked, f: usize, out: R<Value>) -> R<Value> {
        let (decl, info) = (&m.module.fns[f], &m.decls.fns[f]);
        let v = self.coerce_at(m, &decl.body.answer, out?, ty(m, &decl.body.answer), &info.ret.ty)?;
        match self.outside(&v, &info.ret, &info.name.text, false) {
            Some(o) => {
                let (fun, shown, range) = (&info.name.text, &o.shown, &o.range);
                let msg = match o.count {
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
        let (callee, fi, vals) = self.place_args(m, e, canon, name, args, vals)?;
        self.call(callee, fi, vals, e.span.line)
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
    fn place_args(&self, m: &'p Checked, e: &Expr, canon: &std::path::Path, name: &str, args: &[Arg], vals: Vec<Value>) -> R<(&'p Checked, usize, Vec<Value>)> {
        let callee: &'p Checked = self.p.module(canon);
        let Some(fi) = callee.decls.fns.iter().position(|f| f.name.text == name) else {
            return Err(self.fail(m, e.span.line, Kind::Range, tr!("関数 `{name}` がありません", "There is no function `{name}`")));
        };
        let info = &callee.decls.fns[fi];
        let mut given: Vec<Value> = vec![Value::None; info.params.len()];
        for (a, v) in args.iter().zip(vals) {
            let Arg::Named(n, _, x) = a else { continue };
            if let Some(pi) = info.params.iter().position(|p| p.name.is(n)) {
                given[pi] = self.coerce_at(m, x, v, ty(m, x), &info.params[pi].slot.ty)?;
            }
        }
        for (p, v) in info.params.iter().zip(&given) {
            if let Some(o) = self.outside(v, &p.slot, &p.name.text, true) {
                let (fun, path, shown, range) = (&info.name.text, &o.path, &o.shown, &o.range);
                let msg = match o.count {
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
        let bound = m.typed.bound.get(&l.value.id).cloned().unwrap_or_else(|| ty(m, &l.value).clone());
        let v = self.coerce_at(m, &l.value, v, ty(m, &l.value), &bound)?;
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
            ExprKind::Record(_, inits) => self.record(m, env, e, inits),
            ExprKind::List(xs) => self.list_expr(m, env, e, xs),
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
            (ExprKind::In(..), v) => self.coerce_at(m, e, v, ty(m, x), ty(m, e)),
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

    /// An element of a list written out, read in the type of the list's elements.
    #[inline(never)]
    fn element(&self, m: &Checked, e: &Expr, x: &Expr, v: Value) -> R<Value> {
        match ty(m, e) {
            Ty::List(t) => self.coerce_at(m, x, v, ty(m, x), t),
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
        let (ta, tb) = (ty(m, a), ty(m, b));
        if op.is_comparison() {
            let order = match (&x, &y) {
                (Value::Num(_), Value::Num(_)) => {
                    let t = common(ta, tb);
                    let (p, q) = (self.coerce_at(m, a, x, ta, t)?, self.coerce_at(m, b, y, tb, t)?);
                    num::cmp(p.num().unwrap_or(Rat::zero()), q.num().unwrap_or(Rat::zero()))
                }
                (Value::Date(p), Value::Date(q)) => p.cmp(q),
                (Value::Enum(p), Value::Enum(q)) if !matches!(op, BinOp::Eq | BinOp::Ne) => self.rank(ta, p).cmp(&self.rank(ta, q)),
                _ => {
                    let same = x == y;
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
                        let t = ty(m, e);
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
            env.vars.push((x.to_string(), v));
        }
        let out = self.branch(m, env, e, &arms[k].body);
        env.vars.truncate(mark);
        out
    }

    /// The arm of a `match` that takes the value, and the name `some` gives it there.
    #[inline(never)]
    fn pick<'a>(&self, m: &Checked, e: &Expr, s: &Expr, arms: &'a [Arm], v: &Value) -> R<(usize, Option<&'a str>)> {
        let st = ty(m, s);
        for (k, arm) in arms.iter().enumerate() {
            for p in &arm.patterns {
                let hit = match (p, v) {
                    (Pattern::None(_), Value::None) => true,
                    (Pattern::Some(x, _), w) if !matches!(w, Value::None) => return Ok((k, Some(x.as_str()))),
                    (Pattern::Bool(b, _), Value::Bool(x)) => b == x,
                    (Pattern::Value(path, _), Value::Enum(x)) => {
                        let word = &path[path.len() - 1].0;
                        match st.present() {
                            Ty::Enum(id) => self.p.values(id).0.iter().any(|n| n.is(word) && n.text == *x),
                            _ => false,
                        }
                    }
                    _ => false,
                };
                if hit {
                    return Ok((k, None));
                }
            }
        }
        Err(self.fail(m, e.span.line, Kind::Range, tr!("`match` のどの分岐も値を受けません", "No arm of the `match` takes the value")))
    }

    fn record(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr, inits: &'p [FieldInit]) -> R<Value> {
        let mut vals = Vec::with_capacity(inits.len());
        for fi in inits {
            vals.push(self.eval(m, env, &fi.value)?);
        }
        self.make_record(m, e, inits, vals)
    }

    /// A record built from the values of its fields as written: in its fields' order and types, a
    /// field of a module's record held to what it declares (a list a rule walks is held to the
    /// rule's ranges by the rule), a field left out `none`.
    #[inline(never)]
    fn make_record(&self, m: &Checked, e: &Expr, inits: &[FieldInit], vals: Vec<Value>) -> R<Value> {
        let Ref::Record(id) = rf(m, e) else { return Ok(Value::None) };
        let fields = self.p.fields(id);
        let mut given: Vec<Value> = vec![Value::None; fields.len()];
        let mut lines: Vec<usize> = vec![e.span.line; fields.len()];
        for (fi, v) in inits.iter().zip(vals) {
            let Some(k) = fields.iter().position(|(n, _)| n.is(&fi.name)) else { continue };
            given[k] = self.coerce_at(m, &fi.value, v, ty(m, &fi.value), &fields[k].1.ty)?;
            lines[k] = fi.span.line;
        }
        // the fields are held to what they declare once every one is worked out, in their order
        if matches!(id, RecId::Module { .. }) {
            for (k, (f, slot)) in fields.iter().enumerate() {
                if let Some(o) = self.outside(&given[k], slot, &f.text, false) {
                    let (rec, path, shown, range) = (self.p.show_ty(m, &Ty::Record(id.clone())), &o.path, &o.shown, &o.range);
                    let msg = match o.count {
                        Some(n) => tr!("{rec} のフィールド `{path}` に置く要素は {n} 個で、要素の数の範囲 {range} の外です", "The field `{path}` of {rec} is given {n} elements, outside its count {range}"),
                        None => tr!("{rec} のフィールド `{path}` に置く {shown} は、範囲 {range} の外です", "The field `{path}` of {rec} is given {shown}, outside its range {range}"),
                    };
                    return Err(self.fail(m, lines[k], Kind::Range, msg));
                }
            }
        }
        Ok(Value::Record(fields.iter().zip(given).map(|((n, _), v)| (n.text.clone(), v)).collect()))
    }

    // ── Calls ───────────────────────────────────────────────────────────────────

    fn call_expr(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr, callee: &'p Expr, args: &'p [Arg]) -> R<Value> {
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
            Ref::Builtin(name) => self.builtin(m, env, e, name, args),
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
                let v = self.coerce_at(m, x, v, ty(m, x), &places[k].1)?;
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
            let v = self.coerce_at(m, x, v, ty(m, x), &place)?;
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
            let v = self.coerce_at(m, x, v, ty(m, x), &params[k].1)?;
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

    // ── Builtins ────────────────────────────────────────────────────────────────

    /// A function given to a builtin, applied: its answer, in the type of its body.
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

    /// A function given to a builtin, applied to each element of the list in turn: what it answers
    /// for each, read in the type `to`, until `stop` says the answers so far decide the builtin.
    fn each(&mut self, m: &'p Checked, env: &mut Env, list: &'p Expr, lam: &'p Expr, to: Option<&Ty>, stop: fn(&Value) -> bool) -> R<(Vec<Value>, Vec<Value>)> {
        let xs = self.list(m, env, list)?;
        let mut out = Vec::with_capacity(xs.len());
        for x in &xs {
            let v = self.apply(m, env, lam, vec![x.clone()])?;
            let v = match to {
                Some(t) => self.coerce_at(m, lam, v, ty(m, lam), t)?,
                None => v,
            };
            let done = stop(&v);
            out.push(v);
            if done {
                break;
            }
        }
        Ok((xs, out))
    }

    fn builtin(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr, name: &str, args: &'p [Arg]) -> R<Value> {
        match (name, args) {
            ("reduce", [list, first, f]) => self.reduce(m, env, e, list.expr(), first.expr(), f.expr()),
            ("count" | "sum" | "any" | "all" | "map" | "filter" | "first" | "max_of" | "min_of" | "running_totals", [list, f]) => self.walk(m, env, e, name, list.expr(), f.expr()),
            _ => {
                let vals = self.args(m, env, args)?;
                self.scalar(m, e, name, args, vals)
            }
        }
    }

    /// A builtin that applies a function to each element of a list.
    fn walk(&mut self, m: &'p Checked, env: &mut Env, e: &'p Expr, name: &str, list: &'p Expr, lam: &'p Expr) -> R<Value> {
        let to = self.lambda_place(m, e, name);
        let stop: fn(&Value) -> bool = match name {
            "any" | "first" => |v| matches!(v, Value::Bool(true)),
            "all" => |v| matches!(v, Value::Bool(false)),
            _ => |_| false,
        };
        let (xs, answers) = self.each(m, env, list, lam, to.as_ref(), stop)?;
        self.fold(m, e, name, xs, answers)
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
        let me = ty(m, e);
        match name {
            "sum" => Some(me.clone()),
            "max_of" | "min_of" => Some(me.present().clone()),
            "map" => match me {
                Ty::List(t) => Some((**t).clone()),
                _ => None,
            },
            "running_totals" => match me {
                Ty::List(t) => match &**t {
                    Ty::Record(RecId::Running(_, w)) => Some((**w).clone()),
                    _ => None,
                },
                _ => None,
            },
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
            "first" => Ok(list.into_iter().zip(answers).find(|(_, a)| yes(a)).map(|(x, _)| x).unwrap_or(Value::None)),
            "sum" => {
                let mut acc = Rat::zero();
                for v in &answers {
                    acc = num::add(acc, v.num().unwrap_or(Rat::zero())).map_err(|a| self.arith(m, e, a))?;
                }
                Ok(Value::Num(acc))
            }
            "max_of" | "min_of" => {
                let t = ty(m, e).present().clone();
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
            _ => Ok(Value::None),
        }
    }

    /// A builtin that takes no function, from the values of its arguments.
    #[inline(never)]
    fn scalar(&self, m: &Checked, e: &Expr, name: &str, args: &[Arg], vals: Vec<Value>) -> R<Value> {
        let me = ty(m, e);
        let xs: Vec<&Expr> = args.iter().map(Arg::expr).collect();
        // the numbers `mod`, `isqrt`, `allocate` and the roundings take: a value that is no number is
        // one the check of types lets no module give them, and the evaluation stops there rather than
        // read it as 0 (DESIGN 8; the model stops there too, DESIGN 15.3)
        let n = |i: usize| match vals.get(i).and_then(Value::num) {
            Some(r) => r,
            None => panic!("kaname: `{name}` is given {:?}, which is no number, and the check of types lets no module do so", vals.get(i)),
        };
        match name {
            "min" | "max" => {
                let mut it = vals.into_iter();
                let (a, b) = (it.next().unwrap_or(Value::None), it.next().unwrap_or(Value::None));
                let (a, b) = (self.coerce_at(m, xs[0], a, ty(m, xs[0]), me)?, self.coerce_at(m, xs[1], b, ty(m, xs[1]), me)?);
                let order = match (&a, &b) {
                    (Value::Num(p), Value::Num(q)) => num::cmp(*p, *q),
                    (Value::Date(p), Value::Date(q)) => p.cmp(q),
                    (Value::Enum(p), Value::Enum(q)) => self.rank(me, p).cmp(&self.rank(me, q)),
                    _ => Ordering::Equal,
                };
                let first = if name == "min" { order != Ordering::Greater } else { order != Ordering::Less };
                Ok(if first { a } else { b })
            }
            "mod" => {
                let d = n(1);
                Ok(Value::Num(if d.is_int() && d.num > 0 { num::modulo(n(0), d.num) } else { Rat::zero() }))
            }
            "isqrt" => match num::isqrt(n(0)) {
                Some(v) => Ok(Value::Num(v)),
                None => {
                    let shown = n(0).to_string();
                    Err(self.fail(m, e.span.line, Kind::Range, tr!("`isqrt` に負の値 {shown} を渡しています", "`isqrt` is given {shown}, which is below 0")))
                }
            },
            "allocate" => {
                let (tc, ts) = (ty(m, xs[1]), ty(m, xs[2]));
                let both = common(tc, ts);
                let c = convert(n(1), &unit_of(tc), &unit_of(both)).map_err(|a| self.arith(m, xs[1], a))?;
                let s = convert(n(2), &unit_of(ts), &unit_of(both)).map_err(|a| self.arith(m, xs[2], a))?;
                let v = num::allocate(n(0), c, s).map_err(|a| self.arith(m, e, a))?;
                convert(v, &unit_of(ty(m, xs[0])), &unit_of(me)).map(Value::Num).map_err(|a| self.arith(m, e, a))
            }
            "up" | "down" | "half_up" | "half_down" | "half_even" => {
                let tx = ty(m, xs[0]);
                let g = convert(n(1), &unit_of(ty(m, xs[1])), &unit_of(tx)).map_err(|a| self.arith(m, xs[1], a))?;
                let way = Rounding::parse(name).unwrap_or(Rounding::Down);
                let v = num::round(n(0), way, g).map_err(|a| self.arith(m, e, a))?;
                convert(v, &unit_of(tx), &unit_of(me)).map(Value::Num).map_err(|a| self.arith(m, e, a))
            }
            "count" => Ok(Value::Num(Rat::int(match vals.first() {
                Some(Value::List(xs)) => xs.len() as i128,
                _ => 0,
            }))),
            "starts_with" => Ok(Value::Bool(matches!((vals.first(), vals.get(1)), (Some(Value::Str(s)), Some(Value::Str(p))) if s.as_bytes().starts_with(p.as_bytes())))),
            "concat" => Ok(Value::Str(
                vals.iter()
                    .filter_map(|v| match v {
                        Value::Str(s) => Some(s.as_str()),
                        _ => None,
                    })
                    .collect(),
            )),
            "text" => {
                let t = ty(m, xs[0]);
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
            _ => Ok(Value::None),
        }
    }
}

/// A number that came in from a rule, held to the rule of int64.
fn num_fit(v: Value) -> Result<Value, Arith> {
    match v {
        Value::Num(r) => num::held(Some(r)).map(Value::Num),
        v => Ok(v),
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
