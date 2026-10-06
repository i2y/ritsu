//! The reference evaluation (DESIGN P1, 4.1): what Cedar decides for one combination. A policy
//! applies when its scope and every `when` hold and no `unless` does; the request is denied when a
//! forbid applies, allowed when a permit applies and no forbid does, and denied when none applies.
//! The policies that decide it are the forbids that apply when it is denied, and the permits that
//! apply when it is allowed (Cedar's determining policies). This is the evaluation the checks and
//! the tests use; Cedar is what evaluates at run time.
//!
//! A condition is compiled for one frame of the walk (a principal type and a resource type of an
//! action, [`crate::walk::Frame`]) into a [`CExpr`] that reads the combination's values by their
//! place, so that walking a combination is a few comparisons.

use crate::ast::Op;
use crate::cells;
use crate::walk::Val;

/// One condition, compiled for a frame.
#[derive(Clone, Debug, PartialEq)]
pub enum CAtom {
    /// Decided by the frame alone (`principal is Customer` in a frame of `User`, an attribute the
    /// type does not have).
    Const(bool),
    /// The principal holds a role of the mask (after `includes`).
    Roles(u128),
    /// The workflow at this place is this one.
    Workflow(usize, usize),
    /// The bool at this place is this value (an absent one is neither).
    Bool(usize, bool),
    /// The enum at this place is this value.
    Enum(usize, usize),
    /// The number at this place (a cell, within which the comparison has one answer) compared
    /// with the constant.
    Cmp(usize, Op, i128),
    /// The two enums at these places are the same value.
    EnumEq(usize, usize),
    /// In the relation at this place, the two terms are the same entity.
    Same(usize, usize, usize),
    /// In the relation at this place, the principal is a member of what the term points to.
    Member(usize, usize),
}

#[derive(Clone, Debug, PartialEq)]
pub enum CExpr {
    Atom(CAtom),
    Not(Box<CExpr>),
    And(Vec<CExpr>),
    Or(Vec<CExpr>),
}

impl CExpr {
    pub fn truth(b: bool) -> CExpr {
        CExpr::Atom(CAtom::Const(b))
    }

    /// Whether it holds for the values of a combination.
    pub fn holds(&self, env: &[Val]) -> bool {
        match self {
            CExpr::Atom(a) => a.holds(env),
            CExpr::Not(e) => !e.holds(env),
            CExpr::And(es) => es.iter().all(|e| e.holds(env)),
            CExpr::Or(es) => es.iter().any(|e| e.holds(env)),
        }
    }

    /// The same expression with what the frame decides folded away.
    pub fn simplify(self) -> CExpr {
        match self {
            CExpr::Not(e) => match e.simplify() {
                CExpr::Atom(CAtom::Const(b)) => CExpr::truth(!b),
                e => CExpr::Not(Box::new(e)),
            },
            CExpr::And(es) => {
                let mut out = Vec::new();
                for e in es {
                    match e.simplify() {
                        CExpr::Atom(CAtom::Const(true)) => {}
                        CExpr::Atom(CAtom::Const(false)) => return CExpr::truth(false),
                        e => out.push(e),
                    }
                }
                match out.len() {
                    0 => CExpr::truth(true),
                    1 => out.pop().unwrap(),
                    _ => CExpr::And(out),
                }
            }
            CExpr::Or(es) => {
                let mut out = Vec::new();
                for e in es {
                    match e.simplify() {
                        CExpr::Atom(CAtom::Const(false)) => {}
                        CExpr::Atom(CAtom::Const(true)) => return CExpr::truth(true),
                        e => out.push(e),
                    }
                }
                match out.len() {
                    0 => CExpr::truth(false),
                    1 => out.pop().unwrap(),
                    _ => CExpr::Or(out),
                }
            }
            e => e,
        }
    }

    /// Whether the frame decides it whatever the values: Some(answer), or None.
    pub fn constant(&self) -> Option<bool> {
        match self {
            CExpr::Atom(CAtom::Const(b)) => Some(*b),
            _ => None,
        }
    }
}

impl CAtom {
    pub fn holds(&self, env: &[Val]) -> bool {
        match self {
            CAtom::Const(b) => *b,
            CAtom::Roles(mask) => matches!(&env[0], Val::Roles { effective, .. } if effective & mask != 0),
            CAtom::Workflow(at, w) => env[*at] == Val::Workflow(*w),
            CAtom::Bool(at, b) => env[*at] == Val::Bool(*b),
            CAtom::Enum(at, v) => env[*at] == Val::Enum(*v),
            CAtom::Cmp(at, op, k) => match &env[*at] {
                Val::Cell(c) => cells::holds(*op, c.lo, *k),
                _ => false,
            },
            CAtom::EnumEq(a, b) => matches!((&env[*a], &env[*b]), (Val::Enum(x), Val::Enum(y)) if x == y),
            CAtom::Same(at, i, j) => match &env[*at] {
                Val::Relation(r) => matches!((r.blocks[*i], r.blocks[*j]), (Some(x), Some(y)) if x == y),
                _ => false,
            },
            CAtom::Member(at, i) => match &env[*at] {
                Val::Relation(r) => r.member(*i),
                _ => false,
            },
        }
    }
}

/// What Cedar decides of one request.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Decision {
    pub allow: bool,
    /// The policies that decide it, by their place in the list the decision was made from.
    pub determining: Vec<usize>,
}

/// The decision, from which of the policies apply (`applies[i]`) and which of them are permits.
pub fn decide(applies: &[bool], permit: &[bool]) -> Decision {
    let forbids: Vec<usize> = (0..applies.len()).filter(|&i| applies[i] && !permit[i]).collect();
    if !forbids.is_empty() {
        return Decision { allow: false, determining: forbids };
    }
    let permits: Vec<usize> = (0..applies.len()).filter(|&i| applies[i] && permit[i]).collect();
    Decision { allow: !permits.is_empty(), determining: permits }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_forbid_wins_and_nothing_applying_denies() {
        let permit = [true, true, false];
        assert_eq!(decide(&[true, false, false], &permit), Decision { allow: true, determining: vec![0] });
        assert_eq!(decide(&[true, true, true], &permit), Decision { allow: false, determining: vec![2] });
        assert_eq!(decide(&[false, false, false], &permit), Decision { allow: false, determining: vec![] });
        assert_eq!(decide(&[true, true, false], &permit), Decision { allow: true, determining: vec![0, 1] });
    }

    #[test]
    fn what_the_frame_decides_folds_away() {
        let e = CExpr::And(vec![CExpr::truth(true), CExpr::Not(Box::new(CExpr::truth(false))), CExpr::Atom(CAtom::Bool(3, true))]);
        assert_eq!(e.simplify(), CExpr::Atom(CAtom::Bool(3, true)));
        let e = CExpr::Or(vec![CExpr::truth(false), CExpr::And(vec![CExpr::truth(false), CExpr::Atom(CAtom::Bool(1, true))])]);
        assert_eq!(e.simplify().constant(), Some(false));
    }
}
