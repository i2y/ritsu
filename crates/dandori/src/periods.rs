//! The period a workflow runs in (`now range` under `workflow`, DESIGN 20), put into the checked
//! tree as statements that hold the clock to it when the workflow runs (`TK::Period`). Every
//! generator writes the statement for its platform, the reference interpreter runs it, and the
//! scenarios try both its ways, so the platforms are held to the interpreter on it as on
//! everything else.
//!
//! Where a check goes:
//!
//! - At the start of the flow (`start`): a run that starts outside the period fails there, before
//!   it calls anything, as a run whose input is outside its range fails; `on failure` does not run.
//! - Before each statement of the flow that reads `now`, in the blocks under calls, arms and loops
//!   too: the check reads the clock once, holds it to the period, and the statement reads what it
//!   read (`TExpr::NowAt` the check's site), so the moment checked is the moment used, and one
//!   statement reads one moment. A run outside the period goes to `on failure` there, as a task's
//!   error that no handler takes does; a loop's `yield` that reads `now` is checked at the end of
//!   each round, where it is read.
//! - Not in `on failure` and `on cancel`: what settles a run is not stopped by the clock. Their
//!   `now` is `now` with no period (`TExpr::Now`).

use crate::model::*;

/// Put the checks of the period into the flow, when the workflow writes one.
pub fn insert(m: &mut Model) {
    if m.period.is_none() {
        return;
    }
    let mut next = 0;
    for s in m.all_stmts() {
        next = next.max(s.site);
    }
    let mut ins = Inserter { next: next + 1 };
    let mut flow = std::mem::take(&mut m.flow);
    ins.block(&mut flow);
    let line = m.period.as_ref().map_or(1, |p| p.line);
    flow.insert(0, TStmt { kind: TK::Period { start: true }, line, site: ins.take() });
    m.flow = flow;
}

struct Inserter {
    next: usize,
}

impl Inserter {
    fn take(&mut self) -> usize {
        let site = self.next;
        self.next += 1;
        site
    }

    fn check(&mut self, line: usize) -> TStmt {
        TStmt { kind: TK::Period { start: false }, line, site: self.take() }
    }

    fn block(&mut self, ss: &mut Vec<TStmt>) {
        let mut out = Vec::with_capacity(ss.len());
        for mut s in std::mem::take(ss) {
            match &mut s.kind {
                TK::Call { handlers, .. } => handlers.iter_mut().for_each(|h| self.block(&mut h.body)),
                TK::Match { arms, .. } => arms.iter_mut().for_each(|a| self.block(&mut a.body)),
                TK::Repeat { body, .. } => self.block(body),
                TK::For { body, .. } => self.block(body),
                _ => {}
            }
            // a loop's yield is read at the end of each round: its check goes there. A round that
            // ends with `succeed`, `fail` or `break` never reads it, and the generators write no
            // `yield` after them
            if let TK::For { body, result: Some((_, y)), .. } = &mut s.kind {
                let ends = body.last().is_some_and(|x| matches!(x.kind, TK::Succeed { .. } | TK::Fail { .. } | TK::Break));
                if y.has_now() && !ends {
                    let c = self.check(s.line);
                    set_now(y, c.site);
                    body.push(c);
                }
            }
            let reads = match &s.kind {
                TK::For { list, .. } => list.has_now(),
                k => k.reads_now(),
            };
            if reads {
                let c = self.check(s.line);
                for e in exprs_mut(&mut s.kind) {
                    set_now(e, c.site);
                }
                out.push(c);
            }
            out.push(s);
        }
        *ss = out;
    }
}

/// The values a statement holds that the check before it reads `now` for: all of them, but a
/// loop's yield, which its own check at the end of each round reads it for.
fn exprs_mut(k: &mut TK) -> Vec<&mut TExpr> {
    match k {
        TK::Call { args, .. } => args.iter_mut().map(|(_, e)| e).collect(),
        TK::Assign { expr, .. } | TK::Match { expr, .. } => vec![expr],
        TK::WaitUntil { at } => vec![at],
        TK::For { list, .. } => vec![list],
        TK::Succeed { fields } => fields.iter_mut().map(|(_, e)| e).collect(),
        TK::Fail { cause, .. } => cause.iter_mut().collect(),
        _ => vec![],
    }
}

/// Every `now` in `e` reads what the check at `site` read.
fn set_now(e: &mut TExpr, site: usize) {
    match e {
        TExpr::Now => *e = TExpr::NowAt(site),
        TExpr::Record { fields, .. } => fields.iter_mut().for_each(|(_, x)| set_now(x, site)),
        TExpr::List { items, .. } => items.iter_mut().for_each(|x| set_now(x, site)),
        TExpr::Interp(parts) => parts.iter_mut().for_each(|p| {
            if let IPart::Hole(x) = p {
                set_now(x, site);
            }
        }),
        _ => {}
    }
}
