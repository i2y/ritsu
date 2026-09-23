//! Fourier–Motzkin elimination, for the overlaps the per-axis sieve cannot decide (§15.126).
//!
//! The region analysis gives every derived value an axis of its own, bounded only by the
//! interval that value can reach. That is **sound and imprecise**: two derived values that
//! share an input are free to move independently there, and a pair of rows that can only
//! meet at a point no real input reaches still looks like an overlap. `check` does not call
//! such a pair an error — it says so with W114 and the generated code carries a runtime
//! guard — but a warning that will never fire is still a warning somebody has to read.
//!
//! A `derive` is a **linear combination of inputs** (§5), every cell is a unary comparison,
//! every range is an interval and every `constraint` is a comparison of two inputs. So the
//! question "can these two rows both match?" is a system of linear inequalities, and over
//! the rationals that is decidable by eliminating one variable at a time: pair every lower
//! bound on it with every upper bound, keep what falls out, repeat.
//!
//! **One direction only.** What this can settle is that no solution exists; when it finds
//! one, over the rationals, that solution may have no integer point in it, so nothing is
//! claimed and W114 stays exactly as it was. Dropping a condition it cannot read is safe for
//! the same reason: a relaxed system that is still unsatisfiable proves the original one is.

use crate::ast::{BinOp, CmpOp, Expr, Lit, RuleFile};
use crate::num::Rat;
use crate::types::{Checked, Ty};
use std::collections::BTreeMap;

/// Where one inequality of a system came from.
///
/// A refutation is a set of multipliers over the inequalities it combined (§15.139), and a
/// certificate that hands one over has to say what each of those inequalities was — which
/// cell of which row, which declared range, which `derive`, which `constraint` — so that a
/// re-checker can build it again from the rule and hold the multipliers to it without
/// trusting this module. `None` is a row nobody will be asked about.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Origin {
    #[default]
    None,
    /// One side of a `derive`'s defining equation: `name − expr <= 0` when `le`, `>= 0`
    /// otherwise.
    Derive { name: String, le: bool },
    /// One end of a declared range.
    Range { name: String, hi: bool },
    /// The `constraint` at this index in the file.
    Constraint(usize),
    /// One comparison of a row's cell: the row (0-based), the column, and which comparison of
    /// the cell (a literal cell is two, `<=` then `>=`).
    Cell { row: usize, col: String, part: usize },
    /// A boolean `define` the question pins to one truth value (§15.127), and which of the
    /// inequalities that truth value becomes.
    Pin { name: String, yes: bool, part: usize },
    /// An atom of a contract, by its index in the list the caller keeps (§15.139).
    Contract(usize),
}

/// One inequality: `Σ c·x + k < 0` when `strict`, `<= 0` otherwise.
#[derive(Debug, Clone)]
pub struct Ineq {
    pub terms: BTreeMap<String, Rat>,
    pub k: Rat,
    pub strict: bool,
    pub origin: Origin,
}

impl Ineq {
    /// The same inequality, saying where it came from.
    pub fn tag(mut self, origin: Origin) -> Ineq {
        self.origin = origin;
        self
    }

    fn constant(&self) -> bool {
        self.terms.values().all(|c| c.num == 0)
    }

    /// Whether the inequality holds at a point that gives every one of its names a value.
    /// `None` when a name has none, or the arithmetic does not fit.
    pub fn holds_at(&self, at: &BTreeMap<String, Rat>) -> Option<bool> {
        let mut k = self.k;
        for (n, c) in &self.terms {
            k = k.checked_add(c.checked_mul(*at.get(n)?)?)?;
        }
        let o = k.checked_cmp(Rat::int(0))?;
        Some(if self.strict { o.is_lt() } else { o.is_le() })
    }

    /// Whether a system with no variables left is already false here.
    fn false_now(&self) -> bool {
        let z = Rat::int(0);
        match self.k.cmp_to(z) {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Equal => self.strict,
            std::cmp::Ordering::Less => false,
        }
    }
}

/// A linear form being built: `Σ c·x + k`.
#[derive(Debug, Clone)]
pub struct Lin {
    pub terms: BTreeMap<String, Rat>,
    pub k: Rat,
}

impl Lin {
    pub fn con(k: Rat) -> Lin {
        Lin { terms: BTreeMap::new(), k }
    }

    pub fn var(n: &str) -> Lin {
        Lin { terms: BTreeMap::from([(n.to_string(), Rat::int(1))]), k: Rat::int(0) }
    }

    pub fn scale(mut self, f: Rat) -> Lin {
        for c in self.terms.values_mut() {
            *c = c.mul(f);
        }
        self.k = self.k.mul(f);
        self
    }

    pub fn plus(mut self, o: &Lin) -> Lin {
        for (n, c) in &o.terms {
            let e = self.terms.entry(n.clone()).or_insert(Rat::int(0));
            *e = e.add(*c);
        }
        self.k = self.k.add(o.k);
        self
    }

    /// `self <= 0`, or `< 0`.
    pub fn le(self, strict: bool) -> Ineq {
        Ineq { terms: self.terms, k: self.k, strict, origin: Origin::None }
    }

    /// `self >= 0`, or `> 0`, written the one way this module reads.
    pub fn ge(self, strict: bool) -> Ineq {
        self.scale(Rat::int(-1)).le(strict)
    }
}

/// An expression as a linear form, or `None` for anything that is not one.
///
/// §5 says a derived value is a linear combination of inputs, so this reads exactly that:
/// names and literals, added and subtracted, and multiplied or divided by a **constant**.
/// A product of two names is not linear and is refused rather than approximated.
pub fn linear(e: &Expr, want: &Ty, c: &Checked) -> Option<Lin> {
    
    match e {
        Expr::Name(n, _) => {
            let ty = c.ty_of(n)?;
            (ty == *want).then(|| Lin::var(n))
        }
        Expr::Lit(l, _) => lit_of(l, want).map(Lin::con),
        Expr::Bin(a, op, b, _) => match op {
            BinOp::Add => Some(linear(a, want, c)?.plus(&linear(b, want, c)?)),
            BinOp::Sub => Some(linear(a, want, c)?.plus(&linear(b, want, c)?.scale(Rat::int(-1)))),
            // One side has to be a constant, and a scalar carries no unit of its own: a rate
            // or a bare number reads as the factor it is.
            BinOp::Mul => match (factor(a), factor(b)) {
                (Some(k), None) => Some(linear(b, want, c)?.scale(k)),
                (None, Some(k)) => Some(linear(a, want, c)?.scale(k)),
                _ => None,
            },
            BinOp::Div => {
                let k = factor(b)?;
                (k.num != 0).then(|| linear(a, want, c).map(|l| l.scale(Rat::int(1).div(k))))?
            }
            _ => None,
        },
        Expr::Call(..) => None,
    }
}

/// A literal in the unit a system is written in. A date is its day number (§2.1).
pub fn lit_of(l: &Lit, want: &Ty) -> Option<Rat> {
    match l {
        Lit::Num(n) => crate::types::lit_value_in_pub(n, want),
        Lit::Date(y, m, d) if *want == Ty::Date => Some(crate::types::date_ord(*y, *m, *d)),
        _ => None,
    }
}

/// A literal with no unit, or a rate, as the factor it multiplies by.
fn factor(e: &Expr) -> Option<Rat> {
    let Expr::Lit(Lit::Num(n), _) = e else { return None };
    match n.unit.as_deref() {
        None => crate::types::lit_value_in_pub(n, &Ty::Number),
        Some("%") | Some("％") => crate::types::lit_value_in_pub(n, &Ty::Rate),
        _ => None,
    }
}


/// What one rule says about the names in a system, whatever is being asked about them: the
/// defining equation of every `derive`, the declared range of every name, and every
/// `constraint`. The caller adds the conditions it is asking about and eliminates.
pub struct Ground {
    /// The names the system quantifies over, all of one type.
    pub vars: Vec<String>,
    /// That type. Every literal a caller adds has to be read in it.
    pub want: Ty,
    pub sys: Vec<Ineq>,
}

/// The grounds of a question, one for each type of number among the names a caller cares
/// about, each closed over what its names are computed from.
///
/// A linear form carries no unit, so two names measured differently cannot share a system
/// without a conversion this does not make. What it does instead is keep them apart: the
/// grams in one system, the yen in another. **Each is a relaxation of the whole question** —
/// it keeps some of the conditions and drops the rest — so whichever of them turns out to
/// have no solution settles the question, and the ones that have a solution settle nothing.
/// The same reasoning drops a `derive` whose expression is not linear: its name stays in the
/// system, free inside its range, and only the equation that would have tied it down is
/// left out. §15.126 gave up on the whole question in both cases; there was no need to.
pub fn grounds(seed: &[String], f: &RuleFile, c: &Checked) -> Vec<Ground> {
    let mut types: Vec<Ty> = Vec::new();
    for n in seed {
        if let Some(ty) = c.ty_of(n) {
            if numeric(&ty) && !types.contains(&ty) {
                types.push(ty);
            }
        }
    }
    types.iter().filter_map(|want| ground_in(seed, want, f, c)).collect()
}

/// Whether a name of this type belongs in a linear system at all. A date is an ordinal and
/// does (§2.1); an enum, a bool or a string constrains nothing here.
fn numeric(ty: &Ty) -> bool {
    matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Number | Ty::Rate | Ty::Date)
}

fn ground_in(seed: &[String], want: &Ty, f: &RuleFile, c: &Checked) -> Option<Ground> {
    let expr_of = |n: &str| -> Option<&Expr> {
        f.items.iter().find_map(|it| match it {
            crate::ast::Item::Derived(d) if d.name.text == n => Some(&d.expr),
            _ => None,
        })
    };
    let mut vars: Vec<String> = Vec::new();
    let mut queue: Vec<String> = seed.to_vec();
    while let Some(n) = queue.pop() {
        if vars.contains(&n) || c.ty_of(&n).as_ref() != Some(want) {
            continue;
        }
        vars.push(n.clone());
        if let Some(l) = expr_of(&n).and_then(|e| linear(e, want, c)) {
            queue.extend(l.terms.keys().cloned());
        }
    }
    if vars.is_empty() {
        return None;
    }
    let mut sys: Vec<Ineq> = Vec::new();
    for n in &vars {
        if let Some(l) = expr_of(n).and_then(|e| linear(e, want, c)) {
            // Every name the expression reads is in the system, because the closure above put
            // it there; a name of another type made `linear` refuse, and then there is no
            // equation to add.
            let d = Lin::var(n).plus(&l.scale(Rat::int(-1)));
            sys.push(d.clone().le(false).tag(Origin::Derive { name: n.clone(), le: true }));
            sys.push(d.ge(false).tag(Origin::Derive { name: n.clone(), le: false }));
        }
        if let Some((lo, hi)) = c.ranges.get(n) {
            if let Some(lo) = lo {
                let q = Lin::var(n).plus(&Lin::con(lo.mul(Rat::int(-1)))).ge(false);
                sys.push(q.tag(Origin::Range { name: n.clone(), hi: false }));
            }
            if let Some(hi) = hi {
                let q = Lin::var(n).plus(&Lin::con(hi.mul(Rat::int(-1)))).le(false);
                sys.push(q.tag(Origin::Range { name: n.clone(), hi: true }));
            }
        }
    }
    for (i, k) in f.constraints.iter().enumerate() {
        if !vars.contains(&k.left) || !vars.contains(&k.right) {
            continue;
        }
        let d = Lin::var(&k.left).plus(&Lin::var(&k.right).scale(Rat::int(-1)));
        sys.push(cmp(d, k.op, false).tag(Origin::Constraint(i)));
    }
    Some(Ground { vars, want: want.clone(), sys })
}

/// `form <op> 0`, written the one way this module reads.
pub fn cmp(d: Lin, op: CmpOp, negate: bool) -> Ineq {
    let op = match (op, negate) {
        (CmpOp::Le, false) | (CmpOp::Gt, true) => CmpOp::Le,
        (CmpOp::Lt, false) | (CmpOp::Ge, true) => CmpOp::Lt,
        (CmpOp::Ge, false) | (CmpOp::Lt, true) => CmpOp::Ge,
        _ => CmpOp::Gt,
    };
    match op {
        CmpOp::Le => d.le(false),
        CmpOp::Lt => d.le(true),
        CmpOp::Ge => d.ge(false),
        CmpOp::Gt => d.ge(true),
    }
}

/// A boolean `define` that something has pinned to one truth value: its body, as the linear
/// condition it is there (§15.127). E113 limits the body to one name against one constant,
/// so there is a single comparison to read.
///
/// `=` holding is two inequalities; `=` failing is a disjunction, which one system cannot
/// carry, so nothing comes back for it.
pub fn pinned(body: &Expr, yes: bool, want: &Ty, c: &Checked) -> Vec<Ineq> {
    let Expr::Bin(a, op, b, _) = body else { return Vec::new() };
    let (Some(la), Some(lb)) = (linear(a, want, c), linear(b, want, c)) else { return Vec::new() };
    let d = la.plus(&lb.scale(Rat::int(-1)));
    match (op, yes) {
        (BinOp::Eq, true) => vec![d.clone().le(false), d.ge(false)],
        (BinOp::Eq, false) => Vec::new(),
        (op, _) => vec![cmp(d, cmp_of(*op), !yes)],
    }
}

fn cmp_of(op: BinOp) -> CmpOp {
    match op {
        BinOp::Le => CmpOp::Le,
        BinOp::Lt => CmpOp::Lt,
        BinOp::Ge => CmpOp::Ge,
        _ => CmpOp::Gt,
    }
}

/// How much work one elimination is allowed to make. Pairing every lower bound with every
/// upper bound squares the count in the worst case, so a system that grows past this is
/// abandoned — which costs nothing but the W114 that was going to be printed anyway.
pub const CAP: usize = 400;

/// Whether the system has **no** rational solution.
///
/// `false` means "not proven unsatisfiable", which is what a caller has to treat it as: it
/// covers a system with a solution, a system too big for the cap, a system whose arithmetic
/// outgrew 128 bits, and a system whose only obstruction is integrality.
pub fn unsat(sys: Vec<Ineq>) -> bool {
    refute(&sys).is_some()
}

/// The refutation of a system: one non-negative multiplier for each of its inequalities, such
/// that adding them up, each times its multiplier, cancels every variable and leaves a
/// constant that is false — positive, or zero where a strict inequality took part (§15.139).
///
/// That sum is the whole proof. It is a handful of rational numbers anybody can check by
/// addition, and checking it needs no trust in the elimination that found it, which is why a
/// certificate carries it rather than the conclusion. Finding it costs nothing extra: every
/// inequality the elimination makes is a positive combination of two it already had, so
/// carrying the combination along gives the multipliers of the first row that comes out
/// false. `None` means exactly what `unsat` returning `false` means. What comes back is held
/// to `farkas_holds` before it is returned, so a slip here costs a proof, never a wrong one.
pub fn refute(sys: &[Ineq]) -> Option<Vec<Rat>> {
    let mut rows: Vec<Row> = sys
        .iter()
        .enumerate()
        .map(|(i, q)| Row { q: q.clone(), y: BTreeMap::from([(i, Rat::int(1))]) })
        .collect();
    loop {
        if let Some(r) = rows.iter().find(|r| r.q.constant() && r.q.false_now()) {
            let y: Vec<Rat> = (0..sys.len()).map(|i| r.y.get(&i).copied().unwrap_or(Rat::int(0))).collect();
            return farkas_holds(sys, &y).then_some(y);
        }
        rows.retain(|r| !r.q.constant());
        let x = pick(&rows)?;
        let (lo, hi) = count(&rows, &x);
        if lo * hi > CAP || rows.len() > CAP {
            return None;
        }
        rows = eliminate(rows, &x, true)?;
        if rows.len() > CAP {
            return None;
        }
    }
}

/// A system and the multipliers that refute it, kept together so that whoever hands the
/// refutation on can say what each inequality was.
#[derive(Debug, Clone)]
pub struct Refutation {
    pub sys: Vec<Ineq>,
    pub y: Vec<Rat>,
}

impl Refutation {
    /// The refutation of `sys`, if elimination finds one.
    pub fn of(sys: Vec<Ineq>) -> Option<Refutation> {
        let y = refute(&sys)?;
        Some(Refutation { sys, y })
    }

    /// Only the inequalities that took part, each with its multiplier. The others add
    /// nothing to the sum and nothing a reader needs.
    pub fn used(&self) -> Vec<(&Ineq, Rat)> {
        self.sys.iter().zip(&self.y).filter(|(_, v)| v.num != 0).map(|(q, v)| (q, *v)).collect()
    }
}

/// Whether the multipliers really refute the system, checked by the addition they claim and
/// nothing else: every one is at least zero, the variables cancel, and what is left is false.
pub fn farkas_holds(sys: &[Ineq], y: &[Rat]) -> bool {
    let check = || -> Option<bool> {
        if y.len() != sys.len() || y.iter().any(|v| v.num < 0) {
            return Some(false);
        }
        let mut terms: BTreeMap<&str, Rat> = BTreeMap::new();
        let mut k = Rat::int(0);
        let mut strict = false;
        for (q, v) in sys.iter().zip(y) {
            if v.num == 0 {
                continue;
            }
            for (n, c) in &q.terms {
                let e = terms.entry(n.as_str()).or_insert(Rat::int(0));
                *e = e.checked_add(c.checked_mul(*v)?)?;
            }
            k = k.checked_add(q.k.checked_mul(*v)?)?;
            strict |= q.strict;
        }
        if terms.values().any(|c| c.num != 0) {
            return Some(false);
        }
        Some(match k.checked_cmp(Rat::int(0))? {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Equal => strict,
            std::cmp::Ordering::Less => false,
        })
    };
    check().unwrap_or(false)
}

/// An inequality in the middle of an elimination, with the multipliers that made it out of
/// the system's own inequalities (by index).
#[derive(Clone)]
struct Row {
    q: Ineq,
    y: BTreeMap<usize, Rat>,
}

/// A point that satisfies the system, or `None` when there is none to find.
///
/// The elimination that decides a system also produces one: eliminate the variables in
/// order, keeping what the system looked like before each step, then assign them back in
/// reverse — at each step every remaining variable already has a value, so the bounds on the
/// one being assigned are numbers. **A whole value is preferred**, because every value this
/// tool quantifies over is whole in its own unit (§2.1), and when the interval holds none
/// the rational midpoint comes back and the caller decides what to do with it.
///
/// Like `unsat`, this gives up rather than guesses: past the cap, or where the arithmetic
/// outgrows 128 bits, it returns `None`, which a caller has to read as "no point was found",
/// not as "none exists".
pub fn solve(sys: Vec<Ineq>) -> Option<BTreeMap<String, Rat>> {
    let mut stack: Vec<(String, Vec<Row>)> = Vec::new();
    let mut cur: Vec<Row> = sys.into_iter().map(|q| Row { q, y: BTreeMap::new() }).collect();
    loop {
        if cur.iter().any(|r| r.q.constant() && r.q.false_now()) {
            return None;
        }
        cur.retain(|r| !r.q.constant());
        let Some(x) = pick(&cur) else { break };
        let (lo, hi) = count(&cur, &x);
        if lo * hi > CAP || cur.len() > CAP || stack.len() > CAP {
            return None;
        }
        let next = eliminate(cur.clone(), &x, false)?;
        stack.push((x, cur));
        cur = next;
        if cur.len() > CAP {
            return None;
        }
    }
    let mut at: BTreeMap<String, Rat> = BTreeMap::new();
    while let Some((x, rows)) = stack.pop() {
        let mut lo: Option<(Rat, bool)> = None;
        let mut hi: Option<(Rat, bool)> = None;
        for r in &rows {
            let q = &r.q;
            let Some(cx) = q.terms.get(&x).copied().filter(|c| c.num != 0) else { continue };
            // Everything but `x` already has a value, so the rest of the row is a number. A
            // variable eliminated later than `x` would have none, which cannot happen for a
            // system taken apart in this order; refuse rather than invent one.
            let mut k = q.k;
            for (n, c) in &q.terms {
                if n != &x {
                    k = k.checked_add(c.checked_mul(*at.get(n)?)?)?;
                }
            }
            // cx·x + k ≤ 0 (or < 0)
            let bound = k.checked_mul(Rat::int(-1))?.checked_div(cx)?;
            // Two bounds at the same value keep the stricter of the two: `x <= 5` and `x < 5`
            // together are `x < 5`.
            let tighter = |old: Option<(Rat, bool)>, upper: bool| -> Option<(Rat, bool)> {
                Some(match old {
                    None => (bound, q.strict),
                    Some((b, bs)) => match b.checked_cmp(bound)? {
                        std::cmp::Ordering::Equal => (b, bs || q.strict),
                        std::cmp::Ordering::Less if upper => (b, bs),
                        std::cmp::Ordering::Greater if !upper => (b, bs),
                        _ => (bound, q.strict),
                    },
                })
            };
            if cx.num > 0 {
                hi = tighter(hi, true);
                hi.as_ref()?;
            } else {
                lo = tighter(lo, false);
                lo.as_ref()?;
            }
        }
        at.insert(x, between(lo, hi)?);
    }
    Some(at)
}

/// A value strictly inside the bounds where they are strict, and a whole one when the
/// interval holds one.
fn between(lo: Option<(Rat, bool)>, hi: Option<(Rat, bool)>) -> Option<Rat> {
    let one = Rat::int(1);
    let floor = |r: Rat| Rat::int(r.num.div_euclid(r.den));
    let ceil = |r: Rat| Rat::int(-((-r.num).div_euclid(r.den)));
    // The first whole value at or after `lo`, and the last at or before `hi`.
    let lo_i = match lo {
        Some((l, s)) => {
            let c = ceil(l);
            Some(if s && c.checked_cmp(l)? == std::cmp::Ordering::Equal { c.checked_add(one)? } else { c })
        }
        None => None,
    };
    let hi_i = match hi {
        Some((h, s)) => {
            let fl = floor(h);
            Some(if s && fl.checked_cmp(h)? == std::cmp::Ordering::Equal { fl.checked_sub(one)? } else { fl })
        }
        None => None,
    };
    match (lo_i, hi_i) {
        (Some(l), Some(h)) if l.checked_cmp(h)?.is_le() => return Some(l),
        (Some(l), None) => return Some(l),
        (None, Some(h)) => return Some(h),
        (None, None) => return Some(Rat::int(0)),
        _ => {}
    }
    // No whole value in it. The midpoint still satisfies the system, and a caller that needs
    // a whole one can see that this is not.
    match (lo, hi) {
        (Some((l, _)), Some((h, _))) if l.checked_cmp(h)? == std::cmp::Ordering::Less => {
            l.checked_add(h)?.checked_div(Rat::int(2))
        }
        _ => None,
    }
}

/// The variable to eliminate next: the one that pairs off smallest.
fn pick(rows: &[Row]) -> Option<String> {
    let mut names: Vec<String> = Vec::new();
    for r in rows {
        for (n, c) in &r.q.terms {
            if c.num != 0 && !names.contains(n) {
                names.push(n.clone());
            }
        }
    }
    names.into_iter().min_by_key(|n| {
        let (lo, hi) = count(rows, n);
        lo * hi
    })
}

/// How many inequalities bound `x` from each side.
fn count(rows: &[Row], x: &str) -> (usize, usize) {
    let mut lo = 0;
    let mut hi = 0;
    for r in rows {
        match r.q.terms.get(x).map(|c| c.num.signum()) {
            Some(1) => hi += 1,
            Some(-1) => lo += 1,
            _ => {}
        }
    }
    (lo, hi)
}

/// One variable out: every inequality that bounds it above, combined with every one that
/// bounds it below, and the ones that do not mention it kept as they are. `track` carries
/// the multipliers along (a refutation needs them; a solution does not). `None` where the
/// arithmetic outgrows 128 bits.
fn eliminate(rows: Vec<Row>, x: &str, track: bool) -> Option<Vec<Row>> {
    let (mut pos, mut neg, mut rest) = (Vec::new(), Vec::new(), Vec::new());
    for r in rows {
        match r.q.terms.get(x).map(|c| c.num.signum()) {
            Some(1) => pos.push(r),
            Some(-1) => neg.push(r),
            _ => rest.push(r),
        }
    }
    for p in &pos {
        for n in &neg {
            let a = *p.q.terms.get(x)?;
            let b = n.q.terms.get(x)?.checked_mul(Rat::int(-1))?;
            // b·p + a·n: the coefficient of x cancels, and both multipliers are positive so
            // the direction of each inequality is kept.
            let mut terms: BTreeMap<String, Rat> = BTreeMap::new();
            for (name, c) in &p.q.terms {
                let e = terms.entry(name.clone()).or_insert(Rat::int(0));
                *e = e.checked_add(c.checked_mul(b)?)?;
            }
            for (name, c) in &n.q.terms {
                let e = terms.entry(name.clone()).or_insert(Rat::int(0));
                *e = e.checked_add(c.checked_mul(a)?)?;
            }
            terms.remove(x);
            terms.retain(|_, c| c.num != 0);
            let k = p.q.k.checked_mul(b)?.checked_add(n.q.k.checked_mul(a)?)?;
            let mut y: BTreeMap<usize, Rat> = BTreeMap::new();
            if track {
                for (i, v) in &p.y {
                    let e = y.entry(*i).or_insert(Rat::int(0));
                    *e = e.checked_add(v.checked_mul(b)?)?;
                }
                for (i, v) in &n.y {
                    let e = y.entry(*i).or_insert(Rat::int(0));
                    *e = e.checked_add(v.checked_mul(a)?)?;
                }
            }
            let mut row = Row { q: Ineq { terms, k, strict: p.q.strict || n.q.strict, origin: Origin::None }, y };
            shrink(&mut row);
            rest.push(row);
            if rest.len() > CAP {
                return Some(rest);
            }
        }
    }
    Some(rest)
}

/// Divide a row by the largest positive number that leaves its coefficients whole, and its
/// multipliers with it. Scaling by a positive number changes neither what the row says nor
/// that the multipliers make it, and without this the numbers grow with every step.
fn shrink(r: &mut Row) {
    let nums = r.q.terms.values().chain(std::iter::once(&r.q.k)).filter(|c| c.num != 0);
    let mut g: i128 = 0;
    let mut l: i128 = 1;
    for c in nums {
        g = gcd_i(g, c.num);
        let Some(m) = lcm_i(l, c.den) else { return };
        l = m;
    }
    if g <= 0 {
        return;
    }
    let Some(f) = Rat::checked_new(l, g) else { return };
    let scaled = || -> Option<Row> {
        let mut q = r.q.clone();
        for c in q.terms.values_mut() {
            *c = c.checked_mul(f)?;
        }
        q.k = q.k.checked_mul(f)?;
        let mut y = BTreeMap::new();
        for (i, v) in &r.y {
            y.insert(*i, v.checked_mul(f)?);
        }
        Some(Row { q, y })
    };
    if let Some(s) = scaled() {
        *r = s;
    }
}

fn gcd_i(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.unsigned_abs(), b.unsigned_abs());
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    i128::try_from(a).unwrap_or(0)
}

fn lcm_i(a: i128, b: i128) -> Option<i128> {
    let g = gcd_i(a, b);
    if g == 0 {
        return Some(0);
    }
    (a / g).checked_mul(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(n: &str) -> Lin {
        Lin::var(n)
    }

    #[test]
    fn 矛盾する範囲は充足不能() {
        // x <= 1 and x >= 2
        let sys = vec![
            v("x").plus(&Lin::con(Rat::int(-1))).le(false),
            v("x").plus(&Lin::con(Rat::int(-2))).ge(false),
        ];
        assert!(unsat(sys));
    }

    #[test]
    fn 両立する範囲は充足可能() {
        let sys = vec![
            v("x").plus(&Lin::con(Rat::int(-2))).le(false),
            v("x").plus(&Lin::con(Rat::int(-1))).ge(false),
        ];
        assert!(!unsat(sys));
    }

    #[test]
    fn 厳密な不等号は端点を落とす() {
        // x < 1 and x > 1 has no solution; <= and >= do.
        assert!(unsat(vec![
            v("x").plus(&Lin::con(Rat::int(-1))).le(true),
            v("x").plus(&Lin::con(Rat::int(-1))).ge(true),
        ]));
        assert!(!unsat(vec![
            v("x").plus(&Lin::con(Rat::int(-1))).le(false),
            v("x").plus(&Lin::con(Rat::int(-1))).ge(false),
        ]));
    }

    #[test]
    fn 共有する入力ごしの矛盾が見える() {
        // a = t - x, b = t - x - y, x >= 0, y >= 0, t >= 0, a <= 1000, b >= 3980.
        // b <= a, so the last two cannot both hold. One variable is shared; the per-axis
        // sieve cannot see it and this can (§15.126).
        let eq = |name: &str, rhs: Lin| {
            let d = v(name).plus(&rhs.clone().scale(Rat::int(-1)));
            vec![d.clone().le(false), d.ge(false)]
        };
        let mut sys = eq("a", v("t").plus(&v("x").scale(Rat::int(-1))));
        sys.extend(eq("b", v("t").plus(&v("x").scale(Rat::int(-1))).plus(&v("y").scale(Rat::int(-1)))));
        sys.push(v("x").ge(false));
        sys.push(v("y").ge(false));
        sys.push(v("t").ge(false));
        sys.push(v("a").plus(&Lin::con(Rat::int(-1000))).le(false));
        sys.push(v("b").plus(&Lin::con(Rat::int(-3980))).ge(false));
        assert!(unsat(sys));
    }

    #[test]
    fn 反駁の係数は足し算で確かめられる() {
        // The same system as above: the multipliers that come back cancel every variable and
        // leave a false constant, and every one of them is at least zero.
        let eq = |name: &str, rhs: Lin| {
            let d = v(name).plus(&rhs.clone().scale(Rat::int(-1)));
            vec![d.clone().le(false), d.ge(false)]
        };
        let mut sys = eq("a", v("t").plus(&v("x").scale(Rat::int(-1))));
        sys.extend(eq("b", v("t").plus(&v("x").scale(Rat::int(-1))).plus(&v("y").scale(Rat::int(-1)))));
        sys.push(v("x").ge(false));
        sys.push(v("y").ge(false));
        sys.push(v("t").ge(false));
        sys.push(v("a").plus(&Lin::con(Rat::int(-1000))).le(false));
        sys.push(v("b").plus(&Lin::con(Rat::int(-3980))).ge(false));
        let y = refute(&sys).expect("充足不能のはず");
        assert!(farkas_holds(&sys, &y), "{y:?}");
        assert!(y.iter().all(|v| v.num >= 0));
        // Multipliers that do not add up are refused, whatever produced them.
        let mut bad = y.clone();
        let i = bad.iter().position(|v| v.num > 0).unwrap();
        bad[i] = Rat::int(0);
        assert!(!farkas_holds(&sys, &bad));
        assert!(!farkas_holds(&sys, &vec![Rat::int(-1); sys.len()]));
    }

    #[test]
    fn 厳密さも係数で運ばれる() {
        // x < 1 and x > 1: the sum is 0 < 0, false only because a strict inequality took part.
        let sys = vec![
            v("x").plus(&Lin::con(Rat::int(-1))).le(true),
            v("x").plus(&Lin::con(Rat::int(-1))).ge(true),
        ];
        let y = refute(&sys).expect("充足不能のはず");
        assert!(farkas_holds(&sys, &y));
        // The same multipliers over the non-strict pair prove nothing.
        let loose = vec![
            v("x").plus(&Lin::con(Rat::int(-1))).le(false),
            v("x").plus(&Lin::con(Rat::int(-1))).ge(false),
        ];
        assert!(!farkas_holds(&loose, &y));
    }

    #[test]
    fn 桁あふれは証明ではなく降参になる() {
        // x >= 0 and p·x + 1 <= 0 is unsatisfiable, but eliminating x multiplies two numbers
        // near 2^100. A wrapped product would be a wrong answer that looks like a right one;
        // what comes back is either nothing or multipliers that really add up.
        let p = Rat::int((1i128 << 100) + 7);
        let q = Rat::int((1i128 << 100) - 3);
        let sys = vec![
            v("x").scale(p).plus(&v("y")).plus(&Lin::con(Rat::int(1))).le(false),
            v("x").scale(q.mul(Rat::int(-1))).plus(&v("y").scale(Rat::int(-1))).le(false),
        ];
        if let Some(y) = refute(&sys) {
            assert!(farkas_holds(&sys, &y), "{y:?}");
        }
        assert_eq!(Rat::int(i128::MAX).checked_mul(Rat::int(2)), None);
    }

    #[test]
    fn 消せない系は証明できないと答える() {
        // Nothing contradictory: not unsat, and it has to say so rather than guess.
        let sys = vec![v("a").plus(&v("b")).le(false), v("a").ge(false)];
        assert!(!unsat(sys));
    }
}

#[cfg(test)]
mod solve_tests {
    use super::*;

    fn v(n: &str) -> Lin {
        Lin::var(n)
    }

    /// Every inequality of the system holds at the point.
    fn holds(sys: &[Ineq], at: &BTreeMap<String, Rat>) -> bool {
        sys.iter().all(|q| {
            let mut k = q.k;
            for (n, c) in &q.terms {
                let Some(x) = at.get(n) else { return false };
                k = k.add(c.mul(*x));
            }
            let o = k.cmp_to(Rat::int(0));
            if q.strict { o == std::cmp::Ordering::Less } else { o.is_le() }
        })
    }

    #[test]
    fn 解が出て_本当に系を満たす() {
        // 0 < total < 1000000, a = total, b = total, 1000 < a, b < 3980.
        let eq = |name: &str, rhs: Lin| {
            let d = v(name).plus(&rhs.scale(Rat::int(-1)));
            vec![d.clone().le(false), d.ge(false)]
        };
        let mut sys = eq("a", v("t"));
        sys.extend(eq("b", v("t")));
        sys.push(v("t").ge(true));
        sys.push(v("t").plus(&Lin::con(Rat::int(-1_000_000))).le(true));
        sys.push(v("a").plus(&Lin::con(Rat::int(-1000))).ge(true));
        sys.push(v("b").plus(&Lin::con(Rat::int(-3980))).le(true));
        let at = solve(sys.clone()).expect("解があるはず");
        assert!(holds(&sys, &at), "{at:?}");
        assert!(at["t"].is_int(), "整数を選ぶはず: {:?}", at["t"]);
    }

    #[test]
    fn 解の無い系には解を出さない() {
        let sys = vec![
            v("x").plus(&Lin::con(Rat::int(-1))).le(false),
            v("x").plus(&Lin::con(Rat::int(-2))).ge(false),
        ];
        assert!(solve(sys).is_none());
    }

    #[test]
    fn 同じ値の厳密な上限は厳密なまま解く() {
        // x <= 5 and x < 5 together are x < 5; the point that comes back has to say so.
        let sys = vec![
            v("x").plus(&Lin::con(Rat::int(-5))).le(false),
            v("x").plus(&Lin::con(Rat::int(-5))).le(true),
            v("x").ge(false),
        ];
        let at = solve(sys.clone()).expect("解があるはず");
        assert!(holds(&sys, &at), "{at:?}");
        // And the same with the bounds the other way round, and from below.
        let sys = vec![
            v("x").plus(&Lin::con(Rat::int(-5))).le(true),
            v("x").plus(&Lin::con(Rat::int(-5))).le(false),
            v("x").plus(&Lin::con(Rat::int(-2))).ge(false),
            v("x").plus(&Lin::con(Rat::int(-2))).ge(true),
        ];
        let at = solve(sys.clone()).expect("解があるはず");
        assert!(holds(&sys, &at), "{at:?}");
    }

    /// Where the answer is decidable, the two agree: a system a point comes back for is one
    /// `unsat` says nothing about, and one `unsat` settles has no point.
    #[test]
    fn 解の有無と充足不能は食い違わない() {
        for (a, b) in [(0i128, 0), (1000, 500), (500, 1000), (3980, 1000), (7, 7)] {
            let sys = vec![
                v("x").plus(&Lin::con(Rat::int(-a))).le(false),
                v("x").plus(&Lin::con(Rat::int(-b))).ge(false),
            ];
            let no = unsat(sys.clone());
            let some = solve(sys).is_some();
            assert!(!(no && some), "a={a} b={b}: 充足不能なのに点が出た");
        }
    }
}
