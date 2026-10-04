//! What a flow calls in the other languages, for ritsu's checks across the borders (ritsu's DESIGN
//! 7.4–7.8; the port of flows, `ritsu_ports::Flows::crossings`): every call of a rule, of a koyomi
//! date and of a chobo transfer, with the places each value it gives can come from, and how long
//! each hold of a book can be held before a call its expiry can refuse.
//!
//! Where a value comes from is gathered as dandori gathers ranges (`ranges`): from every place a
//! variable is given a value anywhere in the flow, joined. The day of a koyomi date and the output
//! of a rule are known by the record they are read from (`due.day`, `fee.amount`), whose type says
//! which date or rule answered it.
//!
//! How long a hold is held is gathered along the ways through the flow, as seconds since the hold
//! was made, the fewest and the most:
//!
//! - The call that makes the hold makes it somewhere inside its own run: after it, the hold is 0
//!   seconds old at the fewest, and at most as old as the call can take.
//! - A call takes 0 seconds at the fewest, and at the most its `timeout` for each attempt its
//!   `retry` allows, with the waits between them. A task with no `timeout`, a rule or a date (whose
//!   calls the flow gives no limit), leaves nothing bounding the most.
//! - `wait <n>` takes exactly its time.
//! - `wait until x.at`, when `x` is the answer of a koyomi date whose date input was given `now`
//!   read after the hold, lasts until the date's time. That time is between the fewest and the
//!   most days koyomi counts from the input to the date, over the whole range of the inputs, at the
//!   date's `at`; and `now` fell somewhere in its day, so from `now` the wait ends after
//!   (fewest − 1) days and the time of `at`, at the fewest, and after most days and the time of
//!   `at`, at the most (ritsu's DESIGN 7.7). It ends no sooner than it starts. Any other `wait
//!   until` takes 0 seconds at the fewest, and nothing bounds the most.
//! - The arms of a `match` and the handlers of a call are joined: the fewest of the fewest, the
//!   most of the most. A loop takes 0 seconds at the fewest, and at the most each round as long as
//!   its longest way, as many rounds as it can run.
//! - `on failure` and `on cancel` start where a failure or a cancellation can stop the run: the
//!   hold is 0 seconds old at the fewest, and at the most as old as it can be at any call or wait.
//!
//! A call that posts or voids the hold acts somewhere inside its own run, so its span runs from the
//! fewest seconds as it starts to the most as it ends.

use crate::diag::Text;
use crate::model::*;
use ritsu_ports::{seconds_text, Amount, CallArg, Crossings, DateCall as PortDate, Found, HoldSpan, Origin, RuleCall, TransferCall};
use std::collections::BTreeMap;

/// Where a value can come from, by the model's own indices.
#[derive(Clone, Debug, PartialEq)]
enum From {
    /// the day of the date `rules[ix]`
    Day(usize),
    /// an output of the rule `rules[ix]`
    Output(usize, String),
    Now,
    Range(Range),
    Unknown(Text),
}

fn add(into: &mut Vec<From>, more: Vec<From>) {
    for f in more {
        if !into.contains(&f) {
            into.push(f);
        }
    }
}

/// Where `e` can come from.
fn origins(m: &Model, vars: &BTreeMap<String, Vec<From>>, e: &TExpr) -> Vec<From> {
    match e {
        TExpr::Int(n) => vec![From::Range(Range::exactly(*n))],
        TExpr::Now => vec![From::Now],
        TExpr::None(_) => vec![],
        TExpr::List { items, .. } => {
            let mut out = Vec::new();
            for x in items {
                add(&mut out, origins(m, vars, x));
            }
            out
        }
        TExpr::Var { name, fields, .. } if fields.is_empty() => vars.get(name).cloned().unwrap_or_default(),
        TExpr::Var { name, fields, .. } => {
            // the record the last field belongs to
            let Some(mut ty) = m.var_ty(name).cloned() else { return vec![] };
            let mut at = None;
            for f in fields {
                let Ty::Record(r) = ty.inner().clone() else { return vec![] };
                let Some(t) = m.field_ty(r, f) else { return vec![] };
                at = Some((r, f));
                ty = t.clone();
            }
            let Some((r, f)) = at else { return vec![] };
            let rd = &m.records[r];
            let unknown = || From::Unknown(tr!("`{}` のフィールド `{f}`", "the field `{f}` of `{}`", rd.name));
            match &rd.origin {
                RecordOrigin::RuleOutputs(ix) => match &m.rules[*ix].kind {
                    RuleKind::Date(_) if f == "day" => vec![From::Day(*ix)],
                    RuleKind::Rule => vec![From::Output(*ix, f.clone())],
                    _ => vec![unknown()],
                },
                _ => match m.field_range(r, f) {
                    Some(rg) => vec![From::Range(rg)],
                    None => vec![unknown()],
                },
            }
        }
        other => vec![From::Unknown(tr!("`{}`", "`{}`", other.show()))],
    }
}

/// Where each variable can come from: every value put in it, joined, until nothing more changes.
fn variables(m: &Model) -> BTreeMap<String, Vec<From>> {
    let mut vars: BTreeMap<String, Vec<From>> = BTreeMap::new();
    for (n, _) in &m.inputs {
        let from = match m.input_ranges.get(n) {
            Some(r) => From::Range(*r),
            None => From::Unknown(tr!("入力 `{n}`", "the input `{n}`")),
        };
        vars.insert(n.clone(), vec![from]);
    }
    let stmts = m.all_stmts();
    for _ in 0..=m.vars.len() + 1 {
        let mut next = vars.clone();
        for s in &stmts {
            let mut put = |name: &str, more: Vec<From>| add(next.entry(name.to_string()).or_default(), more);
            match &s.kind {
                TK::Call { target: Some(Target::Let(x)), callee: Callee::Task(t), .. } => {
                    let t = &m.tasks[*t];
                    let from = match t.result_range {
                        Some(r) => From::Range(r),
                        None => From::Unknown(tr!("`{}` の結果", "the answer of `{}`", t.name)),
                    };
                    put(x, vec![from]);
                }
                TK::Assign { name, expr } => put(name, origins(m, &vars, expr)),
                TK::For { var, list, result, .. } => {
                    put(var, origins(m, &vars, list));
                    if let Some((r, y)) = result {
                        put(r, origins(m, &vars, y));
                    }
                }
                TK::Match { expr, arms } => {
                    for a in arms {
                        if let Some(x) = &a.some {
                            put(x, origins(m, &vars, expr));
                        }
                    }
                }
                _ => {}
            }
        }
        if next == vars {
            break;
        }
        vars = next;
    }
    vars
}

fn port(m: &Model, f: &From) -> Origin {
    match f {
        From::Day(ix) => Origin::Day { file: m.rules[*ix].info.path.clone(), date: m.rules[*ix].date().map(|d| d.date.clone()).unwrap_or_default() },
        From::Output(ix, o) => Origin::Output { rule: m.rules[*ix].info.path.clone(), output: o.clone() },
        From::Now => Origin::Now,
        From::Range(r) => Origin::Range(r.lo.map(i128::from), r.hi.map(i128::from)),
        From::Unknown(t) => Origin::Unknown(t.clone()),
    }
}

/// Everything the checked flow `m` calls in the other languages, and how long each of its holds
/// can be held before a call that its expiry can refuse.
pub fn of(m: &Model) -> Crossings {
    let vars = variables(m);
    let ranges = crate::ranges::variables(m);
    let from = |e: &TExpr| origins(m, &vars, e).iter().map(|f| port(m, f)).collect::<Vec<_>>();
    let mut out = Crossings::default();
    for s in m.all_stmts() {
        let TK::Call { callee, args, .. } = &s.kind else { continue };
        match callee {
            Callee::Rule(r) if m.rules[*r].is_rule() => out.rules.push(RuleCall {
                line: s.line,
                rule: m.rules[*r].info.path.clone(),
                name: m.rules[*r].name.clone(),
                args: args
                    .iter()
                    .map(|(p, e)| {
                        let est = crate::ranges::estimate(m, &ranges, e);
                        let range = if est.unknown.is_none() { est.known } else { None };
                        CallArg { input: p.clone(), shown: e.show(), range: range.map(|r| (r.lo.map(i128::from), r.hi.map(i128::from))), unknown: est.unknown.clone(), from: from(e) }
                    })
                    .collect(),
            }),
            Callee::Rule(r) => {
                let Some(d) = m.rules[*r].date() else { continue };
                let Some((input, ..)) = d.params.iter().find(|p| p.2) else { continue };
                let Some((_, e)) = args.iter().find(|(p, _)| p == input) else { continue };
                out.dates.push(PortDate { line: s.line, file: m.rules[*r].info.path.clone(), name: d.file.clone(), date: d.date.clone(), input: input.clone(), shown: e.show(), from: from(e) });
            }
            Callee::Task(t) => {
                let task = &m.tasks[*t];
                let Some(op) = task.book() else { continue };
                let book = &m.books[op.book];
                let Some(tr) = book.transfer(&op.transfer) else { continue };
                let amounts = tr.params.iter().filter(|p| p.unit.is_some()).filter_map(|p| args.iter().find(|(n, _)| *n == p.name).map(|(_, e)| Amount { param: p.name.clone(), shown: e.show(), from: from(e) })).collect();
                out.transfers.push(TransferCall {
                    line: s.line,
                    book: book.path.clone(),
                    transfer: op.transfer.clone(),
                    op: op.op.clone(),
                    task: task.name.clone(),
                    handles: task.errors.iter().map(|e| e.name.clone()).collect(),
                    amounts,
                });
            }
        }
    }
    out.holds = Spans::of(m);
    out
}

/// Seconds since a hold was made, at a point of the flow.
#[derive(Clone, Debug, PartialEq)]
struct Since {
    lo: u64,
    /// None: nothing bounds it
    hi: Option<u64>,
    /// what makes up `lo`, a statement at a time
    why: Vec<Text>,
    /// why `hi` is None: the first thing that bounds nothing
    open: Option<Text>,
    /// the line of the call that made the hold (the first, when more than one can)
    made: usize,
}

impl Since {
    /// Add a statement that takes `lo` seconds at the fewest and `hi` at the most.
    fn add(&mut self, lo: u64, hi: &Result<u64, Text>, why: Option<Text>) {
        self.lo += lo;
        if let (true, Some(w)) = (lo > 0, why) {
            self.why.push(w);
        }
        match hi {
            Ok(h) => self.hi = self.hi.map(|x| x + h),
            Err(t) => {
                if self.hi.is_some() {
                    self.hi = None;
                    self.open = Some(t.clone());
                }
            }
        }
    }

    fn join(a: &Since, b: &Since) -> Since {
        let low = if a.lo <= b.lo { a } else { b };
        let hi = a.hi.zip(b.hi).map(|(x, y)| x.max(y));
        let open = if hi.is_none() { if a.hi.is_none() { a.open.clone() } else { b.open.clone() } } else { None };
        Since { lo: low.lo, hi, why: low.why.clone(), open, made: a.made.min(b.made) }
    }
}

fn join_one(a: &Option<Since>, b: &Option<Since>) -> Option<Since> {
    match (a, b) {
        (Some(a), Some(b)) => Some(Since::join(a, b)),
        (Some(x), None) | (None, Some(x)) => Some(x.clone()),
        (None, None) => None,
    }
}

/// A variable that holds the answer of a koyomi date whose date input was given `now`: the date,
/// the line, and how old each hold was when `now` was read.
#[derive(Clone, Debug, PartialEq)]
struct Anchor {
    rule: usize,
    line: usize,
    since: Vec<Option<Since>>,
}

/// What is known at a point of the flow, on the ways that reach it.
#[derive(Clone, Debug, PartialEq)]
struct State {
    /// for each case: how old its hold is, when it is made
    cases: Vec<Option<Since>>,
    /// the variables that hold the answer of a koyomi date given `now`
    anchors: BTreeMap<String, Anchor>,
    /// the variables given `now` itself: the line, and how old each hold was then
    nows: BTreeMap<String, (usize, Vec<Option<Since>>)>,
}

impl State {
    fn forget(&mut self, name: &str) {
        self.anchors.remove(name);
        self.nows.remove(name);
    }
}

fn join_states(a: Option<State>, b: Option<State>) -> Option<State> {
    match (a, b) {
        (Some(a), Some(b)) => {
            let cases = a.cases.iter().zip(&b.cases).map(|(x, y)| join_one(x, y)).collect();
            let since = |x: &[Option<Since>], y: &[Option<Since>]| x.iter().zip(y).map(|(p, q)| join_one(p, q)).collect::<Vec<_>>();
            let anchors = a
                .anchors
                .iter()
                .filter_map(|(k, x)| b.anchors.get(k).filter(|y| y.rule == x.rule).map(|y| (k.clone(), Anchor { rule: x.rule, line: x.line.min(y.line), since: since(&x.since, &y.since) })))
                .collect();
            let nows = a.nows.iter().filter_map(|(k, x)| b.nows.get(k).map(|y| (k.clone(), (x.0.min(y.0), since(&x.1, &y.1))))).collect();
            Some(State { cases, anchors, nows })
        }
        (x, None) | (None, x) => x,
    }
}

/// The variables a block gives a value to.
fn assigned(stmts: &[TStmt]) -> Vec<String> {
    let mut out = Vec::new();
    Model::walk(stmts, &mut |s| match &s.kind {
        TK::Call { target: Some(Target::Let(x)), .. } | TK::Assign { name: x, .. } => out.push(x.clone()),
        TK::For { var, result, .. } => {
            out.push(var.clone());
            if let Some((r, _)) = result {
                out.push(r.clone());
            }
        }
        TK::Match { arms, .. } => out.extend(arms.iter().filter_map(|a| a.some.clone())),
        _ => {}
    });
    out
}

/// The walk of the ways through a flow, for the spans of its holds.
struct Spans<'a> {
    m: &'a Model,
    /// for each case: the seconds after which its hold expires, when it follows a hold that does
    expiring: Vec<Option<u64>>,
    out: Vec<HoldSpan>,
    /// for each case: how old its hold can be at a point where a failure or a cancellation can stop
    /// the run, for `on failure` and `on cancel`
    stops: Vec<Option<Since>>,
    /// for each loop being walked: the states at its `break`s
    breaks: Vec<Vec<State>>,
}

impl<'a> Spans<'a> {
    fn of(m: &'a Model) -> Vec<HoldSpan> {
        let expiring: Vec<Option<u64>> = m
            .cases
            .iter()
            .map(|c| match &m.rules[c.rule].kind {
                RuleKind::Hold { book, transfer } => match m.books[*book].transfer(transfer).and_then(|t| t.pending) {
                    Some(ritsu_ports::Expiry::After(secs)) => Some(secs),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        if expiring.iter().all(Option::is_none) {
            return vec![];
        }
        let n = m.cases.len();
        let mut w = Spans { m, expiring, out: Vec::new(), stops: vec![None; n], breaks: Vec::new() };
        let start = State { cases: vec![None; n], anchors: BTreeMap::new(), nows: BTreeMap::new() };
        w.block(&m.flow, start);
        // `on failure` and `on cancel` start where the run can be stopped
        let stopped = State { cases: w.stops.iter().map(|s| s.clone().map(|s| Since { lo: 0, why: vec![], ..s })).collect(), anchors: BTreeMap::new(), nows: BTreeMap::new() };
        for block in [&m.on_failure, &m.on_cancel].into_iter().flatten() {
            w.block(block, stopped.clone());
        }
        w.out.sort_by_key(|s| s.line);
        w.out
    }

    fn block(&mut self, stmts: &[TStmt], mut st: State) -> Option<State> {
        for s in stmts {
            st = self.stmt(s, st)?;
        }
        Some(st)
    }

    /// A failure or a cancellation can stop the run here.
    fn stoppable(&mut self, st: &State) {
        for (i, c) in st.cases.iter().enumerate() {
            self.stops[i] = join_one(&self.stops[i], c);
        }
    }

    /// The most seconds a call can take, or why nothing bounds it.
    fn duration(&self, callee: &Callee, line: usize) -> Result<u64, Text> {
        match callee {
            Callee::Task(t) => {
                let t = &self.m.tasks[*t];
                let Some(timeout) = t.timeout else {
                    return Err(tr!("{line} 行目の `{}` には `timeout` がありません", "`{}` (line {line}) has no `timeout`", t.name; t.name));
                };
                let (times, waits) = match &t.retry {
                    Some(r) => (u64::from(r.times), (0..r.times).map(|k| (r.every as f64 * r.backoff.powi(k as i32)).ceil() as u64).sum()),
                    None => (0, 0),
                };
                Ok((times + 1) * timeout + waits)
            }
            Callee::Rule(r) => {
                let u = &self.m.rules[*r];
                Err(if u.date().is_some() {
                    tr!("{line} 行目の日付 `{}` の呼び出しにかかる時間は、`.flow` が限りません", "nothing in the flow bounds how long the call of the date `{}` (line {line}) takes", u.name; u.name)
                } else {
                    tr!("{line} 行目の規則 `{}` の呼び出しにかかる時間は、`.flow` が限りません", "nothing in the flow bounds how long the call of the rule `{}` (line {line}) takes", u.name; u.name)
                })
            }
        }
    }

    /// The most seconds one way through a block can take.
    fn longest(&self, stmts: &[TStmt]) -> Result<u64, Text> {
        let mut total = 0u64;
        for s in stmts {
            total += match &s.kind {
                TK::Call { callee, handlers, .. } => {
                    let mut h = 0;
                    for x in handlers {
                        h = h.max(self.longest(&x.body)?);
                    }
                    self.duration(callee, s.line)? + h
                }
                TK::Match { arms, .. } => {
                    let mut most = 0;
                    for a in arms {
                        most = most.max(self.longest(&a.body)?);
                    }
                    most
                }
                TK::Wait { seconds } => *seconds,
                TK::WaitUntil { .. } => return Err(until_open(s.line)),
                TK::Repeat { times, body } => u64::from(*times) * self.longest(body)?,
                TK::For { max, body, .. } => u64::from(*max) * self.longest(body)?,
                _ => 0,
            };
        }
        Ok(total)
    }

    fn stmt(&mut self, s: &TStmt, mut st: State) -> Option<State> {
        match &s.kind {
            TK::Call { target, callee, args, handlers } => {
                let dur = self.duration(callee, s.line);
                let at_start = st.cases.clone();
                if let (Some(Target::Case(c)), Callee::Task(t)) = (target, callee) {
                    // a post or a void, which the hold's expiry can refuse
                    if let (true, Some(TaskMachine::Sends { event, .. }), Some(since)) = (self.expiring[*c].is_some(), &self.m.tasks[*t].machine, st.cases[*c].clone()) {
                        if event == "post" || event == "void" {
                            self.point(*c, s.line, *t, event, &since, &dur);
                        }
                    }
                }
                for c in st.cases.iter_mut().flatten() {
                    c.add(0, &dur, None);
                }
                // the hold this call makes, somewhere inside its run
                if let (Some(Target::Case(c)), Callee::Task(t)) = (target, callee) {
                    if self.expiring[*c].is_some() && matches!(self.m.tasks[*t].machine, Some(TaskMachine::Starts { .. })) {
                        let open = dur.as_ref().err().cloned();
                        st.cases[*c] = Some(Since { lo: 0, hi: dur.as_ref().ok().copied(), why: vec![], open, made: s.line });
                    }
                }
                self.stoppable(&st);
                if let Some(Target::Let(x)) = target {
                    st.forget(x);
                    // the answer of a koyomi date whose date input is given `now`, read as the statement starts
                    let given = match callee {
                        Callee::Rule(r) => self.m.rules[*r].date().and_then(|d| d.params.iter().find(|p| p.2)).and_then(|(input, ..)| args.iter().find(|(p, _)| p == input)).map(|(_, e)| (*r, e)),
                        Callee::Task(_) => None,
                    };
                    if let Some((r, e)) = given {
                        let since = match e {
                            TExpr::Now => Some(at_start),
                            TExpr::Var { name, fields, .. } if fields.is_empty() => st.nows.get(name).map(|n| n.1.clone()),
                            _ => None,
                        };
                        if let Some(since) = since {
                            st.anchors.insert(x.clone(), Anchor { rule: r, line: s.line, since });
                        }
                    }
                }
                let mut out = Some(st.clone());
                for h in handlers {
                    let hs = self.block(&h.body, st.clone());
                    out = join_states(out, hs);
                }
                out
            }
            TK::Assign { name, expr } => {
                let copied = match expr {
                    TExpr::Var { name: from, fields, .. } if fields.is_empty() => (st.anchors.get(from).cloned(), st.nows.get(from).cloned()),
                    _ => (None, None),
                };
                st.forget(name);
                if matches!(expr, TExpr::Now) {
                    st.nows.insert(name.clone(), (s.line, st.cases.clone()));
                }
                if let Some(a) = copied.0 {
                    st.anchors.insert(name.clone(), a);
                }
                if let Some(n) = copied.1 {
                    st.nows.insert(name.clone(), n);
                }
                Some(st)
            }
            TK::Match { arms, .. } => {
                let mut out = None;
                for a in arms {
                    let mut ast = st.clone();
                    if let Some(x) = &a.some {
                        ast.forget(x);
                    }
                    let end = self.block(&a.body, ast);
                    out = join_states(out, end);
                }
                out
            }
            TK::Wait { seconds } => {
                let why = tr!("{} 行目の `wait` が {}待ちます", "the wait of line {} waits {}", s.line, seconds_text(*seconds).ja; s.line, seconds_text(*seconds).en);
                for c in st.cases.iter_mut().flatten() {
                    c.add(*seconds, &Ok(*seconds), Some(why.clone()));
                }
                self.stoppable(&st);
                Some(st)
            }
            TK::WaitUntil { at } => {
                self.until(&mut st, at, s.line);
                self.stoppable(&st);
                Some(st)
            }
            TK::Repeat { times, body } => self.rounds(u64::from(*times), body, st),
            TK::For { var, result, max, body, .. } => {
                st.forget(var);
                if let Some((r, _)) = result {
                    st.forget(r);
                }
                self.rounds(u64::from(*max), body, st)
            }
            TK::Break => {
                if let Some(b) = self.breaks.last_mut() {
                    b.push(st);
                }
                None
            }
            // a check of a precondition takes no time: the run goes on as it was, or fails there
            TK::Pass | TK::Check(_) => Some(st),
            TK::Succeed { .. } | TK::Fail { .. } => None,
        }
    }

    /// A loop of at most `n` rounds.
    fn rounds(&mut self, n: u64, body: &[TStmt], st: State) -> Option<State> {
        let round = self.longest(body);
        let more = |k: u64| round.clone().map(|r| r * k);
        let set = assigned(body);
        // a round after the first starts after the rounds before it
        let mut entry = st.clone();
        for c in entry.cases.iter_mut().flatten() {
            c.add(0, &more(n.saturating_sub(1)), None);
        }
        for v in &set {
            entry.forget(v);
        }
        self.breaks.push(Vec::new());
        let end = self.block(body, entry);
        let breaks = self.breaks.pop().unwrap_or_default();
        let inner = breaks.into_iter().fold(end, |acc, b| join_states(acc, Some(b)));
        // after the loop: a hold made before it was held through every round; a hold made in a
        // round, through the rounds after it
        let mut after = st;
        for c in after.cases.iter_mut().flatten() {
            c.add(0, &more(n), None);
        }
        if let Some(inner) = inner {
            for (i, c) in inner.cases.iter().enumerate() {
                if let (true, Some(c)) = (after.cases[i].is_none(), c) {
                    let mut c = c.clone();
                    c.add(0, &more(n.saturating_sub(1)), None);
                    after.cases[i] = Some(c);
                }
            }
        }
        for v in &set {
            after.forget(v);
        }
        Some(after)
    }

    /// `wait until <at>`.
    fn until(&mut self, st: &mut State, at: &TExpr, line: usize) {
        let anchor = match at {
            TExpr::Var { name, fields, .. } if fields.len() == 1 && fields[0] == "at" => st.anchors.get(name).cloned(),
            _ => None,
        };
        // the date's days from its input as koyomi counts them, and the minutes of its `at`
        let counted = anchor.as_ref().and_then(|a| {
            let u = &self.m.rules[a.rule];
            let d = u.date()?;
            let facts = crate::sources::dates(&u.info.path).ok()?;
            let minutes = facts.functions.iter().find(|f| f.name == d.date)?.at?;
            match crate::sources::date_span(&u.info.path, &d.date).ok()? {
                Found::Value(span) => Some((span, minutes, d.clone(), facts.inputs.iter().find(|i| i.kind == ritsu_ports::DateKind::Date).map(|i| i.name.clone()).unwrap_or_default())),
                Found::Undecided(_) => None,
            }
        });
        let shown = at.show();
        for (i, cur) in st.cases.iter_mut().enumerate() {
            let Some(cur) = cur else { continue };
            let found = anchor.as_ref().zip(counted.as_ref()).and_then(|(a, c)| a.since[i].as_ref().map(|s| (a, s, c)));
            let Some((a, since, (span, minutes, d, input))) = found else {
                cur.add(0, &Err(until_open_of(line, &shown)), None);
                continue;
            };
            let at_secs = i64::from(*minutes) * 60;
            let least = since.lo as i64 + (span.fewest - 1) * 86_400 + at_secs;
            if least > cur.lo as i64 {
                let example = match span.fewest_at {
                    Some(day) => {
                        let (from, to) = (ritsu_ports::day_text(day), ritsu_ports::day_text(day + span.fewest));
                        Text { ja: format!("（{from} なら {to}）"), en: format!(" (from {from} to {to})") }
                    }
                    None => Text::default(),
                };
                let clock = format!("{:02}:{:02}", minutes / 60, minutes % 60);
                let why = tr!(
                    "{line} 行目の `wait until {shown}` は、koyomi の日付 `{}` の時刻まで待ちます。その日は `{input}` から早くて {} 日後{}の {clock} で、日付に渡した `now` は仮押さえのあと（{} 行目）に読んでいます",
                    "the wait until `{shown}` (line {line}) lasts until the time of koyomi's date `{}`: {} days after `{input}` at the fewest{}, at {clock}, and the `now` given to the date was read after the hold (line {})",
                    d.date, span.fewest, example.ja, a.line; d.date, span.fewest, example.en, a.line
                );
                cur.lo = least as u64;
                cur.why = since.why.clone();
                cur.why.push(why);
            }
            let most = since.hi.map(|h| (h as i64 + span.most * 86_400 + at_secs).max(0) as u64);
            match (cur.hi, most) {
                (Some(x), Some(y)) => cur.hi = Some(x.max(y)),
                (Some(_), None) => {
                    cur.hi = None;
                    cur.open = since.open.clone();
                }
                (None, _) => {}
            }
        }
    }

    /// The span from the hold of the case `c` to the call on it of line `line`.
    fn point(&mut self, c: usize, line: usize, t: usize, op: &str, since: &Since, dur: &Result<u64, Text>) {
        let case = &self.m.cases[c];
        let RuleKind::Hold { book, transfer } = &self.m.rules[case.rule].kind else { return };
        let most = match (since.hi, dur) {
            (Some(h), Ok(d)) => Ok(h + d),
            (None, _) => Err(since.open.clone().unwrap_or_else(|| tr!("上限が分かりません", "nothing bounds it"))),
            (Some(_), Err(why)) => Err(why.clone()),
        };
        self.out.push(HoldSpan {
            case: case.name.clone(),
            book: self.m.books[*book].path.clone(),
            transfer: transfer.clone(),
            made: since.made,
            line,
            task: self.m.tasks[t].name.clone(),
            op: op.to_string(),
            least: since.lo,
            least_why: since.why.clone(),
            most,
        });
    }
}

fn until_open(line: usize) -> Text {
    tr!("{line} 行目の `wait until` がいつまで待つかに上限がありません", "nothing bounds how long the wait until of line {line} waits")
}

fn until_open_of(line: usize, shown: &str) -> Text {
    tr!("{line} 行目の `wait until {shown}` がいつまで待つかに上限がありません", "nothing bounds how long the wait until `{shown}` (line {line}) waits")
}
