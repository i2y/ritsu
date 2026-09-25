//! The second pass: walk the flow as it can run, keeping for every point what is known —
//! which variables are surely set, and for every case whether it has been started and
//! which states it can be in, each with the shortest run that shows it. From that come
//! the checks that depend on the run: a match that misses a value or has an arm that can
//! never be taken, a variable read before it is set, an event sent where the machine
//! refuses it, and a case left in a state that is not final when the workflow ends.

use crate::diag::{Diag, Severity, Step};
use crate::model::*;
use crate::rulec::Outcome;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tri {
    No,
    Yes,
    Maybe,
}

fn tri(a: Tri, b: Tri) -> Tri {
    if a == b {
        a
    } else {
        Tri::Maybe
    }
}

type Path = Vec<Step>;

fn shorter(a: &Path, b: &Path) -> Path {
    if b.len() < a.len() {
        b.clone()
    } else {
        a.clone()
    }
}

#[derive(Clone, Debug)]
struct CaseAbs {
    started: Tri,
    /// a run on which the case has not been started, when `started` is not `Yes`
    unstarted: Option<Path>,
    states: BTreeMap<usize, Path>,
}

#[derive(Clone, Debug)]
struct Abs {
    live: bool,
    /// for each variable: whether it is set, and a run on which it is not
    set: BTreeMap<String, (Tri, Option<Path>)>,
    cases: Vec<CaseAbs>,
    /// for a value narrowed by a match: the values it can have, each with its run
    narrow: BTreeMap<String, BTreeMap<String, Path>>,
    path: Path,
}

impl Abs {
    fn dead() -> Abs {
        Abs { live: false, set: BTreeMap::new(), cases: vec![], narrow: BTreeMap::new(), path: vec![] }
    }

    /// Record a step of the run, on the run to this point and on every witness it carries.
    fn step(mut self, s: Step) -> Abs {
        for c in &mut self.cases {
            for p in c.states.values_mut() {
                p.push(s.clone());
            }
            if let Some(p) = &mut c.unstarted {
                p.push(s.clone());
            }
        }
        for (_, (_, p)) in self.set.iter_mut() {
            if let Some(p) = p {
                p.push(s.clone());
            }
        }
        for vals in self.narrow.values_mut() {
            for p in vals.values_mut() {
                p.push(s.clone());
            }
        }
        self.path.push(s);
        self
    }

    fn same(&self, o: &Abs) -> bool {
        self.live == o.live
            && self.set.iter().map(|(k, v)| (k, v.0)).eq(o.set.iter().map(|(k, v)| (k, v.0)))
            && self.cases.len() == o.cases.len()
            && self.cases.iter().zip(&o.cases).all(|(a, b)| a.started == b.started && a.states.keys().eq(b.states.keys()))
            && self.narrow.len() == o.narrow.len()
            && self.narrow.iter().zip(&o.narrow).all(|((ka, va), (kb, vb))| ka == kb && va.keys().eq(vb.keys()))
    }
}

fn join(a: &Abs, b: &Abs) -> Abs {
    if !a.live {
        return b.clone();
    }
    if !b.live {
        return a.clone();
    }
    let mut set = BTreeMap::new();
    for (k, (ta, pa)) in &a.set {
        let (tb, pb) = b.set.get(k).cloned().unwrap_or((Tri::No, Some(b.path.clone())));
        let t = tri(*ta, tb);
        let p = match (pa, &pb) {
            (Some(x), Some(y)) => Some(shorter(x, y)),
            (Some(x), None) => Some(x.clone()),
            (None, Some(y)) => Some(y.clone()),
            (None, None) => None,
        };
        set.insert(k.clone(), (t, if t == Tri::Yes { None } else { p }));
    }
    let cases = a
        .cases
        .iter()
        .zip(&b.cases)
        .map(|(x, y)| {
            let mut states = x.states.clone();
            for (s, p) in &y.states {
                match states.get(s) {
                    Some(q) if q.len() <= p.len() => {}
                    _ => {
                        states.insert(*s, p.clone());
                    }
                }
            }
            let started = tri(x.started, y.started);
            let unstarted = match (&x.unstarted, &y.unstarted) {
                (Some(p), Some(q)) => Some(shorter(p, q)),
                (Some(p), None) | (None, Some(p)) => Some(p.clone()),
                (None, None) => None,
            };
            CaseAbs { started, unstarted: if started == Tri::Yes { None } else { unstarted }, states }
        })
        .collect();
    let mut narrow = BTreeMap::new();
    for (k, v) in &a.narrow {
        if let Some(w) = b.narrow.get(k) {
            let mut u = v.clone();
            for (val, p) in w {
                match u.get(val) {
                    Some(q) if q.len() <= p.len() => {}
                    _ => {
                        u.insert(val.clone(), p.clone());
                    }
                }
            }
            narrow.insert(k.clone(), u);
        }
    }
    Abs { live: true, set, cases, narrow, path: shorter(&a.path, &b.path) }
}

pub struct Flow<'a> {
    m: &'a Model,
    diags: BTreeMap<(&'static str, usize, usize), Diag>,
    loops: Vec<Vec<Abs>>,
    failures: Vec<(Abs, usize, String)>,
    monitors: BTreeMap<usize, (usize, BTreeSet<usize>)>,
    in_on_failure: bool,
}

pub struct FlowResult {
    pub diags: Vec<Diag>,
    pub monitors: BTreeMap<usize, (usize, Vec<String>)>,
}

pub fn analyze(m: &Model) -> FlowResult {
    let mut f = Flow { m, diags: BTreeMap::new(), loops: vec![], failures: vec![], monitors: BTreeMap::new(), in_on_failure: false };
    let mut start = Abs { live: true, set: BTreeMap::new(), cases: vec![], narrow: BTreeMap::new(), path: vec![] };
    for (v, _) in &m.vars {
        let is_input = m.inputs.iter().any(|(i, _)| i == v);
        start.set.insert(v.clone(), if is_input { (Tri::Yes, None) } else { (Tri::No, Some(vec![])) });
    }
    for _ in &m.cases {
        start.cases.push(CaseAbs { started: Tri::No, unstarted: Some(vec![]), states: BTreeMap::new() });
    }
    let end = f.stmts(&m.flow, start);
    if end.live {
        let line = m.flow.last().map(|s| s.line).unwrap_or(1);
        if !m.outputs.is_empty() {
            f.push(
                Diag::error("E009", line, 1, "the flow can reach its end without `succeed`, but the workflow has outputs", "出力があるのに、`succeed` を通らずに flow の終わりに着くことがあります")
                    .with_path(end.path.clone()),
            );
        } else {
            f.exit(&end, line, &[], Exit::End);
        }
    }
    let failures = std::mem::take(&mut f.failures);
    match &m.on_failure {
        Some(block) => {
            f.in_on_failure = true;
            let mut entry = Abs::dead();
            for (a, _, _) in &failures {
                entry = join(&entry, a);
            }
            if entry.live {
                let line = block.first().map(|s| s.line).unwrap_or(1);
                let entry = entry.step(Step::new(line, "on failure", "on failure"));
                let end = f.stmts(block, entry);
                if end.live {
                    let line = block.last().map(|s| s.line).unwrap_or(1);
                    f.exit(&end, line, &[], Exit::FailEnd);
                }
            }
            let inner = std::mem::take(&mut f.failures);
            f.warn_failures(&inner, true);
        }
        None => f.warn_failures(&failures, false),
    }
    let monitors = f
        .monitors
        .iter()
        .map(|(site, (c, set))| {
            let mc = m.machine(*c);
            (*site, (*c, set.iter().map(|s| mc.states[*s].clone()).collect()))
        })
        .collect();
    let mut diags: Vec<Diag> = f.diags.into_values().collect();
    diags.sort_by(|a, b| (a.line, a.col, a.code).cmp(&(b.line, b.col, b.code)));
    FlowResult { diags, monitors }
}

#[derive(Clone, PartialEq, Eq)]
enum Exit {
    Succeed,
    Fail(String),
    End,
    FailEnd,
}

impl<'a> Flow<'a> {
    fn push(&mut self, d: Diag) {
        self.diags.insert((d.code, d.line, d.col), d);
    }

    fn case_name(&self, c: usize) -> &str {
        &self.m.cases[c].name
    }

    fn state_name(&self, c: usize, s: usize) -> &str {
        &self.m.machine(c).states[s]
    }

    fn names(&self, c: usize, set: impl Iterator<Item = usize>) -> Vec<String> {
        set.map(|s| self.state_name(c, s).to_string()).collect()
    }

    fn is_refused(&self, c: usize, o: &Outcome) -> bool {
        match &self.m.cases[c].refused_when {
            Some((i, v)) => o.produces.get(*i).cloned().flatten().as_deref() == Some(v.as_str()),
            None => false,
        }
    }

    /// The states a case can be in by the time the workflow acts on it again: the last known
    /// ones, and every state the events of the other side can lead to from them.
    fn closure(&self, c: usize, states: &BTreeMap<usize, Path>) -> BTreeMap<usize, Path> {
        let case = &self.m.cases[c];
        let mc = self.m.machine(c);
        let mut out = states.clone();
        let mut work: Vec<usize> = out.keys().cloned().collect();
        while let Some(s) = work.pop() {
            let p = out[&s].clone();
            for (axis, coord, ev) in &case.external {
                let mut fixed = case.held.clone();
                fixed.push((*axis, *coord));
                for o in mc.outcomes(s, &fixed) {
                    if self.is_refused(c, &o) || out.contains_key(&o.next) {
                        continue;
                    }
                    let mut np = p.clone();
                    let (from, to) = (self.state_name(c, s), self.state_name(c, o.next));
                    np.push(Step::new(
                        0,
                        format!("`{ev}` happens on the other side: {} {from} → {to}", case.name),
                        format!("相手の側で `{ev}` が起きる: {} {from} → {to}", case.name),
                    ));
                    out.insert(o.next, np);
                    work.push(o.next);
                }
            }
        }
        out
    }

    /// Every state the machine reaches from its start, by any event, each with a run.
    fn reachable(&self, c: usize, base: Path) -> BTreeMap<usize, Path> {
        let case = &self.m.cases[c];
        let mc = self.m.machine(c);
        let mut out: BTreeMap<usize, Path> = BTreeMap::new();
        out.insert(mc.initial, base);
        let mut work = vec![mc.initial];
        let events: Vec<(usize, usize, String)> = (0..mc.axes.len())
            .filter(|a| *a != mc.state_axis && !case.held.iter().any(|(h, _)| h == a))
            .flat_map(|a| mc.axes[a].coords.iter().enumerate().map(move |(ci, v)| (a, ci, v.clone())))
            .collect();
        while let Some(s) = work.pop() {
            let p = out[&s].clone();
            for (axis, coord, ev) in &events {
                let mut fixed = case.held.clone();
                fixed.push((*axis, *coord));
                for o in mc.outcomes(s, &fixed) {
                    if self.is_refused(c, &o) || out.contains_key(&o.next) {
                        continue;
                    }
                    let mut np = p.clone();
                    let (from, to) = (self.state_name(c, s), self.state_name(c, o.next));
                    np.push(Step::new(0, format!("before the workflow looks: `{ev}`, {} {from} → {to}", case.name), format!("ワークフローが見る前に `{ev}`: {} {from} → {to}", case.name)));
                    out.insert(o.next, np);
                    work.push(o.next);
                }
            }
        }
        out
    }

    fn event_axis(&mut self, c: usize, event: &str, column: Option<&str>, line: usize) -> Option<(usize, usize)> {
        let mc = self.m.machine(c);
        let axis = match column {
            Some(col) => match mc.axis_of(col) {
                Some(a) => Some(a),
                None => {
                    self.push(Diag::error("E008", line, 1, format!("the machine's table does not read `{col}`"), format!("ステートマシンの表は `{col}` を読みません")));
                    None
                }
            },
            None => {
                let axes = mc.axes_with_value(event);
                match axes.len() {
                    1 => Some(axes[0]),
                    0 => {
                        self.push(Diag::error("E008", line, 1, format!("no input of the machine `{}` has the event `{event}`", mc.name), format!("ステートマシン `{}` のどの入力にも出来事 `{event}` はありません", mc.name)));
                        None
                    }
                    _ => {
                        self.push(Diag::error("E008", line, 1, format!("more than one input has the value `{event}`; write `sends <input> = {event}`"), format!("値 `{event}` を持つ入力が二つ以上あります。`sends <入力> = {event}` と書いてください")));
                        None
                    }
                }
            }
        }?;
        match mc.axes[axis].coords.iter().position(|x| x == event) {
            Some(ci) => Some((axis, ci)),
            None => {
                self.push(Diag::error("E008", line, 1, format!("`{event}` is not a value of `{}`", mc.axes[axis].column), format!("`{event}` は `{}` の値ではありません", mc.axes[axis].column)));
                None
            }
        }
    }

    /// Every variable the expression reads must have its value here.
    fn check_reads(&mut self, e: &TExpr, a: &Abs, line: usize) {
        for x in e.vars() {
            self.check_set(x, a, line);
        }
    }

    fn check_set(&mut self, e: &TExpr, a: &Abs, line: usize) {
        if let TExpr::Var { name, .. } = e {
            if let Some(ci) = self.m.case_index(name) {
                let cabs = &a.cases[ci];
                if cabs.started != Tri::Yes {
                    let p = cabs.unstarted.clone().unwrap_or_default();
                    let (en, ja) = if cabs.started == Tri::No {
                        (format!("the case `{name}` has not been started here"), format!("ここでは案件 `{name}` はまだ始まっていません"))
                    } else {
                        (format!("the case `{name}` may not have been started here"), format!("ここでは、案件 `{name}` が始まっていないことがあります"))
                    };
                    self.push(Diag::error("E013", line, 1, en, ja).with_path(p));
                }
                return;
            }
            if let Some((t, p)) = a.set.get(name) {
                if *t != Tri::Yes {
                    let (en, ja) = if *t == Tri::No {
                        (format!("`{name}` has not been set here"), format!("ここでは `{name}` はまだ値を持っていません"))
                    } else {
                        (format!("`{name}` may not have been set here"), format!("ここでは、`{name}` が値を持っていないことがあります"))
                    };
                    self.push(Diag::error("E012", line, 1, en, ja).with_path(p.clone().unwrap_or_default()));
                }
            }
        }
    }

    fn assign(a: &mut Abs, name: &str) {
        a.set.insert(name.to_string(), (Tri::Yes, None));
        let pre = format!("{name}.");
        a.narrow.retain(|k, _| k != name && !k.starts_with(&pre));
    }

    fn stmts(&mut self, ss: &[TStmt], mut a: Abs) -> Abs {
        for s in ss {
            if !a.live {
                break;
            }
            a = self.stmt(s, a);
        }
        a
    }

    fn stmt(&mut self, s: &TStmt, a: Abs) -> Abs {
        match &s.kind {
            TK::Call { target, callee, args, handlers } => self.call(s, target.as_ref(), callee, args, handlers, a),
            TK::Match { expr, arms } => self.matching(s, expr, arms, a),
            TK::Wait { seconds } => a.step(Step::new(s.line, format!("wait {}", show_dur(*seconds)), format!("{} 待つ", show_dur_ja(*seconds)))),
            TK::WaitUntil { at } => {
                self.check_reads(at, &a, s.line);
                let shown = show(at);
                a.step(Step::new(s.line, format!("wait until {shown}"), format!("{shown} まで待つ")))
            }
            TK::Assign { name, expr } => {
                self.check_reads(expr, &a, s.line);
                let mut a = a.step(Step::new(s.line, format!("{name} = {}", show(expr)), format!("{name} = {}", show(expr))));
                Flow::assign(&mut a, name);
                a
            }
            TK::For { var, list, max, parallel, body, result, locals } => {
                self.check_reads(list, &a, s.line);
                let entry = a.step(Step::new(s.line, format!("for {var} in {} (at most {max})", show(list)), format!("for {var} in {}（{max} 個まで）", show(list))));
                match parallel {
                    None => {
                        // like `repeat`, but the list may be empty, so the loop can also end before a round
                        let mut head = entry;
                        let mut rounds = 0;
                        loop {
                            rounds += 1;
                            self.loops.push(vec![]);
                            let mut inner = head.clone();
                            Flow::assign(&mut inner, var);
                            let end = self.stmts(body, inner);
                            let breaks = self.loops.pop().unwrap_or_default();
                            let next = join(&head, &end);
                            if next.same(&head) || rounds > 64 {
                                if let Some((_, y)) = result {
                                    if end.live {
                                        self.check_reads(y, &end, s.line);
                                    }
                                }
                                let mut after = next;
                                for b in &breaks {
                                    after = join(&after, b);
                                }
                                if let Some((r, _)) = result {
                                    if after.live {
                                        Flow::assign(&mut after, r);
                                    }
                                }
                                return after;
                            }
                            head = next;
                        }
                    }
                    Some(_) => {
                        // every round starts from here; none changes a case or a variable outside it
                        let mut inner = entry.clone();
                        Flow::assign(&mut inner, var);
                        let before = self.failures.len();
                        self.loops.push(vec![]);
                        let end = self.stmts(body, inner);
                        self.loops.pop();
                        if let Some((_, y)) = result {
                            if end.live {
                                self.check_reads(y, &end, s.line);
                            }
                        }
                        // a round's failure ends the loop only after every round is done, and `on failure`
                        // does not see the round's own variables
                        for f in self.failures[before..].iter_mut() {
                            for l in locals {
                                f.0.set.insert(l.clone(), (Tri::No, Some(entry.path.clone())));
                            }
                        }
                        let mut after = entry;
                        if let Some((r, _)) = result {
                            Flow::assign(&mut after, r);
                        }
                        after
                    }
                }
            }
            TK::Repeat { times, body } => {
                let mut head = a.clone().step(Step::new(s.line, format!("repeat (at most {times} times)"), format!("repeat（{times} 回まで）")));
                let mut rounds = 0;
                loop {
                    rounds += 1;
                    self.loops.push(vec![]);
                    let end = self.stmts(body, head.clone());
                    let breaks = self.loops.pop().unwrap_or_default();
                    let next = join(&head, &end);
                    if next.same(&head) || rounds > 64 {
                        let mut after = end;
                        for b in &breaks {
                            after = join(&after, b);
                        }
                        return after;
                    }
                    head = next;
                }
            }
            TK::Pass => a,
            TK::Break => {
                let a2 = a.step(Step::new(s.line, "break", "break"));
                if let Some(l) = self.loops.last_mut() {
                    l.push(a2);
                }
                Abs::dead()
            }
            TK::Succeed { fields } => {
                for (_, e) in fields {
                    self.check_reads(e, &a, s.line);
                }
                self.exit(&a, s.line, &[], Exit::Succeed);
                Abs::dead()
            }
            TK::Fail { error, leaving, cause } => {
                if let Some(c) = cause {
                    self.check_reads(c, &a, s.line);
                }
                self.exit(&a, s.line, leaving, Exit::Fail(error.clone()));
                Abs::dead()
            }
        }
    }

    /// The checks at an end of the workflow. The runs in `a` stop just before the end, so
    /// that the events of the other side that can still come show before it.
    fn exit(&mut self, a: &Abs, line: usize, leaving: &[usize], how: Exit) {
        for c in 0..self.m.cases.len() {
            if leaving.contains(&c) || a.cases[c].started == Tri::No {
                continue;
            }
            let cl = self.closure(c, &a.cases[c].states);
            let mc = self.m.machine(c);
            let bad: Vec<(usize, Path)> = cl.iter().filter(|(s, _)| !mc.is_final(**s)).map(|(s, p)| (*s, p.clone())).collect();
            if bad.is_empty() {
                continue;
            }
            let names = self.names(c, bad.iter().map(|(s, _)| *s)).join(", ");
            let finals = self.names(c, mc.finals.iter().cloned()).join(", ");
            let cn = self.case_name(c).to_string();
            let (en, ja) = match &how {
                Exit::Succeed | Exit::End => (
                    format!("the workflow can end here with the case `{cn}` in {names}, which is not final ({finals} are)"),
                    format!("案件 `{cn}` が {names} のまま、ここでワークフローが終わることがあります（終わりの状態は {finals}）"),
                ),
                Exit::Fail(_) | Exit::FailEnd => (
                    format!("the workflow can fail here with the case `{cn}` in {names}, which is not final ({finals} are); settle it first, or write `leaving {cn}` to hand it over as it is"),
                    format!("案件 `{cn}` が {names} のまま、ここでワークフローが失敗することがあります（終わりの状態は {finals}）。先に片付けるか、そのまま引き渡すなら `leaving {cn}` と書いてください"),
                ),
            };
            let mut p = bad[0].1.clone();
            let (xen, xja) = match &how {
                Exit::Succeed => ("succeed".to_string(), "succeed".to_string()),
                Exit::End => ("the flow ends".to_string(), "flow が終わる".to_string()),
                Exit::Fail(e) => (format!("fail {e}"), format!("fail {e}")),
                Exit::FailEnd => ("`on failure` ends and the workflow fails".to_string(), "`on failure` が終わり、ワークフローが失敗する".to_string()),
            };
            p.push(Step::new(line, xen, xja));
            self.push(Diag::error("E020", line, 1, en, ja).with_path(p));
        }
    }

    fn warn_failures(&mut self, points: &[(Abs, usize, String)], settling: bool) {
        // one warning per case: how many calls can fail with it unfinished, and the first
        for c in 0..self.m.cases.len() {
            let mut first: Option<(usize, String, Path, Vec<String>)> = None;
            let mut count = 0;
            for (a, line, callee) in points {
                if a.cases[c].started == Tri::No {
                    continue;
                }
                let cl = self.closure(c, &a.cases[c].states);
                let mc = self.m.machine(c);
                let bad: Vec<usize> = cl.keys().filter(|s| !mc.is_final(**s)).cloned().collect();
                if bad.is_empty() {
                    continue;
                }
                count += 1;
                if first.is_none() {
                    first = Some((*line, callee.clone(), cl[&bad[0]].clone(), self.names(c, bad.into_iter())));
                }
            }
            if let Some((line, callee, path, states)) = first {
                let cn = self.case_name(c).to_string();
                let st = states.join(", ");
                let (en, ja) = if settling {
                    (
                        format!("if `{callee}` fails while `on failure` is settling `{cn}`, the workflow fails with `{cn}` in {st} ({count} such call(s))"),
                        format!("`on failure` が `{cn}` を片付けている最中に `{callee}` が失敗すると、`{cn}` が {st} のままワークフローが失敗します（そうなる呼び出しは {count} か所）"),
                    )
                } else {
                    (
                        format!("if `{callee}` fails, the workflow fails with the case `{cn}` in {st}; {count} call(s) can fail like this. Handle the error at the call, or add `on failure` to settle the case"),
                        format!("`{callee}` が失敗すると、案件 `{cn}` が {st} のままワークフローが失敗します。そうなる呼び出しは {count} か所です。呼び出しでエラーを受けるか、`on failure` を足して案件を片付けてください"),
                    )
                };
                self.push(Diag::warning("W101", line, 1, en, ja).with_path(path));
            }
        }
    }

    fn call(&mut self, s: &TStmt, target: Option<&Target>, callee: &Callee, args: &[(String, TExpr)], handlers: &[THandler], a: Abs) -> Abs {
        for (_, e) in args {
            self.check_reads(e, &a, s.line);
        }
        let callee_name = match callee {
            Callee::Task(t) => self.m.tasks[*t].name.clone(),
            Callee::Rule(r) => self.m.rules[*r].name.clone(),
        };
        let handled: Vec<&HErr> = handlers.iter().flat_map(|h| h.errors.iter()).collect();
        let catches_all = handled.contains(&&HErr::Failure);
        let handles = |err: &HErr| handled.contains(&err) || catches_all;

        // What the call does to the known facts: on success, on the refusal, on other errors.
        let mut ok = a.clone();
        let mut refused: Option<Abs> = None;
        let mut other_err = a.clone();
        match target {
            Some(Target::Let(v)) => {
                ok = ok.step(Step::new(s.line, format!("{v} = {callee_name}(…)"), format!("{v} = {callee_name}(…)")));
                Flow::assign(&mut ok, v);
            }
            Some(Target::Case(c)) => {
                let c = *c;
                let t = match callee {
                    Callee::Task(t) => &self.m.tasks[*t],
                    _ => unreachable!("lowering refuses a rule on a case"),
                };
                let cn = self.case_name(c).to_string();
                match t.machine.clone() {
                    Some(TaskMachine::Starts { then, .. }) => {
                        match a.cases[c].started {
                            Tri::Yes => self.push(Diag::error("E013", s.line, 1, format!("the case `{cn}` has been started already"), format!("案件 `{cn}` はもう始まっています")).with_path(a.path.clone())),
                            Tri::Maybe => self.push(Diag::error("E013", s.line, 1, format!("the case `{cn}` may have been started already"), format!("案件 `{cn}` がもう始まっていることがあります")).with_path(a.path.clone())),
                            Tri::No => {}
                        }
                        let mc = self.m.machine(c);
                        let mut states: BTreeMap<usize, Path> = BTreeMap::new();
                        states.insert(mc.initial, a.path.clone());
                        for ev in &then {
                            let (axis, coord) = match self.event_axis(c, ev, None, s.line) {
                                Some(x) => x,
                                None => return Abs::dead(),
                            };
                            let mut fixed = self.m.cases[c].held.clone();
                            fixed.push((axis, coord));
                            let mut next: BTreeMap<usize, Path> = BTreeMap::new();
                            for (st, p) in &states {
                                for o in self.m.machine(c).outcomes(*st, &fixed) {
                                    if self.is_refused(c, &o) {
                                        let sn = self.state_name(c, *st).to_string();
                                        self.push(Diag::error("E021", s.line, 1, format!("the machine refuses `{ev}` in {sn}, so `{}` cannot start the case with it", t.name), format!("ステートマシンは {sn} で `{ev}` を断るので、`{}` はそれで案件を始められません", t.name)));
                                        continue;
                                    }
                                    next.entry(o.next).or_insert_with(|| p.clone());
                                }
                            }
                            states = next;
                        }
                        let mut st2 = BTreeMap::new();
                        for (st, p) in states {
                            let mut np = p;
                            np.push(Step::new(s.line, format!("{}: {cn} starts in {}", t.name, self.state_name(c, st)), format!("{}: {cn} が {} で始まる", t.name, self.state_name(c, st))));
                            st2.insert(st, np);
                        }
                        self.monitors.entry(s.site).or_insert((c, BTreeSet::new())).1.extend(st2.keys().cloned());
                        ok.cases[c] = CaseAbs { started: Tri::Yes, unstarted: None, states: st2 };
                        ok.path.push(Step::new(s.line, format!("{}: {cn} starts", t.name), format!("{}: {cn} が始まる", t.name)));
                        Flow::assign(&mut ok, &cn);
                        if !t.key {
                            self.push(Diag::warning(
                                "W103",
                                s.line,
                                1,
                                format!("if `{}` fails after the other side made the case, the workflow has no hold on it; give the task `key`, so that a retry finds the same one", t.name),
                                format!("相手の側で案件ができたあとに `{}` が失敗すると、ワークフローはその案件を見失います。やり直したときに同じ案件が返るよう、タスクに `key` を付けてください", t.name),
                            ));
                        }
                    }
                    Some(TaskMachine::Sends { event, column }) => {
                        if !self.need_started(c, &a, s.line) {
                            return Abs::dead();
                        }
                        let (axis, coord) = match self.event_axis(c, &event, column.as_deref(), s.line) {
                            Some(x) => x,
                            None => return Abs::dead(),
                        };
                        let now = self.closure(c, &a.cases[c].states);
                        let mut fixed = self.m.cases[c].held.clone();
                        fixed.push((axis, coord));
                        let mut next: BTreeMap<usize, Path> = BTreeMap::new();
                        let mut refusing: BTreeMap<usize, Path> = BTreeMap::new();
                        for (st, p) in &now {
                            for o in self.m.machine(c).outcomes(*st, &fixed) {
                                if self.is_refused(c, &o) {
                                    refusing.entry(*st).or_insert_with(|| p.clone());
                                    continue;
                                }
                                let mut np = p.clone();
                                np.push(Step::new(
                                    s.line,
                                    format!("{}: {cn} {} → {}", t.name, self.state_name(c, *st), self.state_name(c, o.next)),
                                    format!("{}: {cn} が {} → {}", t.name, self.state_name(c, *st), self.state_name(c, o.next)),
                                ));
                                match next.get(&o.next) {
                                    Some(q) if q.len() <= np.len() => {}
                                    _ => {
                                        next.insert(o.next, np);
                                    }
                                }
                            }
                        }
                        let refused_err = t.refused_as.clone();
                        if next.is_empty() {
                            let names = self.names(c, now.keys().cloned()).join(", ");
                            let p = now.values().next().cloned().unwrap_or_default();
                            self.push(Diag::error("E021", s.line, 1, format!("`{cn}` can be in {names} here, and the machine refuses `{event}` in every one of them"), format!("ここで `{cn}` は {names} のどれかで、ステートマシンはどの状態でも `{event}` を断ります")).with_path(p));
                            ok = Abs::dead();
                        } else {
                            self.monitors.entry(s.site).or_insert((c, BTreeSet::new())).1.extend(next.keys().cloned());
                            ok.cases[c].states = next.clone();
                            ok.path.push(Step::new(s.line, format!("{}: {event}", t.name), format!("{}: {event}", t.name)));
                            Flow::assign(&mut ok, &cn);
                        }
                        if !refusing.is_empty() {
                            let names = self.names(c, refusing.keys().cloned()).join(", ");
                            let p = refusing.values().next().cloned().unwrap_or_default();
                            match &refused_err {
                                None => self.push(
                                    Diag::error(
                                        "E022",
                                        s.line,
                                        1,
                                        format!("the machine may refuse `{event}` here (when `{cn}` is in {names}), but `{}` does not say how a refusal comes back; write `refused as <error>` under the task", t.name),
                                        format!("ここではステートマシンが `{event}` を断ることがあります（`{cn}` が {names} のとき）。`{}` には断られたときの返り方が書かれていません。タスクの下に `refused as <エラー>` を書いてください", t.name),
                                    )
                                    .with_path(p),
                                ),
                                Some(err) => {
                                    if !handles(&HErr::Declared(err.clone())) {
                                        self.push(
                                            Diag::error(
                                                "E022",
                                                s.line,
                                                1,
                                                format!("the machine may refuse `{event}` here, when `{cn}` is in {names}; handle it with `on {err} =>`"),
                                                format!("ここではステートマシンが `{event}` を断ることがあります（`{cn}` が {names} のとき）。`on {err} =>` で受けてください"),
                                            )
                                            .with_path(p),
                                        );
                                    }
                                    let mut r = a.clone();
                                    r.cases[c].states = refusing.clone();
                                    refused = Some(r.step(Step::new(s.line, format!("{}: {event} is refused ({err})", t.name), format!("{}: {event} が断られる（{err}）", t.name))));
                                }
                            }
                        } else if let Some(err) = &refused_err {
                            let only_refusal = handlers.iter().any(|h| h.errors.len() == 1 && h.errors[0] == HErr::Declared(err.clone()));
                            if only_refusal {
                                let names = self.names(c, now.keys().cloned()).join(", ");
                                self.push(Diag::warning("W102", s.line, 1, format!("`on {err}` never runs here: `{cn}` is in {names}, where `{event}` is never refused"), format!("ここでは `on {err}` は動きません。`{cn}` は {names} のどれかで、そこでは `{event}` は断られません")));
                            }
                        }
                        // any other error: the event may or may not have happened on the other side
                        let mut both = now.clone();
                        for (st, p) in &next {
                            both.entry(*st).or_insert_with(|| p.clone());
                        }
                        other_err.cases[c].states = both;
                    }
                    Some(TaskMachine::Observes) => {
                        // looking at a case that this run has not started reads one that already
                        // exists: it can be in any state the machine reaches from its start
                        let mut now = if a.cases[c].started != Tri::No { self.closure(c, &a.cases[c].states) } else { BTreeMap::new() };
                        if a.cases[c].started != Tri::Yes {
                            let base = a.cases[c].unstarted.clone().unwrap_or_default();
                            for (st, p) in self.reachable(c, base) {
                                now.entry(st).or_insert(p);
                            }
                        }
                        let mut seen = BTreeMap::new();
                        for (st, p) in &now {
                            let mut np = p.clone();
                            np.push(Step::new(s.line, format!("{}: {cn} is {}", t.name, self.state_name(c, *st)), format!("{}: {cn} は {}", t.name, self.state_name(c, *st))));
                            seen.insert(*st, np);
                        }
                        self.monitors.entry(s.site).or_insert((c, BTreeSet::new())).1.extend(seen.keys().cloned());
                        ok.cases[c].states = seen;
                        ok.cases[c].started = Tri::Yes;
                        ok.cases[c].unstarted = None;
                        ok.path.push(Step::new(s.line, format!("{}: look at {cn}", t.name), format!("{}: {cn} を見る", t.name)));
                        Flow::assign(&mut ok, &cn);
                    }
                    None => {}
                }
            }
            None => {}
        }

        // the errors the call can end with, and which of them nothing here takes
        let mut kinds: Vec<(HErr, String)> = Vec::new();
        if let Callee::Task(t) = callee {
            for e in &self.m.tasks[*t].errors {
                kinds.push((HErr::Declared(e.name.clone()), e.name.clone()));
            }
        }
        kinds.push((HErr::Timeout, "timeout".into()));
        kinds.push((HErr::Failure, "failure".into()));
        let refused_err = match (target, callee) {
            (Some(Target::Case(_)), Callee::Task(t)) => self.m.tasks[*t].refused_as.clone(),
            _ => None,
        };
        let unhandled: Vec<String> = kinds
            .iter()
            .filter(|(k, n)| !handles(k) && Some(n) != refused_err.as_ref())
            .map(|(_, n)| n.clone())
            .collect();
        if !unhandled.is_empty() {
            let fa = other_err.clone().step(Step::new(s.line, format!("{callee_name} fails ({})", unhandled.join(", ")), format!("{callee_name} が失敗する（{}）", unhandled.join("・"))));
            self.failures.push((fa, s.line, callee_name.clone()));
        }

        let mut after = ok;
        for h in handlers {
            let names: Vec<String> = h
                .errors
                .iter()
                .map(|e| match e {
                    HErr::Declared(n) => n.clone(),
                    HErr::Timeout => "timeout".into(),
                    HErr::Failure => "failure".into(),
                })
                .collect();
            let is_refusal = refused_err.as_ref().map(|r| names.len() == 1 && names[0] == *r).unwrap_or(false);
            let entry = if is_refusal {
                match &refused {
                    Some(r) => r.clone(),
                    None => continue,
                }
            } else {
                let mut en = other_err.clone();
                if let Some(r) = &refused {
                    if names.iter().any(|n| Some(n) == refused_err.as_ref()) || h.errors.contains(&HErr::Failure) {
                        en = join(&en, r);
                    }
                }
                en.step(Step::new(h.line, format!("{callee_name} fails: on {}", names.join(", ")), format!("{callee_name} が失敗する: on {}", names.join(", "))))
            };
            let end = self.stmts(&h.body, entry);
            after = join(&after, &end);
        }
        after
    }

    fn need_started(&mut self, c: usize, a: &Abs, line: usize) -> bool {
        let cabs = &a.cases[c];
        if cabs.started == Tri::Yes {
            return true;
        }
        let cn = self.case_name(c).to_string();
        let (en, ja) = if cabs.started == Tri::No {
            (format!("the case `{cn}` has not been started here"), format!("ここでは案件 `{cn}` はまだ始まっていません"))
        } else {
            (format!("the case `{cn}` may not have been started here"), format!("ここでは、案件 `{cn}` が始まっていないことがあります"))
        };
        self.push(Diag::error("E013", line, 1, en, ja).with_path(cabs.unstarted.clone().unwrap_or_default()));
        cabs.started == Tri::Maybe
    }

    fn matching(&mut self, s: &TStmt, expr: &TExpr, arms: &[TArm], a: Abs) -> Abs {
        let (name, fields) = match expr {
            TExpr::Var { name, fields, .. } => (name.clone(), fields.clone()),
            _ => (String::new(), vec![]),
        };
        let key = std::iter::once(name.clone()).chain(fields.iter().cloned()).collect::<Vec<_>>().join(".");
        let case_state = self.m.case_index(&name).filter(|c| fields.len() == 1 && self.m.cases[*c].state_field == fields[0]);
        if case_state.is_none() {
            // a case's state is read with a `none` arm for the runs that did not start it
            self.check_set(expr, &a, s.line);
        }
        let ety = expr.ty();
        let optional = matches!(ety, Ty::Opt(_));
        let mut domain: Vec<String> = match ety.inner() {
            Ty::Enum(e) => self.m.enums[*e].values.clone(),
            Ty::Bool => vec!["true".into(), "false".into()],
            _ => vec![],
        };
        // a value that may be absent: `none`, and its values, or `some` when they are not listed
        let some_values: Vec<String> = if domain.is_empty() { vec!["some".into()] } else { domain.clone() };
        if optional {
            if domain.is_empty() {
                domain.push("some".into());
            }
            domain.push("none".into());
        }
        // what the value can be here, with a run for each
        let mut possible: BTreeMap<String, Path> = BTreeMap::new();
        let narrowed;
        match case_state {
            Some(c) => {
                let cabs = &a.cases[c];
                if cabs.started != Tri::Yes {
                    possible.insert("none".into(), cabs.unstarted.clone().unwrap_or_default());
                }
                if cabs.started != Tri::No {
                    for (st, p) in &cabs.states {
                        possible.insert(self.state_name(c, *st).to_string(), p.clone());
                    }
                }
                narrowed = true;
            }
            None => match a.narrow.get(&key) {
                Some(vals) => {
                    for (v, p) in vals {
                        possible.insert(v.clone(), p.clone());
                    }
                    narrowed = true;
                }
                None => {
                    for v in &domain {
                        possible.insert(v.clone(), a.path.clone());
                    }
                    narrowed = false;
                }
            },
        }
        let listed: Vec<String> = possible.keys().cloned().collect();
        let mut covered: BTreeSet<String> = BTreeSet::new();
        let mut after = Abs::dead();
        for arm in arms {
            let mut vals: Vec<String> = arm.values.clone();
            if arm.none {
                vals.push("none".into());
            }
            if arm.some.is_some() {
                vals.extend(some_values.iter().cloned());
            }
            for v in &vals {
                if !possible.contains_key(v) {
                    if narrowed || (v == "none" && !optional) {
                        let (en, ja) = if v == "none" {
                            (format!("the arm `none` can never be taken: the case `{name}` has been started on every run that gets here"), format!("行き先 `none` は通りません。ここに来るときは、いつも案件 `{name}` が始まっています"))
                        } else {
                            (format!("the arm `{v}` can never be taken: `{key}` is one of {} here", listed.join(", ")), format!("行き先 `{v}` は通りません。ここで `{key}` は {} のどれかです", listed.join("・")))
                        };
                        let why = if v == "none" { a.path.clone() } else { possible.values().next().cloned().unwrap_or_else(|| a.path.clone()) };
                        self.push(Diag::error("E011", arm.line, 1, en, ja).with_path(why));
                    }
                } else {
                    covered.insert(v.clone());
                }
            }
            // the facts inside the arm
            let mut inner = a.clone();
            let here: Vec<String> = vals.iter().filter(|v| possible.contains_key(*v)).cloned().collect();
            if here.is_empty() {
                // an arm that cannot be taken is still checked, as if it could be
                inner = inner.step(Step::new(arm.line, format!("match {key}: {}", vals.join(", ")), format!("match {key}: {}", vals.join(", "))));
            } else {
                inner = inner.step(Step::new(arm.line, format!("match {key}: {}", here.join(", ")), format!("match {key}: {}", here.join(", "))));
            }
            match case_state {
                Some(c) => {
                    let keep: BTreeSet<usize> = here.iter().filter(|v| *v != "none").filter_map(|v| self.m.machine(c).state_index(v)).collect();
                    inner.cases[c].states.retain(|st, _| keep.contains(st));
                    let with_none = here.iter().any(|v| v == "none");
                    inner.cases[c].started = match (with_none, keep.is_empty()) {
                        (true, true) => Tri::No,
                        (true, false) => Tri::Maybe,
                        (false, _) => Tri::Yes,
                    };
                    if inner.cases[c].started == Tri::Yes {
                        inner.cases[c].unstarted = None;
                        let cn = self.case_name(c).to_string();
                        inner.set.insert(cn, (Tri::Yes, None));
                    }
                }
                None => {
                    if let Some(v) = &arm.some {
                        Flow::assign(&mut inner, v);
                    }
                    if !key.is_empty() && !here.is_empty() {
                        let vals: BTreeMap<String, Path> = here.iter().filter_map(|v| possible.get(v).map(|p| {
                            let mut p = p.clone();
                            p.push(Step::new(arm.line, format!("match {key}: {v}"), format!("match {key}: {v}")));
                            (v.clone(), p)
                        })).collect();
                        if let Some(p) = vals.values().min_by_key(|p| p.len()) {
                            inner.path = p.clone();
                        }
                        inner.narrow.insert(key.clone(), vals);
                    }
                }
            }
            let end = self.stmts(&arm.body, inner);
            after = join(&after, &end);
        }
        let missing: Vec<(String, Path)> = possible.iter().filter(|(v, _)| !covered.contains(*v)).map(|(v, p)| (v.clone(), p.clone())).collect();
        if !missing.is_empty() {
            let names: Vec<String> = missing.iter().map(|(v, _)| v.clone()).collect();
            let (en, ja) = (
                format!("`match` has no arm for {}, which `{key}` can be here", names.join(", ")),
                format!("`match` に {} の行き先がありません。ここで `{key}` はその値を取りえます", names.join("・")),
            );
            let path = missing[0].1.clone();
            self.push(Diag::error("E010", s.line, 1, en, ja).with_path(path));
        }
        after
    }
}

pub fn show_dur(s: u64) -> String {
    if s % 86400 == 0 {
        let d = s / 86400;
        format!("{d} day{}", if d == 1 { "" } else { "s" })
    } else if s % 3600 == 0 {
        let h = s / 3600;
        format!("{h} hour{}", if h == 1 { "" } else { "s" })
    } else if s % 60 == 0 {
        let m = s / 60;
        format!("{m} minute{}", if m == 1 { "" } else { "s" })
    } else {
        format!("{s} second{}", if s == 1 { "" } else { "s" })
    }
}

pub fn show_dur_ja(s: u64) -> String {
    if s % 86400 == 0 {
        format!("{} 日", s / 86400)
    } else if s % 3600 == 0 {
        format!("{} 時間", s / 3600)
    } else if s % 60 == 0 {
        format!("{} 分", s / 60)
    } else {
        format!("{s} 秒")
    }
}

pub fn severity_is_error(d: &Diag) -> bool {
    d.severity == Severity::Error
}

fn show(e: &TExpr) -> String {
    match e {
        TExpr::Str(s) => format!("\"{s}\""),
        TExpr::Interp(_) => format!("\"{}\"", e.show()),
        other => other.show(),
    }
}
