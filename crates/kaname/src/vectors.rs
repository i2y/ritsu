//! The vectors of a function (DESIGN 9): inputs chosen from what its parameters declare and from what
//! the function does with them, each run in the reference interpreter. They are what the generated
//! code is held to in every language (DESIGN 16.4), where examples are looked for (DESIGN 5.7), and
//! what a person reads the answers in.
//!
//! The candidates of a parameter, and of each field and element it holds:
//!
//! - a number, an amount, a quantity, a rate: the ends of its range, 0 when the range holds it, each
//!   literal the function compares it with (in the functions it calls too, and in the functions it
//!   gives to builtins) and the steps on either side of it, and the ends of the range of each input of
//!   a rule or a date it is given to as it is, with the step outside each end;
//! - a date: the same, a day a step;
//! - an enum, a bool: every value; a value that may be none: `none`, and the candidates of the value;
//! - a string: `"a"`, each string it is compared with, each prefix `starts_with` asks of it;
//! - a list: as many elements as the lower end of its count, 0, 1, 2, 3 and the upper end, those its
//!   count holds, each element the base value; and the first element moved over its candidates;
//! - a record: one field at a time moved over its candidates, the others at their base values.
//!
//! The base value is the middle of the range for a number (on its step), the lower end for a date, the
//! first value of an enum, `true`, `"a"`, a list of the fewest elements but one at least, a record of
//! base fields. Every candidate is inside what its place declares: what is outside is the entry's
//! failure `input`, which tells nothing of the function.
//!
//! The inputs: every combination of the parameters' candidates when there are 256 or fewer; else
//! each parameter moved alone, the others at their base values, and every pair of candidates of two
//! parameters that meet in one comparison or one call. A function has 2,000 vectors at most. Nothing
//! is random, and the order is the same every run.

use crate::ast::*;
use crate::check::Checked;
use crate::interp::{Event, Interp, Stop};
use crate::names::{column_slot, date_input_slot};
use crate::num::{self, Rat};
use crate::suite::Suite;
use crate::ty::*;
use crate::types::Ref;
use crate::ranges::{Boundary, Lin};
use crate::value::{Program, TransferValue, Value, num_text};
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// The inputs of one function at most, beyond which the rest are left out.
pub const MOST: usize = 2_000;
/// The most combinations taken whole.
pub const WHOLE: usize = 256;

/// A step from a parameter to what it holds.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    Field(String),
    /// An element of a list: the first, where an input is built (DESIGN 9).
    Elem,
}

/// A parameter, by its place among the function's, and the steps to a field or an element of it.
pub type Path = (usize, Vec<Step>);

/// What the function does with a part of a parameter that says which values of it to try.
#[derive(Clone, Debug)]
enum Hint {
    /// A literal it is compared with, in the unit of the literal's type.
    Lit(Value, Ty),
    /// A prefix `starts_with` asks of it.
    Prefix(String),
    /// The range of an input of a rule or a date it is given to as it is, in the input's type, and
    /// the input as a person names it.
    Inner(Ty, Range, Text),
}

/// What a function does with its parameters, as far as the vectors read it.
#[derive(Clone, Default)]
struct Hints {
    by: BTreeMap<Path, Vec<Hint>>,
    /// The parameters that meet in one comparison or one call.
    pairs: BTreeSet<(usize, usize)>,
}

/// The walk of a function's body that gathers its [`Hints`].
struct Gather<'p> {
    p: &'p Program,
    /// The functions whose hints are being gathered, which a recursion does not enter again.
    busy: BTreeSet<(PathBuf, String)>,
}

/// What a name in a function holds: a part of a parameter, or nothing the vectors can try.
type Scope = Vec<(String, Option<Path>)>;

impl<'p> Gather<'p> {
    fn function(&mut self, m: &'p Checked, f: usize) -> Hints {
        let key = (m.canon.clone(), m.decls.fns[f].name.text.clone());
        if !self.busy.insert(key.clone()) {
            return Hints::default();
        }
        let mut h = Hints::default();
        let mut scope: Scope = m.decls.fns[f].params.iter().enumerate().map(|(i, p)| (p.name.text.clone(), Some((i, Vec::new())))).collect();
        self.block(m, &mut scope, &m.module.fns[f].body, &mut h);
        self.busy.remove(&key);
        h
    }

    fn block(&mut self, m: &'p Checked, scope: &mut Scope, b: &'p Block, h: &mut Hints) {
        let mark = scope.len();
        for l in &b.lets {
            self.expr(m, scope, &l.value, h);
            let at = path_of(m, scope, &l.value);
            scope.push((l.name.clone(), at));
        }
        self.expr(m, scope, &b.answer, h);
        scope.truncate(mark);
    }

    fn expr(&mut self, m: &'p Checked, scope: &mut Scope, e: &'p Expr, h: &mut Hints) {
        match &e.kind {
            ExprKind::Bin(a, op, b) => {
                if op.is_comparison() {
                    for (x, y) in [(a, b), (b, a)] {
                        if let (Some(at), Some(lit)) = (path_of(m, scope, x), literal(m, y)) {
                            h.by.entry(at).or_default().push(lit);
                        }
                    }
                    meet(m, scope, &[a, b], h);
                }
                self.expr(m, scope, a, h);
                self.expr(m, scope, b, h);
            }
            ExprKind::Call(callee, args) => {
                let exprs: Vec<&Expr> = args.iter().map(Arg::expr).collect();
                match m.typed.refs.get(callee.id) {
                    Some(Ref::Rule(canon)) => {
                        meet(m, scope, &exprs, h);
                        if let Some(facts) = self.p.rules.get(canon) {
                            for a in args {
                                let Arg::Named(n, _, x) = a else { continue };
                                let Some(c) = facts.inputs.iter().find(|c| c.name == *n || c.alias == *n) else { continue };
                                let slot = column_slot(c, canon);
                                if let (Some(at), Some(r)) = (path_of(m, scope, x), slot.range) {
                                    let rule = text_of(m, callee);
                                    let label = tr!("規則 {rule} の入力 {}", "the input {} of the rule {rule}", c.name; c.name);
                                    h.by.entry(at).or_default().push(Hint::Inner(slot.ty, r, label));
                                }
                            }
                        }
                    }
                    Some(Ref::Date(canon, _)) => {
                        meet(m, scope, &exprs, h);
                        if let Some(facts) = self.p.dates.get(canon) {
                            for a in args {
                                let Arg::Named(n, _, x) = a else { continue };
                                let Some(input) = facts.inputs.iter().find(|i| i.name == *n || i.alias == *n) else { continue };
                                let Some(slot) = date_input_slot(facts, &input.name) else { continue };
                                if let (Some(at), Some(r)) = (path_of(m, scope, x), slot.range) {
                                    let date = text_of(m, callee);
                                    let label = tr!("日付 {date} の入力 {}", "the input {} of the date {date}", input.name; input.name);
                                    h.by.entry(at).or_default().push(Hint::Inner(slot.ty, r, label));
                                }
                            }
                        }
                    }
                    Some(Ref::Fn(canon, name)) => {
                        meet(m, scope, &exprs, h);
                        let callee_m: &'p Checked = self.p.module(canon);
                        if let Some(g) = callee_m.decls.fns.iter().position(|f| f.name.text == *name) {
                            let inner = self.function(callee_m, g);
                            for a in args {
                                let Arg::Named(n, _, x) = a else { continue };
                                let (Some(at), Some(q)) = (path_of(m, scope, x), callee_m.decls.fns[g].params.iter().position(|p| p.name.is(n))) else { continue };
                                for ((qi, steps), hs) in &inner.by {
                                    if *qi == q {
                                        let mut to = at.1.clone();
                                        to.extend(steps.iter().cloned());
                                        h.by.entry((at.0, to)).or_default().extend(hs.iter().cloned());
                                    }
                                }
                            }
                        }
                    }
                    Some(Ref::Transfer(_)) => meet(m, scope, &exprs, h),
                    Some(Ref::Builtin(name)) => {
                        if *name == "starts_with"
                            && let [s, pre] = exprs.as_slice()
                            && let (Some(at), ExprKind::Str(text)) = (path_of(m, scope, s), &pre.kind)
                        {
                            h.by.entry(at).or_default().push(Hint::Prefix(text.clone()));
                        }
                        // a function given to a builtin takes the elements of the list it walks
                        let list = exprs.first().and_then(|l| path_of(m, scope, l)).map(|(i, mut steps)| {
                            steps.push(Step::Elem);
                            (i, steps)
                        });
                        for x in &exprs {
                            if let ExprKind::Lambda(ps, body) = &x.kind {
                                let mark = scope.len();
                                for (k, (p, _)) in ps.iter().enumerate() {
                                    // reduce's function takes the running value first, then an element
                                    let elem = if ps.len() == 2 { k == 1 } else { k == 0 };
                                    scope.push((p.clone(), if elem { list.clone() } else { None }));
                                }
                                self.expr(m, scope, body, h);
                                scope.truncate(mark);
                            } else {
                                self.expr(m, scope, x, h);
                            }
                        }
                        return;
                    }
                    _ => {}
                }
                for x in exprs {
                    self.expr(m, scope, x, h);
                }
            }
            ExprKind::Match(s, arms) => {
                self.expr(m, scope, s, h);
                let at = path_of(m, scope, s);
                for arm in arms {
                    let mark = scope.len();
                    for p in &arm.patterns {
                        if let Pattern::Some(x, _) = p {
                            scope.push((x.clone(), at.clone()));
                        }
                    }
                    self.block(m, scope, &arm.body, h);
                    scope.truncate(mark);
                }
            }
            ExprKind::If(c, t, f) => {
                self.expr(m, scope, c, h);
                self.block(m, scope, t, h);
                self.block(m, scope, f, h);
            }
            ExprKind::Block(b) => self.block(m, scope, b, h),
            ExprKind::Record(_, inits) => {
                for fi in inits {
                    self.expr(m, scope, &fi.value, h);
                }
            }
            ExprKind::List(xs) => {
                for x in xs {
                    self.expr(m, scope, x, h);
                }
            }
            ExprKind::Neg(x) | ExprKind::Not(x) | ExprKind::In(x, _, _) | ExprKind::Field(x, _, _) => self.expr(m, scope, x, h),
            _ => {}
        }
    }
}

/// The part of a parameter an expression is, as it is: a parameter, a field of one, a name that
/// holds one.
fn path_of(m: &Checked, scope: &Scope, e: &Expr) -> Option<Path> {
    match (&e.kind, m.typed.refs.get(e.id)) {
        (ExprKind::Var(w), Some(Ref::Local)) => scope.iter().rev().find(|(n, _)| n == w).and_then(|(_, p)| p.clone()),
        (ExprKind::Field(base, _, _), Some(Ref::Field(name))) => {
            let (i, mut steps) = path_of(m, scope, base)?;
            steps.push(Step::Field(name.clone()));
            Some((i, steps))
        }
        _ => None,
    }
}

/// A literal an expression is: a number with its type, a date, a string.
fn literal(m: &Checked, e: &Expr) -> Option<Hint> {
    let t = m.typed.types.get(e.id)?.clone();
    match &e.kind {
        ExprKind::Num(n) => Some(Hint::Lit(Value::Num(if n.unit == "%" { n.value.div(Rat::int(100)) } else { n.value }), t)),
        ExprKind::Neg(x) => match literal(m, x)? {
            Hint::Lit(Value::Num(r), _) => Some(Hint::Lit(Value::Num(Rat::zero().sub(r)), t)),
            _ => None,
        },
        ExprKind::Date(d) => Some(Hint::Lit(Value::Date(*d), Ty::Date)),
        ExprKind::Str(s) => Some(Hint::Lit(Value::Str(s.clone()), Ty::Str)),
        _ => None,
    }
}

/// The parameters the expressions hold parts of, as meeting in one comparison or one call.
fn meet(m: &Checked, scope: &Scope, es: &[&Expr], h: &mut Hints) {
    let mut found = BTreeSet::new();
    for e in es {
        let mut all = Vec::new();
        e.walk(&mut all);
        for x in all {
            if let Some((i, _)) = path_of(m, scope, x) {
                found.insert(i);
            }
        }
    }
    let found: Vec<usize> = found.into_iter().collect();
    for (k, a) in found.iter().enumerate() {
        for b in &found[k + 1..] {
            h.pairs.insert((*a, *b));
        }
    }
}

/// The source of an expression on its line.
fn text_of(m: &Checked, e: &Expr) -> String {
    let line = m.src.lines().nth(e.span.line.saturating_sub(1)).unwrap_or("");
    let cs: Vec<char> = line.chars().collect();
    let (a, b) = (e.span.col.saturating_sub(1).min(cs.len()), e.span.end.saturating_sub(1).min(cs.len()));
    cs[a..b.max(a)].iter().collect::<String>().trim().to_string()
}

/// One input of a function, and why it was chosen (None for the base values alone).
#[derive(Clone, Debug)]
pub struct Vector {
    pub input: Vec<Value>,
    pub why: Option<Text>,
}

/// A candidate of a place: the value, and why it is tried (None for the base value).
type Cand = (Value, Option<Text>);

/// The values a place is tried with, and its base value.
struct Choose<'p> {
    p: &'p Program,
    hints: Hints,
}

impl Choose<'_> {
    fn hints(&self, at: &Path) -> &[Hint] {
        self.hints.by.get(at).map(Vec::as_slice).unwrap_or(&[])
    }

    /// The range a number or a date of the place holds: what it declares, a rate with a step 0% to
    /// 100% (DESIGN 3.9).
    fn range(slot: &Slot, t: &Ty) -> Option<Range> {
        match (&slot.range, t) {
            (Some(r), _) => Some(r.clone()),
            (None, Ty::Num(n)) if n.is_rate() && n.step.is_some() => Some(Range { min: Some(Rat::zero()), max: Some(Rat::int(1)) }),
            _ => None,
        }
    }

    /// The step a value of the type lands on: a rate's, one unit, one day.
    fn step(t: &Ty) -> Rat {
        match t {
            Ty::Num(n) if n.is_rate() => n.step.unwrap_or(Rat::new(1, 100)),
            _ => Rat::int(1),
        }
    }

    fn base(&self, slot: &Slot, t: &Ty) -> Value {
        match t {
            Ty::Opt(x) => self.base(&Slot { ty: (**x).clone(), ..slot.clone() }, x),
            Ty::Num(_) => {
                let s = Self::step(t);
                let v = match Self::range(slot, t) {
                    Some(Range { min: Some(lo), max: Some(hi) }) => {
                        let half = hi.sub(lo).div(Rat::int(2)).div(s);
                        lo.add(Rat::int(half.num.div_euclid(half.den)).mul(s))
                    }
                    Some(Range { min: Some(lo), max: None }) => lo,
                    Some(Range { min: None, max: Some(hi) }) => hi.min_with(Rat::zero()),
                    _ => Rat::zero(),
                };
                Value::Num(v)
            }
            Ty::Date => Value::Date(slot.range.as_ref().and_then(|r| r.min.or(r.max)).map(|d| d.num as i64).unwrap_or(0)),
            Ty::Bool => Value::Bool(true),
            Ty::Str => Value::Str("a".into()),
            Ty::Enum(id) => self.p.values(id).0.first().map(|v| Value::Enum(v.text.clone())).unwrap_or(Value::None),
            Ty::List(x) => {
                let n = Self::base_count(slot);
                let elem = Slot { ty: (**x).clone(), range: slot.range.clone(), count: None };
                Value::List((0..n).map(|_| self.base(&elem, x)).collect())
            }
            Ty::Record(id) => Value::Record(self.p.fields(id).iter().map(|(f, s)| (f.text.clone(), self.base(s, &s.ty))).collect()),
            Ty::Transfer(id) => {
                let params = self.p.transfer(id).map(|(_, ps)| ps).unwrap_or_default();
                Value::Transfer(Box::new(TransferValue { id: id.clone(), args: params.iter().map(|(n, t)| (n.clone(), self.base(&Slot::of(t.clone()), t))).collect() }))
            }
            _ => Value::None,
        }
    }

    /// The fewest elements but one at least, as the count holds them.
    fn base_count(slot: &Slot) -> usize {
        let lo = slot.count.as_ref().and_then(|c| c.min).map(|v| v.num.max(0) as usize).unwrap_or(0);
        let hi = slot.count.as_ref().and_then(|c| c.max).map(|v| v.num.max(0) as usize).unwrap_or(usize::MAX);
        lo.max(1).min(hi)
    }

    /// The candidates of a place at `at`, which a person names `shown`.
    fn cands(&self, slot: &Slot, t: &Ty, at: &Path, shown: &str) -> Vec<Cand> {
        let said = |why: Text| -> Option<Text> { Some(tr!("{shown}: {}", "{shown}: {}", why.ja; why.en)) };
        match t {
            Ty::Opt(x) => {
                let mut out = vec![(Value::None, said(Text::same("none")))];
                out.extend(self.cands(&Slot { ty: (**x).clone(), ..slot.clone() }, x, at, shown));
                out
            }
            Ty::Num(n) => {
                let range = Self::range(slot, t);
                let s = Self::step(t);
                let inside = |v: Rat| range.as_ref().is_none_or(|r| r.min.is_none_or(|lo| num::cmp(v, lo).is_ge()) && r.max.is_none_or(|hi| num::cmp(v, hi).is_le())) && num::fits(v);
                let on_step = |v: Rat| v.on_grid(s);
                let mut out: Vec<Cand> = Vec::new();
                let mut push = |v: Rat, why: Text| {
                    if inside(v) && on_step(v) && !out.iter().any(|(x, _)| *x == Value::Num(v)) {
                        out.push((Value::Num(v), said(why)));
                    }
                };
                if let Some(r) = &range {
                    if let Some(lo) = r.min {
                        push(lo, tr!("範囲の下の端", "the lower end"));
                    }
                    if let Some(hi) = r.max {
                        push(hi, tr!("範囲の上の端", "the upper end"));
                    }
                }
                push(Rat::zero(), tr!("0", "zero"));
                for h in self.hints(at) {
                    match h {
                        Hint::Lit(Value::Num(v), Ty::Num(lt)) => {
                            let Some(x) = convert_num(*v, &lt.unit, &n.unit) else { continue };
                            let lit = num_text(*v, &lt.unit);
                            push(x.sub(s), tr!("{lit} の一つ下", "a step below {lit}"));
                            push(x, tr!("{lit} と比べる", "compared with {lit}"));
                            push(x.add(s), tr!("{lit} の一つ上", "a step above {lit}"));
                        }
                        Hint::Inner(Ty::Num(it), r, label) => {
                            if let Some(lo) = r.min.and_then(|v| convert_num(v, &it.unit, &n.unit)) {
                                push(lo, tr!("{} の範囲の下の端", "the lower end of {}", label.ja; label.en));
                                push(lo.sub(s), tr!("{} の範囲の下の端の一つ外", "a step below the lower end of {}", label.ja; label.en));
                            }
                            if let Some(hi) = r.max.and_then(|v| convert_num(v, &it.unit, &n.unit)) {
                                push(hi, tr!("{} の範囲の上の端", "the upper end of {}", label.ja; label.en));
                                push(hi.add(s), tr!("{} の範囲の上の端の一つ外", "a step above the upper end of {}", label.ja; label.en));
                            }
                        }
                        _ => {}
                    }
                }
                out
            }
            Ty::Date => {
                let range = slot.range.clone();
                let inside = |d: i64| (crate::date::MIN..=crate::date::MAX).contains(&d) && range.as_ref().is_none_or(|r| r.min.is_none_or(|lo| d as i128 >= lo.num) && r.max.is_none_or(|hi| d as i128 <= hi.num));
                let mut out: Vec<Cand> = Vec::new();
                let mut push = |d: i64, why: Text| {
                    if inside(d) && !out.iter().any(|(x, _)| *x == Value::Date(d)) {
                        out.push((Value::Date(d), said(why)));
                    }
                };
                if let Some(r) = &range {
                    if let Some(lo) = r.min {
                        push(lo.num as i64, tr!("範囲の下の端", "the lower end"));
                    }
                    if let Some(hi) = r.max {
                        push(hi.num as i64, tr!("範囲の上の端", "the upper end"));
                    }
                }
                for h in self.hints(at) {
                    match h {
                        Hint::Lit(Value::Date(d), _) => {
                            let lit = crate::date::text(*d);
                            push(d - 1, tr!("{lit} の前の日", "the day before {lit}"));
                            push(*d, tr!("{lit} と比べる", "compared with {lit}"));
                            push(d + 1, tr!("{lit} の次の日", "the day after {lit}"));
                        }
                        Hint::Inner(_, r, label) => {
                            if let Some(lo) = r.min {
                                push(lo.num as i64, tr!("{} の範囲の下の端", "the lower end of {}", label.ja; label.en));
                                push(lo.num as i64 - 1, tr!("{} の範囲の下の端の一つ外", "a day before the lower end of {}", label.ja; label.en));
                            }
                            if let Some(hi) = r.max {
                                push(hi.num as i64, tr!("{} の範囲の上の端", "the upper end of {}", label.ja; label.en));
                                push(hi.num as i64 + 1, tr!("{} の範囲の上の端の一つ外", "a day after the upper end of {}", label.ja; label.en));
                            }
                        }
                        _ => {}
                    }
                }
                out
            }
            Ty::Bool => vec![(Value::Bool(true), said(Text::same("true"))), (Value::Bool(false), said(Text::same("false")))],
            Ty::Enum(id) => self.p.values(id).0.iter().map(|v| (Value::Enum(v.text.clone()), said(Text::same(v.text.clone())))).collect(),
            Ty::Str => {
                let mut out: Vec<Cand> = vec![(Value::Str("a".into()), None)];
                let mut push = |s: String, why: Text| {
                    if !out.iter().any(|(x, _)| *x == Value::Str(s.clone())) {
                        out.push((Value::Str(s), said(why)));
                    }
                };
                for h in self.hints(at) {
                    match h {
                        Hint::Lit(Value::Str(s), _) => push(s.clone(), tr!("\"{s}\" と比べる", "compared with \"{s}\"")),
                        Hint::Prefix(pre) => {
                            push(pre.clone(), tr!("\"{pre}\" で始まる", "starts with \"{pre}\""));
                            if "a".starts_with(pre.as_str()) {
                                push("b".into(), tr!("\"{pre}\" で始まらない", "does not start with \"{pre}\""));
                            }
                        }
                        _ => {}
                    }
                }
                out
            }
            Ty::List(x) => {
                let elem = Slot { ty: (**x).clone(), range: slot.range.clone(), count: None };
                let base_elem = self.base(&elem, x);
                let (lo, hi) = (
                    slot.count.as_ref().and_then(|c| c.min).map(|v| v.num.max(0)).unwrap_or(0),
                    slot.count.as_ref().and_then(|c| c.max).map(|v| v.num.max(0)).unwrap_or(i128::MAX),
                );
                let mut out: Vec<Cand> = Vec::new();
                for n in [lo, 0, 1, 2, 3, hi] {
                    if n < lo || n > hi || n > MOST as i128 || out.iter().any(|(v, _)| matches!(v, Value::List(xs) if xs.len() as i128 == n)) {
                        continue;
                    }
                    let why = if n as usize == Self::base_count(slot) { None } else { said(tr!("要素 {n} 個", "{n} elements")) };
                    out.push((Value::List(vec![base_elem.clone(); n as usize]), why));
                }
                // the first element moved over its candidates, among as many as the base list holds
                let n = Self::base_count(slot);
                if n >= 1 {
                    let mut steps = at.1.clone();
                    steps.push(Step::Elem);
                    for (c, why) in self.cands(&elem, x, &(at.0, steps), &format!("{shown}[0]")) {
                        if c == base_elem {
                            continue;
                        }
                        let mut xs = vec![base_elem.clone(); n];
                        xs[0] = c;
                        let v = Value::List(xs);
                        if !out.iter().any(|(x, _)| *x == v) {
                            out.push((v, why));
                        }
                    }
                }
                out
            }
            Ty::Record(id) => {
                let base = self.base(slot, t);
                let mut out: Vec<Cand> = vec![(base.clone(), None)];
                let Value::Record(fields) = &base else { return out };
                for (k, (f, fslot)) in self.p.fields(id).iter().enumerate() {
                    let mut steps = at.1.clone();
                    steps.push(Step::Field(f.text.clone()));
                    for (c, why) in self.cands(fslot, &fslot.ty, &(at.0, steps), &format!("{shown}.{}", f.text)) {
                        if c == fields[k].1 {
                            continue;
                        }
                        let mut fs = fields.clone();
                        fs[k].1 = c;
                        let v = Value::Record(fs);
                        if !out.iter().any(|(x, _)| *x == v) {
                            out.push((v, why));
                        }
                    }
                }
                out
            }
            _ => vec![(self.base(slot, t), None)],
        }
    }
}

trait MinWith {
    fn min_with(self, o: Rat) -> Rat;
}

impl MinWith for Rat {
    fn min_with(self, o: Rat) -> Rat {
        if num::cmp(self, o).is_le() { self } else { o }
    }
}

/// A number counted in `from`, counted in `to`; a number and a rate count alike.
fn convert_num(v: Rat, from: &ritsu_units::Unit, to: &ritsu_units::Unit) -> Option<Rat> {
    use ritsu_units::Dim;
    if v.num == 0 || (matches!(from.dim, Dim::Number | Dim::Rate) && matches!(to.dim, Dim::Number | Dim::Rate)) {
        return Some(v);
    }
    factor(from, to).map(|f| v.mul(f))
}

/// The vectors of the function `f` of module `m`, and whether some were left out (more than
/// [`MOST`]): with the boundaries the analysis of ranges found in it, when it has run.
pub fn choose(p: &Program, m: &Checked, f: usize) -> (Vec<Vector>, bool) {
    let bounds = m.analysis.get().and_then(|a| a.fns.get(f)).map(|x| x.boundaries.clone()).unwrap_or_default();
    choose_with(p, m, f, &bounds)
}

/// The vectors of `f`, with the inputs that reach `bounds` (DESIGN 9): every combination of the
/// candidates or each parameter moved alone, then the boundaries, then the pairs.
pub fn choose_with(p: &Program, m: &Checked, f: usize, bounds: &[Boundary]) -> (Vec<Vector>, bool) {
    let hints = Gather { p, busy: BTreeSet::new() }.function(m, f);
    let pairs = hints.pairs.clone();
    let c = Choose { p, hints };
    let info = &m.decls.fns[f];
    let cands: Vec<Vec<Cand>> = info
        .params
        .iter()
        .enumerate()
        .map(|(i, prm)| {
            let cs = c.cands(&prm.slot, &prm.slot.ty, &(i, Vec::new()), &prm.name.text);
            if cs.is_empty() { vec![(c.base(&prm.slot, &prm.slot.ty), None)] } else { cs }
        })
        .collect();
    let base: Vec<Value> = info.params.iter().map(|prm| c.base(&prm.slot, &prm.slot.ty)).collect();
    let mut out: Vec<Vector> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut cut = false;
    let mut add = |input: Vec<Value>, whys: Vec<Option<Text>>, out: &mut Vec<Vector>| {
        let key = format!("{input:?}");
        if !seen.insert(key) {
            return;
        }
        if out.len() >= MOST {
            cut = true;
            return;
        }
        let said: Vec<Text> = whys.into_iter().flatten().collect();
        let why = (!said.is_empty()).then(|| Text::join(&said, "、", "; "));
        out.push(Vector { input, why });
    };
    // the inputs that reach each boundary of a comparison of sums of parameters, a step below it,
    // on it and a step above it, each parameter moved alone from the base values
    let mut reaching: Vec<(Vec<Value>, Vec<Option<Text>>)> = Vec::new();
    for b in bounds {
        for (target, at) in [(b.step.mul(Rat::int(-1)), Where::Below), (Rat::zero(), Where::On), (b.step, Where::Above)] {
            for (input, atom) in reach(p, m, f, &base, &b.lin, target) {
                let shown = path_text(info, &atom);
                let text = &b.text;
                let why = match at {
                    Where::Below => tr!("{shown}: `{text}` の境目の一つ下", "{shown}: a step below the boundary of `{text}`"),
                    Where::On => tr!("{shown}: `{text}` の境目", "{shown}: on the boundary of `{text}`"),
                    Where::Above => tr!("{shown}: `{text}` の境目の一つ上", "{shown}: a step above the boundary of `{text}`"),
                };
                let mut whys = vec![None; base.len()];
                whys[atom.0] = Some(why);
                reaching.push((input, whys));
            }
        }
    }
    let whole = cands.iter().try_fold(1usize, |acc, cs| acc.checked_mul(cs.len())).filter(|n| *n <= WHOLE);
    if let Some(total) = whole {
        // every combination, the last parameter moving fastest
        for n in 0..total {
            let mut k = n;
            let mut idx = vec![0usize; cands.len()];
            for i in (0..cands.len()).rev() {
                idx[i] = k % cands[i].len();
                k /= cands[i].len();
            }
            let input = idx.iter().enumerate().map(|(i, k)| cands[i][*k].0.clone()).collect();
            let whys = idx.iter().enumerate().map(|(i, k)| cands[i][*k].1.clone()).collect();
            add(input, whys, &mut out);
        }
        for (input, whys) in reaching {
            add(input, whys, &mut out);
        }
    } else {
        add(base.clone(), vec![None; base.len()], &mut out);
        for (i, cs) in cands.iter().enumerate() {
            for (v, why) in cs {
                let mut input = base.clone();
                input[i] = v.clone();
                let mut whys = vec![None; base.len()];
                whys[i] = why.clone();
                add(input, whys, &mut out);
            }
        }
        for (input, whys) in reaching {
            add(input, whys, &mut out);
        }
        for (a, b) in &pairs {
            for (va, wa) in &cands[*a] {
                for (vb, wb) in &cands[*b] {
                    let mut input = base.clone();
                    input[*a] = va.clone();
                    input[*b] = vb.clone();
                    let mut whys = vec![None; base.len()];
                    whys[*a] = wa.clone();
                    whys[*b] = wb.clone();
                    add(input, whys, &mut out);
                }
            }
        }
    }
    (out, cut)
}

/// Where an input is put against a boundary.
enum Where {
    Below,
    On,
    Above,
}

/// The base values of the parameters of `f` (DESIGN 9): what an input moves one part of.
pub fn base_input(p: &Program, m: &Checked, f: usize) -> Vec<Value> {
    let c = Choose { p, hints: Hints::default() };
    m.decls.fns[f].params.iter().map(|prm| c.base(&prm.slot, &prm.slot.ty)).collect()
}

/// A part of a parameter as a person names it: `invoice.invoiced`, `lines[0].weight`.
pub fn path_text(info: &crate::names::FnInfo, at: &Path) -> String {
    let mut s = info.params.get(at.0).map(|p| p.name.text.clone()).unwrap_or_default();
    for st in &at.1 {
        match st {
            Step::Field(f) => {
                s.push('.');
                s.push_str(f);
            }
            Step::Elem => s.push_str("[0]"),
        }
    }
    s
}

/// What a part of a parameter declares: its type, its range, its count.
pub fn slot_at(p: &Program, info: &crate::names::FnInfo, at: &Path) -> Option<Slot> {
    let mut slot = info.params.get(at.0)?.slot.clone();
    for st in &at.1 {
        let mut t = slot.ty.clone();
        while let Ty::Opt(x) = t {
            t = *x;
        }
        slot = match (st, t) {
            (Step::Elem, Ty::List(x)) => Slot { ty: *x, range: slot.range.clone(), count: None },
            (Step::Field(f), Ty::Record(id)) => p.fields(&id).into_iter().find(|(n, _)| n.text == *f)?.1,
            _ => return None,
        };
    }
    Some(slot)
}

/// The value at a part of a parameter in an input.
pub fn value_at<'v>(input: &'v [Value], at: &Path) -> Option<&'v Value> {
    let mut v = input.get(at.0)?;
    for st in &at.1 {
        v = match (st, v) {
            (Step::Elem, Value::List(xs)) => xs.first()?,
            (Step::Field(f), Value::Record(fs)) => fs.iter().find(|(n, _)| n == f).map(|(_, x)| x)?,
            _ => return None,
        };
    }
    Some(v)
}

/// The input with the value at a part of a parameter replaced; None where the input has no such
/// part (an empty list).
pub fn set_at(input: &[Value], at: &Path, to: Value) -> Option<Vec<Value>> {
    fn put(v: &Value, steps: &[Step], to: Value) -> Option<Value> {
        match steps.split_first() {
            None => Some(to),
            Some((Step::Elem, rest)) => match v {
                Value::List(xs) if !xs.is_empty() => {
                    let mut ys = xs.clone();
                    ys[0] = put(&xs[0], rest, to)?;
                    Some(Value::List(ys))
                }
                _ => None,
            },
            Some((Step::Field(f), rest)) => match v {
                Value::Record(fs) => {
                    let mut gs = fs.clone();
                    let k = gs.iter().position(|(n, _)| n == f)?;
                    gs[k].1 = put(&fs[k].1, rest, to)?;
                    Some(Value::Record(gs))
                }
                _ => None,
            },
        }
    }
    let mut out = input.to_vec();
    out[at.0] = put(input.get(at.0)?, &at.1, to)?;
    Some(out)
}

/// The inputs of `f` at which the sum of parameters `lin` comes to `target`, each moving one part of
/// a parameter from the base values `base`, and the part it moves: a value on the part's step and in
/// what it declares.
pub fn reach(p: &Program, m: &Checked, f: usize, base: &[Value], lin: &Lin, target: Rat) -> Vec<(Vec<Value>, Path)> {
    let info = &m.decls.fns[f];
    let num = |v: &Value| -> Option<Rat> {
        match v {
            Value::Num(r) => Some(*r),
            Value::Date(d) => Some(Rat::int(*d as i128)),
            _ => None,
        }
    };
    let mut out = Vec::new();
    for (atom, c) in &lin.terms {
        // the rest of the sum at the base values
        let mut rest = Some(lin.k);
        for (other, d) in &lin.terms {
            if other == atom {
                continue;
            }
            rest = rest.zip(value_at(base, other).and_then(num)).and_then(|(r, v)| r.checked_add(d.checked_mul(v)?));
        }
        let Some(x) = rest.and_then(|r| target.checked_sub(r)).and_then(|t| t.checked_div(*c)) else { continue };
        let Some(slot) = slot_at(p, info, atom) else { continue };
        let mut t = slot.ty.clone();
        while let Ty::Opt(y) = t {
            t = *y;
        }
        let value = match &t {
            Ty::Num(_) => {
                let step = Choose::step(&t);
                let range = Choose::range(&slot, &t);
                let inside = range.as_ref().is_none_or(|r| r.min.is_none_or(|lo| num::cmp(x, lo).is_ge()) && r.max.is_none_or(|hi| num::cmp(x, hi).is_le()));
                if !inside || !x.on_grid(step) || !num::fits(x) {
                    continue;
                }
                Value::Num(x)
            }
            Ty::Date => {
                if !x.is_int() || !(crate::date::MIN as i128..=crate::date::MAX as i128).contains(&x.num) {
                    continue;
                }
                let d = x.num as i64;
                if slot.range.as_ref().is_some_and(|r| r.min.is_some_and(|lo| (d as i128) < lo.num) || r.max.is_some_and(|hi| (d as i128) > hi.num)) {
                    continue;
                }
                Value::Date(d)
            }
            _ => continue,
        };
        if let Some(input) = set_at(base, atom, value) {
            out.push((input, atom.clone()));
        }
    }
    out
}

/// One vector run, as `vectors` writes it: the function, the input on the wire, its answer, its
/// failure, or that it did not stop, why the input was chosen, and with `trace` the calls it made.
pub fn line(p: &Program, suite: &Suite, m: &Checked, f: usize, v: &Vector, trace: bool, lang: Lang) -> Json {
    let info = &m.decls.fns[f];
    let input = Json::obj(info.params.iter().zip(&v.input).map(|(prm, x)| (prm.name.text.clone(), p.to_json(x, &prm.slot.ty))));
    let mut it = Interp::new(p, suite);
    let outcome = match it.check_entry(m, f, &v.input) {
        Err(fl) => Err(Stop::Failed(fl)),
        Ok(()) => it.call(m, f, v.input.clone(), 0),
    };
    let mut fields: Vec<(String, Json)> = vec![("function".into(), Json::str(info.name.text.clone())), ("in".into(), input)];
    match outcome {
        Ok(x) => fields.push(("out".into(), p.to_json(&x, &info.ret.ty))),
        Err(Stop::Failed(fl)) => fields.push((
            "fails".into(),
            Json::obj([
                ("kind", Json::str(fl.kind.word())),
                ("module", Json::str(fl.module.clone())),
                ("line", Json::int(fl.line as i128)),
                ("message", Json::str(ritsu_base::text::spaced(&fl.message, lang))),
            ]),
        )),
        Err(Stop::Diverged(d)) => fields.push(("diverged".into(), Json::obj([("module", Json::str(d.module.clone())), ("line", Json::int(d.line as i128)), ("depth", Json::int(d.depth as i128))]))),
    }
    if trace {
        let calls: Vec<Json> = it
            .trace
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Call(c) => Some(c.json.clone()),
                Event::Let(_) => None,
            })
            .collect();
        fields.push(("calls".into(), Json::arr(calls)));
    }
    let why = v.why.clone().unwrap_or_else(|| tr!("基本の値", "the base values"));
    fields.push(("why".into(), Json::str(why.get(lang).to_string())));
    Json::Obj(fields)
}
