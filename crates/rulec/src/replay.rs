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
        rep.excluded.push(("missing_field", tr!("フィールドが欠けていて既定値も無い", "missing a field that has no default value"), l.dropped));
    }
    if !l.problems.is_empty() {
        rep.excluded.push(("bad_format", tr!("形式が宣言と食い違う", "not matching the declared format"), l.problems.len()));
    }

    for r in l.records.iter() {
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
            transition(&r.trace, &fired, None)
        };
        let rows_moved = !r.trace.is_empty() && !same_rows(&r.trace, &r.trace_labels, &fired, c);
        if !same {
            rep.mismatches.push(Mismatch { line: r.line, tag: r.tag.clone(), input: r.input.clone(), outs: pairs, err: None, fired: key });
        } else if rows_moved {
            rep.moved.push(Mismatch { line: r.line, tag: r.tag.clone(), input: r.input.clone(), outs: pairs, err: None, fired: key });
        }
    }
    // A machine's records as cases (§15.148): what was recorded is what came before.
    rep.cases = cases((f, c), l, &|r, _| f.outputs.iter().map(|o| (o.name.text.clone(), wire(c, &o.name.text, r.observed.get(&o.name.text)))).collect());
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
        rep.excluded.push(("missing_field", tr!("フィールドが欠けていて既定値も無い", "missing a field that has no default value"), l.dropped));
    }
    if !l.problems.is_empty() {
        rep.excluded.push(("bad_format", tr!("形式が宣言と食い違う", "not matching the declared format"), l.problems.len()));
    }

    for r in l.records.iter() {
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
                line: r.line,
                tag: r.tag.clone(),
                input: r.input.clone(),
                outs: pairs,
                err: None,
                fired: transition(&o_fired, &n_fired, Some((old.1, new.1))),
            });
        }
    }
    // A machine's records as cases (§15.148), played by the new version with the old one's
    // answer as what came before. Until the two part they carry the same state, so the first
    // call where they part is found exactly.
    rep.cases = cases(new, l, &|_, ins| {
        let (outs, _, _, _) = eval::run_all_traced(old.0, old.1, ins.clone());
        outs.iter().map(|(n, v)| (n.clone(), wire(new.1, n, v.as_ref()))).collect()
    });
    rep
}

/// Pair up the rows the two versions fired, table by table. The label (`表 基本送料 行2→行5`)
/// is rendered from this afterwards — the transition itself is data, so nothing has to read a
/// row number back out of a sentence.
///
/// A labelled row is the same row whatever its number: when both versions label the rows
/// and the labels agree, the pair reads as unmoved (the new number stands for both).
fn transition(old: &[(String, usize)], new: &[(String, usize)], cs: Option<(&Checked, &Checked)>) -> Vec<Fired> {
    let mut out = Vec::new();
    let mut i = 0;
    let mut j = 0;
    while i < old.len() || j < new.len() {
        match (old.get(i), new.get(j)) {
            (Some((ta, ra)), Some((tb, rb))) if ta == tb => {
                let same_label = cs.is_some_and(|(oc, nc)| {
                    matches!((oc.label_of(ta, *ra), nc.label_of(tb, *rb)), (Some(a), Some(b)) if a == b)
                });
                let from = if same_label { Some(*rb) } else { Some(*ra) };
                out.push(Fired::Moved { table: ta.clone(), from, to: Some(*rb) });
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

/// Whether the rows a record carries are the rule's rows: the same tables in the same order,
/// and for each the same label when the record has one and the rule labels the row, else the
/// same number.
fn same_rows(rec: &[(String, usize)], labels: &[Option<String>], fired: &[(String, usize)], c: &Checked) -> bool {
    rec.len() == fired.len()
        && rec.iter().enumerate().zip(fired).all(|((k, (ta, ra)), (tb, rb))| {
            ta == tb
                && match (labels.get(k).and_then(|l| l.as_deref()), c.label_of(tb, *rb)) {
                    (Some(a), Some(b)) => a == b,
                    _ => ra == rb,
                }
        })
}


/// A machine's records as cases (§15.148). The records that share a `tag` are one case's
/// calls in the order they came in. The case is played again from its first record — its
/// recorded state — and each later call is passed the state `now` answered to the call before,
/// not the state the record says; `before` gives, per record, what the call answered before
/// (the record's own `observed` for `replay`, the old version's answer for `diff`).
pub fn cases(
    now: (&RuleFile, &Checked),
    l: &Load,
    before: &dyn Fn(&crate::fixtures::Record, &std::collections::HashMap<String, crate::eval::Val>) -> Vec<(String, Option<String>)>,
) -> Option<crate::report::Cases> {
    let (f, c) = now;
    let m = f.machine.as_ref()?;
    let (cin, cout) = m.carried()?;
    let a = crate::machine::analyze(f, c, crate::region::DEFAULT_BUDGET as usize)?;
    // Whether a case can still finish depends on the world it is in: the coordinates of its
    // `held` inputs (§15.149).
    let can_finish = a.can_finish();
    // The cases, in the order their first record came in.
    let mut order: Vec<String> = Vec::new();
    let mut by: std::collections::BTreeMap<String, Vec<&crate::fixtures::Record>> = std::collections::BTreeMap::new();
    for r in &l.records {
        if r.tag.is_empty() {
            continue;
        }
        if !by.contains_key(&r.tag) {
            order.push(r.tag.clone());
        }
        by.entry(r.tag.clone()).or_default().push(r);
    }
    // A record the version cannot read at all refuses its case at that line.
    let mut refused_at: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for pb in &l.problems {
        if !pb.tag.is_empty() {
            let e = refused_at.entry(pb.tag.clone()).or_insert(pb.line);
            *e = (*e).min(pb.line);
            if !by.contains_key(&pb.tag) && !order.contains(&pb.tag) {
                order.push(pb.tag.clone());
            }
        }
    }
    if order.is_empty() {
        return None;
    }
    let mut cs = crate::report::Cases { total: order.len(), ..Default::default() };
    for tag in &order {
        // A case is told by what happens to it first: a call answered differently before the
        // record the version refuses is a case that diverged, and one after it is never made.
        let refused = refused_at.get(tag).copied();
        let recs: &[&crate::fixtures::Record] = by.get(tag).map(|v| v.as_slice()).unwrap_or(&[]);
        let mut state: Option<crate::eval::Val> = None;
        let mut parted: Option<usize> = None;
        let mut refused = refused;
        let mut kept: Option<Vec<Option<crate::eval::Val>>> = None;
        for r in recs {
            if refused.is_some_and(|rl| rl < r.line) {
                break;
            }
            // A case holds its `held` inputs (§15.149); a record that changes one is not a
            // call of the same case, and the version refuses the case there.
            let now: Vec<Option<crate::eval::Val>> = m.held.iter().map(|n| r.input.get(&n.text).cloned()).collect();
            match &kept {
                None => kept = Some(now),
                Some(was) if *was != now => {
                    refused = Some(refused.map_or(r.line, |rl| rl.min(r.line)));
                    break;
                }
                Some(_) => {}
            }
            let mut ins: std::collections::HashMap<String, crate::eval::Val> = r.input.clone().into_iter().collect();
            if let Some(st) = &state {
                // The call is passed what this version answered to the one before.
                if ins.get(cin) != Some(st) && parted.is_none() {
                    parted = Some(r.line);
                }
                ins.insert(cin.to_string(), st.clone());
            }
            let (outs, _, _, _) = eval::run_all_traced(f, c, ins.clone());
            let mine: Vec<(String, Option<String>)> = outs.iter().map(|(n, v)| (n.clone(), wire(c, n, v.as_ref()))).collect();
            if parted.is_none() && mine != before(r, &ins) {
                parted = Some(r.line);
            }
            state = outs.iter().find(|(n, _)| n == cout).and_then(|(_, v)| v.clone());
            if state.is_none() {
                break;
            }
        }
        match (parted, refused) {
            (Some(line), _) => cs.diverged.push((tag.clone(), line)),
            (None, Some(line)) => {
                cs.refused.push((tag.clone(), line));
                continue;
            }
            (None, None) => cs.followed += 1,
        }
        if refused.is_some() {
            continue;
        }
        if let Some(crate::eval::Val::Enum(last)) = &state {
            let first = recs.first().map(|r| &r.input);
            let world = first.and_then(|i| a.world_of(|n| i.get(n)));
            if a.finals.contains(last) {
                cs.ended += 1;
            } else if !a.finals.is_empty() && world.is_some_and(|w| !can_finish.contains(&(last.clone(), w))) {
                cs.stranded.push((tag.clone(), last.clone()));
            }
        }
    }
    Some(cs)
}
