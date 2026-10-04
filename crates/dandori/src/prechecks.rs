//! The preconditions of rules that ritsu could not decide where the flow calls them (ritsu's X2,
//! its DESIGN 7.4 item 3; dandori's DESIGN 1.17), put into the checked tree as statements that
//! check them when the workflow runs (`TK::Check`). Every generator writes the statement for its
//! platform, the reference interpreter runs it, and the scenarios try both its ways, so the
//! platforms are held to the interpreter on it as on everything else.
//!
//! Where a check goes: as early as the run is sure to reach the call with the values the check
//! reads. From the call, it moves back over the statements before it in the same block while each
//! of them leaves those values alone and can only go on to the next statement or fail the run (a
//! `let` of another variable, a wait, a call whose handlers all go on); it stops after the
//! statement that makes one of the values (the task whose answer it is), or after one that may take
//! the run elsewhere (a `match`, a loop, a call with a handler that ends the run or leaves a loop),
//! or at the start of the block. Any earlier, and a run that would never call the rule (one that
//! takes another arm, or ends at a handler) would fail for values it never gives the rule.

use crate::model::*;
use ritsu_ports::{Precondition, UndecidedPrecondition};
use std::collections::BTreeSet;

/// The error a run fails with when its values break a precondition.
pub const ERROR: &str = "Dandori.BrokenPrecondition";

/// Put a check of each precondition in `undecided` into the flow, before the call it is for (by
/// the call's line and the rule's file), as early as it can go. A precondition of no call of the
/// flow is passed over.
pub fn insert(m: &mut Model, undecided: &[UndecidedPrecondition]) {
    if undecided.is_empty() {
        return;
    }
    let mut next = 0;
    for s in m.all_stmts() {
        next = next.max(s.site);
    }
    let mut ins = Inserter { m, undecided, next: next + 1 };
    let mut flow = std::mem::take(&mut ins.m.flow);
    ins.block(&mut flow);
    ins.m.flow = flow;
    if let Some(mut f) = ins.m.on_failure.take() {
        ins.block(&mut f);
        ins.m.on_failure = Some(f);
    }
    if let Some(mut f) = ins.m.on_cancel.take() {
        ins.block(&mut f);
        ins.m.on_cancel = Some(f);
    }
}

struct Inserter<'a> {
    m: &'a mut Model,
    undecided: &'a [UndecidedPrecondition],
    next: usize,
}

impl Inserter<'_> {
    fn block(&mut self, ss: &mut Vec<TStmt>) {
        for s in ss.iter_mut() {
            match &mut s.kind {
                TK::Call { handlers, .. } => handlers.iter_mut().for_each(|h| self.block(&mut h.body)),
                TK::Match { arms, .. } => arms.iter_mut().for_each(|a| self.block(&mut a.body)),
                TK::Repeat { body, .. } | TK::For { body, .. } => self.block(body),
                _ => {}
            }
        }
        // (where it goes, the check), in the order of the calls
        let mut put: Vec<(usize, TStmt)> = Vec::new();
        for (i, s) in ss.iter().enumerate() {
            let TK::Call { callee: Callee::Rule(r), args, .. } = &s.kind else { continue };
            let path = self.m.rules[*r].info.path.clone();
            for u in self.undecided.iter().filter(|u| u.line == s.line && u.rule == path) {
                let Some(test) = test_of(&u.precondition, args) else { continue };
                let check = PreCheck { rule: *r, call_line: s.line, test };
                let reads: BTreeSet<String> = check.exprs().iter().flat_map(|e| e.vars()).filter_map(|v| match v {
                    TExpr::Var { name, .. } => Some(name.clone()),
                    _ => None,
                }).collect();
                let at = place(self.m, ss, i, &reads);
                let site = self.next;
                self.next += 1;
                put.push((at, TStmt { kind: TK::Check(check), line: s.line, site }));
            }
        }
        // from the last place to the first, so that the places before it stay as they are; checks
        // that go to the same place keep the order of their calls
        put.sort_by_key(|(at, _)| *at);
        for (at, stmt) in put.into_iter().rev() {
            ss.insert(at, stmt);
        }
    }
}

/// The test of a precondition over the values a call gives: None when the call does not give an
/// input it reads, or the precondition is one dandori cannot check (a bound on a list, which no
/// rule dandori calls walks: its E005).
fn test_of(p: &Precondition, args: &[(String, TExpr)]) -> Option<PreTest> {
    let arg = |n: &str| args.iter().find(|(p, _)| p == n).map(|(_, e)| e.clone());
    match p {
        Precondition::Relation { left, op, right } => {
            if !matches!(op.as_str(), "<=" | "<" | ">=" | ">") {
                return None;
            }
            Some(PreTest::Relation { left: (left.clone(), arg(left)?), op: op.clone(), right: (right.clone(), arg(right)?) })
        }
        Precondition::Days { input, file, date, days } => Some(PreTest::Days { input: input.clone(), value: arg(input)?, file: file.clone(), date: date.clone(), days: days.iter().map(|d| civil(*d)).collect() }),
        Precondition::Sum { .. } | Precondition::Length { .. } => None,
    }
}

/// Where in the block `ss` the check of the call at `i` goes, reading the variables `reads`.
fn place(m: &Model, ss: &[TStmt], i: usize, reads: &BTreeSet<String>) -> usize {
    let mut at = i;
    while at > 0 {
        let x = &ss[at - 1];
        if sets(m, x, reads) || !passable(x) {
            break;
        }
        at -= 1;
    }
    at
}

/// Whether the statement (or a block under it) puts a value in one of the variables `names`.
fn sets(m: &Model, s: &TStmt, names: &BTreeSet<String>) -> bool {
    let set = match &s.kind {
        TK::Call { target: Some(Target::Let(x)), .. } => names.contains(x),
        TK::Call { target: Some(Target::Case(c)), .. } => names.contains(&m.cases[*c].name),
        TK::Assign { name, .. } => names.contains(name),
        TK::For { var, result, .. } => names.contains(var) || result.as_ref().is_some_and(|(r, _)| names.contains(r)),
        TK::Match { arms, .. } => arms.iter().any(|a| a.some.as_ref().is_some_and(|v| names.contains(v))),
        _ => false,
    };
    set || under(s).iter().any(|b| b.iter().any(|x| sets(m, x, names)))
}

/// The blocks under a statement.
fn under(s: &TStmt) -> Vec<&[TStmt]> {
    match &s.kind {
        TK::Call { handlers, .. } => handlers.iter().map(|h| h.body.as_slice()).collect(),
        TK::Match { arms, .. } => arms.iter().map(|a| a.body.as_slice()).collect(),
        TK::Repeat { body, .. } | TK::For { body, .. } => vec![body.as_slice()],
        _ => vec![],
    }
}

/// Whether a run that comes to the statement goes on to the next one, unless it fails: a `let`, a
/// wait, a `pass`, a call whose handlers neither end the run well nor leave a loop.
fn passable(s: &TStmt) -> bool {
    match &s.kind {
        TK::Assign { .. } | TK::Wait { .. } | TK::WaitUntil { .. } | TK::Pass => true,
        TK::Call { handlers, .. } => handlers.iter().all(|h| !leaves(&h.body)),
        _ => false,
    }
}

/// Whether a block can end the run well or leave a loop: a `succeed` or a `break` anywhere in it.
fn leaves(ss: &[TStmt]) -> bool {
    ss.iter().any(|s| matches!(s.kind, TK::Succeed { .. } | TK::Break) || under(s).iter().any(|b| leaves(b)))
}

/// A day number (days since 1970-01-01) as `YYYY-MM-DD` (Howard Hinnant's civil_from_days).
pub fn civil(days: i64) -> String {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if mo <= 2 { 1 } else { 0 };
    format!("{y:04}-{mo:02}-{d:02}")
}

/// Whether the check holds over these values (the reference interpreter's reading, which every
/// platform's code is held to): a relation over two whole numbers, a day among the days.
pub fn holds(t: &PreTest, value: &dyn Fn(&TExpr) -> serde_json::Value) -> bool {
    match t {
        PreTest::Relation { left, op, right } => {
            let (Some(l), Some(r)) = (value(&left.1).as_f64(), value(&right.1).as_f64()) else { return false };
            match op.as_str() {
                "<=" => l <= r,
                "<" => l < r,
                ">=" => l >= r,
                ">" => l > r,
                _ => false,
            }
        }
        PreTest::Days { value: v, days, .. } => value(v).as_str().is_some_and(|d| days.iter().any(|x| x == d)),
    }
}

#[cfg(test)]
mod tests {
    use super::civil;

    #[test]
    fn a_day_number_is_its_date() {
        assert_eq!(civil(0), "1970-01-01");
        assert_eq!(civil(20454), "2026-01-01");
        assert_eq!(civil(-1), "1969-12-31");
    }
}
