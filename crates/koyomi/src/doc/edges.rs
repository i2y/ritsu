//! What the approver's page learns by computing every input of the range once more (DESIGN
//! 7): the edge cases it shows, how often each `else` is used and what the other two ways
//! of handling a missing day would change, and how far the calendar moves the days.
//!
//! The check's [`Report`] already has the counts; this walk adds what only a page wants, so
//! the check stays as fast as it is.

use crate::ast::Cmp;
use crate::calendar::{Calendar, Reason};
use crate::check::{Inputs, Report};
use crate::date::{Day, Missing};
use ritsu_base::text::Text;
use crate::interp::{self, Ev, OpFail};
use crate::resolve::{CK, Model, ROp, Start};
use std::collections::HashSet;

/// A name in Japanese prose: as it is when it reads as a noun (`支払日`, `満了日_翌日`), and in
/// 「」 when it ends in hiragana, like a verb (`月数を足して寄せる`), so that the particle after
/// it is not read as part of it.
pub fn ja_name(name: &str) -> String {
    match name.chars().last() {
        Some(c) if ('ぁ'..='ゟ').contains(&c) => format!("「{name}」"),
        _ => name.to_string(),
    }
}

/// The most rows the edge cases take (DESIGN 7).
pub const MAX_EDGES: usize = 16;

/// The most reasons a `roll` is shown moving a day for (DESIGN 7).
const MAX_ROLL_REASONS: usize = 6;

/// One edge case: an input, where it comes in the order the check walks the inputs, and
/// why it was picked.
#[derive(Clone, Debug)]
pub struct Edge {
    pub order: u64,
    pub vals: Vec<i64>,
    pub why: Vec<Text>,
}

/// The first input an operation lands on a missing day for: the input, the day it did not
/// find, and the day `else` gave instead.
pub type FirstMissing = (Vec<i64>, (i32, u32, u32), Day);

/// An operation with `else` written, and what the range made of it.
#[derive(Clone, Debug)]
pub struct ElseUse {
    pub date: usize,
    pub op: usize,
    pub how: Missing,
    /// Inputs on which the operation landed on a day its month does not have.
    pub used: u64,
    /// The first of them.
    pub first: Option<FirstMissing>,
    /// For each of the other two ways: the inputs on which the date would come out
    /// different, or not at all.
    pub swaps: Vec<(Missing, u64)>,
}

/// An operation that can move a day off a closed one (`roll`, `if closed`): the largest
/// move, in days, and the first input that made it.
#[derive(Clone, Debug)]
pub struct MoveUse {
    pub date: usize,
    pub op: usize,
    pub most: Option<(i64, Vec<i64>)>,
}

#[derive(Clone, Debug, Default)]
pub struct Survey {
    pub edges: Vec<Edge>,
    pub elses: Vec<ElseUse>,
    pub moves: Vec<MoveUse>,
}

/// Where an input comes in the walk: the integer inputs turn slowest, in the order they are
/// declared, and the date fastest (`check::Inputs`).
pub fn order_of(m: &Model, vals: &[i64]) -> u64 {
    let mut o: u64 = 0;
    for k in m.int_inputs() {
        let i = &m.inputs[k];
        o = o * i.size() + (vals[k] - i.lo) as u64;
    }
    let d = m.date_in();
    o * d.size() + (vals[m.date_input] - d.lo) as u64
}

struct Cand {
    prio: u8,
    order: u64,
    vals: Vec<i64>,
    why: Text,
}

/// The candidates for the edge cases, each the first input of the walk that meets
/// something, under a key that says what.
struct Picks {
    seen: HashSet<String>,
    cands: Vec<Cand>,
}

impl Picks {
    fn wants(&self, key: &str) -> bool {
        !self.seen.contains(key)
    }

    fn add(&mut self, key: String, prio: u8, order: u64, vals: &[i64], why: Text) {
        if self.seen.insert(key) {
            self.cands.push(Cand { prio, order, vals: vals.to_vec(), why });
        }
    }
}

/// The first reason a day is closed, as a sentence of the page names it: `日曜` and
/// `a Sunday`, `憲法記念日`, `年末年始`.
fn first_reason(cal: &Calendar, d: Day) -> Option<Text> {
    cal.reasons(d).first().map(|r| match r {
        Reason::Weekday(_) => {
            let t = r.text();
            Text { ja: t.ja, en: format!("a {}", t.en) }
        }
        _ => r.text(),
    })
}

/// The name of the table holiday on a day, when there is one.
fn holiday_name(cal: &Calendar, d: Day) -> Option<String> {
    cal.reasons(d).into_iter().find_map(|r| match r {
        Reason::Holiday { name, table } => Some(if name.is_empty() { table } else { name }),
        _ => None,
    })
}

/// The date `k` for one input with its operation `op` replaced by `alt`: what another way of
/// handling a missing day would have given.
fn date_with(m: &Model, vals: &[i64], out: &[Day], k: usize, op: usize, alt: &ROp) -> Result<Day, OpFail> {
    let d = &m.dates[k];
    let mut x = match d.start {
        Start::Input => Day(vals[m.date_input] as i32),
        Start::Date(s) => out[s],
    };
    let mut ev = Vec::new();
    for (i, o) in d.ops.iter().enumerate() {
        let used = if i == op { alt } else { &o.op };
        x = interp::apply(m.cal.as_ref(), x, used, vals, &mut ev)?;
    }
    Ok(x)
}

/// The operation with another way of handling a missing day.
fn with_missing(op: &ROp, p: Missing) -> Option<ROp> {
    Some(match op {
        ROp::Months { sign, n, per, .. } => ROp::Months { sign: *sign, n: *n, per: *per, missing: Some(p) },
        ROp::DayOfMonth { n, sign, k, .. } => ROp::DayOfMonth { n: *n, sign: *sign, k: *k, missing: Some(p) },
        ROp::CloseDay(n, _) => ROp::CloseDay(*n, Some(p)),
        ROp::IfClosed(inner) => ROp::IfClosed(Box::new(with_missing(inner, p)?)),
        _ => return None,
    })
}

/// The way a missing day is handled, as the operation writes it.
fn written_missing(op: &ROp) -> Option<Missing> {
    match op {
        ROp::Months { missing, .. } | ROp::DayOfMonth { missing, .. } | ROp::CloseDay(_, missing) => *missing,
        ROp::IfClosed(inner) => written_missing(inner),
        _ => None,
    }
}

pub fn missing_word(p: Missing) -> &'static str {
    match p {
        Missing::EndOfMonth => "end_of_month",
        Missing::StartOfNextMonth => "start_of_next_month",
        Missing::Reject => "reject",
        Missing::Never => "",
    }
}

/// Walk every input once more and gather what the page shows.
pub fn survey(m: &Model, rep: &Report) -> Survey {
    let cal = m.cal.as_ref();
    let di = m.date_input;
    let din_en = &m.date_in().name;
    let din = &ja_name(din_en);
    let mut picks = Picks { seen: HashSet::new(), cands: Vec::new() };

    // The operations with `else` written, and the two other ways for each.
    struct Swap {
        date: usize,
        op: usize,
        how: Missing,
        alts: Vec<(Missing, ROp, u64)>,
        first: Option<FirstMissing>,
    }
    let mut swaps: Vec<Swap> = Vec::new();
    for (k, d) in m.dates.iter().enumerate() {
        for (i, o) in d.ops.iter().enumerate() {
            if let Some(how) = written_missing(&o.op) {
                let alts = [Missing::EndOfMonth, Missing::StartOfNextMonth, Missing::Reject]
                    .into_iter()
                    .filter(|p| *p != how)
                    .filter_map(|p| with_missing(&o.op, p).map(|op| (p, op, 0u64)))
                    .collect();
                swaps.push(Swap { date: k, op: i, how, alts, first: None });
            }
        }
    }
    let mut moves: Vec<MoveUse> = Vec::new();
    let mut roll_reasons: Vec<usize> = Vec::new();
    for (k, d) in m.dates.iter().enumerate() {
        for (i, o) in d.ops.iter().enumerate() {
            if matches!(o.op, ROp::Roll(_) | ROp::IfClosed(_)) {
                moves.push(MoveUse { date: k, op: i, most: None });
                roll_reasons.push(0);
            }
        }
    }

    let mut out = vec![Day(0); m.dates.len()];
    let mut evs: Vec<(u32, u32, Ev)> = Vec::new();
    let mut it = Inputs::new(m);
    let mut order: u64 = 0;
    while let Some(vals) = it.next_input() {
        let here = order;
        order += 1;
        evs.clear();
        if interp::run(m, vals, &mut out, &mut evs).is_err() {
            continue;
        }
        let z = Day(vals[di] as i32);
        // What the input day is (DESIGN 7: month ends, closed days, the days around a holiday).
        let (_, mo, dd) = z.ymd();
        let len = crate::date::month_len(z.year() as i64, mo);
        if dd == len {
            let (key, why) = match (mo, len) {
                (2, 29) => ("feb29", tr!("{}が 2 月 29 日", "{} is 29 February", din; din_en)),
                (2, _) => ("feb28", tr!("{}が 2 月 28 日（うるう年でない年の月末）", "{} is 28 February, the end of the month in a year that is not a leap year", din; din_en)),
                (_, 31) => ("eom31", tr!("{}が月末（31 日）", "{} is the 31st, the end of its month", din; din_en)),
                _ => ("eom30", tr!("{}が月末（30 日）", "{} is the 30th, the end of its month", din; din_en)),
            };
            if picks.wants(key) {
                picks.add(key.into(), 6, here, vals, why);
            }
        }
        if let Some(c) = cal {
            if picks.wants("closed") && c.is_open(z) == Ok(false) {
                let r = first_reason(c, z).unwrap_or_default();
                picks.add("closed".into(), 5, here, vals, tr!("{}が{}で休み", "{} is {}, a closed day", din, r.ja; din_en, r.en));
            }
            if picks.wants("every") && c.is_open(z) == Ok(false) {
                let (_, mm, d2) = z.ymd();
                if let Some(e) = c.every.iter().find(|e| e.covers(mm, d2)) {
                    let name = e.name.clone().unwrap_or_else(|| e.text());
                    picks.add("every".into(), 5, here, vals, tr!("{}が{name}に当たる", "{} falls in {name}", din; din_en));
                }
            }
            if picks.wants("before")
                && let Some(next) = z.next()
                && c.is_open(z) == Ok(true)
                && let Some(h) = holiday_name(c, next)
            {
                picks.add("before".into(), 5, here, vals, tr!("{}が{h}の前日", "{} is the day before {h}", din; din_en));
            }
            if picks.wants("after")
                && let Some(prev) = z.prev()
                && c.is_open(z) == Ok(true)
                && let Some(h) = holiday_name(c, prev)
            {
                picks.add("after".into(), 5, here, vals, tr!("{}が{h}の翌日", "{} is the day after {h}", din; din_en));
            }
        }
        // What the operations did.
        for (k, i, e) in &evs {
            let (k, i) = (*k as usize, *i as usize);
            let d = &m.dates[k];
            let o = &d.ops[i];
            match (e, &o.op) {
                (Ev::Moved { from, to }, ROp::Roll(_)) => {
                    let Some(c) = cal else { continue };
                    let mi = moves.iter().position(|x| x.date == k && x.op == i).unwrap();
                    let dist = (to.0 - from.0).abs() as i64;
                    if moves[mi].most.as_ref().is_none_or(|(best, _)| dist > *best) {
                        moves[mi].most = Some((dist, vals.to_vec()));
                    }
                    if roll_reasons[mi] < MAX_ROLL_REASONS
                        && let Some(r) = first_reason(c, *from)
                    {
                        let key = format!("roll:{k}:{i}:{}", r.en);
                        if picks.wants(&key) {
                            roll_reasons[mi] += 1;
                            let (jn, name) = (ja_name(&d.name), &d.name);
                            let why = if to < from {
                                tr!("{jn}が{}に当たり、前営業日に動く", "{name} falls on {} and moves to the business day before", r.ja; r.en)
                            } else {
                                tr!("{jn}が{}に当たり、翌営業日に動く", "{name} falls on {} and moves to the next business day", r.ja; r.en)
                            };
                            picks.add(key, 4, here, vals, why);
                        }
                    }
                }
                (Ev::Moved { from, to }, ROp::IfClosed(_)) => {
                    let mi = moves.iter().position(|x| x.date == k && x.op == i).unwrap();
                    let dist = (to.0 - from.0).abs() as i64;
                    if moves[mi].most.as_ref().is_none_or(|(best, _)| dist > *best) {
                        moves[mi].most = Some((dist, vals.to_vec()));
                    }
                }
                (Ev::IfClosed { closed: true }, _) => {
                    let key = format!("if:{k}:{i}");
                    if picks.wants(&key) {
                        let before = if i == 0 {
                            match d.start {
                                Start::Input => z,
                                Start::Date(s) => out[s],
                            }
                        } else {
                            interp::before_op(m, vals, k, i).unwrap_or(z)
                        };
                        let r = cal.and_then(|c| first_reason(c, before)).unwrap_or_default();
                        let (jn, name, text) = (ja_name(&d.name), &d.name, &o.text);
                        picks.add(key, 2, here, vals, tr!("{jn}が{}に当たり、`{text}` が効く", "{name} falls on {}, and `{text}` acts", r.ja; r.en));
                    }
                }
                (Ev::Else { y, m: mm, d: dd, .. }, _) => {
                    if let Some(s) = swaps.iter_mut().find(|s| s.date == k && s.op == i)
                        && s.first.is_none()
                        && let Ok(before) = interp::before_op(m, vals, k, i)
                        && let Ok(gave) = interp::apply(cal, before, &o.op, vals, &mut Vec::new())
                    {
                        s.first = Some((vals.to_vec(), (*y, *mm, *dd), gave));
                    }
                    let key = format!("else:{k}:{i}");
                    if picks.wants(&key) {
                        let missing = interp::ymd(*y, *mm, *dd);
                        let (jn, name, how) = (ja_name(&d.name), &d.name, missing_word(written_missing(&o.op).unwrap_or(Missing::Never)));
                        picks.add(key, 2, here, vals, tr!(
                            "{jn}の計算が無い日 {missing} に当たり、`else {how}` を使う",
                            "computing {name} lands on {missing}, which does not exist, and uses `else {how}`"
                        ));
                    }
                }
                _ => {}
            }
        }
        // What the other ways of handling a missing day would give.
        for s in &mut swaps {
            for (_, alt, n) in &mut s.alts {
                match date_with(m, vals, &out, s.date, s.op, alt) {
                    Ok(x) if x == out[s.date] => {}
                    _ => *n += 1,
                }
            }
        }
    }

    // From the check's report: the claims, the longest and the shortest.
    for (ci, c) in m.claims.iter().enumerate() {
        let cr = &rep.claims[ci];
        let name = &c.name;
        if let Some(vals) = &cr.first {
            picks.add(format!("claim:{ci}"), 1, order_of(m, vals), vals, tr!("条件「{name}」が成り立たない", "the claim {name} fails"));
        } else if let (CK::Compare(a, cmp, b), Some((_, vals, l, r))) = (&c.kind, &cr.tightest)
            && *cmp != Cmp::Eq
        {
            let (is, limit) = crate::check::compare_parts(m, a, *cmp, b, *l, *r, vals, false);
            let why = match limit {
                Some(lim) => tr!(
                    "条件「{name}」の余裕がいちばん少ない（{}、条件は{}{}）",
                    "the least room for the claim {name} ({}; the claim allows {})",
                    is.ja,
                    crate::doc::digit_space(&lim.ja),
                    lim.ja;
                    is.en,
                    lim.en
                ),
                None => tr!("条件「{name}」の余裕がいちばん少ない（{}）", "the least room for the claim {name} ({})", is.ja; is.en),
            };
            picks.add(format!("claim:{ci}"), 1, order_of(m, vals), vals, why);
        }
    }
    // The most and the fewest days from the input to a date that no other date starts from:
    // the results. Dates that reach theirs on the same input with the same count are said
    // together.
    let leaves: Vec<usize> = (0..m.dates.len()).filter(|k| !m.dates.iter().any(|d| d.start == Start::Date(*k))).collect();
    // (the most, not the fewest; where in the walk; the days; the input; the dates)
    struct Extreme {
        long: bool,
        order: u64,
        n: i64,
        vals: Vec<i64>,
        names: Vec<String>,
    }
    let mut extremes: Vec<Extreme> = Vec::new();
    for k in leaves {
        let dr = &rep.dates[k];
        let (Some((hi, his)), Some((lo, los))) = (&dr.longest, &dr.shortest) else { continue };
        if hi == lo {
            continue;
        }
        for (long, n, v) in [(true, *hi, his.first()), (false, *lo, los.first())] {
            let Some(v) = v else { continue };
            let o = order_of(m, v);
            match extremes.iter_mut().find(|e| e.long == long && e.order == o && e.n == n) {
                Some(e) => e.names.push(m.dates[k].name.clone()),
                None => extremes.push(Extreme { long, order: o, n, vals: v.clone(), names: vec![m.dates[k].name.clone()] }),
            }
        }
    }
    for Extreme { long, order: o, n, vals: v, names } in extremes {
        let ja: Vec<String> = names.iter().map(|x| ja_name(x)).collect();
        let (ja, en) = (crate::doc::ja_list(&ja), Text::list(&names.iter().map(|x| Text::same(x.clone())).collect::<Vec<_>>()).en);
        let why = if long {
            tr!("{}から{}までの日数がいちばん多い（{n} 日）", "the most days from {} to {} ({n})", din, ja; din_en, en)
        } else {
            tr!("{}から{}までの日数がいちばん少ない（{n} 日）", "the fewest days from {} to {} ({n})", din, ja; din_en, en)
        };
        picks.add(format!("extreme:{long}:{o}:{n}"), 3, o, &v, why);
    }
    for mv in &moves {
        if let Some((dist, vals)) = &mv.most
            && *dist > 1
            && matches!(m.dates[mv.date].ops[mv.op].op, ROp::Roll(_))
        {
            let (jn, name) = (ja_name(&m.dates[mv.date].name), &m.dates[mv.date].name);
            picks.add(format!("most:{}:{}", mv.date, mv.op), 4, order_of(m, vals), vals, tr!("{jn}がいちばん大きく動く（{dist} 日）", "{name} moves the most ({dist} days)"));
        }
    }

    // The rows: by what they show first, at most MAX_EDGES inputs, then in the walk's order.
    let mut cands = picks.cands;
    cands.sort_by_key(|c| (c.prio, c.order));
    let mut edges: Vec<Edge> = Vec::new();
    for c in cands {
        if let Some(e) = edges.iter_mut().find(|e| e.order == c.order) {
            e.why.push(c.why);
        } else if edges.len() < MAX_EDGES {
            edges.push(Edge { order: c.order, vals: c.vals, why: vec![c.why] });
        }
    }
    edges.sort_by_key(|e| e.order);

    let elses = swaps
        .into_iter()
        .map(|s| ElseUse {
            date: s.date,
            op: s.op,
            how: s.how,
            used: rep.ops[s.date][s.op].elses,
            first: s.first,
            swaps: s.alts.into_iter().map(|(p, _, n)| (p, n)).collect(),
        })
        .collect();
    Survey { edges, elses, moves }
}
