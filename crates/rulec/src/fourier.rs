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

use crate::num::Rat;
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
        let mut names: Vec<String> = Vec::new();
        for q in &sys {
            for (n, c) in &q.terms {
                if c.num != 0 && !names.contains(n) {
                    names.push(n.clone());
                }
            }
        }
        let Some(x) = names.into_iter().min_by_key(|n| {
            let (lo, hi) = count(&sys, n);
            lo * hi
        }) else {
            return false;
        };
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
