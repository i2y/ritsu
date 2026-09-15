//! Replay (§10.3, §10.4).
//!
//! There are two.
//!
//! - **replay** — applies the rule to past records and compares against the values actually
//!   produced at the time (`observed`). The counterpart is a dead record rather than a live
//!   process, so no adapter is needed.
//! - **diff** — applies two versions of the rule to the same records and reports **how many
//!   change and by how much**. The stage that turns "deploy, then look at the numbers" into
//!   "look before deploying".
//!
//! After the comparison both hand over to the shared machinery of §10.4 (fired-row clusters,
//! Δ statistics, rounding-difference tags). The only differences are who the counterpart is,
//! and that replay alone needs the filled records separated out.

use crate::ast::RuleFile;
use crate::eval;
use crate::fixtures::{Load, Manifest};
use crate::report::{wire, Fired, Mismatch, Report};
use crate::types::Checked;

/// Run the records through the rule and compare against `observed`.
pub fn replay(f: &RuleFile, c: &Checked, l: &Load, m: &Manifest, source: &str) -> Report {
    let mut rep = Report::new(f, &tr!("観測", "observed"));
    rep.impl_id = source.into();
    rep.fills_used = m.shown.clone();
    if l.dropped > 0 {
        rep.excluded.push(("missing_field", tr!("欄が欠けていて既定値も無い", "missing a field that has no default value"), l.dropped));
    }
    if !l.problems.is_empty() {
        rep.excluded.push(("bad_format", tr!("形式が宣言と食い違う", "not matching the declared format"), l.problems.len()));
    }

    for (id, r) in l.records.iter().enumerate() {
        let (outs, _, fired, _) = eval::run_all_traced(f, c, r.input.clone().into_iter().collect());
        let pairs: Vec<(String, Option<crate::eval::Val>, Option<String>)> = outs
            .into_iter()
            .map(|(n, v)| {
                let theirs = wire(c, &n, r.observed.get(&n));
                (n, v, theirs)
            })
            .collect();
        let same = pairs.iter().all(|(n, a, b)| wire(c, n, a.as_ref()) == *b);

        // §10.3: observed and filled records are always tallied separately; the headline
        // match rate comes from the observed records only.
        if r.filled.is_empty() {
            rep.total += 1;
            if same {
                rep.agreed += 1;
            }
        } else {
            rep.filled_total += 1;
            if same {
                rep.filled_agreed += 1;
            }
            for n in &r.filled {
                *rep.filled.entry(n.clone()).or_insert(0) += 1;
            }
        }
        // A record that carries the rows that matched (the generated code's record function
        // writes them, §15.35) is compared row by row as well: its key is the move from the
        // recorded row to the rule's, in the shape `diff` uses, so a row that moved reads as
        // `行2→行5`. A record without a trace is keyed on the rule's rows alone, as before.
        let key = if r.trace.is_empty() {
            fired.iter().map(|(t, r)| Fired::One { table: t.clone(), row: *r }).collect()
        } else {
            transition(&r.trace, &fired)
        };
        let rows_moved = !r.trace.is_empty() && r.trace != fired;
        if !same {
            rep.mismatches.push(Mismatch { id, tag: r.tag.clone(), input: r.input.clone(), outs: pairs, err: None, fired: key });
        } else if rows_moved {
            rep.moved.push(Mismatch { id, tag: r.tag.clone(), input: r.input.clone(), outs: pairs, err: None, fired: key });
        }
    }
    rep
}

/// Apply two versions to the same records and report how many change and by what amounts.
/// The cluster key is the **transition of the fired rows on both sides** (`old row → new row`).
/// As §10.4 says, the old output value is not part of the key: for a table lookup the pair of
/// rows already determines the pair of values, so it adds no information, and for a computed
/// output the clusters would split once per distinct value and the summary would be useless.
pub fn diff(
    old: (&RuleFile, &Checked),
    new: (&RuleFile, &Checked),
    l: &Load,
    m: &Manifest,
    label: (&str, &str),
) -> Report {
    let mut rep = Report::new(new.0, &tr!("旧版", "old version"));
    rep.impl_id = format!("{} → {}", label.0, label.1);
    rep.fills_used = m.shown.clone();
    if l.dropped > 0 {
        rep.excluded.push(("missing_field", tr!("欄が欠けていて既定値も無い", "missing a field that has no default value"), l.dropped));
    }
    if !l.problems.is_empty() {
        rep.excluded.push(("bad_format", tr!("形式が宣言と食い違う", "not matching the declared format"), l.problems.len()));
    }

    for (id, r) in l.records.iter().enumerate() {
        let ins: std::collections::HashMap<_, _> = r.input.clone().into_iter().collect();
        let (o_outs, _, o_fired, _) = eval::run_all_traced(old.0, old.1, ins.clone());
        let (n_outs, _, n_fired, _) = eval::run_all_traced(new.0, new.1, ins);

        let pairs: Vec<(String, Option<crate::eval::Val>, Option<String>)> = n_outs
            .into_iter()
            .map(|(n, v)| {
                let theirs =
                    o_outs.iter().find(|(m, _)| *m == n).and_then(|(_, v)| wire(new.1, &n, v.as_ref()));
                (n, v, theirs)
            })
            .collect();
        let same = pairs.iter().all(|(n, a, b)| wire(new.1, n, a.as_ref()) == *b);

        if r.filled.is_empty() {
            rep.total += 1;
            if same {
                rep.agreed += 1;
            }
        } else {
            rep.filled_total += 1;
            if same {
                rep.filled_agreed += 1;
            }
            for n in &r.filled {
                *rep.filled.entry(n.clone()).or_insert(0) += 1;
            }
        }
        if !same {
            // "row 3" when the row stayed, "row 3→row 5" when it moved. What the reader is
            // looking for is the row that moved.
            rep.mismatches.push(Mismatch {
                id,
                tag: r.tag.clone(),
                input: r.input.clone(),
                outs: pairs,
                err: None,
                fired: transition(&o_fired, &n_fired),
            });
        }
    }
    rep
}

/// Pair up the rows the two versions fired, table by table. The label (`表 基本送料 行2→行5`)
/// is rendered from this afterwards — the transition itself is data, so nothing has to read a
/// row number back out of a sentence.
fn transition(old: &[(String, usize)], new: &[(String, usize)]) -> Vec<Fired> {
    let mut out = Vec::new();
    let mut i = 0;
    let mut j = 0;
    while i < old.len() || j < new.len() {
        match (old.get(i), new.get(j)) {
            (Some((ta, ra)), Some((tb, rb))) if ta == tb => {
                out.push(Fired::Moved { table: ta.clone(), from: Some(*ra), to: Some(*rb) });
                i += 1;
                j += 1;
            }
            // A version that added or removed a table itself. Emit both sides without
            // disturbing the order.
            (Some((ta, ra)), _) => {
                out.push(Fired::Moved { table: ta.clone(), from: Some(*ra), to: None });
                i += 1;
            }
            (None, Some((tb, rb))) => {
                out.push(Fired::Moved { table: tb.clone(), from: None, to: Some(*rb) });
                j += 1;
            }
            (None, None) => break,
        }
    }
    out
}
