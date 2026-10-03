//! What a contract says about several fields at once (§15.140).
//!
//! §15.132 held a contract to a rule one field at a time: the range Protovalidate's `gte` and
//! `lte` give a number, the strings an `enum` lists. A contract also relates its fields to each
//! other — `this.min_weight <= this.max_weight` in a CEL expression on the message, a `oneof`
//! that lets one of its members be set, an `if`/`then` in a JSON Schema that caps the weight of
//! an express order — and a rule relates its inputs too, with `constraint` and with the rows of
//! its tables. This module is where the two meet: a condition over the fields of one contract,
//! atoms under `and` and `or`, and three questions asked of it.
//!
//! - **What can one field be?** The values of one field that some message satisfying the
//!   condition has. That is narrower than the field's own rules whenever another rule reaches
//!   it, and it is what E122 holds an input to.
//! - **Can the condition hold together with these?** Together with the cells of a row and the
//!   rule's ranges, perhaps not: the row is then reached only by messages the contract refuses
//!   (W124), and the multipliers that refute each case are the evidence.
//! - **Is there a message that satisfies it and breaks this?** A point in whole numbers that
//!   satisfies the condition, the rule's ranges and the negation of one of its `constraint`s is
//!   a message the contract lets through and the generated code refuses at the door (E123).
//!
//! **The condition is read wider than the contract, never narrower.** An atom this cannot read
//! is kept as `Unknown` while negations are pushed down to the atoms, and only after that is it
//! made true. A negation cannot turn it into a narrowing that way, and everything else follows:
//! a field can only look wider than it is, a row can only look reachable, a combination can only
//! look admitted. It is the direction every check of §15.132 took: this may speak where it need
//! not, and never stays quiet where it should speak.

use crate::fourier::{self, Ineq, Origin};
use crate::num::Rat;
use std::collections::{BTreeMap, BTreeSet};

/// What a condition can be about: a field reached from the root of the contract
/// (`shipping.zone` is `Field(["shipping", "zone"])`), or the number of elements of a repeated
/// one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Term {
    Field(Vec<String>),
    Size(Vec<String>),
}

impl Term {
    pub fn path(&self) -> &[String] {
        match self {
            Term::Field(p) | Term::Size(p) => p,
        }
    }

    /// Whether it lies at or under `prefix`.
    pub fn under(&self, prefix: &[String]) -> bool {
        self.path().starts_with(prefix)
    }

    /// How a message names it: `shipping.zone`, `size(lines)`.
    pub fn word(&self) -> String {
        match self {
            Term::Field(p) => p.join("."),
            Term::Size(p) => format!("size({})", p.join(".")),
        }
    }
}

/// A linear combination of terms, `Σ c·t + k`. The arithmetic is checked: a form whose numbers
/// outgrow 128 bits is not built, and whoever asked reads the atom as unknown.
#[derive(Debug, Clone, PartialEq)]
pub struct Lin {
    pub terms: BTreeMap<Term, Rat>,
    pub k: Rat,
}

impl Lin {
    pub fn con(k: Rat) -> Lin {
        Lin { terms: BTreeMap::new(), k }
    }

    pub fn term(t: Term) -> Lin {
        Lin { terms: BTreeMap::from([(t, Rat::int(1))]), k: Rat::int(0) }
    }

    pub fn scale(&self, f: Rat) -> Option<Lin> {
        let mut out = self.clone();
        for c in out.terms.values_mut() {
            *c = c.checked_mul(f)?;
        }
        out.k = out.k.checked_mul(f)?;
        out.terms.retain(|_, c| c.num != 0);
        Some(out)
    }

    pub fn plus(&self, o: &Lin) -> Option<Lin> {
        let mut out = self.clone();
        for (t, c) in &o.terms {
            let e = out.terms.entry(t.clone()).or_insert(Rat::int(0));
            *e = e.checked_add(*c)?;
        }
        out.terms.retain(|_, c| c.num != 0);
        out.k = out.k.checked_add(o.k)?;
        Some(out)
    }

    pub fn minus(&self, o: &Lin) -> Option<Lin> {
        self.plus(&o.scale(Rat::int(-1))?)
    }

    /// The value, when no term is left in it.
    pub fn constant(&self) -> Option<Rat> {
        self.terms.is_empty().then_some(self.k)
    }

    /// The same form over the names a linear system uses.
    pub fn named(&self, name: &dyn Fn(&Term) -> String) -> fourier::Lin {
        fourier::Lin { terms: self.terms.iter().map(|(t, c)| (name(t), *c)).collect(), k: self.k }
    }
}

/// How a linear form stands to zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rel {
    Le,
    Lt,
    Eq,
    Ne,
}

/// One condition a formula is built from.
#[derive(Debug, Clone, PartialEq)]
pub enum Atom {
    /// `lin <rel> 0`, over whole-number terms.
    Num(Lin, Rel),
    /// A string term is one of these (`true`), or none of them (`false`).
    Str(Term, Vec<String>, bool),
    /// A boolean term has this value.
    Bool(Term, bool),
    /// Something that was not read. It is made true once the negations are all on atoms.
    Unknown,
}

impl Atom {
    /// The atom that holds exactly where this one does not. Unknown stays unknown.
    pub fn not(&self) -> Atom {
        let neg = |l: &Lin| l.scale(Rat::int(-1));
        match self {
            // ¬(l ≤ 0) is l > 0, which is −l < 0; ¬(l < 0) is −l ≤ 0.
            Atom::Num(l, Rel::Le) => neg(l).map_or(Atom::Unknown, |n| Atom::Num(n, Rel::Lt)),
            Atom::Num(l, Rel::Lt) => neg(l).map_or(Atom::Unknown, |n| Atom::Num(n, Rel::Le)),
            Atom::Num(l, Rel::Eq) => Atom::Num(l.clone(), Rel::Ne),
            Atom::Num(l, Rel::Ne) => Atom::Num(l.clone(), Rel::Eq),
            Atom::Str(t, s, yes) => Atom::Str(t.clone(), s.clone(), !yes),
            Atom::Bool(t, v) => Atom::Bool(t.clone(), !v),
            Atom::Unknown => Atom::Unknown,
        }
    }

    fn terms(&self, out: &mut BTreeSet<Term>) {
        match self {
            Atom::Num(l, _) => out.extend(l.terms.keys().cloned()),
            Atom::Str(t, ..) | Atom::Bool(t, _) => {
                out.insert(t.clone());
            }
            Atom::Unknown => {}
        }
    }
}

/// A condition on a message: atoms under `and` and `or`, with every negation already on an
/// atom.
#[derive(Debug, Clone, PartialEq)]
pub enum Formula {
    True,
    False,
    Atom(Atom),
    And(Vec<Formula>),
    Or(Vec<Formula>),
}

impl Formula {
    /// An atom, folded to a constant when it has no term left in it, and a numeric one put in
    /// the form that says the most over whole numbers (§15.142). Every term a condition speaks
    /// about is a whole number — a field of an integer kind, or a count — so `a < b` is
    /// `a − b + 1 <= 0`, `2a <= 3` is `a <= 1`, `2a = 1` is false, and `a ≠ b` is the two
    /// cases `a − b <= −1` and `a − b >= 1`. Read this way, the rationals a refutation is found
    /// over have no point between two whole ones for a condition to slip through.
    pub fn atom(a: Atom) -> Formula {
        if let Atom::Num(l, rel) = &a {
            if !l.terms.is_empty() {
                if let Some(f) = whole_atom(l, *rel) {
                    return f;
                }
            }
        }
        let holds = match &a {
            Atom::Num(l, rel) => l.constant().map(|k| {
                let o = k.cmp_to(Rat::int(0));
                match rel {
                    Rel::Le => o.is_le(),
                    Rel::Lt => o.is_lt(),
                    Rel::Eq => o.is_eq(),
                    Rel::Ne => o.is_ne(),
                }
            }),
            // Nothing is in an empty set, and everything is outside it.
            Atom::Str(_, s, yes) if s.is_empty() => Some(!*yes),
            _ => None,
        };
        match holds {
            Some(true) => Formula::True,
            Some(false) => Formula::False,
            None => Formula::Atom(a),
        }
    }

    /// `a <rel> b`, as `a − b <rel> 0`. Unknown where the arithmetic does not fit.
    pub fn cmp(a: &Lin, rel: Rel, b: &Lin) -> Formula {
        match a.minus(b) {
            Some(d) => Formula::atom(Atom::Num(d, rel)),
            None => Formula::Atom(Atom::Unknown),
        }
    }

    pub fn unknown() -> Formula {
        Formula::Atom(Atom::Unknown)
    }

    pub fn and(v: Vec<Formula>) -> Formula {
        let mut out = Vec::new();
        for f in v {
            match f {
                Formula::True => {}
                Formula::False => return Formula::False,
                Formula::And(xs) => out.extend(xs),
                f => {
                    if !out.contains(&f) {
                        out.push(f)
                    }
                }
            }
        }
        match out.len() {
            0 => Formula::True,
            1 => out.pop().unwrap(),
            _ => Formula::And(out),
        }
    }

    pub fn or(v: Vec<Formula>) -> Formula {
        let mut out = Vec::new();
        for f in v {
            match f {
                Formula::False => {}
                Formula::True => return Formula::True,
                Formula::Or(xs) => out.extend(xs),
                f => {
                    if !out.contains(&f) {
                        out.push(f)
                    }
                }
            }
        }
        match out.len() {
            0 => Formula::False,
            1 => out.pop().unwrap(),
            _ => Formula::Or(out),
        }
    }

    /// The negation, pushed down to the atoms.
    pub fn not(&self) -> Formula {
        match self {
            Formula::True => Formula::False,
            Formula::False => Formula::True,
            Formula::Atom(a) => Formula::atom(a.not()),
            Formula::And(xs) => Formula::or(xs.iter().map(Formula::not).collect()),
            Formula::Or(xs) => Formula::and(xs.iter().map(Formula::not).collect()),
        }
    }

    /// Every unknown atom made true. Called once, on the whole condition: before that, a
    /// negation could still land on one.
    pub fn widen(&self) -> Formula {
        match self {
            Formula::Atom(Atom::Unknown) => Formula::True,
            Formula::And(xs) => Formula::and(xs.iter().map(Formula::widen).collect()),
            Formula::Or(xs) => Formula::or(xs.iter().map(Formula::widen).collect()),
            f => f.clone(),
        }
    }

    /// Whether some part of it went unread.
    pub fn has_unknown(&self) -> bool {
        match self {
            Formula::Atom(Atom::Unknown) => true,
            Formula::And(xs) | Formula::Or(xs) => xs.iter().any(Formula::has_unknown),
            _ => false,
        }
    }

    pub fn terms(&self, out: &mut BTreeSet<Term>) {
        match self {
            Formula::Atom(a) => a.terms(out),
            Formula::And(xs) | Formula::Or(xs) => xs.iter().for_each(|x| x.terms(out)),
            _ => {}
        }
    }
}

/// A numeric atom over whole-number terms, with whole coefficients that share no factor and
/// the constant rounded the way the inequality allows. `None` where the arithmetic outgrows
/// 128 bits, and the atom is then kept as it was.
fn whole_atom(l: &Lin, rel: Rel) -> Option<Formula> {
    let mut m: i128 = 1;
    for c in l.terms.values() {
        m = lcm(m, c.den)?;
    }
    let scaled = l.scale(Rat::int(m))?;
    let mut g: i128 = 0;
    for c in scaled.terms.values() {
        let (mut a, mut b) = (g.abs(), c.num.abs());
        while b != 0 {
            let t = a % b;
            a = b;
            b = t;
        }
        g = a;
    }
    if g == 0 {
        return None;
    }
    let base = scaled.scale(Rat::new(1, g))?;
    let with = |k: i128| Lin { terms: base.terms.clone(), k: Rat::int(k) };
    let neg = |k: i128| -> Option<Lin> { Some(Lin { terms: base.terms.iter().map(|(t, c)| (t.clone(), c.mul(Rat::int(-1)))).collect(), k: Rat::int(k) }) };
    let k = base.k;
    Some(match rel {
        Rel::Le => Formula::Atom(Atom::Num(with(ceil(k)), Rel::Le)),
        Rel::Lt => Formula::Atom(Atom::Num(with(floor(k).checked_add(1)?), Rel::Le)),
        Rel::Eq if k.is_int() => Formula::Atom(Atom::Num(with(k.num), Rel::Eq)),
        Rel::Eq => Formula::False,
        Rel::Ne if k.is_int() => Formula::or(vec![
            Formula::Atom(Atom::Num(with(k.num.checked_add(1)?), Rel::Le)),
            Formula::Atom(Atom::Num(neg(k.num.checked_mul(-1)?.checked_add(1)?)?, Rel::Le)),
        ]),
        Rel::Ne => Formula::True,
    })
}

/// How many cases a condition may open into before it is given up on. Each `or` under an
/// `and` multiplies them; a contract that goes past this is read as saying nothing across its
/// fields, which is the wide reading.
pub const CASES: usize = 256;

/// A condition, opened into the cases it is the disjunction of: each case is the conjunction of
/// the atoms it lists, by index into `atoms`.
#[derive(Debug, Clone)]
pub struct Relation {
    pub atoms: Vec<Atom>,
    /// `None` when the cases grew past `CASES`: nothing is known across the fields then.
    pub cases: Option<Vec<Vec<usize>>>,
}

/// Why one case cannot hold, or keeps what it is asked to.
#[derive(Debug, Clone)]
pub enum Proof {
    /// The strings or the truth values of one term cannot all hold at once.
    Clash(Term),
    /// The numbers cannot: the system, and the multipliers that refute it (§15.139).
    Farkas(fourier::Refutation),
    /// The strings the case lets a term be are all among the ones asked for (§15.142).
    Within,
}

/// What a search for a point found.
#[derive(Debug, Clone)]
pub enum Found {
    /// A whole value for every numeric name the case mentions, and the strings and truth
    /// values it fixes.
    Point(BTreeMap<String, Rat>, BTreeMap<Term, String>, BTreeMap<Term, bool>),
    /// There is none, and each case says why.
    None(Vec<Proof>),
    /// Neither could be settled within the limits.
    Unknown,
}

/// Conditions from the rule's side, held beside the contract's: linear ones over the names
/// the system uses, and the values an enum or a boolean input is tested for.
#[derive(Debug, Clone, Default)]
pub struct Extra {
    pub ineqs: Vec<Ineq>,
    pub strs: Vec<(Term, Vec<String>)>,
    pub bools: Vec<(Term, bool)>,
}

impl Relation {
    pub fn of(f: &Formula) -> Relation {
        let mut atoms = Vec::new();
        let cases = cases(&f.widen(), &mut atoms);
        Relation { atoms, cases }
    }

    /// A relation that says nothing: one case with no atoms.
    pub fn any() -> Relation {
        Relation { atoms: Vec::new(), cases: Some(vec![Vec::new()]) }
    }

    /// Whether it says anything at all across its fields.
    pub fn is_trivial(&self) -> bool {
        match &self.cases {
            None => true,
            Some(cs) => cs.iter().any(Vec::is_empty),
        }
    }

    /// The cases, or the single case with nothing in it when there were too many to open.
    fn each(&self) -> Vec<Vec<usize>> {
        self.cases.clone().unwrap_or_else(|| vec![Vec::new()])
    }

    /// One case taken apart: its linear inequalities (each tagged with the atom it came from),
    /// its disequalities, and the first term whose strings or truth values clash.
    fn split(&self, case: &[usize], x: &Extra, name: &dyn Fn(&Term) -> String) -> Split {
        let mut ineqs = Vec::new();
        let mut nes = Vec::new();
        let mut only: BTreeMap<Term, Vec<String>> = BTreeMap::new();
        let mut except: BTreeMap<Term, Vec<String>> = BTreeMap::new();
        let mut truth: BTreeMap<Term, bool> = BTreeMap::new();
        let mut clash = None;
        let mut narrow = |t: &Term, set: &[String], yes: bool, clash: &mut Option<Term>| {
            if yes {
                let e = only.entry(t.clone()).or_insert_with(|| set.to_vec());
                e.retain(|v| set.contains(v));
            } else {
                except.entry(t.clone()).or_default().extend(set.iter().cloned());
            }
            if let Some(o) = only.get(t) {
                let ex = except.get(t);
                if o.iter().all(|v| ex.is_some_and(|ex| ex.contains(v))) && clash.is_none() {
                    *clash = Some(t.clone());
                }
            }
        };
        for &i in case {
            match &self.atoms[i] {
                Atom::Num(l, rel) => {
                    let f = l.named(name);
                    let tag = |part| Origin::Contract { atom: i, part };
                    match rel {
                        Rel::Le => ineqs.push(f.le(false).tag(tag(0))),
                        Rel::Lt => ineqs.push(f.le(true).tag(tag(0))),
                        Rel::Eq => {
                            ineqs.push(f.clone().le(false).tag(tag(0)));
                            ineqs.push(f.ge(false).tag(tag(1)));
                        }
                        Rel::Ne => nes.push(f),
                    }
                }
                Atom::Str(t, set, yes) => narrow(t, set, *yes, &mut clash),
                Atom::Bool(t, v) => {
                    if truth.insert(t.clone(), *v).is_some_and(|old| old != *v) && clash.is_none() {
                        clash = Some(t.clone());
                    }
                }
                Atom::Unknown => {}
            }
        }
        for (t, set) in &x.strs {
            narrow(t, set, true, &mut clash);
        }
        for (t, v) in &x.bools {
            if truth.insert(t.clone(), *v).is_some_and(|old| old != *v) && clash.is_none() {
                clash = Some(t.clone());
            }
        }
        ineqs.extend(x.ineqs.iter().cloned());
        Split { ineqs, nes, only, except, truth, clash }
    }

    /// Whether every case fails together with `x`, and why each one does. `None` as soon as
    /// one case is not refuted — which covers a case that holds and a case that could not be
    /// decided.
    pub fn refute(&self, x: &Extra, name: &dyn Fn(&Term) -> String) -> Option<Vec<Proof>> {
        let mut out = Vec::new();
        for case in self.each() {
            let s = self.split(&case, x, name);
            if let Some(t) = s.clash {
                out.push(Proof::Clash(t));
                continue;
            }
            // A disequality is left out: the system without it is wider, so a refutation of
            // it refutes the case.
            out.push(Proof::Farkas(fourier::Refutation::of(s.ineqs)?));
        }
        Some(out)
    }

    /// A point that satisfies one of the cases together with `x`, in whole numbers.
    pub fn find(&self, x: &Extra, name: &dyn Fn(&Term) -> String) -> Found {
        let mut proofs = Vec::new();
        let mut unsure = false;
        for case in self.each() {
            let s = self.split(&case, x, name);
            if let Some(t) = s.clash {
                proofs.push(Proof::Clash(t));
                continue;
            }
            if let Some(r) = fourier::Refutation::of(s.ineqs.clone()) {
                proofs.push(Proof::Farkas(r));
                continue;
            }
            let mut budget = BRANCHES;
            match whole_point(s.ineqs.clone(), &s.nes, &mut budget) {
                Some(Some(at)) => {
                    let strs = s
                        .only
                        .iter()
                        .filter_map(|(t, vs)| {
                            let ex = s.except.get(t);
                            vs.iter().find(|v| !ex.is_some_and(|e| e.contains(v))).map(|v| (t.clone(), v.clone()))
                        })
                        .collect();
                    return Found::Point(at, strs, s.truth);
                }
                // No whole point in this case, settled by splitting it; there is no single
                // refutation to show for that, and none is needed to say there is no point.
                Some(None) => {}
                None => unsure = true,
            }
        }
        if unsure { Found::Unknown } else { Found::None(proofs) }
    }

    /// The whole values a numeric term takes across the cases: closed intervals, each with the
    /// points a disequality takes out of it. `None` when a case could not be taken apart, which
    /// a caller reads as "nothing learned".
    #[allow(clippy::type_complexity)]
    pub fn span(&self, t: &Term, x: &Extra, name: &dyn Fn(&Term) -> String) -> Option<Vec<(Option<i128>, Option<i128>, Vec<i128>)>> {
        let key = name(t);
        let mut out = Vec::new();
        for case in self.each() {
            let s = self.split(&case, x, name);
            if s.clash.is_some() {
                continue;
            }
            let Some((lo, hi)) = fourier::bounds(s.ineqs, &key)? else { continue };
            let lo = lo.map(|(l, strict)| {
                let c = ceil(l);
                if strict && Rat::int(c) == l { c + 1 } else { c }
            });
            let hi = hi.map(|(h, strict)| {
                let f = floor(h);
                if strict && Rat::int(f) == h { f - 1 } else { f }
            });
            if matches!((lo, hi), (Some(a), Some(b)) if a > b) {
                continue;
            }
            // A disequality on this term alone takes a point out; one over several terms is
            // left out, which only widens.
            let holes = s
                .nes
                .iter()
                .filter(|f| f.terms.len() == 1 && f.terms.contains_key(&key))
                .filter_map(|f| {
                    let c = f.terms[&key];
                    let v = f.k.checked_mul(Rat::int(-1))?.checked_div(c)?;
                    v.is_int().then_some(v.num)
                })
                .filter(|h| lo.is_none_or(|l| *h >= l) && hi.is_none_or(|x| *h <= x))
                .collect();
            out.push((lo, hi, holes));
        }
        Some(out)
    }

    /// The strings a term can be across the cases, when every case that can hold lists them;
    /// `None` when some case lets it be any string, or could not be decided.
    pub fn strings(&self, t: &Term, name: &dyn Fn(&Term) -> String) -> Option<Vec<String>> {
        let mut out: Vec<String> = Vec::new();
        for case in self.each() {
            let s = self.split(&case, &Extra::default(), name);
            if s.clash.is_some() || fourier::unsat(s.ineqs.clone()) {
                continue;
            }
            let ex = s.except.get(t);
            for v in s.only.get(t)? {
                if !ex.is_some_and(|e| e.contains(v)) && !out.contains(v) {
                    out.push(v.clone());
                }
            }
        }
        Some(out)
    }

    /// Why each case lets a string term be only one of `values` (§15.142): its strings or
    /// truth values clash, the strings it lists for the term are all among `values`, or its
    /// numbers cannot hold. `None` as soon as one case lets the term be something else.
    pub fn within(&self, t: &Term, values: &[String], name: &dyn Fn(&Term) -> String) -> Option<Vec<Proof>> {
        let mut out = Vec::new();
        for case in self.each() {
            let s = self.split(&case, &Extra::default(), name);
            if let Some(c) = s.clash {
                out.push(Proof::Clash(c));
                continue;
            }
            let ex = s.except.get(t);
            if s.only.get(t).is_some_and(|vs| vs.iter().all(|v| ex.is_some_and(|e| e.contains(v)) || values.contains(v))) {
                out.push(Proof::Within);
                continue;
            }
            out.push(Proof::Farkas(fourier::Refutation::of(s.ineqs)?));
        }
        Some(out)
    }

    /// Every term the relation mentions.
    pub fn terms(&self) -> BTreeSet<Term> {
        let mut out = BTreeSet::new();
        for a in &self.atoms {
            a.terms(&mut out);
        }
        out
    }
}

struct Split {
    ineqs: Vec<Ineq>,
    nes: Vec<fourier::Lin>,
    only: BTreeMap<Term, Vec<String>>,
    except: BTreeMap<Term, Vec<String>>,
    truth: BTreeMap<Term, bool>,
    clash: Option<Term>,
}

/// How many systems one search for a whole point may solve.
const BRANCHES: usize = 64;

/// A point in whole numbers that satisfies `sys` and every disequality, found by solving over
/// the rationals and, where the point is not whole or lands on a disequality, splitting the
/// system in two and trying each side. `Some(None)` when every side is refuted; `None` when the
/// budget ran out or the elimination gave up.
fn whole_point(sys: Vec<Ineq>, nes: &[fourier::Lin], budget: &mut usize) -> Option<Option<BTreeMap<String, Rat>>> {
    if *budget == 0 {
        return None;
    }
    *budget -= 1;
    if fourier::unsat(sys.clone()) {
        return Some(None);
    }
    let at = fourier::solve(sys.clone())?;
    // A name no inequality mentions has no value yet; any whole value will do.
    let mut at = at;
    for f in nes {
        for n in f.terms.keys() {
            at.entry(n.clone()).or_insert(Rat::int(0));
        }
    }
    if let Some((n, v)) = at.iter().find(|(_, v)| !v.is_int()) {
        let (lo, hi) = (floor(*v), ceil(*v));
        let below = fourier::Lin::var(n).plus(&fourier::Lin::con(Rat::int(-lo))).le(false);
        let above = fourier::Lin::var(n).plus(&fourier::Lin::con(Rat::int(-hi))).ge(false);
        return branch(sys, below, above, nes, budget);
    }
    for f in nes {
        let mut k = f.k;
        for (n, c) in &f.terms {
            k = k.checked_add(c.checked_mul(*at.get(n)?)?)?;
        }
        if k.num == 0 {
            // f ≠ 0 with f whole on whole points: f ≤ −1 or f ≥ 1 once it is scaled to whole
            // coefficients.
            let w = whole(f)?;
            let below = w.clone().plus(&fourier::Lin::con(Rat::int(1))).le(false);
            let above = w.plus(&fourier::Lin::con(Rat::int(-1))).ge(false);
            return branch(sys, below, above, nes, budget);
        }
    }
    Some(Some(at))
}

fn branch(sys: Vec<Ineq>, a: Ineq, b: Ineq, nes: &[fourier::Lin], budget: &mut usize) -> Option<Option<BTreeMap<String, Rat>>> {
    let mut left = sys.clone();
    left.push(a);
    let l = whole_point(left, nes, budget);
    if let Some(Some(p)) = l {
        return Some(Some(p));
    }
    let mut right = sys;
    right.push(b);
    match (l, whole_point(right, nes, budget)) {
        (_, Some(Some(p))) => Some(Some(p)),
        (Some(None), Some(None)) => Some(None),
        _ => None,
    }
}

/// A form scaled by a positive number until every coefficient and the constant are whole.
fn whole(f: &fourier::Lin) -> Option<fourier::Lin> {
    let mut l: i128 = 1;
    for c in f.terms.values().chain(std::iter::once(&f.k)) {
        l = lcm(l, c.den)?;
    }
    let mut out = f.clone();
    for c in out.terms.values_mut() {
        *c = c.checked_mul(Rat::int(l))?;
    }
    out.k = out.k.checked_mul(Rat::int(l))?;
    Some(out)
}

fn lcm(a: i128, b: i128) -> Option<i128> {
    let (mut x, mut y) = (a.abs(), b.abs());
    while y != 0 {
        let t = x % y;
        x = y;
        y = t;
    }
    if x == 0 { Some(0) } else { (a / x).checked_mul(b).map(i128::abs) }
}

fn floor(r: Rat) -> i128 {
    r.num.div_euclid(r.den)
}

fn ceil(r: Rat) -> i128 {
    -(-r.num).div_euclid(r.den)
}

/// The cases of a condition that is already widened, its atoms interned into `atoms`. A case
/// whose strings or truth values already clash is dropped as it is made.
fn cases(f: &Formula, atoms: &mut Vec<Atom>) -> Option<Vec<Vec<usize>>> {
    match f {
        Formula::True => Some(vec![Vec::new()]),
        Formula::False => Some(Vec::new()),
        Formula::Atom(a) => Some(vec![vec![intern(atoms, a)]]),
        Formula::Or(xs) => {
            let mut out = Vec::new();
            for x in xs {
                for c in cases(x, atoms)? {
                    if !out.contains(&c) {
                        out.push(c);
                    }
                }
                if out.len() > CASES {
                    return None;
                }
            }
            Some(out)
        }
        Formula::And(xs) => {
            let mut out: Vec<Vec<usize>> = vec![Vec::new()];
            for x in xs {
                let right = cases(x, atoms)?;
                let mut next = Vec::new();
                for l in &out {
                    for r in &right {
                        let mut c = l.clone();
                        for i in r {
                            if !c.contains(i) {
                                c.push(*i);
                            }
                        }
                        c.sort_unstable();
                        if !clashes(&c, atoms) && !next.contains(&c) {
                            next.push(c);
                        }
                        if next.len() > CASES {
                            return None;
                        }
                    }
                }
                out = next;
            }
            Some(out)
        }
    }
}

fn intern(atoms: &mut Vec<Atom>, a: &Atom) -> usize {
    match atoms.iter().position(|x| x == a) {
        Some(i) => i,
        None => {
            atoms.push(a.clone());
            atoms.len() - 1
        }
    }
}

/// Whether the strings or truth values of a case already contradict each other. The numbers are
/// the elimination's to judge.
fn clashes(case: &[usize], atoms: &[Atom]) -> bool {
    let mut only: BTreeMap<&Term, Vec<&String>> = BTreeMap::new();
    let mut except: BTreeMap<&Term, Vec<&String>> = BTreeMap::new();
    let mut truth: BTreeMap<&Term, bool> = BTreeMap::new();
    for &i in case {
        match &atoms[i] {
            Atom::Str(t, set, true) => {
                let e = only.entry(t).or_insert_with(|| set.iter().collect());
                e.retain(|v| set.contains(v));
            }
            Atom::Str(t, set, false) => except.entry(t).or_default().extend(set.iter()),
            Atom::Bool(t, v) => {
                if truth.insert(t, *v).is_some_and(|old| old != *v) {
                    return true;
                }
            }
            _ => {}
        }
    }
    only.iter().any(|(t, vs)| vs.iter().all(|v| except.get(t).is_some_and(|ex| ex.contains(v))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(p: &str) -> Term {
        Term::Field(p.split('.').map(str::to_string).collect())
    }

    fn le(a: &str, b: &str) -> Formula {
        Formula::cmp(&Lin::term(f(a)), Rel::Le, &Lin::term(f(b)))
    }

    fn at_most(a: &str, k: i128) -> Formula {
        Formula::cmp(&Lin::term(f(a)), Rel::Le, &Lin::con(Rat::int(k)))
    }

    fn at_least(a: &str, k: i128) -> Formula {
        Formula::cmp(&Lin::con(Rat::int(k)), Rel::Le, &Lin::term(f(a)))
    }

    fn key(t: &Term) -> String {
        t.word()
    }

    #[test]
    fn 否定は原子まで下りて_読めないものは読めないまま() {
        let g = Formula::and(vec![le("a", "b"), Formula::unknown()]);
        // ¬(a ≤ b ∧ U) = b < a ∨ U, and U widens to true: the negation says nothing.
        assert_eq!(g.not().widen(), Formula::True);
        // Widened first, the same negation would have claimed b < a: narrower than the truth.
        assert_ne!(g.widen().not(), Formula::True);
    }

    #[test]
    fn 一つのフィールドの幅は他のフィールドの規則で狭まる() {
        // a ≤ b, b ≤ 100, a ≥ 1: a is 1 to 100.
        let r = Relation::of(&Formula::and(vec![le("a", "b"), at_most("b", 100), at_least("a", 1)]));
        let got = r.span(&f("a"), &Extra::default(), &key).unwrap();
        assert_eq!(got, vec![(Some(1), Some(100), vec![])]);
    }

    #[test]
    fn 場合分けは和集合になる() {
        // (x = 0) ∨ (5 ≤ x ≤ 10), and x ≠ 7.
        let eq0 = Formula::cmp(&Lin::term(f("x")), Rel::Eq, &Lin::con(Rat::int(0)));
        let band = Formula::and(vec![at_least("x", 5), at_most("x", 10)]);
        let ne7 = Formula::cmp(&Lin::term(f("x")), Rel::Ne, &Lin::con(Rat::int(7)));
        let r = Relation::of(&Formula::and(vec![Formula::or(vec![eq0, band]), ne7]));
        let got = r.span(&f("x"), &Extra::default(), &key).unwrap();
        // `x ≠ 7` is the two cases `x <= 6` and `x >= 8` over whole numbers.
        assert!(got.contains(&(Some(0), Some(0), vec![])), "{got:?}");
        assert!(got.contains(&(Some(5), Some(6), vec![])), "{got:?}");
        assert!(got.contains(&(Some(8), Some(10), vec![])), "{got:?}");
    }

    #[test]
    fn 整数の条件は整数の形にする() {
        // 2x < 5 is x <= 2; 2x = 5 has no whole solution.
        let two_x = Lin::term(f("x")).scale(Rat::int(2)).unwrap();
        let lt = Formula::cmp(&two_x, Rel::Lt, &Lin::con(Rat::int(5)));
        let Formula::Atom(Atom::Num(l, Rel::Le)) = &lt else { panic!("{lt:?}") };
        assert_eq!((l.terms.get(&f("x")), l.k), (Some(&Rat::int(1)), Rat::int(-2)));
        assert_eq!(Formula::cmp(&two_x, Rel::Eq, &Lin::con(Rat::int(5))), Formula::False);
    }

    #[test]
    fn 行と両立しない条件は反駁が付く() {
        // The contract says a ≤ b; the row asks a > b.
        let r = Relation::of(&le("a", "b"));
        let row = fourier::Lin::var("a").plus(&fourier::Lin::var("b").scale(Rat::int(-1))).ge(true);
        let x = Extra { ineqs: vec![row], ..Extra::default() };
        let proofs = r.refute(&x, &key).expect("反駁できるはず");
        assert!(matches!(&proofs[..], [Proof::Farkas(_)]));
        // And a row that asks a < b is reachable.
        let row = fourier::Lin::var("a").plus(&fourier::Lin::var("b").scale(Rat::int(-1))).le(true);
        assert!(r.refute(&Extra { ineqs: vec![row], ..Extra::default() }, &key).is_none());
    }

    #[test]
    fn 文字列と真偽の食い違いも反駁になる() {
        let zone = Formula::atom(Atom::Str(f("zone"), vec!["honshu".into(), "okinawa".into()], true));
        let r = Relation::of(&zone);
        let x = Extra { strs: vec![(f("zone"), vec!["hokkaido".into()])], ..Extra::default() };
        assert!(matches!(r.refute(&x, &key).as_deref(), Some([Proof::Clash(_)])));
        assert_eq!(r.strings(&f("zone"), &key), Some(vec!["honshu".to_string(), "okinawa".to_string()]));
    }

    #[test]
    fn 整数の点を探す() {
        // 2a = 2b + 1 has rational points and no whole one.
        let two = |t: &str| Lin::term(f(t)).scale(Rat::int(2)).unwrap();
        let odd = Formula::cmp(&two("a"), Rel::Eq, &two("b").plus(&Lin::con(Rat::int(1))).unwrap());
        let r = Relation::of(&Formula::and(vec![odd, at_least("a", 0), at_most("a", 10)]));
        assert!(matches!(r.find(&Extra::default(), &key), Found::None(_) | Found::Unknown));
        // a ≤ b with a ≠ b and both in 0..1: the one point is a = 0, b = 1.
        let ne = Formula::cmp(&Lin::term(f("a")), Rel::Ne, &Lin::term(f("b")));
        let r = Relation::of(&Formula::and(vec![le("a", "b"), ne, at_least("a", 0), at_most("b", 1)]));
        match r.find(&Extra::default(), &key) {
            Found::Point(at, ..) => {
                assert_eq!(at.get("a"), Some(&Rat::int(0)));
                assert_eq!(at.get("b"), Some(&Rat::int(1)));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn 場合が多すぎれば何も言わない() {
        let many: Vec<Formula> = (0..10).map(|i| Formula::or(vec![at_most(&format!("x{i}"), 0), at_least(&format!("x{i}"), 5)])).collect();
        let r = Relation::of(&Formula::and(many));
        assert!(r.cases.is_none());
        assert!(r.is_trivial());
    }
}
