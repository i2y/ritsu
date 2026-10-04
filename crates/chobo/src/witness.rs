//! Lists of operations that show something: a refusal, a stuck balance, a key that two calls
//! share. They are built by calling the book in the reference interpreter, and what they
//! claim is read back from what the interpreter answered, never assumed (DESIGN 3.1).
//!
//! The scenarios (`scenarios.rs`), the report of what each operation can be refused with
//! (`check.rs`) and the diagnostics' operations all come from the `Builder` here.

use crate::diag::OpLine;
use ritsu_base::text::Text;
use crate::interp::{AccountId, At, Call, Op, Outcome, State, Val};
use crate::model::*;
use crate::scenario::{self, Scenario, Step, StepOut};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// How many transfers deep `put` may go to fund the transfer that funds an account.
pub const MAX_DEPTH: usize = 3;

#[derive(Clone)]
pub struct Builder<'b> {
    pub book: &'b Book,
    pub state: State,
    pub steps: Vec<Step>,
    pub outs: Vec<StepOut>,
    /// the number the next fresh string ends in
    n: usize,
    amounts: BTreeSet<i128>,
    /// the strings `fresh` made
    made: BTreeSet<String>,
}

/// `注文` of `注文-12`.
fn fresh_stem(s: &str) -> Option<&str> {
    let (stem, n) = s.rsplit_once('-')?;
    n.chars().all(|c| c.is_ascii_digit()).then_some(stem)
}

struct Mark {
    state: State,
    steps: usize,
    n: usize,
    amounts: BTreeSet<i128>,
}

impl<'b> Builder<'b> {
    pub fn new(book: &'b Book) -> Builder<'b> {
        Builder { book, state: State::default(), steps: vec![], outs: vec![], n: 1, amounts: BTreeSet::new(), made: BTreeSet::new() }
    }

    fn mark(&self) -> Mark {
        Mark { state: self.state.clone(), steps: self.steps.len(), n: self.n, amounts: self.amounts.clone() }
    }

    fn reset(&mut self, m: Mark) {
        self.state = m.state;
        self.steps.truncate(m.steps);
        self.outs.truncate(m.steps);
        self.n = m.n;
        self.amounts = m.amounts;
    }

    /// `<param>-<n>`, with a number no other value of the scenario has.
    pub fn fresh(&mut self, param: &str) -> String {
        let s = format!("{param}-{}", self.n);
        self.n += 1;
        self.made.insert(s.clone());
        s
    }

    /// The smallest positive amount the scenario has not used yet.
    pub fn fresh_amount(&mut self) -> i128 {
        let (lo, hi) = held_to().unwrap_or((1, i128::MAX));
        let mut v = lo;
        while self.amounts.contains(&v) && v < hi {
            v += 1;
        }
        self.amounts.insert(v);
        v
    }

    /// Arguments for a `do` or `hold` of `k`: the fixed ones, and fresh values for the rest.
    pub fn args(&mut self, k: usize, fixed: &BTreeMap<usize, Val>) -> Vec<Option<Val>> {
        let t = &self.book.transfers[k];
        let mut out = Vec::new();
        for (i, p) in t.params.iter().enumerate() {
            let v = match fixed.get(&i) {
                Some(v) => {
                    if let Val::Amt(a) = v {
                        self.amounts.insert(*a);
                    }
                    v.clone()
                }
                None => match p.ty {
                    Ty::Str => Val::Str(self.fresh(&p.name)),
                    Ty::Amount(_) => Val::Amt(self.fresh_amount()),
                },
            };
            out.push(Some(v));
        }
        out
    }

    /// A `do` or `hold` of `k`, as the kind takes it.
    pub fn first_call(&mut self, k: usize, fixed: &BTreeMap<usize, Val>) -> Call {
        let op = if self.book.transfers[k].is_pending() { Op::Hold } else { Op::Do };
        let args = self.args(k, fixed);
        Call { op, kind: k, args, amounts: None }
    }

    /// The `post` or `void` of the hold `hold` made.
    pub fn follow(&self, hold: &Call, op: Op, amounts: Option<BTreeMap<usize, i128>>) -> Call {
        let t = &self.book.transfers[hold.kind];
        let args = (0..t.params.len()).map(|i| if t.key.contains(&i) { hold.args[i].clone() } else { None }).collect();
        Call { op, kind: hold.kind, args, amounts }
    }

    /// Make the call and write it down. None when the call is not well formed.
    pub fn call(&mut self, c: &Call) -> Option<Outcome> {
        let o = self.state.apply(self.book, c).ok()?;
        self.steps.push(Step::Call(c.clone()));
        self.outs.push(StepOut::Call(o.clone()));
        Some(o)
    }

    pub fn pass(&mut self, secs: u64) {
        let e = self.state.pass(secs);
        self.steps.push(Step::Pass(secs, scenario::format_duration(secs)));
        self.outs.push(StepOut::Pass(e));
    }

    /// Write down a `together` as the last step. What it does to the state is not kept: a
    /// `together` has more than one way to come out.
    pub fn together(&mut self, callers: Vec<Vec<Call>>) {
        self.steps.push(Step::Together(callers));
        self.outs.push(StepOut::Together(vec![]));
    }

    /// Fund what `c` takes from, so that it goes through, or is refused at the move `keep`
    /// and nowhere before it. `before` are calls to make first in the trial (and only there).
    pub fn prepare(&mut self, before: &[Call], c: &Call, keep: Option<usize>, depth: usize) -> bool {
        for _ in 0..8 {
            let mut sim = self.state.clone();
            for b in before {
                if sim.apply(self.book, b).is_err() {
                    return false;
                }
            }
            match sim.apply(self.book, c) {
                Ok(Outcome::Done) => return true,
                Ok(Outcome::Refused(r)) => match r.at {
                    Some(at) if Some(at.move_index) == keep => return true,
                    Some(at) if !at.upper => {
                        let l = self.book.accounts[at.account.kind].lower.as_ref().map(|l| l.value).unwrap_or(0);
                        // and what the later moves take from the same account, so that one
                        // funding covers the whole call
                        let t = &self.book.transfers[c.kind];
                        let later: i128 = t.moves.iter().skip(at.move_index + 1).filter(|m| c.account(&m.from) == at.account).map(|m| c.amount(m)).sum();
                        let deficit = l + at.amount + later - (at.bal.posted - at.bal.held_out);
                        if depth >= MAX_DEPTH || !self.put(&at.account, deficit, depth + 1) {
                            return false;
                        }
                    }
                    _ => return false,
                },
                _ => return false,
            }
        }
        false
    }

    /// Put `amount` into `x`, posted, with a transfer of the book that moves an amount it is
    /// given into that kind of account. When the transfer's other moves touch `x` too, the
    /// amount it is given is worked out so that what stays in `x` is `amount`.
    pub fn put(&mut self, x: &AccountId, amount: i128, depth: usize) -> bool {
        if amount <= 0 {
            return true;
        }
        let book = self.book;
        let mut cands: Vec<(usize, usize)> = Vec::new();
        for pending in [false, true] {
            for (k, t) in book.transfers.iter().enumerate() {
                if t.is_pending() != pending {
                    continue;
                }
                for (i, m) in t.moves.iter().enumerate() {
                    if m.to.kind == x.kind && matches!(m.amount, Amount::Param(_)) {
                        cands.push((k, i));
                    }
                }
            }
        }
        for (k, i) in cands {
            let t = &book.transfers[k];
            let m = &t.moves[i];
            let Amount::Param(p) = m.amount else { continue };
            let mut fixed: BTreeMap<usize, Val> = BTreeMap::new();
            let mut fits = true;
            for (j, a) in m.to.args.iter().enumerate() {
                match a {
                    Arg::Lit(s) => fits &= *s == x.args[j],
                    Arg::Param(q) => match fixed.get(q) {
                        Some(Val::Str(s)) if *s != x.args[j] => fits = false,
                        _ => {
                            fixed.insert(*q, Val::Str(x.args[j].clone()));
                        }
                    },
                }
            }
            if !fits || matches!(fixed.get(&p), Some(Val::Str(_))) {
                continue;
            }
            fixed.insert(p, Val::Amt(amount));
            let mark = self.mark();
            let mut c = self.first_call(k, &fixed);
            // what stays in x is coef × (the amount given) + rest
            let (mut coef, mut rest) = (0i128, 0i128);
            for mv in &t.moves {
                let sign = (c.account(&mv.to) == *x) as i128 - (c.account(&mv.from) == *x) as i128;
                if mv.amount == Amount::Param(p) {
                    coef += sign;
                } else {
                    rest += sign * c.amount(mv);
                }
            }
            if coef <= 0 || (amount - rest) < 0 || (amount - rest) % coef != 0 {
                self.reset(mark);
                continue;
            }
            c.args[p] = Some(Val::Amt((amount - rest) / coef));
            let before = self.state.balance(x).posted;
            let mut ok = self.prepare(&[], &c, None, depth) && self.call(&c) == Some(Outcome::Done);
            if ok && t.is_pending() {
                let post = self.follow(&c, Op::Post, None);
                ok = self.call(&post) == Some(Outcome::Done);
            }
            if ok && self.state.balance(x).posted - before == amount {
                return true;
            }
            self.reset(mark);
        }
        false
    }

    /// A `do` or `hold` of `k` that goes through, written down after what it takes is funded.
    /// The amounts not fixed are fresh ones, or, when those do not go through, small ones
    /// tried in turn (a transfer whose later move takes what an earlier one put in needs the
    /// earlier amount to be the larger). Every amount is at least `min`.
    pub fn passing_call(&mut self, k: usize, fixed: &BTreeMap<usize, Val>, min: i128) -> Option<Call> {
        let t = &self.book.transfers[k];
        let free: Vec<usize> = t.amount_params().into_iter().filter(|p| !fixed.contains_key(p)).collect();
        // amounts held to a range that ends below `min` give no such call: no fresh amount reaches it
        if !free.is_empty() && held_to().is_some_and(|(_, hi)| hi < min) {
            return None;
        }
        let mut fixed1 = fixed.clone();
        for p in &free {
            let mut v = self.fresh_amount();
            while v < min {
                v = self.fresh_amount();
            }
            fixed1.insert(*p, Val::Amt(v));
        }
        let mut tries = vec![fixed1];
        let cands: Vec<i128> = candidates().into_iter().filter(|v| *v >= min).collect();
        let shown = free.len().min(4);
        for combo in grid(shown, &cands) {
            let mut f = fixed.clone();
            for (p, v) in free.iter().zip(&combo) {
                f.insert(*p, Val::Amt(*v));
            }
            // past the four shown, the least the range allows (1 when the amounts are not held)
            let least = held_to().map_or(min.max(1), |(lo, _)| min.max(lo));
            for p in free.iter().skip(shown) {
                f.insert(*p, Val::Amt(least));
            }
            tries.push(f);
        }
        // first a call that goes through as things stand, then one that needs funding
        for fund in [false, true] {
            for f in &tries {
                let mark = self.mark();
                let c = self.first_call(k, f);
                let ready = if fund { self.prepare(&[], &c, None, 0) } else { self.state.clone().apply(self.book, &c) == Ok(Outcome::Done) };
                if ready && self.call(&c) == Some(Outcome::Done) {
                    return Some(c);
                }
                self.reset(mark);
            }
        }
        None
    }

    /// Fill `x` up to `posted` (from where it is), as `put` does.
    pub fn fill(&mut self, x: &AccountId, posted: i128) -> bool {
        let have = self.state.balance(x).posted;
        if posted < have {
            return false;
        }
        self.put(x, posted - have, 0)
    }

    /// The steps as a scenario. The fresh values are numbered again in the order the steps
    /// first use them, so that a reader meets `注文-1` before `注文-2`; the steps mean the same.
    pub fn scenario(&self, name: impl Into<String>) -> Scenario {
        let mut map: BTreeMap<String, String> = BTreeMap::new();
        let mut next = 1;
        let mut steps = self.steps.clone();
        let mut renumber = |c: &mut Call| {
            let t = &self.book.transfers[c.kind];
            for (i, a) in c.args.iter_mut().enumerate() {
                if let Some(Val::Str(s)) = a {
                    if !self.made.contains(s.as_str()) {
                        continue;
                    }
                    let new = map.entry(s.clone()).or_insert_with(|| {
                        let v = format!("{}-{next}", fresh_stem(s).unwrap_or(&t.params[i].name));
                        next += 1;
                        v
                    });
                    *s = new.clone();
                }
            }
        };
        for st in &mut steps {
            match st {
                Step::Call(c) => renumber(c),
                Step::Together(cs) => cs.iter_mut().flatten().for_each(&mut renumber),
                Step::Pass(..) => {}
            }
        }
        Scenario { name: name.into(), steps }
    }

    /// The last outcome written down.
    pub fn last(&self) -> Option<&Outcome> {
        match self.outs.last() {
            Some(StepOut::Call(o)) => Some(o),
            _ => None,
        }
    }

    /// The operations as a diagnostic shows them, numbered as `scenario` numbers them, and
    /// run again so that what they answer is read from those very steps.
    pub fn op_lines(&self) -> Vec<OpLine> {
        let s = self.scenario("");
        match scenario::run(self.book, &s) {
            Ok(runs) if runs.len() == 1 => op_lines(self.book, &s.steps, &runs[0].steps),
            _ => op_lines(self.book, &self.steps, &self.outs),
        }
    }

    /// The operations as the report and `api` write them: each step with its result.
    pub fn example_json(&self) -> Vec<Value> {
        self.op_lines().into_iter().map(|o| o.json).collect()
    }
}

/// The amounts tried when the first ones do not do.
pub const CANDIDATES: &[i128] = &[1, 2, 3, 5, 10, 100, 1000];

thread_local! {
    /// The amounts every call is held to while ritsu's port asks which refusals a transfer can
    /// come to with its amounts in a range (`Books::refusals`, ritsu's DESIGN 7.6): None for the
    /// check, which tries the candidates above.
    static WITHIN: std::cell::Cell<Option<(i128, i128)>> = const { std::cell::Cell::new(None) };
}

/// Runs `f` with every amount the search tries held to `lo..=hi` (at least 0: chobo takes an
/// amount of 0, which moves nothing, so a range that is only 0 is searched with 0).
pub fn within<R>(lo: i128, hi: i128, f: impl FnOnce() -> R) -> R {
    let old = WITHIN.with(|w| w.replace(Some((lo.max(0), hi))));
    struct Back(Option<(i128, i128)>);
    impl Drop for Back {
        fn drop(&mut self) {
            WITHIN.with(|w| w.set(self.0));
        }
    }
    let _back = Back(old);
    f()
}

/// The range the amounts are held to, if they are.
fn held_to() -> Option<(i128, i128)> {
    WITHIN.with(|w| w.get())
}

/// Whether an amount may be tried: inside the range the port holds the amounts to, if it does.
fn allowed(v: i128) -> bool {
    held_to().is_none_or(|(lo, hi)| lo <= v && v <= hi)
}

/// The amounts tried when the first ones do not do: the fixed candidates, or with the amounts held
/// to a range, its ends, the steps in from them and its middle, with the candidates inside it.
pub fn candidates() -> Vec<i128> {
    match held_to() {
        None => CANDIDATES.to_vec(),
        Some((lo, hi)) => {
            let mut v: Vec<i128> = vec![lo, lo.saturating_add(1), lo + (hi - lo) / 2, hi.saturating_sub(1), hi];
            v.extend(CANDIDATES.iter().copied());
            v.retain(|x| lo <= *x && *x <= hi);
            v.sort_unstable();
            v.dedup();
            v
        }
    }
}

/// Every way to give `n` parameters one of `cands` each: different values before repeated
/// ones, then the smaller sums first.
pub fn grid(n: usize, cands: &[i128]) -> Vec<Vec<i128>> {
    let mut out: Vec<Vec<i128>> = vec![vec![]];
    for _ in 0..n {
        out = out
            .into_iter()
            .flat_map(|v| {
                cands.iter().map(move |c| {
                    let mut w = v.clone();
                    w.push(*c);
                    w
                })
            })
            .collect();
    }
    out.sort_by_key(|v| {
        let mut d = v.clone();
        d.sort();
        d.dedup();
        (d.len() != v.len(), v.iter().sum::<i128>(), v.clone())
    });
    out
}

/// What a refused move found, for a person: which move, how much, and the balance.
pub fn at_text(book: &Book, at: &At) -> Text {
    let n = at.move_index + 1;
    let acct = at.account.text(book);
    let a = at.amount;
    let b = at.bal;
    let (p, o, i) = (b.posted, b.held_out, b.held_in);
    if !at.upper {
        let mut t = tr!("{n} つ目の移動が {acct} から {a} を取る。確定 {p}、出ていく仮押さえ {o}", "move {n} takes {a} from {acct}: posted {p}, held out {o}");
        if i != 0 {
            t.ja.push_str(&format!("、入ってくる仮押さえ {i}"));
            t.en.push_str(&format!(", held in {i}"));
        }
        t
    } else {
        let mut t = tr!("{n} つ目の移動が {acct} へ {a} を入れる。確定 {p}、入ってくる仮押さえ {i}", "move {n} puts {a} into {acct}: posted {p}, held in {i}");
        if o != 0 {
            t.ja.push_str(&format!("、出ていく仮押さえ {o}"));
            t.en.push_str(&format!(", held out {o}"));
        }
        t
    }
}

fn outcome_line(book: &Book, o: &Outcome) -> Text {
    let base = scenario::outcome_text(o);
    match o {
        Outcome::Refused(r) if r.at.is_some() => {
            let d = at_text(book, r.at.as_ref().unwrap());
            Text { ja: format!("{}（{}）", base.ja, d.ja), en: format!("{} ({})", base.en, d.en) }
        }
        _ => base,
    }
}

fn result_fields(m: &mut Map<String, Value>, o: &Outcome) {
    m.insert("result".into(), json!(o.result()));
    if let Some(r) = o.reason() {
        m.insert("reason".into(), json!(r));
    }
}

pub fn op_lines(book: &Book, steps: &[Step], outs: &[StepOut]) -> Vec<OpLine> {
    let mut lines = Vec::new();
    for (st, out) in steps.iter().zip(outs) {
        match (st, out) {
            (Step::Call(c), StepOut::Call(o)) => {
                let call = scenario::call_text(book, c);
                let r = outcome_line(book, o);
                let text = Text { ja: format!("{call}  {}", r.ja), en: format!("{call}  {}", r.en) };
                let mut j = match scenario::call_to_json(book, c) {
                    Value::Object(m) => m,
                    _ => Map::new(),
                };
                result_fields(&mut j, o);
                lines.push(OpLine { text, json: Value::Object(j) });
            }
            (Step::Pass(_, d), StepOut::Pass(e)) => {
                let mut text = Text::same(format!("pass {d}"));
                if !e.is_empty() {
                    let names: Vec<String> = e.iter().map(|(k, key)| scenario::hold_text(book, *k, key)).collect();
                    let names = names.join(", ");
                    let x = tr!("{names} が期限切れ", "{names} expired");
                    text.ja.push_str(&format!("  {}", x.ja));
                    text.en.push_str(&format!("  {}", x.en));
                }
                lines.push(OpLine { text, json: json!({"op": "pass", "duration": d}) });
            }
            (Step::Together(cs), _) => {
                let mut callers = Vec::new();
                let mut parts = Vec::new();
                for ops in cs {
                    callers.push(ops.iter().map(|c| scenario::call_to_json(book, c)).collect::<Vec<_>>());
                    parts.push(ops.iter().map(|c| scenario::call_text(book, c)).collect::<Vec<_>>().join(", "));
                }
                let text = Text { ja: format!("together: {}", parts.join(" / ")), en: format!("together: {}", parts.join(" / ")) };
                lines.push(OpLine { text, json: json!({"op": "together", "callers": callers}) });
            }
            _ => {}
        }
    }
    lines
}

// ── the examples ──────────────────────────────────────────────────────────

/// Operations that end in `k` refused at move `i` by the lower bound of what it takes from
/// (`upper` false) or the upper bound of what it puts into (true). The amount at move `i`
/// is one that bound refuses: on an account nothing has touched, more than it can give or
/// more than its bound; or 1, after the account is filled to its bound. When the moves
/// before it get in the way, small amounts are tried in turn.
pub fn bound_refusal(book: &Book, k: usize, i: usize, upper: bool) -> Option<Builder<'_>> {
    let t = &book.transfers[k];
    let m = &t.moves[i];
    let bound = if upper { book.accounts[m.to.kind].upper.as_ref()? } else { book.accounts[m.from.kind].lower.as_ref()? };
    let p = match m.amount {
        Amount::Param(p) => Some(p),
        Amount::Lit(_) => None,
    };
    // (the amount at move i, the balance to fill the account to first)
    let shapes: Vec<(i128, Option<i128>)> = if upper {
        vec![(1, Some(bound.value)), (bound.value + 1, None)]
    } else {
        vec![((1 - bound.value).max(1), None)]
    };
    let ap = t.amount_params();
    let shown = ap.len().min(4);
    let mut tries: Vec<(BTreeMap<usize, Val>, Option<i128>)> = Vec::new();
    for (a, fill) in &shapes {
        // an amount the port holds the amounts away from is not tried (ritsu's DESIGN 7.6)
        if p.is_some() && !allowed(*a) {
            continue;
        }
        let mut first = BTreeMap::new();
        if let Some(p) = p {
            first.insert(p, Val::Amt(*a));
        }
        tries.push((first, *fill));
    }
    for (a, fill) in &shapes {
        let mut cands: Vec<i128> = candidates();
        if !cands.contains(a) && allowed(*a) {
            cands.push(*a);
        }
        for combo in grid(shown, &cands) {
            let f: BTreeMap<usize, Val> = ap.iter().zip(&combo).map(|(q, v)| (*q, Val::Amt(*v))).collect();
            tries.push((f, *fill));
        }
    }
    let wanted = |o: &Outcome| matches!(o, Outcome::Refused(r) if r.reason == bound.refusal && r.at.as_ref().is_some_and(|x| x.move_index == i && x.upper == upper));
    // first one that needs nothing funded, then one that does
    for fund in [false, true] {
        for (fixed, fill) in &tries {
            let mut b = Builder::new(book);
            let c = b.first_call(k, fixed);
            if let Some(f) = fill {
                let x = c.account(&m.to);
                if !b.fill(&x, *f) {
                    continue;
                }
            }
            let ready = if fund { b.prepare(&[], &c, Some(i), 0) } else { b.state.clone().apply(book, &c).is_ok_and(|o| wanted(&o)) };
            if !ready {
                continue;
            }
            let Some(o) = b.call(&c) else { continue };
            if wanted(&o) {
                return Some(b);
            }
        }
    }
    None
}

/// A call that goes through, and the same key again with another value of a parameter
/// that is not in the key: the first amount, or else the first such string.
pub fn key_conflict(book: &Book, k: usize) -> Option<(Builder<'_>, usize)> {
    let t = &book.transfers[k];
    let param = t.amount_params().into_iter().next().or_else(|| (0..t.params.len()).find(|i| !t.key.contains(i)))?;
    key_conflict_on(book, k, param).map(|b| (b, param))
}

/// A call that goes through, and the same key again with another value of `param`.
pub fn key_conflict_on(book: &Book, k: usize, param: usize) -> Option<Builder<'_>> {
    let t = &book.transfers[k];
    if t.key.contains(&param) {
        return None;
    }
    let mut b = Builder::new(book);
    let c = b.passing_call(k, &BTreeMap::new(), 0)?;
    let mut c2 = c.clone();
    c2.args[param] = Some(match t.params[param].ty {
        Ty::Str => Val::Str(b.fresh(&t.params[param].name)),
        Ty::Amount(_) => Val::Amt(b.fresh_amount()),
    });
    match b.call(&c2)? {
        Outcome::Refused(r) if r.reason == "key_conflict" => Some(b),
        _ => None,
    }
}

/// A call a bound refuses, and the same call again.
pub fn already_refused(book: &Book, k: usize) -> Option<Builder<'_>> {
    let t = &book.transfers[k];
    for i in 0..t.moves.len() {
        for upper in [false, true] {
            if let Some(mut b) = bound_refusal(book, k, i, upper) {
                let Some(Step::Call(c)) = b.steps.last().cloned() else { continue };
                if matches!(b.call(&c), Some(Outcome::Refused(r)) if r.reason == "already_refused") {
                    return Some(b);
                }
            }
        }
    }
    None
}

/// A call whose move `i` moves from an account to itself.
pub fn same_account(book: &Book, k: usize, i: usize) -> Option<Builder<'_>> {
    let t = &book.transfers[k];
    let m = &t.moves[i];
    if m.from.kind != m.to.kind {
        return None;
    }
    let mut b = Builder::new(book);
    let mut fixed: BTreeMap<usize, Val> = BTreeMap::new();
    for (x, y) in m.from.args.iter().zip(&m.to.args) {
        match (x, y) {
            (Arg::Lit(s), Arg::Lit(u)) if s != u => return None,
            (Arg::Lit(_), Arg::Lit(_)) => {}
            (Arg::Param(p), Arg::Lit(s)) | (Arg::Lit(s), Arg::Param(p)) => match fixed.get(p) {
                Some(Val::Str(v)) if v != s => return None,
                _ => {
                    fixed.insert(*p, Val::Str(s.clone()));
                }
            },
            (Arg::Param(p), Arg::Param(q)) => {
                let v = match fixed.get(p).or_else(|| fixed.get(q)) {
                    Some(v) => v.clone(),
                    None => Val::Str(b.fresh(&t.params[*p].name)),
                };
                if fixed.get(p).is_some_and(|x| *x != v) || fixed.get(q).is_some_and(|x| *x != v) {
                    return None;
                }
                fixed.insert(*p, v.clone());
                fixed.insert(*q, v);
            }
        }
    }
    let c = b.first_call(k, &fixed);
    match b.call(&c)? {
        Outcome::Refused(r) if r.reason == "same_account" => Some(b),
        _ => None,
    }
}

/// A hold of `k` that goes through, written down. The amounts are at least 2, so that a
/// part of it can be posted.
pub fn held<'b>(b: &mut Builder<'b>, k: usize) -> Option<Call> {
    b.passing_call(k, &BTreeMap::new(), 2)
}

/// Amounts for a `post` of the hold `c`: each amount parameter set by `f` from what it held.
pub fn amounts_of(book: &Book, c: &Call, f: impl Fn(i128) -> i128) -> Option<BTreeMap<usize, i128>> {
    let t = &book.transfers[c.kind];
    let ap = t.amount_params();
    if ap.is_empty() {
        return None;
    }
    Some(
        ap.into_iter()
            .map(|p| {
                let held = match &c.args[p] {
                    Some(Val::Amt(v)) => *v,
                    _ => 0,
                };
                (p, f(held))
            })
            .collect(),
    )
}

/// The operations that end in `op` (post or void) of `k` refused with `reason`.
pub fn hold_refusal<'b>(book: &'b Book, k: usize, op: Op, reason: &str) -> Option<Builder<'b>> {
    let t = &book.transfers[k];
    let mut b = Builder::new(book);
    let last = match reason {
        "no_such_hold" => {
            let c = b.first_call(k, &BTreeMap::new());
            b.follow(&c, op, None)
        }
        "already_posted" | "already_voided" | "expired" | "over_hold" | "key_conflict" => {
            let c = held(&mut b, k)?;
            match reason {
                "already_posted" => {
                    let p = b.follow(&c, Op::Post, None);
                    if b.call(&p)? != Outcome::Done {
                        return None;
                    }
                }
                "already_voided" => {
                    let v = b.follow(&c, Op::Void, None);
                    if b.call(&v)? != Outcome::Done {
                        return None;
                    }
                }
                "expired" => {
                    let Some(Expiry::After(s)) = t.pending else { return None };
                    b.pass(s);
                }
                "key_conflict" => {
                    let p = b.follow(&c, Op::Post, None);
                    if b.call(&p)? != Outcome::Done {
                        return None;
                    }
                }
                _ => {}
            }
            let amounts = match reason {
                "over_hold" => Some(amounts_of(book, &c, |h| h + 1)?),
                "key_conflict" => Some(amounts_of(book, &c, |h| h - 1)?),
                _ => None,
            };
            b.follow(&c, op, amounts)
        }
        _ => return None,
    };
    match b.call(&last)? {
        Outcome::Refused(r) if r.reason == reason => Some(b),
        _ => None,
    }
}

/// A hold that ends (expires, or is voided when it cannot), and the same hold again with the
/// same key, which answers done_before and holds nothing (DESIGN 2.3).
pub fn hold_again(book: &Book, k: usize) -> Option<Builder<'_>> {
    let t = &book.transfers[k];
    let mut b = Builder::new(book);
    let c = held(&mut b, k)?;
    match t.pending {
        Some(Expiry::After(s)) => b.pass(s),
        _ => {
            let v = b.follow(&c, Op::Void, None);
            if b.call(&v)? != Outcome::Done {
                return None;
            }
        }
    }
    match b.call(&c)? {
        Outcome::DoneBefore => Some(b),
        _ => None,
    }
}
