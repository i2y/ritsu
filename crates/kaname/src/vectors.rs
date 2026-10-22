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
//!   a rule or a date it is given to as it is, with the step outside each end; an end not written is
//!   the type's (the least and the greatest of int64), and a number whose range is not written takes
//!   1 and −1 too (DESIGN 22.4);
//! - a date: the same, a day a step, an end not written 0001-01-01 or 9999-12-31;
//! - an enum, a bool: every value; a value that may be none: `none`, and the candidates of the value;
//! - a string: `"a"`, each string it is compared with, each prefix `starts_with` asks of it; where it
//!   is given to a builtin of strings or compared in the order of strings, the strings the three
//!   languages read differently ([`HARD`]); where it declares a count, as many code points as each
//!   end of it (DESIGN 25.8);
//! - a list: as many elements as the lower end of its count, 0, 1, 2, 3 and the upper end, those its
//!   count holds, each element the base value; and the first element moved over its candidates;
//! - a record: one field at a time moved over its candidates, the others at their base values.
//!
//! The base value is the middle of the range for a number (on its step), 0 for one whose range is not
//! written, the lower end for a date (1970-01-01 for one whose range is not written), the first value
//! of an enum, `true`, `"a"`, a list of the fewest elements but one at least, a record of base fields. Every candidate is inside what its place declares: what is outside is the entry's
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

/// How many times a candidate holds a value of a record or an enum that holds itself on a path: its
/// own, and one inside it (DESIGN 9, 28.1). A base value holds it once.
const CANDS: usize = 2;

/// The most values of records and enums that hold themselves a value is built inside: a guard, which
/// the counts above keep far from.
const NESTS: usize = 64;

/// A step from a parameter to what it holds.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    Field(String),
    /// An element of a list: the first, where an input is built (DESIGN 9).
    Elem,
    /// A value of an enum that has fields, by its name: the value where it is that one (the value a
    /// `match` arm names, `placed p`), whose fields the next steps read (DESIGN 23.7).
    Value(String),
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
    /// It is given to a builtin of strings, or compared in their order: the strings the languages
    /// read differently are tried (DESIGN 25.8).
    Hard,
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
                    // two strings in the order of their code points
                    if !matches!(op, BinOp::Eq | BinOp::Ne) {
                        hard(m, scope, &[a, b], h);
                    }
                    meet(m, scope, &[a, b], h);
                }
                self.expr(m, scope, a, h);
                self.expr(m, scope, b, h);
            }
            ExprKind::Call(callee, args) => {
                // a builtin written after a value takes the value first (DESIGN 21.4)
                let mut exprs: Vec<&Expr> = Vec::new();
                if let (Some(Ref::Method(_)), ExprKind::Field(base, _, _)) = (m.typed.refs.get(callee.id), &callee.kind) {
                    exprs.push(base);
                }
                exprs.extend(args.iter().map(Arg::expr));
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
                    Some(Ref::Builtin(name) | Ref::Method(name)) => {
                        if *name == "starts_with"
                            && let [s, pre] = exprs.as_slice()
                            && let (Some(at), ExprKind::Str(text)) = (path_of(m, scope, s), &pre.kind)
                        {
                            h.by.entry(at).or_default().push(Hint::Prefix(text.clone()));
                        }
                        // the strings a builtin of strings counts, cuts, finds or reads, and those `min`
                        // and `max` compare
                        if HARD_BUILTINS.contains(name) || matches!(*name, "count" | "slice" | "take" | "drop" | "contains" | "min" | "max") {
                            hard(m, scope, &exprs, h);
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
                        match p {
                            Pattern::Some(x, _) => scope.push((x.clone(), at.clone())),
                            // a value with fields an arm names: its fields are the parameter's, through it
                            Pattern::Value(path, Some((x, _)), _) => {
                                let word = &path[path.len() - 1].0;
                                // `ok x` and `err e`: the value the `ok` or the `err` holds (DESIGN 26.3)
                                let to = if matches!(m.typed.types.get(s.id).map(Ty::present), Some(Ty::Result(..))) {
                                    at.clone().map(|(i, mut steps)| {
                                        steps.push(Step::Value(word.clone()));
                                        steps.push(Step::Field(if word == "ok" { "value" } else { "error" }.into()));
                                        (i, steps)
                                    })
                                } else {
                                    let value = value_name(m, s, word);
                                    at.clone().zip(value).map(|((i, mut steps), v)| {
                                        steps.push(Step::Value(v));
                                        (i, steps)
                                    })
                                };
                                scope.push((x.clone(), to));
                            }
                            // a literal an arm takes: the value is compared with it
                            Pattern::Lit(x) => {
                                if let (Some(at), Some(lit)) = (at.clone(), literal(m, x)) {
                                    h.by.entry(at).or_default().push(lit);
                                }
                            }
                            _ => {}
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
            ExprKind::Record(_, base, inits) => {
                if let Some(b) = base {
                    self.expr(m, scope, b, h);
                }
                for fi in inits {
                    self.expr(m, scope, &fi.value, h);
                }
            }
            ExprKind::List(xs) => {
                for x in xs {
                    self.expr(m, scope, x, h);
                }
            }
            ExprKind::Dict(pairs) => {
                for (k, v) in pairs {
                    self.expr(m, scope, k, h);
                    self.expr(m, scope, v, h);
                }
            }
            ExprKind::Neg(x) | ExprKind::Not(x) | ExprKind::In(x, _, _) | ExprKind::Field(x, _, _) => self.expr(m, scope, x, h),
            // a lambda made a value (DESIGN 27.1): its body reads the names in scope, its parameters
            // are no part of one
            ExprKind::Lambda(ps, body) => {
                let mark = scope.len();
                for (p, _) in ps {
                    scope.push((p.clone(), None));
                }
                self.expr(m, scope, body, h);
                scope.truncate(mark);
            }
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

/// The builtins of strings whose strings are tried with the strings the languages read differently
/// (DESIGN 25.8); `count`, `slice`, `take`, `drop`, `contains`, `min` and `max` too where what they
/// are given is a string.
const HARD_BUILTINS: [&str; 16] = [
    "chars", "code_points", "index_of", "ends_with", "split", "words", "replace", "trim", "trim_start", "trim_end", "lower_ascii", "upper_ascii", "pad_start", "pad_end", "parse_number", "parse_date",
];

/// The strings the three languages read differently, each with what it tries (DESIGN 25.8): their
/// counts in UTF-16 units and bytes, white space at the ends and inside, the space each `trim` takes
/// or keeps, a letter outside ASCII and one made of two code points, numbers and days written as
/// strings.
pub const HARD: [(&str, &str, &str); 15] = [
    ("", "空の文字列", "the empty string"),
    (" a ", "前後に空白", "white space at both ends"),
    ("a  b", "中に空白二つ", "two spaces inside"),
    ("\u{c9}", "ASCII の外の文字", "a letter outside ASCII"),
    ("e\u{301}", "文字と、それに付くアクセント", "a letter and the accent put on it"),
    ("\u{1f44d}\u{1f3fd}", "UTF-16 で二単位ずつのコードポイント二つ", "two code points of two UTF-16 units each"),
    ("\u{85}a", "空白の U+0085", "U+0085, white space"),
    ("\u{feff}a", "空白ではない U+FEFF", "U+FEFF, which is no white space"),
    ("\u{a0}a", "空白の U+00A0", "U+00A0, white space"),
    ("\u{ff21}\u{ff11}", "全角の英字と数字", "a full-width letter and digit"),
    ("-12", "負の数", "a number below zero"),
    ("+12", "`+` を付けた数", "a number with `+`"),
    ("9223372036854775808", "int64 を超える数", "a number past int64"),
    ("2024-02-29", "うるう年の 2 月 29 日", "the 29th of February of a leap year"),
    ("2026-02-29", "暦に無い日", "a day no calendar has"),
];

/// The strings among the expressions, each a part of a parameter, take [`Hint::Hard`].
fn hard(m: &Checked, scope: &Scope, es: &[&Expr], h: &mut Hints) {
    for x in es {
        if matches!(m.typed.types.get(x.id), Some(Ty::Str))
            && let Some(at) = path_of(m, scope, x)
            && !h.by.get(&at).is_some_and(|hs| hs.iter().any(|y| matches!(y, Hint::Hard)))
        {
            h.by.entry(at).or_default().push(Hint::Hard);
        }
    }
}

/// The declared name of the value of an enum an arm names by a word (its name or its alias), the
/// `match` splitting `s`.
fn value_name(m: &Checked, s: &Expr, word: &str) -> Option<String> {
    let Some(Ty::Enum(id)) = m.typed.types.get(s.id).map(Ty::present) else { return None };
    let c: &Checked = if matches!(&id.owner, Owner::Module(f) if *f == m.canon) {
        m
    } else {
        match &id.owner {
            Owner::Module(f) => m.world.modules.get(f)?,
            Owner::Rule(_) => return None,
        }
    };
    let e = c.decls.enums.iter().find(|e| e.name.text == id.name)?;
    e.values.iter().find(|v| v.is(word)).map(|v| v.text.clone())
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
        self.build(slot, t, &[], 1).unwrap_or(Value::None)
    }

    /// The values of records and enums that hold themselves (DESIGN 28.1) a value of `t` would be
    /// inside: `keys`, with `t`'s own where it is one.
    fn inside(&self, t: &Ty, keys: &[Ty]) -> Vec<Ty> {
        let mut out = keys.to_vec();
        if let Some(k) = self.p.nest_key(t) {
            out.push(k);
        }
        out
    }

    /// Whether a value of `t` (`count` the count written on it) can be made holding no value of the
    /// types `keys`, nor one of its own inside one of its own (DESIGN 28.1): an optional (none), a
    /// list, a dict or a set that may be empty, a result whose `ok` or `err` can, a record whose
    /// fields can, an enum one of whose values' fields can.
    fn free(&self, t: &Ty, count: Option<&Range>, keys: &[Ty]) -> bool {
        if keys.is_empty() {
            return true;
        }
        if keys.len() > NESTS {
            return false;
        }
        let empty = count.is_none_or(|c| c.min.is_none_or(|m| m.num <= 0));
        if let Some(k) = self.p.nest_key(t)
            && keys.contains(&k)
        {
            return false;
        }
        let inner = self.inside(t, keys);
        match t {
            Ty::Opt(_) => true,
            Ty::List(x) | Ty::Set(x) => empty || self.free(x, None, keys),
            Ty::Dict(_, x) => empty || self.free(x, None, keys),
            Ty::Result(a, e) => self.free(a, None, keys) || self.free(e, None, keys),
            Ty::Enum(id) if self.p.is_sum(id) => self.p.values(id).0.iter().any(|v| self.p.value_fields(id, &v.text).iter().all(|(_, s)| self.free(&s.ty, s.count.as_ref(), &inner))),
            Ty::Record(id @ (RecId::Module { .. } | RecId::Variant { .. })) => self.p.fields(id).iter().all(|(_, s)| self.free(&s.ty, s.count.as_ref(), &inner)),
            _ => true,
        }
    }

    /// The base value of `t` where the values of records and enums that hold themselves on the path to
    /// it are `keys` (DESIGN 9, 28.1): `t` itself is made even where it is one of them, as the caller
    /// asks; inside it, where there is a choice (an optional, a list or a dict that may be empty, a
    /// value of an enum, `ok` or `err`), the one that holds none of them and none of its own is taken.
    /// None where such a value would hold one of them more than `limit` times on a path.
    fn build(&self, slot: &Slot, t: &Ty, keys: &[Ty], limit: usize) -> Option<Value> {
        if let Some(k) = self.p.nest_key(t)
            && (keys.iter().filter(|x| **x == k).count() >= limit || keys.len() > NESTS)
        {
            return None;
        }
        let inner = self.inside(t, keys);
        let free = |x: &Ty, count: Option<&Range>| self.free(x, count, &inner);
        Some(match t {
            Ty::Opt(x) => {
                if !free(x, None) {
                    return Some(Value::None);
                }
                return self.build(&Slot { ty: (**x).clone(), ..slot.clone() }, x, keys, limit);
            }
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
            Ty::Str => Value::Str(Self::str_base(slot)),
            // a sum: its first value, with base fields where it has fields; inside a value that holds
            // itself, its first value whose fields hold none of the values around it (DESIGN 28.1)
            Ty::Enum(id) if self.p.is_sum(id) => {
                let values = self.p.values(id).0;
                let pick = values.iter().find(|v| self.p.value_fields(id, &v.text).iter().all(|(_, s)| free(&s.ty, s.count.as_ref()))).or(values.first());
                return pick.and_then(|v| self.variant_build(id, &v.text, &inner, limit));
            }
            Ty::Enum(id) => self.p.values(id).0.first().map(|v| Value::Enum(v.text.clone())).unwrap_or(Value::None),
            Ty::Record(r @ RecId::Variant { value, .. }) => return r.sum().and_then(|id| self.variant_build(&id, value, &inner, limit)),
            // a `result`: an `ok` of the base value (DESIGN 26.3); an `err` where the `ok` would hold a
            // value around it
            Ty::Result(a, e) => {
                if free(a, None) || !free(e, None) {
                    Value::Variant("ok".into(), vec![("value".into(), self.build(&Slot::of((**a).clone()), a, &inner, limit)?)])
                } else {
                    Value::Variant("err".into(), vec![("error".into(), self.build(&Slot::of((**e).clone()), e, &inner, limit)?)])
                }
            }
            Ty::List(x) => {
                let n = if free(x, None) { Self::base_count(slot) } else { Self::count_ends(slot).0 as usize };
                let elem = Slot { ty: (**x).clone(), range: slot.range.clone(), count: None };
                Value::List((0..n).map(|_| self.build(&elem, x, &inner, limit)).collect::<Option<Vec<_>>>()?)
            }
            Ty::Record(id) => Value::Record(self.p.fields(id).iter().map(|(f, s)| Some((f.text.clone(), self.build(s, &s.ty, &inner, limit)?))).collect::<Option<Vec<_>>>()?),
            // a dict and a set: as many keys as a list's base has elements, each another (DESIGN 24.4)
            Ty::Dict(k, x) => {
                let n = if free(x, None) { Self::base_count(slot) } else { Self::count_ends(slot).0 as usize };
                let val = if n == 0 { Value::None } else { self.build(&Slot { ty: (**x).clone(), range: slot.range.clone(), count: None }, x, &inner, limit)? };
                let keys = self.keys(&Slot::of((**k).clone()), k, n);
                Value::Dict(self.p.dict_of(k, keys.into_iter().map(|key| (key, val.clone())).collect()))
            }
            Ty::Set(x) => {
                let n = Self::base_count(slot);
                let members = self.keys(&Slot { ty: (**x).clone(), range: slot.range.clone(), count: None }, x, n);
                Value::Set(self.p.set_of(x, members))
            }
            Ty::Transfer(id) => {
                let params = self.p.transfer(id).map(|(_, ps)| ps).unwrap_or_default();
                Value::Transfer(Box::new(TransferValue { id: id.clone(), args: params.iter().map(|(n, t)| (n.clone(), self.base(&Slot::of(t.clone()), t))).collect() }))
            }
            _ => Value::None,
        })
    }

    /// The base string: `"a"`, or as many `a` as the count of code points it declares takes at least,
    /// or none where it takes none (DESIGN 25.8).
    fn str_base(slot: &Slot) -> String {
        let (lo, hi) = Self::count_ends(slot);
        "a".repeat((1i128.max(lo).min(hi).max(0)) as usize)
    }

    /// A value of a sum, its fields at their base values: none for a value with no fields.
    fn variant_base(&self, id: &EnumId, v: &str) -> Value {
        let inner = self.inside(&Ty::Enum(id.clone()), &[]);
        self.variant_build(id, v, &inner, 1).unwrap_or(Value::None)
    }

    /// The same, inside the values `inner` (the sum's own with them, DESIGN 28.1), a path holding one of
    /// them at most `limit` times.
    fn variant_build(&self, id: &EnumId, v: &str, inner: &[Ty], limit: usize) -> Option<Value> {
        let fields = self.p.value_fields(id, v);
        if fields.is_empty() {
            return Some(Value::Enum(v.to_string()));
        }
        Some(Value::Variant(v.to_string(), fields.iter().map(|(f, s)| Some((f.text.clone(), self.build(s, &s.ty, inner, limit)?))).collect::<Option<Vec<_>>>()?))
    }

    /// `n` keys of a place, each another: the place's base value, its candidates, then more of the
    /// type (the next numbers, days and letters) where they are not enough; fewer where the type has
    /// fewer values (a bool, an enum).
    fn keys(&self, slot: &Slot, t: &Ty, n: usize) -> Vec<Value> {
        let mut out: Vec<Value> = Vec::new();
        let push = |out: &mut Vec<Value>, v: Value| {
            if out.len() < n && !out.iter().any(|x| self.p.key_cmp(t, x, &v) == std::cmp::Ordering::Equal) {
                out.push(v);
            }
        };
        push(&mut out, self.base(slot, t));
        for (c, _) in self.cands(slot, t, &(usize::MAX, Vec::new()), "") {
            push(&mut out, c);
        }
        let mut more = 0i128;
        while out.len() < n && more < 64 {
            more += 1;
            let v = match t {
                Ty::Num(_) => {
                    let s = Self::step(t);
                    let start = match self.base(slot, t) {
                        Value::Num(b) => b,
                        _ => Rat::zero(),
                    };
                    let v = start.add(Rat::int(more).mul(s));
                    let inside = Self::range(slot, t).is_none_or(|r| r.max.is_none_or(|hi| num::cmp(v, hi).is_le())) && num::fits(v);
                    if !inside {
                        continue;
                    }
                    Value::Num(v)
                }
                Ty::Date => {
                    let start = match self.base(slot, t) {
                        Value::Date(d) => d,
                        _ => 0,
                    };
                    let d = start + more as i64;
                    if slot.range.as_ref().is_some_and(|r| r.max.is_some_and(|hi| d as i128 > hi.num)) || d > crate::date::MAX {
                        continue;
                    }
                    Value::Date(d)
                }
                Ty::Str => Value::Str(char::from_u32('a' as u32 + more as u32).map(|c| c.to_string()).unwrap_or_else(|| format!("a{more}"))),
                _ => break,
            };
            push(&mut out, v);
        }
        out
    }

    /// The ends of a count, 0 and the greatest where none is written.
    fn count_ends(slot: &Slot) -> (i128, i128) {
        (
            slot.count.as_ref().and_then(|c| c.min).map(|v| v.num.max(0)).unwrap_or(0),
            slot.count.as_ref().and_then(|c| c.max).map(|v| v.num.max(0)).unwrap_or(i128::MAX),
        )
    }

    /// The fewest elements but one at least, as the count holds them.
    fn base_count(slot: &Slot) -> usize {
        let lo = slot.count.as_ref().and_then(|c| c.min).map(|v| v.num.max(0) as usize).unwrap_or(0);
        let hi = slot.count.as_ref().and_then(|c| c.max).map(|v| v.num.max(0) as usize).unwrap_or(usize::MAX);
        lo.max(1).min(hi)
    }

    /// The candidates of a place at `at`, which a person names `shown`.
    fn cands(&self, slot: &Slot, t: &Ty, at: &Path, shown: &str) -> Vec<Cand> {
        self.cands_in(slot, t, at, shown, &[])
    }

    /// The same, where the values of records and enums that hold themselves on the path to the place
    /// are `keys` (DESIGN 28.1): a candidate holds one of them at most [`CANDS`] times on a path, so a
    /// value of a type that holds itself is tried with one of its own inside, and that one with none.
    fn cands_in(&self, slot: &Slot, t: &Ty, at: &Path, shown: &str, keys: &[Ty]) -> Vec<Cand> {
        let said = |why: Text| -> Option<Text> { Some(tr!("{shown}: {}", "{shown}: {}", why.ja; why.en)) };
        if let Some(k) = self.p.nest_key(t)
            && (keys.iter().filter(|x| **x == k).count() >= CANDS || keys.len() > NESTS)
        {
            return Vec::new();
        }
        let inner = self.inside(t, keys);
        match t {
            Ty::Opt(x) => {
                let mut out = vec![(Value::None, said(Text::same("none")))];
                let xs = Slot { ty: (**x).clone(), ..slot.clone() };
                if self.build(&xs, x, keys, CANDS).is_some() {
                    // where the base value is none (a value of a type that holds itself, DESIGN 28.1),
                    // the value's own base value is moved from it, and says so
                    let none_base = matches!(self.build(slot, t, keys, CANDS), Some(Value::None));
                    for (c, why) in self.cands_in(&xs, x, at, shown, keys) {
                        let why = if why.is_none() && none_base { said(tr!("値がある", "a value")) } else { why };
                        out.push((c, why));
                    }
                }
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
                // an end not written is the type's: the least and the greatest of int64 (DESIGN 22.4)
                if !n.is_rate() {
                    if range.as_ref().is_none_or(|r| r.min.is_none()) {
                        push(Rat::int(INT_MIN), tr!("型の下の端", "the lower end of the type"));
                    }
                    if range.as_ref().is_none_or(|r| r.max.is_none()) {
                        push(Rat::int(INT_MAX), tr!("型の上の端", "the upper end of the type"));
                    }
                }
                push(Rat::zero(), tr!("0", "zero"));
                if range.is_none() && !n.is_rate() {
                    push(Rat::int(1), tr!("1", "one"));
                    push(Rat::int(-1), tr!("-1", "minus one"));
                }
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
                // an end not written is the type's: 0001-01-01 and 9999-12-31 (DESIGN 22.4)
                if range.as_ref().is_none_or(|r| r.min.is_none()) {
                    push(crate::date::MIN, tr!("型の下の端", "the lower end of the type"));
                }
                if range.as_ref().is_none_or(|r| r.max.is_none()) {
                    push(crate::date::MAX, tr!("型の上の端", "the upper end of the type"));
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
            // a sum: each value at its base fields, then each field of each value over its candidates
            Ty::Enum(id) if self.p.is_sum(id) => {
                let names: Vec<String> = self.p.values(id).0.iter().map(|v| v.text.clone()).collect();
                self.variant_cands(id, &names, at, shown, &inner)
            }
            Ty::Enum(id) => self.p.values(id).0.iter().map(|v| (Value::Enum(v.text.clone()), said(Text::same(v.text.clone())))).collect(),
            Ty::Record(r @ RecId::Variant { value, .. }) => match r.sum() {
                Some(id) => self.variant_cands(&id, std::slice::from_ref(value), at, shown, &inner),
                None => self.build(slot, t, keys, CANDS).map(|v| (v, None)).into_iter().collect(),
            },
            // a `result`: `ok` and `err` at their base values, then the value each holds over its
            // candidates (DESIGN 26.3)
            Ty::Result(a, e) => {
                let mut out: Vec<Cand> = Vec::new();
                for (v, f, held) in [("ok", "value", a), ("err", "error", e)] {
                    let islot = Slot::of((**held).clone());
                    let Some(hb) = self.build(&islot, held, &inner, CANDS) else { continue };
                    let base = Value::Variant(v.into(), vec![(f.into(), hb)]);
                    let why = if v == "ok" { None } else { said(Text::same("err")) };
                    if !out.iter().any(|(x, _)| *x == base) {
                        out.push((base, why));
                    }
                    let mut steps = at.1.clone();
                    steps.push(Step::Value(v.into()));
                    steps.push(Step::Field(f.into()));
                    for (c, why) in self.cands_in(&islot, held, &(at.0, steps), &format!("{shown}.{v}"), &inner) {
                        let x = Value::Variant(v.into(), vec![(f.into(), c)]);
                        if !out.iter().any(|(y, _)| *y == x) {
                            out.push((x, why));
                        }
                    }
                }
                out
            }
            Ty::Str => {
                // every candidate within the count of code points the place declares (DESIGN 25.8)
                let (lo, hi) = Self::count_ends(slot);
                let fits = |s: &str| (lo..=hi).contains(&(crate::text::count(s) as i128));
                let mut out: Vec<Cand> = vec![(Value::Str(Self::str_base(slot)), None)];
                let mut push = |s: String, why: Text| {
                    if fits(&s) && !out.iter().any(|(x, _)| *x == Value::Str(s.clone())) {
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
                        Hint::Hard => {
                            for (x, ja, en) in HARD {
                                push(x.to_string(), tr!("{ja}", "{en}"));
                            }
                        }
                        _ => {}
                    }
                }
                // as many code points as each end of a count written
                if slot.count.is_some() {
                    for (n, why) in [(lo, tr!("コードポイントの数の下の端", "the lower end of the count")), (hi, tr!("コードポイントの数の上の端", "the upper end of the count"))] {
                        if n <= MOST as i128 {
                            push("a".repeat(n as usize), why);
                        }
                    }
                }
                out
            }
            Ty::List(x) => {
                let elem = Slot { ty: (**x).clone(), range: slot.range.clone(), count: None };
                // an element that would hold a value around it more than a candidate may: none
                let base_elem = self.build(&elem, x, &inner, CANDS);
                let (lo, hi) = (
                    slot.count.as_ref().and_then(|c| c.min).map(|v| v.num.max(0)).unwrap_or(0),
                    slot.count.as_ref().and_then(|c| c.max).map(|v| v.num.max(0)).unwrap_or(i128::MAX),
                );
                let base_n = self.build(slot, t, keys, CANDS).and_then(|v| match v {
                    Value::List(xs) => Some(xs.len()),
                    _ => None,
                });
                let mut out: Vec<Cand> = Vec::new();
                for n in [lo, 0, 1, 2, 3, hi] {
                    if n < lo || n > hi || n > MOST as i128 || out.iter().any(|(v, _)| matches!(v, Value::List(xs) if xs.len() as i128 == n)) {
                        continue;
                    }
                    let Some(be) = base_elem.clone().or_else(|| (n == 0).then_some(Value::None)) else { continue };
                    let why = if Some(n as usize) == base_n { None } else { said(tr!("要素 {n} 個", "{n} elements")) };
                    out.push((Value::List(vec![be; n as usize]), why));
                }
                // the first element moved over its candidates, among as many as the base list holds
                let n = Self::base_count(slot);
                if let Some(base_elem) = base_elem.filter(|_| n >= 1) {
                    let mut steps = at.1.clone();
                    steps.push(Step::Elem);
                    for (c, why) in self.cands_in(&elem, x, &(at.0, steps), &format!("{shown}[0]"), &inner) {
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
            // a dict: each count a list's is tried with, each key another and each value its base; then
            // the first value, and the first key, moved over their candidates (DESIGN 24.4)
            Ty::Dict(k, x) => {
                let vslot = Slot { ty: (**x).clone(), range: slot.range.clone(), count: None };
                let kslot = Slot::of((**k).clone());
                let base_val = self.build(&vslot, x, &inner, CANDS);
                let (lo, hi) = Self::count_ends(slot);
                let base_n = match self.build(slot, t, keys, CANDS) {
                    Some(Value::Dict(ps)) => Some(ps.len()),
                    _ => None,
                };
                let mut out: Vec<Cand> = Vec::new();
                for n in [lo, 0, 1, 2, 3, hi] {
                    if n < lo || n > hi || n > MOST as i128 {
                        continue;
                    }
                    let Some(bv) = base_val.clone().or_else(|| (n == 0).then_some(Value::None)) else { continue };
                    let ks = self.keys(&kslot, k, n as usize);
                    if ks.len() as i128 != n {
                        continue;
                    }
                    let v = Value::Dict(self.p.dict_of(k, ks.into_iter().map(|key| (key, bv.clone())).collect()));
                    if out.iter().any(|(y, _)| *y == v) {
                        continue;
                    }
                    let why = if Some(n as usize) == base_n { None } else { said(tr!("組 {n} 個", "{n} pairs")) };
                    out.push((v, why));
                }
                let Some(Value::Dict(base)) = self.build(slot, t, keys, CANDS) else { return out };
                if let Some((k0, _)) = base.first().cloned() {
                    let nowhere = (usize::MAX, Vec::new());
                    for (c, why) in self.cands_in(&vslot, x, &nowhere, &format!("{shown}[0][1]"), &inner) {
                        let mut ps = base.clone();
                        ps[0].1 = c;
                        let v = Value::Dict(ps);
                        if !out.iter().any(|(y, _)| *y == v) {
                            out.push((v, why));
                        }
                    }
                    for (c, why) in self.cands(&kslot, k, &nowhere, &format!("{shown}[0][0]")) {
                        if base.iter().any(|(y, _)| self.p.key_cmp(k, y, &c) == std::cmp::Ordering::Equal) {
                            continue;
                        }
                        let mut ps: Vec<(Value, Value)> = base.iter().filter(|(y, _)| *y != k0).cloned().collect();
                        ps.push((c, base[0].1.clone()));
                        let v = Value::Dict(self.p.dict_of(k, ps));
                        if !out.iter().any(|(y, _)| *y == v) {
                            out.push((v, why));
                        }
                    }
                }
                out
            }
            // a set: each count, each member another; then the first member moved over its candidates
            Ty::Set(x) => {
                let eslot = Slot { ty: (**x).clone(), range: slot.range.clone(), count: None };
                let (lo, hi) = Self::count_ends(slot);
                let mut out: Vec<Cand> = Vec::new();
                for n in [lo, 0, 1, 2, 3, hi] {
                    if n < lo || n > hi || n > MOST as i128 {
                        continue;
                    }
                    let members = self.keys(&eslot, x, n as usize);
                    if members.len() as i128 != n {
                        continue;
                    }
                    let v = Value::Set(self.p.set_of(x, members));
                    if out.iter().any(|(y, _)| *y == v) {
                        continue;
                    }
                    let why = if n as usize == Self::base_count(slot) { None } else { said(tr!("要素 {n} 個", "{n} members")) };
                    out.push((v, why));
                }
                let Value::Set(base) = self.base(slot, t) else { return out };
                if let Some(m0) = base.first().cloned() {
                    for (c, why) in self.cands(&eslot, x, &(usize::MAX, Vec::new()), &format!("{shown}[0]")) {
                        if base.iter().any(|y| self.p.key_cmp(x, y, &c) == std::cmp::Ordering::Equal) {
                            continue;
                        }
                        let mut ms: Vec<Value> = base.iter().filter(|y| **y != m0).cloned().collect();
                        ms.push(c);
                        let v = Value::Set(self.p.set_of(x, ms));
                        if !out.iter().any(|(y, _)| *y == v) {
                            out.push((v, why));
                        }
                    }
                }
                out
            }
            Ty::Record(id) => {
                let Some(base) = self.build(slot, t, keys, CANDS) else { return Vec::new() };
                let mut out: Vec<Cand> = vec![(base.clone(), None)];
                let Value::Record(fields) = &base else { return out };
                for (k, (f, fslot)) in self.p.fields(id).iter().enumerate() {
                    let mut steps = at.1.clone();
                    steps.push(Step::Field(f.text.clone()));
                    for (c, why) in self.cands_in(fslot, &fslot.ty, &(at.0, steps), &format!("{shown}.{}", f.text), &inner) {
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
            _ => self.build(slot, t, keys, CANDS).map(|v| (v, None)).into_iter().collect(),
        }
    }
}

impl Choose<'_> {
    /// The candidates of a value of a sum, of the values `names`: each value with its base fields (the
    /// first one the base value), then for each value with fields, one field at a time moved over its
    /// candidates, the others at their base values (DESIGN 23.7, as a record's). `inner`: the values of
    /// records and enums that hold themselves the value is inside, the sum's own with them (DESIGN
    /// 28.1); a value that cannot be made within them is not tried.
    fn variant_cands(&self, id: &EnumId, names: &[String], at: &Path, shown: &str, inner: &[Ty]) -> Vec<Cand> {
        let said = |why: Text| -> Option<Text> { Some(tr!("{shown}: {}", "{shown}: {}", why.ja; why.en)) };
        let mut out: Vec<Cand> = Vec::new();
        // the first value made is the base value
        let mut first = true;
        for v in names {
            let Some(base) = self.variant_build(id, v, inner, CANDS) else { continue };
            let why = if first { None } else { said(Text::same(v.clone())) };
            first = false;
            if !out.iter().any(|(x, _)| *x == base) {
                out.push((base.clone(), why));
            }
            let Value::Variant(_, fields) = &base else { continue };
            for (i, (f, fslot)) in self.p.value_fields(id, v).iter().enumerate() {
                let mut steps = at.1.clone();
                steps.push(Step::Value(v.clone()));
                steps.push(Step::Field(f.text.clone()));
                for (c, why) in self.cands_in(fslot, &fslot.ty, &(at.0, steps), &format!("{shown}.{v}.{}", f.text), inner) {
                    if c == fields[i].1 {
                        continue;
                    }
                    let mut fs = fields.clone();
                    fs[i].1 = c;
                    let x = Value::Variant(v.clone(), fs);
                    if !out.iter().any(|(y, _)| *y == x) {
                        out.push((x, why));
                    }
                }
            }
        }
        out
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

/// A number counted in `from`, counted in `to`; a number and a rate count alike, and a temperature is
/// read with its unit's zero (`-40℃` is `-40℉`), as the reference interpreter reads it.
fn convert_num(v: Rat, from: &ritsu_units::Unit, to: &ritsu_units::Unit) -> Option<Rat> {
    use ritsu_units::Dim;
    if v.num == 0 && !(from.compares_only() && to.compares_only()) || (matches!(from.dim, Dim::Number | Dim::Rate) && matches!(to.dim, Dim::Number | Dim::Rate)) {
        return Some(v);
    }
    if from.compares_only() && to.compares_only() && from.offset() != to.offset() {
        return crate::interp::convert(v, from, to).ok();
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
    // an inner function has none: no host calls it, and it is tried through the functions that call
    // it (DESIGN 21.3)
    if m.decls.fns[f].inner() {
        return (Vec::new(), false);
    }
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

/// `n` keys of a place of type `t`, each another (fewer where the type has fewer values): what a dict
/// or a set of `n` is made of where an input is built (DESIGN 24.4).
pub fn keys_of(p: &Program, slot: &Slot, t: &Ty, n: usize) -> Vec<Value> {
    Choose { p, hints: Hints::default() }.keys(slot, t, n)
}

/// A value of `t` that nests the values of a record or an enum that holds itself `n` deep (DESIGN
/// 28.1), the rest of it at its base values: what a host may give, and one deeper than it may, which
/// the entry fails. The first such type a value of `t` can hold is nested, each value of it in one of
/// its fields; the count takes in what the base values of the other fields nest. None where a value of
/// `t` holds no such value, or none comes out `n` deep.
pub fn nested_value(p: &Program, t: &Ty, n: usize) -> Option<Value> {
    let c = Choose { p, hints: Hints::default() };
    let target = c.first_nested(t, &mut Vec::new())?;
    let made = |k: usize| -> Option<Value> {
        let mut v = c.base(&Slot::of(target.clone()), &target);
        for _ in 1..k {
            v = c.holding(&target, &target, v, true, &mut Vec::new())?;
        }
        c.holding(t, &target, v, false, &mut Vec::new())
    };
    // the base values of the other fields can nest one or two more: as many fewer of the chain
    let first = made(n)?;
    let over = p.nesting(&first, t).checked_sub(n)?;
    let v = if over == 0 { first } else { made(n.checked_sub(over)?)? };
    (p.nesting(&v, t) == n).then_some(v)
}

impl Choose<'_> {
    /// The first record or enum that holds itself a value of `t` can hold (DESIGN 28.1), as its values
    /// are counted (`Program::nest_key`).
    fn first_nested(&self, t: &Ty, seen: &mut Vec<Ty>) -> Option<Ty> {
        if let Some(k) = self.p.nest_key(t) {
            return Some(k);
        }
        match t {
            Ty::Opt(x) | Ty::List(x) | Ty::Set(x) => self.first_nested(x, seen),
            Ty::Dict(_, x) => self.first_nested(x, seen),
            Ty::Result(a, e) => self.first_nested(a, seen).or_else(|| self.first_nested(e, seen)),
            Ty::Record(id @ (RecId::Module { .. } | RecId::Variant { .. })) => {
                if seen.contains(t) {
                    return None;
                }
                seen.push(t.clone());
                self.p.fields(id).iter().find_map(|(_, s)| self.first_nested(&s.ty, seen))
            }
            Ty::Enum(id) if self.p.is_sum(id) => {
                if seen.contains(t) {
                    return None;
                }
                seen.push(t.clone());
                self.p.values(id).0.iter().find_map(|v| self.p.value_fields(id, &v.text).iter().find_map(|(_, s)| self.first_nested(&s.ty, seen)))
            }
            _ => None,
        }
    }

    /// A value of `t` that holds `inner`, a value of `target`, where the first field it can is, the
    /// other fields at their base values; `own`: `t` is `target` itself, made around `inner` (as a
    /// value of the type of one of its values is).
    fn holding(&self, t: &Ty, target: &Ty, inner: Value, own: bool, seen: &mut Vec<Ty>) -> Option<Value> {
        let variant = matches!(t, Ty::Record(RecId::Variant { .. }));
        if !own && !variant && self.p.nest_key(t).as_ref() == Some(target) {
            return Some(inner);
        }
        // the fields of a record or of a value, the first that can hold it holding it
        let fields = |me: &Self, slots: Vec<(crate::ast::Name, Slot)>, seen: &mut Vec<Ty>| -> Option<Vec<(String, Value)>> {
            for (k, (_, s)) in slots.iter().enumerate() {
                if let Some(v) = me.holding(&s.ty, target, inner.clone(), false, seen) {
                    return Some(slots.iter().enumerate().map(|(j, (f, fs))| (f.text.clone(), if j == k { v.clone() } else { me.base(fs, &fs.ty) })).collect());
                }
            }
            None
        };
        match t {
            Ty::Opt(x) => self.holding(x, target, inner, false, seen),
            Ty::List(x) => Some(Value::List(vec![self.holding(x, target, inner, false, seen)?])),
            Ty::Dict(k, x) => {
                let v = self.holding(x, target, inner, false, seen)?;
                let key = self.keys(&Slot::of((**k).clone()), k, 1).into_iter().next()?;
                Some(Value::Dict(self.p.dict_of(k, vec![(key, v)])))
            }
            Ty::Result(a, e) => match self.holding(a, target, inner.clone(), false, seen) {
                Some(v) => Some(Value::Variant("ok".into(), vec![("value".into(), v)])),
                None => Some(Value::Variant("err".into(), vec![("error".into(), self.holding(e, target, inner, false, seen)?)])),
            },
            Ty::Record(id @ RecId::Module { .. }) => {
                if !own && seen.contains(t) {
                    return None;
                }
                seen.push(t.clone());
                Some(Value::Record(fields(self, self.p.fields(id), seen)?))
            }
            Ty::Record(r @ RecId::Variant { value, .. }) => {
                if !own && seen.contains(t) {
                    return None;
                }
                seen.push(t.clone());
                Some(Value::Variant(value.clone(), fields(self, r.sum().map(|id| self.p.value_fields(&id, value)).unwrap_or_default(), seen)?))
            }
            Ty::Enum(id) if self.p.is_sum(id) => {
                if !own && seen.contains(t) {
                    return None;
                }
                seen.push(t.clone());
                self.p.values(id).0.iter().find_map(|v| Some(Value::Variant(v.text.clone(), fields(self, self.p.value_fields(id, &v.text), &mut seen.clone())?)))
            }
            _ => None,
        }
    }
}

/// The base value of a place.
pub fn base_of(p: &Program, slot: &Slot) -> Value {
    Choose { p, hints: Hints::default() }.base(slot, &slot.ty)
}

/// The base values of the parameters of `f` (DESIGN 9): what an input moves one part of.
pub fn base_input(p: &Program, m: &Checked, f: usize) -> Vec<Value> {
    let c = Choose { p, hints: Hints::default() };
    m.decls.fns[f].params.iter().map(|prm| c.base(&prm.slot, &prm.slot.ty)).collect()
}

/// The base values of `f`'s parameters, where the part at `at` is there to be set: a value of a sum
/// the path goes through (`o.placed.on`) is that value, its fields at their base values.
pub fn base_at(p: &Program, m: &Checked, f: usize, at: &Path) -> Vec<Value> {
    within(p, m, f, &base_input(p, m, f), at)
}

/// An input with the part at `at` there to be set: where the path goes through a value of a sum the
/// input does not hold, that value, its fields at their base values.
pub fn within(p: &Program, m: &Checked, f: usize, input: &[Value], at: &Path) -> Vec<Value> {
    fn reach(c: &Choose, v: &Value, t: &Ty, steps: &[Step]) -> Value {
        let mut t = t;
        while let Ty::Opt(x) = t {
            t = x;
        }
        let Some((st, rest)) = steps.split_first() else { return v.clone() };
        match (st, t, v) {
            (Step::Elem, Ty::List(x), Value::List(xs)) if !xs.is_empty() => {
                let mut ys = xs.clone();
                ys[0] = reach(c, &xs[0], x, rest);
                Value::List(ys)
            }
            (Step::Field(name), Ty::Record(id), Value::Record(fs) | Value::Variant(_, fs)) => {
                let Some((_, fslot)) = c.p.fields(id).into_iter().find(|(n, _)| n.text == *name) else { return v.clone() };
                let gs = fs.iter().map(|(n, x)| (n.clone(), if n == name { reach(c, x, &fslot.ty, rest) } else { x.clone() })).collect();
                match v {
                    Value::Variant(tag, _) => Value::Variant(tag.clone(), gs),
                    _ => Value::Record(gs),
                }
            }
            (Step::Value(name), Ty::Enum(id), _) => {
                let Some(r) = RecId::variant(id, name.clone()) else { return v.clone() };
                let w = if v.tag() == Some(name.as_str()) { v.clone() } else { c.variant_base(id, name) };
                reach(c, &w, &Ty::Record(r), rest)
            }
            (Step::Value(_), Ty::Record(_), _) => reach(c, v, t, rest),
            // a `result`: the `ok` or the `err` the path goes through, and the value it holds
            (Step::Value(name), Ty::Result(a, e), _) => {
                let (f, inner) = if name == "ok" { ("value", a) } else { ("error", e) };
                let w = match v {
                    Value::Variant(tag, _) if tag == name => v.clone(),
                    _ => Value::Variant(name.clone(), vec![(f.into(), c.base(&Slot::of((**inner).clone()), inner))]),
                };
                let Some((Step::Field(_), more)) = rest.split_first() else { return w };
                let Value::Variant(tag, fs) = &w else { return w };
                Value::Variant(tag.clone(), fs.iter().map(|(n, x)| (n.clone(), reach(c, x, inner, more))).collect())
            }
            _ => v.clone(),
        }
    }
    let c = Choose { p, hints: Hints::default() };
    let mut input = input.to_vec();
    if let Some(prm) = m.decls.fns[f].params.get(at.0)
        && at.0 < input.len()
    {
        input[at.0] = reach(&c, &input[at.0], &prm.slot.ty, &at.1);
    }
    input
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
            Step::Value(v) => {
                s.push('.');
                s.push_str(v);
            }
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
            (Step::Value(v), Ty::Enum(id)) => Slot::of(Ty::Record(RecId::variant(&id, v.clone())?)),
            (Step::Value(v), Ty::Record(r)) if matches!(&r, RecId::Variant { value, .. } if value == v) => Slot::of(Ty::Record(r)),
            // a `result`: the value its `ok` or its `err` holds (DESIGN 26.3)
            (Step::Value(_), t @ Ty::Result(..)) => Slot::of(t),
            (Step::Field(f), Ty::Result(a, e)) => Slot::of(if f == "value" { *a } else { *e }),
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
            (Step::Field(f), Value::Record(fs) | Value::Variant(_, fs)) => fs.iter().find(|(n, _)| n == f).map(|(_, x)| x)?,
            (Step::Value(name), Value::Variant(x, _)) if x == name => v,
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
                Value::Variant(x, fs) => {
                    let mut gs = fs.clone();
                    let k = gs.iter().position(|(n, _)| n == f)?;
                    gs[k].1 = put(&fs[k].1, rest, to)?;
                    Some(Value::Variant(x.clone(), gs))
                }
                _ => None,
            },
            // the value it is where it is that value; an input of another value has no such part
            Some((Step::Value(name), rest)) => match v {
                Value::Variant(x, _) if x == name => put(v, rest, to),
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
        // the base values, holding the value of a sum the part is in (DESIGN 23.6)
        let base = &within(p, m, f, base, atom);
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
