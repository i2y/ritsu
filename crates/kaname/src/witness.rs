//! The search for an example (DESIGN 5.7): where the analysis of ranges cannot show that a place
//! keeps what it takes, an input of the function that breaks it, run in the reference interpreter.
//!
//! The analysis is an estimate from above, so the end of an interval may be a value that is never
//! given; to say that a place can fail is to show an input that makes it fail. The inputs tried are
//! the function's vectors (DESIGN 9), and, for a place whose value is a sum of parameters
//! (`ranges::Lin`), the inputs that take that sum a step past each end of what the place takes. Each
//! is run with the interpreter watching the expressions the places are given by: a value given
//! that breaks what the place declares, or an operation that fails there, is an example (E301,
//! E302, E303). A place no input breaks is undecided (W301, W302, W303): the generated code checks it
//! when it runs.

use crate::check::Checked;
use crate::interp::{Interp, Kind, Stop, Watch, coerce};
use crate::ranges::{Outcome, Place, What, Witness};
use crate::suite::Suite;
use crate::ty::*;
use crate::value::{Program, Value};
use crate::vectors::{self, Vector};
use crate::num::{self, Rat};
use std::collections::BTreeSet;

/// For each place of the function `f` the analysis left undecided, an input that breaks it, when
/// one is found: its outcome becomes an example; the others stay undecided.
pub fn search(p: &Program, suite: &Suite, m: &Checked, f: usize, places: &mut [Place], bounds: &[crate::ranges::Boundary]) {
    let open: Vec<usize> = places.iter().enumerate().filter(|(_, x)| matches!(x.outcome, Outcome::Undecided)).map(|(i, _)| i).collect();
    if open.is_empty() {
        return;
    }
    let (mut inputs, _) = vectors::choose_with(p, m, f, bounds);
    let base = vectors::base_input(p, m, f);
    for &i in &open {
        for target in targets(&places[i]) {
            if let Some(lin) = &places[i].lin {
                for (input, _) in vectors::reach(p, m, f, &base, lin, target) {
                    inputs.push(Vector { input, why: None });
                }
            }
        }
    }
    let only: BTreeSet<(std::path::PathBuf, usize)> = open.iter().map(|i| (m.canon.clone(), places[*i].expr)).collect();
    let mut found: Vec<Option<Witness>> = vec![None; places.len()];
    let mut tried: BTreeSet<String> = BTreeSet::new();
    for v in inputs {
        if !tried.insert(format!("{:?}", v.input)) {
            continue;
        }
        let mut it = Interp::new(p, suite);
        if it.check_entry(m, f, &v.input).is_err() {
            continue;
        }
        it.watch = Some(Watch { only: Some(only.clone()), seen: Vec::new() });
        let out = it.call(m, f, v.input.clone(), 0);
        let failed = match &out {
            Err(Stop::Failed(fl)) if fl.file == m.path => fl.expr.map(|e| (fl.kind, e)),
            _ => None,
        };
        let seen = it.watch.take().map(|w| w.seen).unwrap_or_default();
        for &i in &open {
            if found[i].is_some() {
                continue;
            }
            if let Some(value) = breaks(&it, m, &places[i], &seen, failed) {
                found[i] = Some(Witness { input: v.input.clone(), value });
            }
        }
        if open.iter().all(|i| found[*i].is_some()) {
            break;
        }
    }
    for i in open {
        if let Some(w) = found[i].take() {
            places[i].outcome = Outcome::Example(Box::new(w));
        }
    }
}

/// The values a place's sum is taken to: a step past each end of what it takes, 0 for a divisor,
/// a step below 0 for `isqrt`, past int64 for an operation.
fn targets(pl: &Place) -> Vec<Rat> {
    let step = match &pl.ty {
        Ty::Num(n) if n.is_rate() => n.step.unwrap_or(Rat::new(1, 100)),
        _ => Rat::int(1),
    };
    let ends = |r: &Range| -> Vec<Rat> { [r.min.map(|v| v.sub(step)), r.max.map(|v| v.add(step))].into_iter().flatten().collect() };
    match &pl.what {
        What::RuleInput { range, .. } | What::DateInt { range, .. } => ends(range),
        What::Param { slot, .. } | What::Answer { slot, .. } | What::Field { slot, .. } => slot.range.as_ref().map(ends).unwrap_or_default(),
        What::Isqrt => vec![Rat::int(-1)],
        What::Zero => vec![Rat::zero()],
        What::Overflow => vec![Rat::int(i64::MAX as i128 + 1), Rat::int(i64::MIN as i128 - 1)],
        _ => Vec::new(),
    }
}

/// Whether a run broke the place: the value it was given there, read in the place's type, or the
/// operation that failed there. The value given, for a place that is given one.
fn breaks(it: &Interp, m: &Checked, pl: &Place, seen: &[(std::path::PathBuf, usize, Value)], failed: Option<(Kind, usize)>) -> Option<Option<Value>> {
    match &pl.what {
        What::Overflow => return (failed == Some((Kind::Overflow, pl.expr))).then_some(None),
        What::Zero => return (failed == Some((Kind::Zero, pl.expr))).then_some(None),
        _ => {}
    }
    let from = m.typed.types.get(pl.expr).cloned().unwrap_or(Ty::Error);
    for (file, id, v) in seen {
        if *file != m.canon || *id != pl.expr {
            continue;
        }
        let Ok(v) = coerce(v.clone(), &from, &pl.ty) else { continue };
        let broken = match &pl.what {
            What::RuleInput { range, .. } | What::DateInt { range, .. } => outside(&v, range),
            What::RuleEnum { values, .. } => matches!(&v, Value::Enum(x) if !values.contains(x)),
            What::RuleField { field, range, .. } => match &v {
                Value::List(xs) => xs.iter().any(|x| x.field(field).is_some_and(|f| outside(f, range))),
                _ => false,
            },
            What::Param { slot, .. } => !it.keeps(&v, slot, true),
            What::Answer { slot, .. } | What::Field { slot, .. } => !it.keeps(&v, slot, false),
            What::Isqrt => v.num().is_some_and(|r| r.num < 0),
            _ => false,
        };
        if broken {
            return Some(Some(v));
        }
    }
    None
}

/// Whether a number is outside a range (a value that is none is in every range).
fn outside(v: &Value, r: &Range) -> bool {
    let Some(x) = v.num() else { return false };
    r.min.is_some_and(|lo| num::cmp(x, lo).is_lt()) || r.max.is_some_and(|hi| num::cmp(x, hi).is_gt())
}
