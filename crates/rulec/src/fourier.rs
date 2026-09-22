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

/// One inequality: `Σ c·x + k < 0` when `strict`, `<= 0` otherwise.
#[derive(Debug, Clone)]
pub struct Ineq {
    pub terms: BTreeMap<String, Rat>,
    pub k: Rat,
    pub strict: bool,
}

impl Ineq {
    fn constant(&self) -> bool {
        self.terms.values().all(|c| c.num == 0)
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
        Ineq { terms: self.terms, k: self.k, strict }
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

/// The ground of a system, seeded with the names a caller cares about and closed over what
/// they are computed from. `None` when the names do not share one type — a linear form has
/// no units in it, so two names measured differently cannot go into one system without a
/// conversion this does not do — or when an expression is not linear.
pub fn ground(seed: &[String], f: &RuleFile, c: &Checked) -> Option<Ground> {
    let expr_of = |n: &str| -> Option<&Expr> {
        f.items.iter().find_map(|it| match it {
            crate::ast::Item::Derived(d) if d.name.text == n => Some(&d.expr),
            _ => None,
        })
    };
    let mut want: Option<Ty> = None;
    let mut vars: Vec<String> = Vec::new();
    let mut queue: Vec<String> = seed.to_vec();
    while let Some(n) = queue.pop() {
        if vars.contains(&n) {
            continue;
        }
        let Some(ty) = c.ty_of(&n) else { continue };
        // A name this arithmetic has no place for — an enum, a bool, a string — constrains
        // nothing here and is simply left out. A date is an ordinal and does belong (§2.1).
        if !matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Number | Ty::Rate | Ty::Date) {
            continue;
        }
        match &want {
            None => want = Some(ty.clone()),
            Some(w) if *w == ty => {}
            Some(_) => return None,
        }
        vars.push(n.clone());
        if let Some(e) = expr_of(&n) {
            let l = linear(e, &ty, c)?;
            queue.extend(l.terms.keys().cloned());
        }
    }
    let want = want?;
    let mut sys: Vec<Ineq> = Vec::new();
    for n in &vars {
        if let Some(e) = expr_of(n) {
            let l = linear(e, &want, c)?;
            let d = Lin::var(n).plus(&l.scale(Rat::int(-1)));
            sys.push(d.clone().le(false));
            sys.push(d.ge(false));
        }
        if let Some((lo, hi)) = c.ranges.get(n) {
            if let Some(lo) = lo {
                sys.push(Lin::var(n).plus(&Lin::con(lo.mul(Rat::int(-1)))).ge(false));
            }
            if let Some(hi) = hi {
                sys.push(Lin::var(n).plus(&Lin::con(hi.mul(Rat::int(-1)))).le(false));
            }
        }
    }
    for k in &f.constraints {
        if !vars.contains(&k.left) || !vars.contains(&k.right) {
            continue;
        }
        let d = Lin::var(&k.left).plus(&Lin::var(&k.right).scale(Rat::int(-1)));
        sys.push(cmp(d, k.op, false));
    }
    Some(Ground { vars, want, sys })
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
/// covers a system with a solution, a system too big for the cap, and a system whose only
/// obstruction is integrality.
pub fn unsat(mut sys: Vec<Ineq>) -> bool {
    // The variables, eliminated in the order that keeps the system smallest: the one with
    // the fewest pairings first.
    loop {
        sys.retain(|q| !q.constant() || !q.k.cmp_to(Rat::int(0)).is_le() || q.strict);
        if sys.iter().any(|q| q.constant() && q.false_now()) {
            return true;
        }
        sys.retain(|q| !q.constant());
        let Some(x) = pick(&sys) else { return false };
        let (lo, hi) = count(&sys, &x);
        if lo * hi > CAP || sys.len() > CAP {
            return false;
        }
        sys = eliminate(sys, &x);
        if sys.len() > CAP {
            return false;
        }
    }
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
/// Like `unsat`, this gives up rather than guesses: past the cap it returns `None`, which a
/// caller has to read as "no point was found", not as "none exists".
pub fn solve(sys: Vec<Ineq>) -> Option<BTreeMap<String, Rat>> {
    let mut stack: Vec<(String, Vec<Ineq>)> = Vec::new();
    let mut cur = sys;
    loop {
        if cur.iter().any(|q| q.constant() && q.false_now()) {
            return None;
        }
        cur.retain(|q| !q.constant());
        let Some(x) = pick(&cur) else { break };
        let (lo, hi) = count(&cur, &x);
        if lo * hi > CAP || cur.len() > CAP || stack.len() > CAP {
            return None;
        }
        let next = eliminate(cur.clone(), &x);
        stack.push((x, cur));
        cur = next;
        if cur.len() > CAP {
            return None;
        }
    }
    let mut at: BTreeMap<String, Rat> = BTreeMap::new();
    while let Some((x, sys)) = stack.pop() {
        let mut lo: Option<(Rat, bool)> = None;
        let mut hi: Option<(Rat, bool)> = None;
        for q in &sys {
            let Some(cx) = q.terms.get(&x).copied().filter(|c| c.num != 0) else { continue };
            // Everything but `x` already has a value, so the rest of the row is a number.
            let mut k = q.k;
            let mut open = false;
            for (n, c) in &q.terms {
                if n == &x {
                    continue;
                }
                match at.get(n) {
                    Some(v) => k = k.add(c.mul(*v)),
                    // A variable eliminated later than `x` has no value yet, which cannot
                    // happen for a system taken apart in this order; refuse rather than
                    // invent one.
                    None => {
                        open = true;
                        break;
                    }
                }
            }
            if open {
                return None;
            }
            // cx·x + k ≤ 0 (or < 0)
            let bound = k.mul(Rat::int(-1)).div(cx);
            if cx.num > 0 {
                hi = Some(match hi {
                    Some((h, hs)) if h.cmp_to(bound).is_le() => (h, hs),
                    Some((h, _)) if h.cmp_to(bound) == std::cmp::Ordering::Equal => (h, true),
                    _ => (bound, q.strict),
                });
            } else {
                lo = Some(match lo {
                    Some((l, ls)) if l.cmp_to(bound).is_ge() => (l, ls),
                    _ => (bound, q.strict),
                });
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
    let lo_i = lo.map(|(l, s)| {
        let c = ceil(l);
        if s && c.cmp_to(l) == std::cmp::Ordering::Equal { c.add(one) } else { c }
    });
    let hi_i = hi.map(|(h, s)| {
        let fl = floor(h);
        if s && fl.cmp_to(h) == std::cmp::Ordering::Equal { fl.sub(one) } else { fl }
    });
    match (lo_i, hi_i) {
        (Some(l), Some(h)) if l.cmp_to(h).is_le() => return Some(l),
        (Some(l), None) => return Some(l),
        (None, Some(h)) => return Some(h),
        (None, None) => return Some(Rat::int(0)),
        _ => {}
    }
    // No whole value in it. The midpoint still satisfies the system, and a caller that needs
    // a whole one can see that this is not.
    match (lo, hi) {
        (Some((l, _)), Some((h, _))) if l.cmp_to(h) == std::cmp::Ordering::Less => {
            Some(l.add(h).div(Rat::int(2)))
        }
        _ => None,
    }
}

/// The variable to eliminate next: the one that pairs off smallest.
fn pick(sys: &[Ineq]) -> Option<String> {
    let mut names: Vec<String> = Vec::new();
    for q in sys {
        for (n, c) in &q.terms {
            if c.num != 0 && !names.contains(n) {
                names.push(n.clone());
            }
        }
    }
    names.into_iter().min_by_key(|n| {
        let (lo, hi) = count(sys, n);
        lo * hi
    })
}

/// How many inequalities bound `x` from each side.
fn count(sys: &[Ineq], x: &str) -> (usize, usize) {
    let mut lo = 0;
    let mut hi = 0;
    for q in sys {
        match q.terms.get(x).map(|c| c.num.signum()) {
            Some(1) => hi += 1,
            Some(-1) => lo += 1,
            _ => {}
        }
    }
    (lo, hi)
}

/// One variable out: every inequality that bounds it above, combined with every one that
/// bounds it below, and the ones that do not mention it kept as they are.
fn eliminate(sys: Vec<Ineq>, x: &str) -> Vec<Ineq> {
    let (mut pos, mut neg, mut rest) = (Vec::new(), Vec::new(), Vec::new());
    for q in sys {
        match q.terms.get(x).map(|c| c.num.signum()) {
            Some(1) => pos.push(q),
            Some(-1) => neg.push(q),
            _ => rest.push(q),
        }
    }
    for p in &pos {
        for n in &neg {
            let (a, b) = (*p.terms.get(x).unwrap(), n.terms.get(x).unwrap().mul(Rat::int(-1)));
            // b·p + a·n: the coefficient of x cancels, and both multipliers are positive so
            // the direction of each inequality is kept.
            let mut terms: BTreeMap<String, Rat> = BTreeMap::new();
            for (name, c) in &p.terms {
                let e = terms.entry(name.clone()).or_insert(Rat::int(0));
                *e = e.add(c.mul(b));
            }
            for (name, c) in &n.terms {
                let e = terms.entry(name.clone()).or_insert(Rat::int(0));
                *e = e.add(c.mul(a));
            }
            terms.remove(x);
            terms.retain(|_, c| c.num != 0);
            rest.push(Ineq { terms, k: p.k.mul(b).add(n.k.mul(a)), strict: p.strict || n.strict });
            if rest.len() > CAP {
                return rest;
            }
        }
    }
    rest
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
