//! What a flow calls in the other languages, for ritsu's checks across the borders (ritsu's DESIGN
//! 7.4–7.8; the port of flows, `ritsu_ports::Flows::crossings`): every call of a rule, of a koyomi
//! date and of a chobo transfer, with the places each value it gives can come from, and how long
//! each hold of a book can be held before a call its expiry can refuse.
//!
//! Where a value comes from is gathered as dandori gathers ranges (`ranges`): a variable given on
//! comes from where the runs that reach the call last gave it a value (`Facts::from`), and where
//! the walk of the runs does not follow it (a variable the rounds of a loop give a value), from
//! every place it is given a value anywhere in the flow, joined (`variables`). The day of a koyomi
//! date and the output
//! of a rule are known by the record they are read from (`due.day`, `fee.amount`), whose type says
//! which date or rule answered it.
//!
//! The calls of rules whose outputs an amount given to a transfer can be (ritsu's X4) are gathered
//! along the runs through the flow instead (`Answers`): the calls whose answers the runs that reach
//! the transfer carry to it, and what those runs can give each call, less what the arms of a
//! `match` before the transfer leave out. The same walk says what the runs that reach a call of an
//! operation of a contract hold there (`reached`, ritsu's X16): what each `match` on the way leaves
//! of the value it read, the calls of rules whose answers the variables still hold, and the
//! variables every run has given a value.
//!
//! How long a hold is held is gathered along the ways through the flow, as seconds since the hold
//! was made, the fewest and the most:
//!
//! - The call that makes the hold makes it somewhere inside its own run: after it, the hold is 0
//!   seconds old at the fewest, and at most as old as the call can take.
//! - A call takes 0 seconds at the fewest, and at the most its `timeout` for each attempt its
//!   `retry` allows, with the waits between them. An attempt of a callback hands its id on before
//!   it waits for the answer, which every target gives a minute at the most
//!   (`check::CALLBACK_HAND_ON_SECONDS`, dandori's DESIGN 17), so each counts its `timeout` and a
//!   minute. A task with no `timeout` leaves nothing bounding the most. A rule or a date takes at
//!   most 33 seconds: every target gives each try of its call 10 seconds and retries it twice, 1
//!   and 2 seconds apart (`check::rule_call_most`, dandori's DESIGN 16).
//! - A task with `timeout until x.at` waits until the time, as `wait until x.at` below does, and a
//!   second more at the most, for the wait is rounded up to the second. A callback hands its id on
//!   first, so its wait ends at the time, or a second after the id is handed on (a minute after the
//!   call starts at the most) when that is later; each retry after a failure waits between, hands
//!   the id on again and waits again, a minute and a second more than the time at the most. When
//!   koyomi does not count the time, nothing bounds the most.
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
//! - A handler of `timeout` alone, under a task that waits for an event, starts when the event's
//!   timer runs out: its `timeout` (a day without one) after the call starts, at the fewest; with
//!   `timeout until x.at`, no sooner than the time, counted as for `wait until x.at`, and as the
//!   call starts when koyomi does not count it. Any other task's timeout can come sooner (on
//!   Temporal, an activity the workflow's own worker serves times out when its heartbeats stop for
//!   30 seconds, and on every target a callback times out when it does not hand its id on within a
//!   minute), so its handlers start as the call ends, 0 seconds at the fewest.
//! - `on failure` and `on cancel` start where a failure or a cancellation can stop the run: the
//!   hold is 0 seconds old at the fewest, and at the most as old as it can be at any call or wait.
//!
//! A call that posts or voids the hold acts somewhere inside its own run, so its span runs from the
//! fewest seconds as it starts to the most as it ends.

use crate::diag::Text;
use crate::model::*;
use ritsu_ports::{seconds_text, Amount, CallArg, CallInput, Crossings, DateCall as PortDate, Found, Held as PortHeld, HoldSpan, Origin, OutputCall, RuleCall, TransferCall};
use std::collections::{BTreeMap, BTreeSet};

/// Where a value can come from, by the model's own indices.
#[derive(Clone, Debug, PartialEq)]
enum From {
    /// the day of the date `rules[ix]`
    Day(usize),
    /// an output of the rule `rules[ix]`
    Output(usize, String),
    Now,
    Range(Range),
    /// a time with a range, or `now` held to the period (whether it is `now`): its moments, and the
    /// range as the `.flow` writes it
    Moment(Range, String, bool),
    Unknown(Text),
}

/// Where a value read from a place of the type `t` with the range `rg` comes from: a time's range
/// is moments, a date's days, and an enum's is not numbers.
fn placed(m: &Model, t: &Ty, rg: Option<Range>, unknown: impl FnOnce() -> From) -> From {
    let mut inner = t;
    while let Ty::Opt(x) | Ty::List(x) = inner {
        inner = x;
    }
    match rg {
        Some(r) if *inner == Ty::Timestamp => From::Moment(r, m.range_text(t, &r), false),
        Some(r) if t.enum_inside().is_none() => From::Range(r),
        _ => unknown(),
    }
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
        // `now` read by a check of the period (DESIGN 20): a moment in the period
        TExpr::NowAt(_) => match &m.period {
            Some(p) => vec![From::Moment(p.range, p.text.clone(), true)],
            None => vec![From::Now],
        },
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
                _ => vec![placed(m, &ty, m.field_range(r, f), unknown)],
            }
        }
        other => vec![From::Unknown(tr!("`{}`", "`{}`", other.show()))],
    }
}

/// Where the input `n` of the type `t` comes from.
fn from_input(m: &Model, n: &str, t: &Ty) -> From {
    placed(m, t, m.input_ranges.get(n).copied(), || From::Unknown(tr!("入力 `{n}`", "the input `{n}`")))
}

/// Where the answer of the task `t` comes from.
fn from_answer(m: &Model, t: &TaskDef) -> From {
    let unknown = || From::Unknown(tr!("`{}` の結果", "the answer of `{}`", t.name));
    match &t.result {
        Some(ty) => placed(m, ty, t.result_range, unknown),
        None => t.result_range.map(From::Range).unwrap_or_else(unknown),
    }
}

/// Where each variable can come from anywhere in the flow: every value put in it, joined, until
/// nothing more changes. What the runs that reach a statement last put in it is `Facts::from`.
fn variables(m: &Model) -> BTreeMap<String, Vec<From>> {
    let mut vars: BTreeMap<String, Vec<From>> = BTreeMap::new();
    for (n, t) in &m.inputs {
        vars.insert(n.clone(), vec![from_input(m, n, t)]);
    }
    let stmts = m.all_stmts();
    for _ in 0..=m.vars.len() + 1 {
        let mut next = vars.clone();
        for s in &stmts {
            let mut put = |name: &str, more: Vec<From>| add(next.entry(name.to_string()).or_default(), more);
            match &s.kind {
                TK::Call { target: Some(Target::Let(x)), callee: Callee::Task(t), .. } => put(x, vec![from_answer(m, &m.tasks[*t])]),
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
        From::Moment(r, text, now) => Origin::Moment { lo: r.lo, hi: r.hi, text: text.clone(), now: *now },
        From::Unknown(t) => Origin::Unknown(t.clone()),
    }
}

/// Everything the checked flow `m` calls in the other languages, and how long each of its holds
/// can be held before a call that its expiry can refuse.
pub fn of(m: &Model) -> Crossings {
    let vars = variables(m);
    let ranges = crate::ranges::variables(m);
    let answered = Answers::of(m, &vars, &ranges);
    let (values, here) = calls_here(m);
    let mut out = Crossings::default();
    for s in m.all_stmts() {
        let TK::Call { callee, args, .. } = &s.kind else { continue };
        // what each variable is on the runs that reach the call, where the walk says less of it
        // than the whole flow does
        let at = here.get(&s.site).map(|h| (over(&vars, &h.from), h.over(&ranges)));
        let (vars, ranges) = match &at {
            Some((v, r)) => (v, r),
            None => (&vars, &ranges),
        };
        let from = |e: &TExpr| origins(m, vars, e).iter().map(|f| port(m, f)).collect::<Vec<_>>();
        match callee {
            Callee::Rule(r) if m.rules[*r].is_rule() => out.rules.push(RuleCall {
                line: s.line,
                rule: m.rules[*r].info.path.clone(),
                name: m.rules[*r].name.clone(),
                args: args
                    .iter()
                    .map(|(p, e)| {
                        let est = crate::ranges::estimate(m, ranges, e);
                        let range = if est.unknown.is_none() { est.known } else { None };
                        CallArg { input: p.clone(), shown: m.shown_at(s.site, p, e), range: range.map(|r| (r.lo.map(i128::from), r.hi.map(i128::from))), unknown: est.unknown.clone(), values: values.get(&(s.site, p.clone())).cloned(), from: from(e) }
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
                let amounts = tr
                    .params
                    .iter()
                    .filter(|p| p.unit.is_some())
                    .filter_map(|p| {
                        let (_, e) = args.iter().find(|(n, _)| *n == p.name)?;
                        let calls = answered.get(&(s.site, p.name.clone())).cloned().flatten();
                        Some(Amount { param: p.name.clone(), shown: m.shown_at(s.site, &p.name, e), from: from(e), calls })
                    })
                    .collect();
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
    /// what makes `hi` as long as it is, when koyomi's days do: the waits until a date's time, and
    /// the day that makes each longest
    most_why: Vec<Text>,
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
            Err(t) => self.unbound(t),
        }
    }

    /// Nothing bounds the most from here on.
    fn unbound(&mut self, why: &Text) {
        if self.hi.is_some() {
            self.hi = None;
            self.open = Some(why.clone());
            self.most_why.clear();
        }
    }

    /// The most becomes at least `most` (seconds since the hold), for the reasons `why`.
    fn reach(&mut self, most: u64, why: Vec<Text>) {
        if let Some(x) = self.hi {
            if most >= x {
                self.hi = Some(most);
                self.most_why = why;
            }
        }
    }

    fn join(a: &Since, b: &Since) -> Since {
        let low = if a.lo <= b.lo { a } else { b };
        let hi = a.hi.zip(b.hi).map(|(x, y)| x.max(y));
        let open = if hi.is_none() { if a.hi.is_none() { a.open.clone() } else { b.open.clone() } } else { None };
        let high = if b.hi > a.hi { b } else { a };
        let most_why = if hi.is_some() { high.most_why.clone() } else { vec![] };
        Since { lo: low.lo, hi, why: low.why.clone(), open, most_why, made: a.made.min(b.made) }
    }
}

/// The time a call of a task with `timeout until` waits until, or a `wait until` waits until, when
/// it is the time of a koyomi date whose date input was given `now` read after a hold: the variable
/// that holds the date's answer, the days koyomi counts from the input to the date, the minutes of
/// its `at`, the date, and its date input.
struct Counted {
    anchor: Anchor,
    span: ritsu_ports::DaySpan,
    minutes: u32,
    date: DateCall,
    input: String,
}

impl Counted {
    /// The fewest and the most seconds from the hold `i` to the time, when `now` was read after it.
    fn bounds(&self, i: usize) -> Option<(i64, Option<u64>, &Since)> {
        let since = self.anchor.since[i].as_ref()?;
        let at_secs = i64::from(self.minutes) * 60;
        let least = since.lo as i64 + (self.span.fewest - 1) * 86_400 + at_secs;
        let most = since.hi.map(|h| (h as i64 + self.span.most * 86_400 + at_secs).max(0) as u64);
        Some((least, most, since))
    }

    fn clock(&self) -> String {
        format!("{:02}:{:02}", self.minutes / 60, self.minutes % 60)
    }

    /// The day the date's input falls on at the fewest days, and the date's day then: ` (from … to …)`.
    fn fewest_example(&self) -> Text {
        match self.span.fewest_at {
            Some(day) => {
                let (from, to) = (ritsu_ports::day_text(day), ritsu_ports::day_text(day + self.span.fewest));
                Text { ja: format!("（{from} なら {to}）"), en: format!(" (from {from} to {to})") }
            }
            None => Text::default(),
        }
    }

    /// The day `now` falls on that makes the wait the longest, and what the wait then lasts until,
    /// as `wait` says it of the date's day (`to`), the time of `at` and the days after `now`'s day:
    /// none when koyomi does not say the day.
    fn longest(&self, wait: impl Fn(&str, &str, i64) -> Text) -> Option<Text> {
        let day = self.span.most_at?;
        let (from, to) = (ritsu_ports::day_text(day), ritsu_ports::day_text(day + self.span.most));
        let (a, date) = (self.anchor.line, &self.date.date);
        let w = wait(&to, &self.clock(), self.span.most);
        Some(tr!("いちばん長くなるのは、{a} 行目の `{date}` に渡した `now` が {from} のときです。{}", "The longest is when the `now` given to `{date}` (line {a}) falls on {from}: {}", w.ja; w.en))
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
    fn duration(&self, callee: &Callee, args: &[(String, TExpr)], line: usize) -> Result<u64, Text> {
        match callee {
            Callee::Task(t) => {
                let t = &self.m.tasks[*t];
                // `timeout until`: the wait ends at a time, which `stmt` counts from where it comes from
                if let Some(at) = crate::render::deadline_arg(self.m, callee, args) {
                    return Err(deadline_open(line, &t.name, &at.show()));
                }
                let Some(timeout) = t.timeout else {
                    return Err(tr!("{line} 行目の `{}` には `timeout` がありません", "`{}` (line {line}) has no `timeout`", t.name; t.name));
                };
                let (times, waits) = retries(t);
                Ok((times + 1) * (hand_on(t) + timeout) + waits)
            }
            // every target gives a try of a rule's or a date's call the same limit, and retries it the
            // same (DESIGN 16)
            Callee::Rule(_) => Ok(crate::check::rule_call_most()),
        }
    }

    /// The time a wait lasts until, when it is the time of a koyomi date given `now` read after a hold.
    fn counted(&self, st: &State, at: &TExpr) -> Option<Counted> {
        let anchor = match at {
            TExpr::Var { name, fields, .. } if fields.len() == 1 && fields[0] == "at" => st.anchors.get(name).cloned(),
            _ => None,
        }?;
        // the date's days from its input as koyomi counts them, and the minutes of its `at`
        let u = &self.m.rules[anchor.rule];
        let d = u.date()?;
        let facts = crate::sources::dates(&u.info.path).ok()?;
        let minutes = facts.functions.iter().find(|f| f.name == d.date)?.at?;
        let input = facts.inputs.iter().find(|i| i.kind == ritsu_ports::DateKind::Date).map(|i| i.name.clone()).unwrap_or_default();
        match crate::sources::date_span(&u.info.path, &d.date).ok()? {
            Found::Value(span) => Some(Counted { anchor, span, minutes, date: d.clone(), input }),
            Found::Undecided(_) => None,
        }
    }

    /// A call's own run, from the hold `i`'s age `c` as it starts: its `dur`, or with `timeout until`,
    /// until the time it waits until (`until`), rounded up to the second and a second at the least.
    /// A callback hands its id on first, so its wait ends at the time, or a second after the id is
    /// handed on when that is later; a retry after a failure hands it on and waits again.
    fn through(c: &mut Since, i: usize, dur: &Result<u64, Text>, until: Option<&Until>) {
        let Some(u) = until else {
            c.add(0, dur, None);
            return;
        };
        let found = u.counted.as_ref().and_then(|k| k.bounds(i).map(|b| (k, b)));
        let Some((k, (_, most, since))) = found else {
            c.unbound(&deadline_open(u.line, &u.task, &u.shown));
            return;
        };
        let Some(most) = most else {
            c.unbound(&since.open.clone().unwrap_or_else(|| deadline_open(u.line, &u.task, &u.shown)));
            return;
        };
        if c.hi.is_none() {
            return;
        }
        let (line, task, shown) = (u.line, &u.task, &u.shown);
        let mut why = since.most_why.clone();
        why.extend(k.longest(|to, clock, most| tr!("{line} 行目の `{task}` は、その日から {most} 日後の {to} の {clock}（`{shown}`）まで待ちます", "`{task}` (line {line}) waits until `{shown}`, {to} at {clock}, {most} days after it")));
        c.hi = c.hi.map(|h| h + u.hand_on);
        c.reach(most, why);
        c.hi = c.hi.map(|h| h + 1 + u.again);
    }

    /// The most seconds one way through a block can take.
    fn longest(&self, stmts: &[TStmt]) -> Result<u64, Text> {
        let mut total = 0u64;
        for s in stmts {
            total += match &s.kind {
                TK::Call { callee, args, handlers, .. } => {
                    let mut h = 0;
                    for x in handlers {
                        h = h.max(self.longest(&x.body)?);
                    }
                    self.duration(callee, args, s.line)? + h
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
                let dur = self.duration(callee, args, s.line);
                // `timeout until`: the wait for the answer ends at the time the call gives
                let until = match (callee, crate::render::deadline_arg(self.m, callee, args)) {
                    (Callee::Task(t), Some(at)) => Some(Until::of(&self.m.tasks[*t], s.line, at.show(), self.counted(&st, at))),
                    _ => None,
                };
                let at_start = st.cases.clone();
                let before: Vec<bool> = at_start.iter().map(Option::is_some).collect();
                if let (Some(Target::Case(c)), Callee::Task(t)) = (target, callee) {
                    // a post or a void, which the hold's expiry can refuse
                    if let (true, Some(TaskMachine::Sends { event, .. }), Some(since)) = (self.expiring[*c].is_some(), &self.m.tasks[*t].machine, st.cases[*c].clone()) {
                        if event == "post" || event == "void" {
                            let mut end = since.clone();
                            Spans::through(&mut end, *c, &dur, until.as_ref());
                            self.point(*c, s.line, *t, event, &since, &end);
                        }
                    }
                }
                for (i, c) in st.cases.iter_mut().enumerate() {
                    if let Some(c) = c {
                        Spans::through(c, i, &dur, until.as_ref());
                    }
                }
                // the hold this call makes, somewhere inside its run
                if let (Some(Target::Case(c)), Callee::Task(t)) = (target, callee) {
                    if self.expiring[*c].is_some() && matches!(self.m.tasks[*t].machine, Some(TaskMachine::Starts { .. })) {
                        let open = dur.as_ref().err().cloned();
                        st.cases[*c] = Some(Since { lo: 0, hi: dur.as_ref().ok().copied(), why: vec![], open, most_why: vec![], made: s.line });
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
                            TExpr::Now | TExpr::NowAt(_) => Some(at_start),
                            TExpr::Var { name, fields, .. } if fields.is_empty() => st.nows.get(name).map(|n| n.1.clone()),
                            _ => None,
                        };
                        if let Some(since) = since {
                            st.anchors.insert(x.clone(), Anchor { rule: r, line: s.line, since });
                        }
                    }
                }
                // a task that waits for an event times out only when its own timer runs out: its
                // `timeout`, or a day without one; with `timeout until`, the time it waits until
                let timer = match callee {
                    Callee::Task(t) if self.m.tasks[*t].event && until.is_none() => Some((self.m.tasks[*t].timeout.unwrap_or(86_400), &self.m.tasks[*t].name)),
                    _ => None,
                };
                let event = matches!(callee, Callee::Task(t) if self.m.tasks[*t].event);
                let mut out = Some(st.clone());
                for h in handlers {
                    let mut entry = st.clone();
                    if let (true, Some(u), [HErr::Timeout]) = (event, &until, h.errors.as_slice()) {
                        for (i, (c, was)) in entry.cases.iter_mut().zip(&before).enumerate() {
                            if let (Some(c), true) = (c, *was) {
                                u.timed_out(c, i);
                            }
                        }
                    }
                    if let (Some((secs, task)), [HErr::Timeout]) = (timer, h.errors.as_slice()) {
                        let line = s.line;
                        let why = tr!(
                            "{line} 行目の `{task}` は、イベントを {}待ってから `on timeout` に進みます",
                            "`{task}` (line {line}) waits {} for its event before `on timeout` runs",
                            seconds_text(secs).ja; seconds_text(secs).en
                        );
                        for (c, was) in entry.cases.iter_mut().zip(&before) {
                            if let (Some(c), true) = (c, *was) {
                                c.add(secs, &Ok(0), Some(why.clone()));
                            }
                        }
                    }
                    let hs = self.block(&h.body, entry);
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
                if matches!(expr, TExpr::Now | TExpr::NowAt(_)) {
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
            TK::Pass | TK::Check(_) | TK::Period { start: true } => Some(st),
            // outside the period, the run goes to `on failure` here, as after a task's error
            TK::Period { start: false } => {
                self.stoppable(&st);
                Some(st)
            }
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
        let counted = self.counted(st, at);
        let shown = at.show();
        for (i, cur) in st.cases.iter_mut().enumerate() {
            let Some(cur) = cur else { continue };
            let Some((k, (least, most, since))) = counted.as_ref().and_then(|k| k.bounds(i).map(|b| (k, b))) else {
                cur.add(0, &Err(until_open_of(line, &shown)), None);
                continue;
            };
            if least > cur.lo as i64 {
                let (a, date, fewest, input, clock, example) = (k.anchor.line, &k.date.date, k.span.fewest, &k.input, k.clock(), k.fewest_example());
                let why = tr!(
                    "{line} 行目の `wait until {shown}` は、koyomi の日付 `{date}` の時刻まで待ちます。その日は `{input}` から早くて {fewest} 日後{}の {clock} で、日付に渡した `now` は仮押さえのあと（{a} 行目）に読んでいます",
                    "the wait until `{shown}` (line {line}) lasts until the time of koyomi's date `{date}`: {fewest} days after `{input}` at the fewest{}, at {clock}, and the `now` given to the date was read after the hold (line {a})",
                    example.ja; example.en
                );
                cur.lo = least as u64;
                cur.why = since.why.clone();
                cur.why.push(why);
            }
            match most {
                Some(y) => {
                    let mut why = since.most_why.clone();
                    why.extend(k.longest(|to, clock, most| tr!("{line} 行目の `wait until {shown}` は、その日から {most} 日後の {to} の {clock} まで待ちます", "the wait until `{shown}` (line {line}) lasts until {to} at {clock}, {most} days after it")));
                    cur.reach(y, why);
                }
                None => cur.unbound(&since.open.clone().unwrap_or_else(|| until_open_of(line, &shown))),
            }
        }
    }

    /// The span from the hold of the case `c` to the call on it of line `line`: from `since`, its age
    /// as the call starts, to `end`, its age as the call's run ends.
    fn point(&mut self, c: usize, line: usize, t: usize, op: &str, since: &Since, end: &Since) {
        let case = &self.m.cases[c];
        let RuleKind::Hold { book, transfer } = &self.m.rules[case.rule].kind else { return };
        let most = end.hi.ok_or_else(|| end.open.clone().unwrap_or_else(|| tr!("上限が分かりません", "nothing bounds it")));
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
            most_why: if most.is_ok() { end.most_why.clone() } else { vec![] },
            most,
        });
    }
}

/// A call of a task with `timeout until`: its line, the task, the time as written, and where the
/// time comes from, when koyomi counts it; and what a callback adds to the wait: the seconds it
/// takes to hand its id on at the most, and for its retries (after a failure: E007 keeps a
/// timeout from being retried), each a hand-on and a second more, and the waits between them.
struct Until {
    line: usize,
    task: String,
    shown: String,
    counted: Option<Counted>,
    hand_on: u64,
    again: u64,
}

impl Until {
    fn of(t: &TaskDef, line: usize, shown: String, counted: Option<Counted>) -> Until {
        let (times, waits) = retries(t);
        Until { line, task: t.name.clone(), shown, counted, hand_on: hand_on(t), again: times * (hand_on(t) + 1) + waits }
    }

    /// The handler of `timeout` alone under a task that waits for an event: it starts when the time
    /// has come, so no sooner than the time is after the hold `i`.
    fn timed_out(&self, c: &mut Since, i: usize) {
        let Some((k, (least, _, since))) = self.counted.as_ref().and_then(|k| k.bounds(i).map(|b| (k, b))) else { return };
        if least <= c.lo as i64 {
            return;
        }
        let (line, task, shown) = (self.line, &self.task, &self.shown);
        let (a, date, fewest, input, clock, example) = (k.anchor.line, &k.date.date, k.span.fewest, &k.input, k.clock(), k.fewest_example());
        let why = tr!(
            "{line} 行目の `{task}` は、イベントを期限 `{shown}` まで待ってから `on timeout` に進みます。期限は koyomi の日付 `{date}` の時刻で、その日は `{input}` から早くて {fewest} 日後{}の {clock} です。日付に渡した `now` は仮押さえのあと（{a} 行目）に読んでいます",
            "`{task}` (line {line}) waits for its event until `{shown}` before `on timeout` runs: the time of koyomi's date `{date}`, {fewest} days after `{input}` at the fewest{}, at {clock}, and the `now` given to the date was read after the hold (line {a})",
            example.ja; example.en
        );
        c.lo = least as u64;
        c.why = since.why.clone();
        c.why.push(why);
    }
}

/// The seconds a try of a task takes to hand a callback's id on, at the most: a minute on every
/// target (`check::CALLBACK_HAND_ON_SECONDS`, dandori's DESIGN 17), and nothing for any other task.
fn hand_on(t: &TaskDef) -> u64 {
    if t.callback {
        crate::check::CALLBACK_HAND_ON_SECONDS
    } else {
        0
    }
}

/// How many times a task's `retry` tries it again, and the seconds it waits between the tries.
fn retries(t: &TaskDef) -> (u64, u64) {
    match &t.retry {
        Some(r) => (u64::from(r.times), (0..r.times).map(|k| (r.every as f64 * r.backoff.powi(k as i32)).ceil() as u64).sum()),
        None => (0, 0),
    }
}

fn deadline_open(line: usize, task: &str, shown: &str) -> Text {
    tr!("{line} 行目の `{task}` が待つ期限 `{shown}` には上限がありません", "nothing bounds the time `{shown}` that `{task}` (line {line}) waits until")
}

fn until_open(line: usize) -> Text {
    tr!("{line} 行目の `wait until` がいつまで待つかに上限がありません", "nothing bounds how long the wait until of line {line} waits")
}

fn until_open_of(line: usize, shown: &str) -> Text {
    tr!("{line} 行目の `wait until {shown}` がいつまで待つかに上限がありません", "nothing bounds how long the wait until `{shown}` (line {line}) waits")
}

/// What a call of a rule gives one of its inputs, as the runs to a point of the flow see it (X4).
#[derive(Clone, Debug, PartialEq)]
struct Given {
    input: String,
    shown: String,
    /// for an enum or a bool: the values it can be, and every value the rule's input takes, by the
    /// rule's names and in its order (a bool: `false`, `true`)
    values: Option<(Vec<String>, Vec<String>)>,
    /// for a number: the range on the wire, when it is less than the rule's input takes
    range: Option<(Option<i128>, Option<i128>)>,
    /// the variable or the field it was read from (`kind`, `order.kind`), while nothing puts
    /// another value in it
    key: Option<String>,
    /// the value as the flow gives it
    expr: TExpr,
    /// what it can be where the call is made, on the runs that make it (`ranges::Est`)
    est: crate::ranges::Est,
}

/// A call of a rule, as the runs to a point of the flow see it.
#[derive(Clone, Debug, PartialEq)]
struct Asked {
    line: usize,
    rule: usize,
    inputs: Vec<Given>,
}

/// What is known at a point of the flow of the calls of rules it has made, on the runs that reach it.
#[derive(Clone, Debug, Default, PartialEq)]
struct Facts {
    /// what a value a `match` has read (`kind`, `order.kind`) can be here, as dandori's check of a
    /// `match` holds it (`flow.rs`)
    narrow: BTreeMap<String, Vec<String>>,
    /// the calls of rules these runs have made, by the site of the statement
    calls: BTreeMap<usize, Asked>,
    /// for a variable that holds the answer of a rule: the calls whose answer it can hold; None
    /// where the walk does not follow it
    answers: BTreeMap<String, Option<BTreeSet<usize>>>,
    /// for a variable: the outputs of calls it can hold, with the output's name (none, where these
    /// runs give it no rule's output); None where the walk does not follow it
    outs: BTreeMap<String, Option<BTreeSet<(usize, String)>>>,
    /// the inputs and the variables every one of these runs has given a value
    set: BTreeSet<String>,
    /// for a variable the walk follows: what the value these runs last put in it can be
    /// (`ranges::Est`), and where it comes from. A variable not here is read as anywhere in the
    /// flow, which says no less (`ranges::variables`, `variables`).
    est: BTreeMap<String, crate::ranges::Est>,
    from: BTreeMap<String, Vec<From>>,
    /// the same, as dandori's check of a `match` reads what a value can be (`ranges::held`: a value
    /// written out is any value of its enum); a variable not here is read as `ranges::held` says
    held: BTreeMap<String, crate::ranges::Est>,
    /// the secrets the value these runs last put in a variable holds (`secrets::of_expr`), where the
    /// walk is asked for them; a variable not here holds every secret put in it anywhere in the flow
    /// (`secrets::variables`)
    secrets: BTreeMap<String, Vec<crate::secrets::Held>>,
}

/// The name the walk keeps what a `match` said of the value `e` under (`kind`, `order.kind`): a
/// variable, or a field of one. None for the state of a case, which a `match` reads apart (another
/// run can move it on), and for a value that is neither.
pub(crate) fn read_key(m: &Model, e: &TExpr) -> Option<String> {
    match e {
        TExpr::Var { name, fields, .. } => {
            let case_state = m.case_index(name).is_some_and(|c| fields.len() == 1 && m.cases[c].state_field == fields[0]);
            (!case_state).then(|| std::iter::once(name.clone()).chain(fields.iter().cloned()).collect::<Vec<_>>().join("."))
        }
        _ => None,
    }
}

/// Whether `key` (`order.kind`) reads the variable `name` (`order`).
fn rooted(key: &str, name: &str) -> bool {
    key == name || key.strip_prefix(name).is_some_and(|rest| rest.starts_with('.'))
}

impl Facts {
    /// `name` is given another value: what a `match` said of it no longer holds, and a `match`
    /// on it no longer says what a call was given.
    fn forget(&mut self, name: &str) {
        self.narrow.retain(|k, _| !rooted(k, name));
        self.est.remove(name);
        self.from.remove(name);
        self.held.remove(name);
        self.secrets.remove(name);
        for c in self.calls.values_mut() {
            for g in &mut c.inputs {
                if g.key.as_deref().is_some_and(|k| rooted(k, name)) {
                    g.key = None;
                }
            }
        }
    }

    /// `name` is given a value the walk does not follow.
    fn lose(&mut self, name: &str) {
        self.forget(name);
        self.answers.insert(name.to_string(), None);
        self.outs.insert(name.to_string(), None);
    }

    /// Inside the arm of a `match` on `key` for the values `here`: a call `key` was given to was
    /// given one of them, and a call that was given none of them was not made on these runs.
    fn narrow_to(&mut self, key: &str, here: &[String]) {
        self.narrow.insert(key.to_string(), here.to_vec());
        let mut gone = BTreeSet::new();
        for (site, c) in &mut self.calls {
            for g in &mut c.inputs {
                if let (Some(k), Some((vals, _))) = (&g.key, &mut g.values) {
                    if k == key {
                        vals.retain(|v| here.contains(v));
                        if vals.is_empty() {
                            gone.insert(*site);
                        }
                    }
                }
            }
        }
        if !gone.is_empty() {
            self.calls.retain(|s, _| !gone.contains(s));
            for set in self.answers.values_mut().flatten() {
                set.retain(|s| !gone.contains(s));
            }
            for set in self.outs.values_mut().flatten() {
                set.retain(|(s, _)| !gone.contains(s));
            }
        }
    }
}

/// The runs of `a` and of `b`, joined: a value a `match` read on both is one of the values either
/// says; a call made on either was given what either gave it, and is still tied to a variable
/// only where both say so; a variable holds what it holds on either (one a run does not give a
/// value to is not read after, dandori's E012), and is not followed where either does not.
fn join_facts(a: Option<Facts>, b: Option<Facts>) -> Option<Facts> {
    let (a, b) = match (a, b) {
        (Some(a), Some(b)) => (a, b),
        (x, None) | (None, x) => return x,
    };
    let est = a.est.iter().filter_map(|(k, x)| Some((k.clone(), x.join(b.est.get(k)?)))).collect();
    let held = a.held.iter().filter_map(|(k, x)| Some((k.clone(), x.join(b.held.get(k)?)))).collect();
    let secrets = a
        .secrets
        .iter()
        .filter_map(|(k, x)| {
            let mut both = x.clone();
            for h in b.secrets.get(k)? {
                if !both.contains(h) {
                    both.push(h.clone());
                }
            }
            Some((k.clone(), both))
        })
        .collect();
    let from = a
        .from
        .iter()
        .filter_map(|(k, x)| {
            let mut both = x.clone();
            add(&mut both, b.from.get(k)?.clone());
            Some((k.clone(), both))
        })
        .collect();
    let narrow = a
        .narrow
        .iter()
        .filter_map(|(k, x)| {
            let y = b.narrow.get(k)?;
            Some((k.clone(), x.iter().chain(y.iter().filter(|v| !x.contains(v))).cloned().collect()))
        })
        .collect();
    let mut calls = a.calls;
    for (site, y) in b.calls {
        match calls.get_mut(&site) {
            None => {
                calls.insert(site, y);
            }
            Some(x) => {
                for (g, h) in x.inputs.iter_mut().zip(&y.inputs) {
                    if g.key != h.key {
                        g.key = None;
                    }
                    if let (Some((vals, full)), Some((more, _))) = (&mut g.values, &h.values) {
                        *vals = full.iter().filter(|v| vals.contains(v) || more.contains(v)).cloned().collect();
                    }
                    // the rounds of a loop make the same call over what each round gives it
                    g.est = g.est.join(&h.est);
                    g.range = match (g.range, h.range) {
                        (Some((alo, ahi)), Some((blo, bhi))) => Some((alo.zip(blo).map(|(p, q)| p.min(q)), ahi.zip(bhi).map(|(p, q)| p.max(q)))),
                        _ => None,
                    };
                }
            }
        }
    }
    fn union<T: Ord>(a: BTreeMap<String, Option<BTreeSet<T>>>, b: BTreeMap<String, Option<BTreeSet<T>>>) -> BTreeMap<String, Option<BTreeSet<T>>> {
        let mut out = a;
        for (k, y) in b {
            match out.remove(&k) {
                None => out.insert(k, y),
                Some(Some(mut x)) => match y {
                    Some(y) => {
                        x.extend(y);
                        out.insert(k, Some(x))
                    }
                    None => out.insert(k, None),
                },
                Some(None) => out.insert(k, None),
            };
        }
        out
    }
    let set = a.set.intersection(&b.set).cloned().collect();
    Some(Facts { narrow, calls, answers: union(a.answers, b.answers), outs: union(a.outs, b.outs), set, est, from, held, secrets })
}

/// The rounds the walk follows every variable of a loop through: past them, a variable whose
/// value a round still changes is read as anywhere in the flow (W107), as dandori's check of a
/// `match` walks a loop for as many rounds (`flow.rs`).
const ROUNDS: usize = 64;

/// The range `r` a number can be in, met with the range `rule` the rule's input takes, when that is
/// less than the input takes; None when it is all of it.
fn less(r: Range, rule: Option<Range>) -> Option<(Option<i128>, Option<i128>)> {
    let (rlo, rhi) = rule.map_or((None, None), |x| (x.lo, x.hi));
    let lo = match (r.lo, rlo) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    };
    let hi = match (r.hi, rhi) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    ((lo, hi) != (rlo, rhi)).then_some((lo.map(i128::from), hi.map(i128::from)))
}

/// The walk of the runs through a flow, for X4: for each amount a transfer is given that can be a
/// rule's output, the calls whose outputs it can be on the runs that reach the transfer, and what
/// those runs can give each call (ritsu's DESIGN 7.6, dandori's DESIGN 18).
///
/// - A number a call gives is held to the range dandori gives the call for X2, the same on every
///   run. A value of an enum or a bool read from a variable or a field is held to what the places
///   it comes from let in, less what the arms of a `match` on it before the call leave out; one
///   written out, to itself.
/// - Inside an arm of a `match` on a variable or a field, a call given it before was given one of
///   the arm's values, while nothing has put another value in it since; a call given none of
///   them was not made on the runs through the arm. A `match` on a case's state says nothing of it.
/// - The arms of a `match` and the handlers of a call are joined; an arm that ends in `fail`,
///   `succeed` or `break` is not. A handler starts from the runs before the call, which gives its
///   variable no value when it fails.
/// - A loop is walked round after round until what its rounds start from no longer changes: each
///   round of a `for` and of a `repeat` starts from the runs before the loop joined with the ends of
///   the rounds before it (the variable of a `for` is read as anywhere in the flow). After a `for`,
///   the runs are those before it (the list may be empty), the ends of its rounds and every `break`;
///   after a `repeat`, which runs at least once, the ends of its rounds and every `break`. The
///   rounds of a `for … in parallel` start from the runs before it and give no variable outside
///   them a value (dandori's E009): it is walked once, and after it the runs are those before it.
///   What the walk holds is finite (the sites of statements, names, the values of enums, the
///   places values come from, and ranges whose ends are ends and constants the `.flow` writes:
///   a flow computes nothing, P1) and a join only widens it, so the rounds stop. A round passes a
///   value on by one `let`, so a loop that passes one back through many variables takes as many
///   rounds: from the [`ROUNDS`]th round on, each variable a round still changes is not followed
///   any more, which reads it as anywhere in the flow (W107). A join keeps such a variable so, so
///   it changes no more: each round after either ends the rounds, reads one more variable so, or
///   changes only the calls and what every run has set, which follow from the variables. Past
///   twice [`ROUNDS`] rounds and one for each name, which no walk reaches, the loop is walked once
///   with every variable it gives a value not followed.
/// - `on failure` and `on cancel` start from every point a failure or a cancellation can stop the
///   run: each call and each wait, in the flow and, for `on cancel`, in `on failure` too. A `fail`
///   and a failed check end the run without either.
/// - An amount the walk does not follow (a variable it reads as anywhere in the flow, a statement
///   no run reaches) says nothing, and X4 counts the outputs it can be over every input of their
///   rules.
struct Answers<'a> {
    m: &'a Model,
    /// where each variable can come from, anywhere in the flow
    vars: &'a BTreeMap<String, Vec<From>>,
    /// what dandori knows each variable can be, anywhere in the flow (`ranges::variables`), and
    /// whether the day of a koyomi date is read as koyomi counts it (`ranges::variables_days`)
    ranges: &'a BTreeMap<String, crate::ranges::Est>,
    days: bool,
    /// what the places each variable's values come from hold them to, as dandori's check of a
    /// `match` reads them (`ranges::held`)
    held: BTreeMap<String, crate::ranges::Est>,
    /// the secrets each variable holds anywhere in the flow (`secrets::variables`), where the walk
    /// is asked what the runs that reach a statement hold of them; None where it is not
    secrets: Option<&'a BTreeMap<String, Vec<crate::secrets::Held>>>,
    /// the runs as they can be at a point where a failure or a cancellation can stop them
    stops: Option<Facts>,
    /// for each loop being walked, the innermost last: the runs at its `break`s
    breaks: Vec<Option<Facts>>,
    /// for each amount (the site of the transfer's statement, the parameter): the calls whose
    /// outputs it can be; None where the walk does not follow it
    found: BTreeMap<(usize, String), Option<Vec<OutputCall>>>,
    /// the statements (by their sites) the runs are kept at, and the runs as they reach each (the
    /// calls of tasks for X16, every statement for dandori's E014)
    wanted: BTreeSet<usize>,
    at: BTreeMap<usize, Facts>,
    /// for each loop (by its line) whose rounds still change a variable past [`ROUNDS`] rounds:
    /// the variables the walk stopped following there (W107)
    unfollowed: BTreeMap<usize, BTreeSet<String>>,
}

/// What the runs that reach a call hold there, by the model's own indices (X16; the port's
/// `Reached` is made of it, `ports.rs`).
#[derive(Clone, Debug, Default)]
pub(crate) struct ReachedAt {
    /// each value a `match` on the way has read, with the values it can be at the call
    pub(crate) held: Vec<(String, Vec<String>)>,
    /// each variable that holds the answer of a call of a rule, with every call it can be the
    /// answer of
    pub(crate) answers: Vec<(String, Vec<AnsweredBy>)>,
    /// the inputs and the variables every run that reaches the call has given a value
    pub(crate) set: Vec<String>,
    /// what each variable the walk follows can be at the call
    est: BTreeMap<String, crate::ranges::Est>,
}

impl ReachedAt {
    /// What each variable can be at the call: what the walk holds of it, over what it can be
    /// anywhere in the flow (`vars`).
    pub(crate) fn over(&self, vars: &BTreeMap<String, crate::ranges::Est>) -> BTreeMap<String, crate::ranges::Est> {
        over(vars, &self.est)
    }

    /// The variables the walk follows to the call, with what each can be there.
    pub(crate) fn followed(&self) -> &BTreeMap<String, crate::ranges::Est> {
        &self.est
    }
}

/// A call of a rule whose answer a variable holds: its line, the rule, and each input with the
/// value given, while nothing has put another value in it since, what the value read, and what
/// the value could be where the rule was called.
#[derive(Clone, Debug)]
pub(crate) struct AnsweredBy {
    pub(crate) line: usize,
    pub(crate) rule: usize,
    pub(crate) inputs: Vec<(String, TExpr, Option<String>, crate::ranges::Est)>,
}

/// What the runs that reach a statement hold there, for the checks that read a value where it is
/// given: what the arms of the `match`es on the way leave of each value they read (`read_key`),
/// and what each variable the walk follows was last given.
#[derive(Clone, Debug, Default)]
pub(crate) struct Here {
    pub(crate) narrow: BTreeMap<String, Vec<String>>,
    pub(crate) est: BTreeMap<String, crate::ranges::Est>,
    from: BTreeMap<String, Vec<From>>,
}

impl Here {
    fn of(st: &Facts) -> Here {
        Here { narrow: st.narrow.clone(), est: st.est.clone(), from: st.from.clone() }
    }

    /// What each variable can be at the statement: what the walk holds of it, over what it can be
    /// anywhere in the flow (`vars`).
    pub(crate) fn over(&self, vars: &BTreeMap<String, crate::ranges::Est>) -> BTreeMap<String, crate::ranges::Est> {
        over(vars, &self.est)
    }
}

/// `anywhere`, with what `here` says of the variables it names in its place.
fn over<T: Clone>(anywhere: &BTreeMap<String, T>, here: &BTreeMap<String, T>) -> BTreeMap<String, T> {
    let mut out = anywhere.clone();
    out.extend(here.iter().map(|(k, v)| (k.clone(), v.clone())));
    out
}

/// What the runs of the checked flow `m` that reach each call of a task in `sites` (by the site
/// of its statement) hold there; a site no run reaches, or the walk does not come to, has none.
pub(crate) fn reached(m: &Model, sites: &BTreeSet<usize>) -> BTreeMap<usize, ReachedAt> {
    if sites.is_empty() {
        return BTreeMap::new();
    }
    let vars = variables(m);
    let ranges = crate::ranges::variables(m);
    let mut w = Answers::new(m, &vars, &ranges, false, sites.clone());
    w.walk();
    w.at
        .into_iter()
        .map(|(site, st)| {
            let answers = st
                .answers
                .iter()
                .filter_map(|(x, calls)| {
                    let calls: Vec<AnsweredBy> = calls
                        .as_ref()?
                        .iter()
                        .filter_map(|c| st.calls.get(c))
                        .map(|c| AnsweredBy { line: c.line, rule: c.rule, inputs: c.inputs.iter().map(|g| (g.input.clone(), g.expr.clone(), g.key.clone(), g.est.clone())).collect() })
                        .collect();
                    (!calls.is_empty()).then(|| (x.clone(), calls))
                })
                .collect();
            (site, ReachedAt { held: st.narrow.into_iter().collect(), answers, set: st.set.into_iter().collect(), est: st.est })
        })
        .collect()
}

/// What the runs that reach each statement of the flow `m` hold there (by its site; a statement
/// no run reaches has none): a value two ways bring different values of is either's, and one a
/// way has not read, or has put another value in since, is not held by what a `match` said.
/// Dandori's own check of a value given to a place with a range reads it (E014, `ranges.rs`), on a
/// flow that may not pass its other checks; `days`, as that check reads the day of a koyomi date
/// (`ranges::variables_days`).
pub(crate) fn here(m: &Model, days: bool) -> BTreeMap<usize, Here> {
    let vars = variables(m);
    let ranges = crate::ranges::variables_days(m, days);
    let wanted = m.all_stmts().iter().map(|s| s.site).collect();
    let mut w = Answers::new(m, &vars, &ranges, days, wanted);
    w.walk();
    w.at.iter().map(|(site, st)| (*site, Here::of(st))).collect()
}

/// W107: each loop whose rounds still change a variable past the rounds the walk follows every
/// variable through ([`ROUNDS`]), at the loop, with the variables the checks read as anywhere in the
/// flow from then on. The walk is the one that follows the most (the secrets, and the day of a
/// koyomi date as koyomi counts it); a flow that puts a value in each name at one place only
/// (`ranges::rebound`) reads each variable as anywhere in the flow already, and has none.
pub(crate) fn check(m: &Model) -> Vec<crate::diag::Diag> {
    let looped = m.all_stmts().iter().any(|s| matches!(s.kind, TK::Repeat { .. } | TK::For { parallel: None, .. }));
    if !looped || !crate::ranges::rebound(m) {
        return Vec::new();
    }
    let vars = variables(m);
    let ranges = crate::ranges::variables_days(m, true);
    let all = crate::secrets::variables(m);
    let mut w = Answers::new(m, &vars, &ranges, true, BTreeSet::new());
    w.secrets = Some(&all);
    w.walk();
    w.unfollowed
        .into_iter()
        .map(|(line, names)| {
            let named = Text::list(&names.iter().map(|n| tr!("`{n}`", "`{n}`")).collect::<Vec<_>>());
            crate::diag::Diag::warning(
                "W107",
                line,
                1,
                tr!(
                    "このループは、検査がたどる回の数（{ROUNDS}）を越えて値を渡していきます。検査は {}を、フローのどこかで入れた値の全部として読みます",
                    "this loop passes values on past the rounds the checks follow ({ROUNDS}): they read {} as every value put in them anywhere in the flow",
                    named.ja;
                    named.en
                ),
            )
            .note(tr!(
                "一回の回は、値を `let` 一つ分だけ先へ渡します。回ごとに一つ前の変数へ渡す値は、通る変数の数だけ回を要します。",
                "A round passes a value on by one `let`: a value passed back one variable a round takes as many rounds as the variables it goes through."
            ))
            .note(tr!(
                "ループの中とあとで、検査がこれらの変数について言うこと（秘密、列挙の値、範囲）は、どの実行もそこへ持ってこない値から来ることがあります。",
                "What the checks say of these variables in the loop and after it (a secret, a value of an enum, a range) may come from a value no run brings there."
            ))
            .note(tr!(
                "値を、一つ前の変数から回ごとに渡すのでなく、出どころからそれぞれの変数に入れてください。",
                "Put the value in each variable from where it comes from, rather than from the variable before it a round at a time."
            ))
        })
        .collect()
}

/// What the runs that reach each `match` of the flow `m` hold in the variables the walk follows, as
/// dandori's check of a `match` reads what a value can be (`ranges::held`: a value written out is
/// any value of its enum), by the site of the `match`; a variable not there is read as
/// `ranges::held` says, and a `match` no run reaches has none (E010 and E011, `flow.rs`).
pub(crate) fn held_here(m: &Model) -> BTreeMap<usize, BTreeMap<String, crate::ranges::Est>> {
    let wanted: BTreeSet<usize> = m.all_stmts().iter().filter(|s| matches!(s.kind, TK::Match { .. })).map(|s| s.site).collect();
    if wanted.is_empty() {
        return BTreeMap::new();
    }
    let vars = variables(m);
    let ranges = crate::ranges::variables(m);
    let mut w = Answers::new(m, &vars, &ranges, false, wanted);
    w.walk();
    w.at.into_iter().map(|(site, st)| (site, st.held)).collect()
}

/// The secrets each variable holds on the runs that reach each call, `succeed` and `fail` of the
/// flow `m`, by the site of the statement, where the walk says less than `all` (every secret put
/// in the variable anywhere in the flow, `secrets::variables`): a value put in it on a way that does
/// not reach the statement is not counted (W904, E906, the port's `sends`). A variable not there,
/// and every variable at a statement no run reaches, holds what `all` says. None for a flow that
/// puts a value in each name at one place only (`ranges::rebound`), where the two are one.
pub(crate) fn secrets_here(m: &Model, all: &BTreeMap<String, Vec<crate::secrets::Held>>) -> BTreeMap<usize, BTreeMap<String, Vec<crate::secrets::Held>>> {
    if !crate::ranges::rebound(m) {
        return BTreeMap::new();
    }
    let wanted: BTreeSet<usize> = m.all_stmts().iter().filter(|s| matches!(s.kind, TK::Call { .. } | TK::Succeed { .. } | TK::Fail { .. })).map(|s| s.site).collect();
    if wanted.is_empty() {
        return BTreeMap::new();
    }
    let vars = variables(m);
    let ranges = crate::ranges::variables(m);
    let mut w = Answers::new(m, &vars, &ranges, false, wanted);
    w.secrets = Some(all);
    w.walk();
    w.at.into_iter().map(|(site, st)| (site, st.secrets)).collect()
}

/// What the calls of the flow `m` give, as the runs that reach each see it.
///
/// For each call of a rule (by the site of its statement) and each input it gives a truth value or
/// a value of an enum: the values it can be on the runs that reach the call, an enum's by its
/// names — what the places it comes from let in, and of those what the arms of the `match`es on
/// the way let through to the call; a value written out is itself (X2 reads it for the rule's
/// `never` lines). A call no run reaches has none.
///
/// And, in a flow that puts a value in one name at more than one place (`ranges::rebound`), what
/// the runs that reach each call hold there ([`Here`]): a variable given is what those runs last
/// put in it, not what any run puts in it anywhere. In a flow that does not, the two are one, and
/// there is none.
pub(crate) fn calls_here(m: &Model) -> (BTreeMap<(usize, String), Vec<ritsu_ports::Value>>, BTreeMap<usize, Here>) {
    let named = |e: &TExpr| matches!(e.ty(), Ty::Bool | Ty::Enum(_));
    let rebound = crate::ranges::rebound(m);
    let gives_named = |s: &TStmt| matches!(&s.kind, TK::Call { callee: Callee::Rule(r), args, .. } if m.rules[*r].is_rule() && args.iter().any(|(_, e)| named(e)));
    let calls: Vec<&TStmt> = m.all_stmts().into_iter().filter(|s| gives_named(s) || (rebound && matches!(s.kind, TK::Call { .. }))).collect();
    if calls.is_empty() {
        return (BTreeMap::new(), BTreeMap::new());
    }
    let vars = variables(m);
    let ranges = crate::ranges::variables(m);
    let wanted = calls.iter().map(|s| s.site).collect();
    let mut w = Answers::new(m, &vars, &ranges, false, wanted);
    w.walk();
    let mut values = BTreeMap::new();
    let mut here = BTreeMap::new();
    for s in calls {
        let Some(st) = w.at.get(&s.site) else { continue };
        if rebound {
            here.insert(s.site, Here::of(st));
        }
        let TK::Call { callee: Callee::Rule(r), args, .. } = &s.kind else { continue };
        if !gives_named(s) {
            continue;
        }
        for g in w.asked(st, *r, args, s.line).inputs {
            let Some((can, _)) = g.values else { continue };
            let truth = matches!(g.expr.ty(), Ty::Bool);
            let named = can.into_iter().map(|v| if truth { ritsu_ports::Value::Bool(v == "true") } else { ritsu_ports::Value::Enum(v) }).collect();
            values.insert((s.site, g.input), named);
        }
    }
    (values, here)
}

/// What a `match` of a flow reads of the answers of rules (`matched_outputs`).
#[derive(Clone, Debug)]
pub(crate) struct Matched {
    /// whether every place the value can come from, anywhere in the flow, is a rule's output
    pub(crate) only_outputs: bool,
    /// the calls whose output the value can be on the runs that reach the `match`, each by the site
    /// of its statement, with the output and what those runs can give the rule there (the port's
    /// `OutputCall`, as X4 reads it); None where dandori does not follow the value to the calls
    pub(crate) calls: Option<Vec<(usize, OutputCall)>>,
}

/// What the `match`es of the flow `m` read of the answers of rules, for each `match` (by the site of
/// its statement) on a value that can be a rule's output. A `match` no run reaches is not among
/// them, nor one on a value that is no rule's output anywhere. `scenarios` steers the values those
/// calls give toward each answer of the rule (`steer_rule`), and asks rulec over what the calls can
/// give whether an arm no scenario takes is one the rule never comes to (`untaken`).
pub(crate) fn matched_outputs(m: &Model) -> BTreeMap<usize, Matched> {
    let vars = variables(m);
    let matched: Vec<(usize, TExpr, bool)> = m
        .all_stmts()
        .into_iter()
        .filter_map(|s| match &s.kind {
            TK::Match { expr, .. } => {
                let from = origins(m, &vars, expr);
                from.iter().any(|f| matches!(f, From::Output(..))).then(|| (s.site, expr.clone(), from.iter().all(|f| matches!(f, From::Output(..)))))
            }
            _ => None,
        })
        .collect();
    if matched.is_empty() {
        return BTreeMap::new();
    }
    let ranges = crate::ranges::variables(m);
    let wanted = matched.iter().map(|(s, ..)| *s).collect();
    let mut w = Answers::new(m, &vars, &ranges, false, wanted);
    w.walk();
    let mut out = BTreeMap::new();
    for (site, e, only_outputs) in matched {
        let Some(st) = w.at.get(&site) else { continue };
        let calls = w.outputs(st, &e).and_then(|outs| outs.into_iter().map(|(c, o)| Some((c, w.port_calls(st, &BTreeSet::from([(c, o)]))?.pop()?))).collect::<Option<Vec<_>>>());
        out.insert(site, Matched { only_outputs, calls });
    }
    out
}

impl<'a> Answers<'a> {
    /// The walk that keeps the runs at the statements `wanted`.
    fn new(m: &'a Model, vars: &'a BTreeMap<String, Vec<From>>, ranges: &'a BTreeMap<String, crate::ranges::Est>, days: bool, wanted: BTreeSet<usize>) -> Answers<'a> {
        Answers { m, vars, ranges, days, held: crate::ranges::held(m), secrets: None, stops: None, breaks: Vec::new(), found: BTreeMap::new(), wanted, at: BTreeMap::new(), unfollowed: BTreeMap::new() }
    }

    fn of(m: &'a Model, vars: &'a BTreeMap<String, Vec<From>>, ranges: &'a BTreeMap<String, crate::ranges::Est>) -> BTreeMap<(usize, String), Option<Vec<OutputCall>>> {
        let mut w = Answers { m, vars, ranges, days: false, held: BTreeMap::new(), secrets: None, stops: None, breaks: Vec::new(), found: BTreeMap::new(), wanted: BTreeSet::new(), at: BTreeMap::new(), unfollowed: BTreeMap::new() };
        // a flow that never gives a transfer a rule's output is not walked
        let any = m.all_stmts().iter().any(|s| match &s.kind {
            TK::Call { callee: Callee::Task(t), args, .. } => {
                let amounts = w.amounts(*t);
                args.iter().any(|(p, e)| amounts.contains(p) && w.from_output(e))
            }
            _ => false,
        });
        if !any {
            return BTreeMap::new();
        }
        w.held = crate::ranges::held(m);
        w.walk();
        w.found
    }

    /// Every run through the flow, from its inputs.
    fn walk(&mut self) {
        let m = self.m;
        let mut start = Facts::default();
        for (n, t) in &m.inputs {
            // an input is no rule's output
            start.outs.insert(n.clone(), Some(BTreeSet::new()));
            if matches!(t.inner(), Ty::Record(_)) {
                start.answers.insert(n.clone(), None);
            }
            start.set.insert(n.clone());
            start.est.insert(n.clone(), crate::ranges::of_input(m, n, t));
            start.from.insert(n.clone(), vec![from_input(m, n, t)]);
        }
        self.block(&m.flow, Some(start));
        // `on failure` runs from where a failure stopped the flow, and `on cancel` from where a
        // cancellation stopped it, in the flow or in `on failure`
        if let Some(block) = &m.on_failure {
            let entry = self.stops.clone();
            self.block(block, entry);
        }
        if let Some(block) = &m.on_cancel {
            let entry = self.stops.clone();
            self.block(block, entry);
        }
    }

    /// The parameters of the transfer the task `t` runs that are amounts.
    fn amounts(&self, t: usize) -> Vec<String> {
        let Some(op) = self.m.tasks[t].book() else { return vec![] };
        self.m.books[op.book].transfer(&op.transfer).map(|tr| tr.params.iter().filter(|p| p.unit.is_some()).map(|p| p.name.clone()).collect()).unwrap_or_default()
    }

    /// Whether `e` can be a rule's output on some run, as the flow's places say (`origins`).
    fn from_output(&self, e: &TExpr) -> bool {
        origins(self.m, self.vars, e).iter().any(|f| matches!(f, From::Output(..)))
    }

    /// The outputs of calls the value `e` can be on these runs: none when it can be no rule's
    /// output anywhere; None where the walk does not follow it.
    fn outputs(&self, st: &Facts, e: &TExpr) -> Option<BTreeSet<(usize, String)>> {
        if !self.from_output(e) {
            return Some(BTreeSet::new());
        }
        match e {
            TExpr::Var { name, fields, .. } if fields.len() == 1 => st.answers.get(name).cloned().flatten().map(|calls| calls.into_iter().map(|c| (c, fields[0].clone())).collect()),
            TExpr::Var { name, fields, .. } if fields.is_empty() => st.outs.get(name).cloned().flatten(),
            _ => None,
        }
    }


    /// What the call of the rule `r` on `line` gives its inputs, on these runs.
    fn asked(&self, st: &Facts, r: usize, args: &[(String, TExpr)], line: usize) -> Asked {
        let m = self.m;
        let env = over(self.ranges, &st.est);
        let inputs = args
            .iter()
            .map(|(p, e)| {
                let key = read_key(m, e);
                let narrowed = |all: Vec<String>| -> Vec<String> {
                    match key.as_ref().and_then(|k| st.narrow.get(k)) {
                        Some(n) => all.into_iter().filter(|v| n.contains(v)).collect(),
                        None => all,
                    }
                };
                // what the value can be here, on the runs that make the call
                let est = crate::ranges::estimate_days(m, &env, e, self.days);
                // what the value read, of any type, while nothing puts another value in it (X16
                // reads it; X4 reads `key` only beside `values`)
                let mut g = Given { input: p.clone(), shown: e.show(), values: None, range: None, key: key.clone(), expr: e.clone(), est: est.clone() };
                match e.ty() {
                    Ty::Enum(en) => {
                        let full: Vec<String> = m.rule_input_values(r, p).map(<[String]>::to_vec).unwrap_or_else(|| m.enums[en].values.clone());
                        let can = match e {
                            TExpr::Enum(v, _) => vec![v.clone()],
                            _ => narrowed(crate::ranges::held_values(m, &self.held, e).unwrap_or_else(|| m.enums[en].values.clone())),
                        };
                        // of those, what the value these runs last put in the variable can be
                        let can: Vec<String> = if est.values.is_empty() { can } else { can.into_iter().filter(|v| est.values.contains(v)).collect() };
                        g.values = Some((full.iter().filter(|v| can.contains(v)).cloned().collect(), full));
                        g.key = key.clone();
                    }
                    Ty::Bool => {
                        let full = vec!["false".to_string(), "true".to_string()];
                        let can = match e {
                            TExpr::Bool(b) => vec![b.to_string()],
                            _ => narrowed(full.clone()),
                        };
                        // of those, what the value these runs last put in the variable can be, as
                        // for an enum
                        let can: Vec<String> = if est.values.is_empty() { can } else { can.into_iter().filter(|v| est.values.contains(v)).collect() };
                        g.values = Some((full.iter().filter(|v| can.contains(v)).cloned().collect(), full));
                        g.key = key.clone();
                    }
                    Ty::Int | Ty::Num(_) => {
                        if est.unknown.is_none() {
                            g.range = est.known.and_then(|k| less(k, m.rule_input_range(r, p)));
                        }
                    }
                    _ => {}
                }
                g
            })
            .collect();
        Asked { line, rule: r, inputs }
    }

    /// The calls `outs` names, as the port says them, in the order of their lines; None when one
    /// is not among the calls these runs made (which the walk keeps from happening).
    fn port_calls(&self, st: &Facts, outs: &BTreeSet<(usize, String)>) -> Option<Vec<OutputCall>> {
        let mut out = Vec::new();
        for (site, output) in outs {
            let c = st.calls.get(site)?;
            out.push(OutputCall {
                line: c.line,
                rule: self.m.rules[c.rule].info.path.clone(),
                output: output.clone(),
                inputs: c
                    .inputs
                    .iter()
                    .map(|g| CallInput {
                        input: g.input.clone(),
                        shown: g.shown.clone(),
                        held: match (&g.values, g.range) {
                            (Some((vals, full)), _) => (vals.len() < full.len()).then(|| PortHeld::Values(vals.clone())),
                            (None, Some((lo, hi))) => Some(PortHeld::Range(lo, hi)),
                            (None, None) => None,
                        },
                        matchable: g.values.is_some() && g.key.is_some(),
                    })
                    .collect(),
            });
        }
        out.sort_by(|a, b| (a.line, &a.output).cmp(&(b.line, &b.output)));
        Some(out)
    }

    /// What the value `e` can be on the runs `st`, and where it comes from on them: a variable is
    /// what these runs last put in it, where the walk follows it, and a value a `match` on the way
    /// has read is one of the values its arm leaves.
    fn read(&self, st: &Facts, e: &TExpr) -> (crate::ranges::Est, Vec<From>) {
        let mut est = crate::ranges::estimate_days(self.m, &over(self.ranges, &st.est), e, self.days);
        if let Some(left) = read_key(self.m, e).and_then(|k| st.narrow.get(&k)) {
            est.values.retain(|v| left.contains(v));
        }
        (est, origins(self.m, &over(self.vars, &st.from), e))
    }

    /// What the value `e` can be on the runs `st`, as dandori's check of a `match` reads it
    /// (`ranges::held`), less what the arms of a `match` on the way leave out.
    fn held_of(&self, st: &Facts, e: &TExpr) -> crate::ranges::Est {
        let mut est = crate::ranges::estimate_held(self.m, &over(&self.held, &st.held), e);
        if let Some(left) = read_key(self.m, e).and_then(|k| st.narrow.get(&k)) {
            est.values.retain(|v| left.contains(v));
        }
        est
    }

    /// What the variable `name` holds once the value `e` is put in it on the runs `st`: what a
    /// `match` reads of it, and the secrets it holds, where the walk is asked for them. `at` is the
    /// statement that writes `e`, for a number written with a unit (`secrets::of_expr`).
    fn reading(&self, st: &Facts, name: &str, e: &TExpr, at: Option<usize>) -> (crate::ranges::Est, Option<Vec<crate::secrets::Held>>) {
        let held = self.held_of(st, e);
        let secrets = self.secrets.map(|all| {
            let ty = self.m.var_ty(name).cloned().unwrap_or(Ty::Json);
            crate::secrets::as_json(&ty, crate::secrets::of_expr(self.m, &over(all, &st.secrets), e, at.map(|site| (site, ""))))
        });
        (held, secrets)
    }

    /// The runs `st` reach the statement at `site`, with any that reached it another way.
    fn keep(&mut self, site: usize, st: &Facts) {
        let joined = join_facts(self.at.remove(&site), Some(st.clone())).expect("the runs here");
        self.at.insert(site, joined);
    }

    /// A failure or a cancellation can stop the run here.
    fn stoppable(&mut self, st: &Facts) {
        self.stops = join_facts(self.stops.take(), Some(st.clone()));
    }

    fn block(&mut self, stmts: &[TStmt], mut st: Option<Facts>) -> Option<Facts> {
        for s in stmts {
            st = self.stmt(s, st?);
        }
        st
    }

    fn stmt(&mut self, s: &TStmt, mut st: Facts) -> Option<Facts> {
        // (a `for` is kept after its rounds are walked, below)
        if self.wanted.contains(&s.site) && !matches!(s.kind, TK::For { .. }) {
            self.keep(s.site, &st);
        }
        match &s.kind {
            TK::Call { target, callee, args, handlers } => {
                // the call can fail before it gives its variable a value, and a case it writes
                // may have heard of it
                let mut stop = st.clone();
                if let Some(Target::Case(c)) = target {
                    let name = self.m.cases[*c].name.clone();
                    stop.lose(&name);
                }
                self.stoppable(&stop);
                let before = st.clone();
                if let Callee::Task(t) = callee {
                    let amounts = self.amounts(*t);
                    for (p, e) in args.iter().filter(|(p, _)| amounts.contains(p)) {
                        let calls = self.outputs(&st, e).and_then(|o| self.port_calls(&st, &o));
                        let key = (s.site, p.clone());
                        let joined = match (self.found.remove(&key), calls) {
                            (None, c) => c,
                            (Some(Some(mut a)), Some(b)) => {
                                // the rounds of a loop come here again
                                for c in b {
                                    if !a.contains(&c) {
                                        a.push(c);
                                    }
                                }
                                Some(a)
                            }
                            _ => None,
                        };
                        self.found.insert(key, joined);
                    }
                }
                match (target, callee) {
                    (Some(Target::Let(x)), Callee::Rule(r)) if self.m.rules[*r].is_rule() => {
                        let asked = self.asked(&st, *r, args, s.line);
                        st.calls.insert(s.site, asked);
                        st.forget(x);
                        st.answers.insert(x.clone(), Some(BTreeSet::from([s.site])));
                        st.outs.insert(x.clone(), Some(BTreeSet::new()));
                        st.set.insert(x.clone());
                        // what a rule answers is a decision, no secret
                        if self.secrets.is_some() {
                            st.secrets.insert(x.clone(), Vec::new());
                        }
                    }
                    (Some(Target::Let(x)), Callee::Rule(_)) => {
                        st.lose(x);
                        st.set.insert(x.clone());
                    }
                    (Some(Target::Let(x)), Callee::Task(t)) => {
                        // a task's answer is no rule's output
                        st.forget(x);
                        st.answers.insert(x.clone(), None);
                        st.outs.insert(x.clone(), Some(BTreeSet::new()));
                        st.set.insert(x.clone());
                        let answer = crate::ranges::of_answer(self.m, &self.m.tasks[*t]);
                        st.est.insert(x.clone(), answer.clone());
                        st.held.insert(x.clone(), answer);
                        st.from.insert(x.clone(), vec![from_answer(self.m, &self.m.tasks[*t])]);
                        if self.secrets.is_some() {
                            st.secrets.insert(x.clone(), crate::secrets::of_answer(self.m, *t, x));
                        }
                    }
                    (Some(Target::Case(c)), _) => {
                        let name = self.m.cases[*c].name.clone();
                        st.lose(&name);
                        st.set.insert(name);
                    }
                    (None, _) => {}
                }
                let mut out = Some(st);
                for h in handlers {
                    let mut entry = before.clone();
                    if let Some(Target::Case(c)) = target {
                        let name = self.m.cases[*c].name.clone();
                        entry.lose(&name);
                    }
                    let end = self.block(&h.body, Some(entry));
                    out = join_facts(out, end);
                }
                out
            }
            TK::Assign { name, expr } => {
                let answers = match expr {
                    TExpr::Var { name: from, fields, .. } if fields.is_empty() => st.answers.get(from).cloned().flatten(),
                    _ => None,
                };
                let outs = self.outputs(&st, expr);
                let (est, from) = self.read(&st, expr);
                let (held, secrets) = self.reading(&st, name, expr, Some(s.site));
                st.forget(name);
                st.answers.insert(name.clone(), answers);
                st.outs.insert(name.clone(), outs);
                st.set.insert(name.clone());
                st.est.insert(name.clone(), est);
                st.from.insert(name.clone(), from);
                st.held.insert(name.clone(), held);
                if let Some(s) = secrets {
                    st.secrets.insert(name.clone(), s);
                }
                Some(st)
            }
            TK::Match { expr, arms } => {
                let key = read_key(self.m, expr).unwrap_or_default();
                // what the value can be here, as dandori's check of a `match` counts it
                let ety = expr.ty();
                let held = if key.is_empty() && matches!(expr, TExpr::Var { .. }) { None } else { crate::ranges::held_values(self.m, &over(&self.held, &st.held), expr) };
                let mut domain: Vec<String> = match (ety.inner(), &held) {
                    (Ty::Enum(_), Some(values)) => values.clone(),
                    (Ty::Enum(e), None) => self.m.enums[*e].values.clone(),
                    (Ty::Bool, _) => vec!["true".into(), "false".into()],
                    _ => vec![],
                };
                let some_values: Vec<String> = if domain.is_empty() { vec!["some".into()] } else { domain.clone() };
                if matches!(ety, Ty::Opt(_)) {
                    if domain.is_empty() {
                        domain.push("some".into());
                    }
                    domain.push("none".into());
                }
                let possible = st.narrow.get(&key).cloned().unwrap_or(domain);
                let (est, from) = self.read(&st, expr);
                let mut out = None;
                for a in arms {
                    let mut inner = st.clone();
                    if let Some(x) = &a.some {
                        let (held, secrets) = self.reading(&st, x, expr, None);
                        inner.lose(x);
                        inner.set.insert(x.clone());
                        inner.est.insert(x.clone(), est.clone());
                        inner.from.insert(x.clone(), from.clone());
                        inner.held.insert(x.clone(), held);
                        if let Some(s) = secrets {
                            inner.secrets.insert(x.clone(), s);
                        }
                    }
                    if !key.is_empty() {
                        let mut vals = a.values.clone();
                        if a.none {
                            vals.push("none".into());
                        }
                        if a.some.is_some() {
                            vals.extend(some_values.iter().cloned());
                        }
                        let here: Vec<String> = vals.into_iter().filter(|v| possible.contains(v)).collect();
                        if !here.is_empty() {
                            inner.narrow_to(&key, &here);
                        }
                    }
                    let end = self.block(&a.body, Some(inner));
                    out = join_facts(out, end);
                }
                out
            }
            TK::Wait { .. } | TK::WaitUntil { .. } => {
                self.stoppable(&st);
                Some(st)
            }
            TK::Repeat { body, .. } => self.rounds(s.line, body, &[], None, Loop::Repeat, st),
            TK::For { var, result, body, locals, parallel, .. } => {
                let mut extra = vec![var.clone()];
                if let Some((r, _)) = result {
                    extra.push(r.clone());
                }
                extra.extend(locals.iter().cloned());
                let kind = if parallel.is_some() { Loop::Parallel } else { Loop::For };
                let after = self.rounds(s.line, body, &extra, Some((var, result.as_ref().map(|(r, _)| r.as_str()))), kind, st);
                // what a `for` yields is read at the end of each round, which the runs after the
                // rounds hold
                if let (true, Some(after)) = (self.wanted.contains(&s.site), &after) {
                    self.keep(s.site, after);
                }
                after
            }
            TK::Pass | TK::Check(_) | TK::Period { start: true } => Some(st),
            // outside the period, the run goes to `on failure` here, as after a task's error
            TK::Period { start: false } => {
                self.stoppable(&st);
                Some(st)
            }
            // a `break` goes on after its loop, with what the run holds here
            TK::Break => {
                if let Some(b) = self.breaks.last_mut() {
                    *b = join_facts(b.take(), Some(st));
                }
                None
            }
            // a `fail` ends the run as it is, without `on failure` (dandori's DESIGN), and so
            // does a failed check
            TK::Succeed { .. } | TK::Fail { .. } => None,
        }
    }

    /// A loop, as [`Answers`] says: round after round until what a round starts from no longer
    /// changes, reading a variable a round still changes past [`ROUNDS`] rounds as anywhere in the
    /// flow; or once with every variable it gives a value not followed (a `for … in parallel`).
    /// `line` is the loop's; `of`, the variable of a `for` and the list its rounds yield; `extra`,
    /// the names the loop gives a value besides its body's.
    fn rounds(&mut self, line: usize, body: &[TStmt], extra: &[String], of: Option<(&str, Option<&str>)>, kind: Loop, st: Facts) -> Option<Facts> {
        // a `for` gives its variable a value before each round, read as anywhere in the flow
        let start = |f: &Facts| -> Facts {
            let mut f = f.clone();
            if let Some((var, _)) = of {
                f.lose(var);
                f.set.insert(var.to_string());
            }
            f
        };
        // and after the rounds, the list of what they yield
        let yielded = |f: Option<Facts>| -> Option<Facts> {
            f.map(|mut f| {
                if let Some((_, Some(result))) = of {
                    f.lose(result);
                    f.set.insert(result.to_string());
                }
                f
            })
        };
        if kind != Loop::Parallel {
            let mut head = st.clone();
            let most = 2 * ROUNDS + names(&st).len() + assigned(body).len() + extra.len();
            for round in 1..=most {
                self.breaks.push(None);
                let end = self.block(body, Some(start(&head)));
                let broke = self.breaks.pop().flatten();
                let mut next = join_facts(Some(head.clone()), end.clone()).expect("the runs before the loop");
                if next == head {
                    let after = match kind {
                        // at least one round
                        Loop::Repeat => join_facts(end, broke),
                        // the list may be empty
                        _ => join_facts(Some(next), broke),
                    };
                    return yielded(after);
                }
                // a variable a round still changes is read as anywhere in the flow from here on
                if round >= ROUNDS {
                    let changed = changed(&head, &next);
                    for v in &changed {
                        next.lose(v);
                    }
                    if !changed.is_empty() {
                        self.unfollowed.entry(line).or_default().extend(changed);
                    }
                }
                head = next;
            }
        }
        let mut set = assigned(body);
        // past the rounds a walk can take (none does), every variable the rounds give a value
        if kind != Loop::Parallel {
            self.unfollowed.entry(line).or_default().extend(set.iter().filter(|v| !extra.contains(v)).cloned());
        }
        Model::walk(body, &mut |s| {
            if let TK::Call { target: Some(Target::Case(c)), .. } = &s.kind {
                set.push(self.m.cases[*c].name.clone());
            }
        });
        set.extend(extra.iter().cloned());
        let mut entry = start(&st);
        for v in &set {
            entry.lose(v);
        }
        if let Some((var, _)) = of {
            entry.set.insert(var.to_string());
        }
        self.breaks.push(None);
        self.block(body, Some(entry));
        self.breaks.pop();
        let mut after = st;
        for v in &set {
            after.lose(v);
        }
        yielded(Some(after))
    }
}

/// The names `f` says anything of: the variables and inputs it follows or does not, and those a
/// `match` has read (a field, by its variable).
fn names(f: &Facts) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = f.est.keys().chain(f.held.keys()).chain(f.secrets.keys()).chain(f.from.keys()).chain(f.answers.keys()).chain(f.outs.keys()).cloned().collect();
    out.extend(f.narrow.keys().map(|k| k.split('.').next().unwrap_or(k).to_string()));
    out
}

/// The names whose value `b` says other than `a` does: what it can be, where it comes from, the
/// secrets and the answers it holds, and what a `match` said of it.
fn changed(a: &Facts, b: &Facts) -> BTreeSet<String> {
    fn differ<T: PartialEq>(a: &BTreeMap<String, T>, b: &BTreeMap<String, T>, out: &mut BTreeSet<String>) {
        for k in a.keys().chain(b.keys()) {
            if a.get(k) != b.get(k) {
                out.insert(k.clone());
            }
        }
    }
    let mut out = BTreeSet::new();
    differ(&a.est, &b.est, &mut out);
    differ(&a.held, &b.held, &mut out);
    differ(&a.secrets, &b.secrets, &mut out);
    differ(&a.from, &b.from, &mut out);
    differ(&a.answers, &b.answers, &mut out);
    differ(&a.outs, &b.outs, &mut out);
    for k in a.narrow.keys().chain(b.narrow.keys()) {
        if a.narrow.get(k) != b.narrow.get(k) {
            out.insert(k.split('.').next().unwrap_or(k).to_string());
        }
    }
    out
}

/// The kind of a loop, as the walk goes through its rounds.
#[derive(Clone, Copy, PartialEq)]
enum Loop {
    /// `repeat`: at least one round, each from where the one before ended
    Repeat,
    /// `for`: none or more rounds, each from where the one before ended
    For,
    /// `for … in parallel`: every round from the runs before the loop
    Parallel,
}
