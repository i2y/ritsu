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
                // an enum's range is not numbers
                _ => match m.field_range(r, f).filter(|_| ty.enum_inside().is_none()) {
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
    for (n, t) in &m.inputs {
        // an enum's range is not numbers
        let from = match m.input_ranges.get(n).filter(|_| t.enum_inside().is_none()) {
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
                    // an enum's range is not numbers
                    let from = match t.result_range.filter(|_| t.result.as_ref().is_none_or(|ty| ty.enum_inside().is_none())) {
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
                            TExpr::Now => Some(at_start),
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
