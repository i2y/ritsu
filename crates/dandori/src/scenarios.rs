//! Scenarios that take every arm: the execution's input and the answers of the calls,
//! chosen so that between them the runs go down every arm of every match, into every
//! handler, and through every way the machine lets a case move. They are found by running
//! the workflow again and again with a record of the choices made so far, taking the next
//! untried choice each time (a depth-first walk by replay), and keeping the runs that
//! reach something no earlier run reached.
//!
//! The runs here only choose answers; what the answers mean is the reference
//! interpreter's business. A choice made here that the interpreter takes differently only
//! costs coverage, never a wrong verdict.

use crate::interp::matches_error;
use crate::model::*;
use crate::render;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

const MAX_RUNS: usize = 5000;
const MAX_STEPS: usize = 2000;
const HOLE: &str = "\u{1}hole:";

enum Ctl {
    Next,
    Break,
    Stop,
}

struct Ex<'a> {
    m: &'a Model,
    /// how often each alternative of each choice has been taken, over all runs so far
    counts: BTreeMap<(String, usize), usize>,
    /// the choices to make first, in order
    prefix: Vec<usize>,
    /// every choice made in this run: (the alternative taken, how many there were)
    trail: Vec<(usize, usize)>,
    /// in a sticky run, a choice made once is made the same way again in the same run,
    /// which is what it takes for a loop to run out
    sticky: bool,
    made: BTreeMap<String, usize>,
    labels: BTreeSet<String>,
    vars: BTreeMap<String, Value>,
    /// the values left open: the type, what was chosen, and the range of the numbers in it
    holes: Vec<(Ty, Option<Value>, Option<Range>)>,
    cases: Vec<Option<usize>>,
    answers: Vec<Value>,
    in_on_failure: bool,
    /// a cancellation stopped the run, and `on cancel` runs next
    cancelled: bool,
    in_on_cancel: bool,
    steps: usize,
    stopped: bool,
    /// how many parallel rounds are running around the current statement
    par_depth: usize,
    /// the current parallel round failed: by a task's error (true) or otherwise (false)
    pending: Option<bool>,
    /// the `for` loops that went through a list with something in it, in this run
    done_loops: Vec<String>,
    /// how many values have been made in this run, so that no two are alike and a target
    /// that mixes up the rounds of a loop, or two answers, shows it
    made_values: usize,
}

pub fn generate(m: &Model) -> Vec<Value> {
    let mut out = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut counts: BTreeMap<(String, usize), usize> = BTreeMap::new();
    // a run is a prefix of choices to replay, then its own choices; every run that reaches
    // something new becomes the ground for runs that change one of its choices
    let mut queue: std::collections::VecDeque<(Vec<usize>, bool)> = std::collections::VecDeque::new();
    queue.push_back((vec![], false));
    queue.push_back((vec![], true));
    let mut runs = 0;
    while let Some((prefix, sticky)) = queue.pop_front() {
        if runs >= MAX_RUNS {
            break;
        }
        runs += 1;
        let mut ex = Ex {
            m,
            counts: std::mem::take(&mut counts),
            prefix: prefix.clone(),
            trail: vec![],
            sticky,
            made: BTreeMap::new(),
            labels: BTreeSet::new(),
            vars: BTreeMap::new(),
            holes: vec![],
            cases: vec![None; m.cases.len()],
            answers: vec![],
            in_on_failure: false,
            cancelled: false,
            in_on_cancel: false,
            steps: 0,
            stopped: false,
            par_depth: 0,
            pending: None,
            done_loops: vec![],
            made_values: 0,
        };
        ex.run();
        counts = std::mem::take(&mut ex.counts);
        let new: Vec<String> = ex.labels.iter().filter(|l| !seen.contains(*l)).cloned().collect();
        if new.is_empty() {
            continue;
        }
        seen.extend(new);
        for i in prefix.len()..ex.trail.len() {
            let (choice, n) = ex.trail[i];
            for a in 0..n {
                if a != choice {
                    let mut p: Vec<usize> = ex.trail[..i].iter().map(|(c, _)| *c).collect();
                    p.push(a);
                    queue.push_back((p, true));
                }
            }
        }
        let input = ex.finish_input();
        let answers: Vec<Value> = ex
            .answers
            .iter()
            .map(|a| {
                let filled = ex.fill(a);
                // a Claude agent's answer with its enum values in another case, as Claude may give them
                if let Some(t) = a.get("recase").and_then(|t| t.as_u64()) {
                    return json!({ "ok": render::recase(m, &filled["ok"], m.tasks[t as usize].result.as_ref().unwrap_or(&Ty::Json)) });
                }
                // Jev's response, whose answers read as the value chosen: as sure as the task asks, or less
                if let Some(j) = a.get("jev").and_then(|t| t.as_u64()).and_then(|t| m.tasks[t as usize].jev()) {
                    return json!({ "ok": render::jev_wire(j, &filled["ok"], a.get("low") == Some(&json!(true))) });
                }
                filled
            })
            .collect();
        out.push(json!({ "name": format!("run {}", out.len() + 1), "input": input, "answers": answers, "covers": ex.labels.iter().collect::<Vec<_>>() }));
    }
    out
}

impl<'a> Ex<'a> {
    /// Take the alternative of this choice that the runs so far have taken least.
    fn choose(&mut self, at: &str, n: usize) -> usize {
        let n = n.max(1);
        let pos = self.trail.len();
        if pos < self.prefix.len() {
            let c = self.prefix[pos].min(n - 1);
            self.trail.push((c, n));
            self.made.insert(at.to_string(), c);
            return c;
        }
        if self.sticky {
            if let Some(c) = self.made.get(at).cloned() {
                if c < n {
                    self.trail.push((c, n));
                    return c;
                }
            }
        }
        let mut best = 0;
        let mut best_count = usize::MAX;
        for i in 0..n {
            let c = *self.counts.get(&(at.to_string(), i)).unwrap_or(&0);
            if c < best_count {
                best = i;
                best_count = c;
            }
        }
        *self.counts.entry((at.to_string(), best)).or_insert(0) += 1;
        self.made.insert(at.to_string(), best);
        self.trail.push((best, n));
        best
    }

    fn hole(&mut self, t: &Ty, rg: Option<Range>) -> Value {
        self.holes.push((t.clone(), None, rg));
        json!(format!("{HOLE}{}", self.holes.len() - 1))
    }

    /// A number in `rg` not made before in this run, as far as the range has room.
    fn number(&mut self, rg: Option<Range>) -> Value {
        self.made_values += 1;
        let k = self.made_values as i128;
        let r = rg.unwrap_or_default();
        let n = match (r.lo.map(i128::from), r.hi.map(i128::from)) {
            _ if r.contains(1000 + k as i64) => 1000 + k,
            (Some(lo), Some(hi)) => lo + k % (hi - lo + 1),
            (Some(lo), None) => lo + k,
            (None, Some(hi)) => hi - k,
            (None, None) => 1000 + k,
        };
        json!(n as i64)
    }

    /// A value of type `t` with its enums, bools, optional values and lists left open, to be
    /// chosen when read; its numbers in `rg`, and its records' in theirs.
    fn template(&mut self, t: &Ty, name: &str, rg: Option<Range>) -> Value {
        match t {
            Ty::Str => {
                self.made_values += 1;
                json!(format!("{name}-{}", self.made_values))
            }
            Ty::Timestamp => json!("2026-10-01T10:00:00Z"),
            Ty::Int | Ty::Num(_) => self.number(rg),
            // an array, so that the targets show they keep a `json` value that is one as one item
            Ty::Json => {
                self.made_values += 1;
                json!([{ "note": format!("{name}-{}", self.made_values) }])
            }
            Ty::Bool | Ty::Enum(_) | Ty::Opt(_) | Ty::List(_) => self.hole(t, rg),
            Ty::Record(r) => {
                let fields = self.m.records[*r].fields.clone();
                let mut o = Map::new();
                for (f, ft) in fields {
                    let v = self.template(&ft, &f, self.m.field_range(*r, &f));
                    o.insert(f, v);
                }
                Value::Object(o)
            }
        }
    }

    /// A value of type `t` with nothing left open: every value that may be absent there, every
    /// list with two items, and the enums' values in turn.
    fn full(&mut self, t: &Ty, name: &str, rg: Option<Range>) -> Value {
        match t {
            Ty::Bool => json!(true),
            Ty::Enum(e) => {
                self.made_values += 1;
                let values = &self.m.enums[*e].values;
                json!(values[self.made_values % values.len()])
            }
            Ty::Opt(inner) => self.full(inner, name, rg),
            Ty::List(inner) => Value::Array((1..=2).map(|i| self.full(inner, &format!("item{i}"), rg)).collect()),
            Ty::Record(r) => {
                let fields = self.m.records[*r].fields.clone();
                let mut o = Map::new();
                for (f, ft) in fields {
                    let v = self.full(&ft, &f, self.m.field_range(*r, &f));
                    o.insert(f, v);
                }
                Value::Object(o)
            }
            _ => self.template(t, name, rg),
        }
    }

    /// A value of type `t` unlike the one a value left open reads as: each enum its last value, a
    /// bool false, where an open one reads as the first value and true.
    fn unlike(&mut self, t: &Ty, name: &str, rg: Option<Range>) -> Value {
        match t {
            Ty::Bool => json!(false),
            Ty::Enum(e) => json!(self.m.enums[*e].values.last()),
            Ty::Record(r) => {
                let fields = self.m.records[*r].fields.clone();
                let mut o = Map::new();
                for (f, ft) in fields {
                    let v = self.unlike(&ft, &f, self.m.field_range(*r, &f));
                    o.insert(f, v);
                }
                Value::Object(o)
            }
            _ => self.template(t, name, rg),
        }
    }

    /// A value of type `t` with a number outside its range, the first there is: None when no
    /// number in it has a range.
    fn outside(&mut self, t: &Ty, rg: Option<Range>) -> Option<Value> {
        match t {
            Ty::Int | Ty::Num(_) => rg?.beyond().map(|n| json!(n)),
            Ty::Opt(inner) => self.outside(inner, rg),
            Ty::List(inner) => self.outside(inner, rg).map(|v| json!([v])),
            Ty::Record(r) => {
                let fields = self.m.records[*r].fields.clone();
                let at = fields.iter().position(|(f, ft)| self.has_outside(ft, self.m.field_range(*r, f)))?;
                let mut o = Map::new();
                for (i, (f, ft)) in fields.iter().enumerate() {
                    let frg = self.m.field_range(*r, f);
                    let v = if i == at { self.outside(ft, frg)? } else { self.full(ft, f, frg) };
                    o.insert(f.clone(), v);
                }
                Some(Value::Object(o))
            }
            _ => None,
        }
    }

    /// Whether `outside` finds a number with a range in a value of type `t`.
    fn has_outside(&self, t: &Ty, rg: Option<Range>) -> bool {
        match t {
            Ty::Int | Ty::Num(_) => rg.and_then(|r| r.beyond()).is_some(),
            Ty::Opt(inner) | Ty::List(inner) => self.has_outside(inner, rg),
            Ty::Record(r) => self.m.records[*r].fields.iter().any(|(f, ft)| self.has_outside(ft, self.m.field_range(*r, f))),
            _ => false,
        }
    }

    fn domain(&self, t: &Ty) -> Vec<Value> {
        match t {
            Ty::Bool => vec![json!(true), json!(false)],
            Ty::Enum(e) => self.m.enums[*e].values.iter().map(|v| json!(v)).collect(),
            // a list left open until the end is empty, an optional value absent
            Ty::List(_) => vec![json!([])],
            _ => vec![Value::Null],
        }
    }

    fn hole_id(v: &Value) -> Option<usize> {
        v.as_str().and_then(|s| s.strip_prefix(HOLE)).and_then(|x| x.parse().ok())
    }

    /// Read a value, choosing an open enum, bool or optional value now.
    fn read(&mut self, v: Value, at: &str) -> Value {
        match Ex::hole_id(&v) {
            Some(id) => {
                if let Some(x) = &self.holes[id].1 {
                    return x.clone();
                }
                let t = self.holes[id].0.clone();
                let picked = match &t {
                    Ty::Opt(inner) => {
                        // absent, or each value of an enum or a bool, or some value
                        let mut dom = vec![Value::Null];
                        let listed = matches!(**inner, Ty::Bool | Ty::Enum(_));
                        if listed {
                            dom.extend(self.domain(inner));
                        }
                        let n = if listed { dom.len() } else { 2 };
                        let c = self.choose(at, n);
                        if c == 0 {
                            Value::Null
                        } else if listed {
                            dom[c].clone()
                        } else {
                            let rg = self.holes[id].2;
                            self.template(inner, "value", rg)
                        }
                    }
                    _ => {
                        let dom = self.domain(&t);
                        let c = self.choose(at, dom.len());
                        dom[c].clone()
                    }
                };
                self.holes[id].1 = Some(picked.clone());
                picked
            }
            None => v,
        }
    }

    /// The items of a list a `for` goes through, choosing the length of an open one now:
    /// empty, one, two, or one more than the loop takes.
    fn items(&mut self, v: Value, max: u32, at: &str) -> Vec<Value> {
        let id = match Ex::hole_id(&v) {
            Some(id) => id,
            None => return v.as_array().cloned().unwrap_or_default(),
        };
        if let Some(x) = &self.holes[id].1 {
            return x.as_array().cloned().unwrap_or_default();
        }
        let elem = match &self.holes[id].0 {
            Ty::List(t) => (**t).clone(),
            _ => return vec![],
        };
        let mut lengths: Vec<usize> = vec![0, 1, 2];
        lengths.retain(|n| *n <= max as usize);
        lengths.push(max as usize + 1);
        let c = self.choose(at, lengths.len());
        let rg = self.holes[id].2;
        let mut items = Vec::new();
        for i in 0..lengths[c] {
            let v = self.template(&elem, &format!("item{}", i + 1), rg);
            items.push(v);
        }
        self.holes[id].1 = Some(Value::Array(items.clone()));
        items
    }

    /// A deliberate end, and for every loop that went through items on the way, the end after
    /// it: so that a run that fills a list and ends well is kept, not only one with an empty list.
    fn end_label(&mut self, end: &str) {
        self.labels.insert(end.to_string());
        for l in self.done_loops.clone() {
            self.labels.insert(format!("{l} then {end}"));
        }
    }

    /// Stop here: the run ends, or inside a parallel round the round does, and the loop
    /// fails when every round is done.
    fn halt(&mut self, task_error: bool) -> Ctl {
        if self.par_depth > 0 {
            if self.pending.is_none() {
                self.pending = Some(task_error);
            }
        } else {
            self.stopped = true;
        }
        Ctl::Stop
    }

    fn fill(&self, v: &Value) -> Value {
        match v {
            Value::String(_) => match Ex::hole_id(v) {
                Some(id) => {
                    let x = self.holes[id].1.clone().unwrap_or_else(|| self.domain(&self.holes[id].0)[0].clone());
                    self.fill(&x)
                }
                None => v.clone(),
            },
            Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), self.fill(x))).collect()),
            Value::Array(a) => Value::Array(a.iter().map(|x| self.fill(x)).collect()),
            other => other.clone(),
        }
    }

    fn finish_input(&self) -> Value {
        let mut o = Map::new();
        for (n, _) in &self.m.inputs {
            o.insert(n.clone(), self.fill(self.vars.get(n).unwrap_or(&Value::Null)));
        }
        Value::Object(o)
    }

    fn value(&mut self, e: &TExpr) -> Value {
        match e {
            TExpr::Var { name, fields, .. } => {
                let mut v = self.vars.get(name).cloned().unwrap_or(Value::Null);
                for f in fields {
                    // a record that was left open and chosen since reads as what was chosen
                    if let Some(id) = Ex::hole_id(&v) {
                        v = self.holes[id].1.clone().unwrap_or(Value::Null);
                    }
                    v = v.get(f).cloned().unwrap_or(Value::Null);
                }
                v
            }
            other => crate::interp::value_of(&self.vars, other),
        }
    }

    fn run(&mut self) {
        for (v, _) in &self.m.vars {
            self.vars.insert(v.clone(), Value::Null);
        }
        let inputs = self.m.inputs.clone();
        for (n, t) in &inputs {
            let v = self.template(t, n, self.m.input_ranges.get(n).copied());
            self.vars.insert(n.clone(), v);
        }
        let flow = self.m.flow.clone();
        self.block(&flow);
        if self.cancelled {
            if let Some(block) = self.m.on_cancel.clone() {
                self.stopped = false;
                self.in_on_cancel = true;
                self.labels.insert("on cancel".into());
                self.block(&block);
            }
        }
    }

    fn block(&mut self, ss: &[TStmt]) -> Ctl {
        for s in ss {
            if self.stopped || self.pending.is_some() {
                return Ctl::Stop;
            }
            self.steps += 1;
            if self.steps > MAX_STEPS {
                self.stopped = true;
                return Ctl::Stop;
            }
            match self.stmt(s) {
                Ctl::Next => {}
                other => return other,
            }
        }
        Ctl::Next
    }

    fn stmt(&mut self, s: &TStmt) -> Ctl {
        match &s.kind {
            TK::Pass | TK::Wait { .. } | TK::WaitUntil { .. } => Ctl::Next,
            TK::Break => Ctl::Break,
            TK::Assign { name, expr } => {
                let v = self.value(expr);
                self.vars.insert(name.clone(), v);
                Ctl::Next
            }
            TK::Succeed { .. } => {
                self.end_label(&format!("{}:succeed", s.site));
                self.halt(false)
            }
            TK::Fail { .. } => {
                self.end_label(&format!("{}:fail", s.site));
                self.halt(false)
            }
            TK::For { var, list, max, parallel, body, result, locals } => {
                let raw = self.value(list);
                let items = self.items(raw, *max, &format!("for {}", s.site));
                if items.len() > *max as usize {
                    self.labels.insert(format!("{}:too many", s.site));
                    return self.halt(false);
                }
                if items.is_empty() {
                    self.labels.insert(format!("{}:empty", s.site));
                }
                let mut out = Vec::new();
                match parallel {
                    None => {
                        let mut ran = 0;
                        for it in items.iter() {
                            self.vars.insert(var.clone(), it.clone());
                            let c = self.block(body);
                            match c {
                                Ctl::Break => {
                                    self.labels.insert(format!("{}:break", s.site));
                                    break;
                                }
                                Ctl::Stop => return Ctl::Stop,
                                Ctl::Next => {
                                    ran += 1;
                                    // a round, and more than one: where the rounds' order shows
                                    let l = if ran > 1 { format!("{}:ran again", s.site) } else { format!("{}:ran", s.site) };
                                    self.labels.insert(l.clone());
                                    if !self.done_loops.contains(&l) {
                                        self.done_loops.push(l);
                                    }
                                    if let Some((_, y)) = result {
                                        let v = self.value(y);
                                        out.push(v);
                                    }
                                }
                            }
                        }
                    }
                    Some(_) => {
                        let mut first: Option<bool> = None;
                        let outer = self.pending.take();
                        for it in items.iter() {
                            for l in locals {
                                self.vars.insert(l.clone(), Value::Null);
                            }
                            self.vars.insert(var.clone(), it.clone());
                            self.par_depth += 1;
                            let _ = self.block(body);
                            self.par_depth -= 1;
                            if self.stopped {
                                // a cancellation after a round failed: the cancellation still decides
                                if self.cancelled && first.is_some() {
                                    self.labels.insert(format!("{}:cancelled after a round failed", s.site));
                                }
                                return Ctl::Stop;
                            }
                            match self.pending.take() {
                                Some(k) => {
                                    self.labels.insert(format!("{}:a round fails", s.site));
                                    if first.is_none() {
                                        first = Some(k);
                                    }
                                }
                                None => {
                                    if let Some((_, y)) = result {
                                        let v = self.value(y);
                                        out.push(v);
                                    }
                                }
                            }
                        }
                        self.pending = outer;
                        if let Some(task_error) = first {
                            if task_error {
                                return self.unhandled();
                            }
                            return self.halt(false);
                        }
                        if !items.is_empty() {
                            // every round, and more than one: where the order of what they yield shows
                            let l = if items.len() > 1 { format!("{}:every one of several rounds done", s.site) } else { format!("{}:every round done", s.site) };
                            self.labels.insert(l.clone());
                            if !self.done_loops.contains(&l) {
                                self.done_loops.push(l);
                            }
                        }
                    }
                }
                if let Some((r, _)) = result {
                    self.vars.insert(r.clone(), Value::Array(out));
                }
                Ctl::Next
            }
            TK::Repeat { times, body } => {
                for n in 0..*times {
                    let c = self.block(body);
                    match c {
                        Ctl::Break => {
                            self.labels.insert(format!("{}:break", s.site));
                            return Ctl::Next;
                        }
                        Ctl::Stop => return Ctl::Stop,
                        Ctl::Next => {}
                    }
                    if n + 1 == *times {
                        self.labels.insert(format!("{}:exhausted", s.site));
                    }
                }
                Ctl::Next
            }
            TK::Match { expr, arms } => {
                let raw = self.value(expr);
                let v = self.read(raw, &format!("match {}", s.site));
                for (i, a) in arms.iter().enumerate() {
                    let hit = (a.none && v.is_null())
                        || (a.some.is_some() && !v.is_null())
                        || a.values.iter().any(|x| match &v {
                            Value::String(s) => s == x,
                            Value::Bool(b) => b.to_string() == *x,
                            _ => false,
                        });
                    if hit {
                        self.labels.insert(format!("{}:arm{}", s.site, i));
                        if let Some(n) = &a.some {
                            self.vars.insert(n.clone(), v.clone());
                        }
                        return self.block(&a.body);
                    }
                }
                self.halt(false)
            }
            TK::Call { target, callee, handlers, .. } => self.call(s, target.as_ref(), callee, handlers),
        }
    }

    fn event_fix(&self, c: usize, event: &str, column: Option<&str>) -> Option<(usize, usize)> {
        let mc = self.m.machine(c);
        let axis = match column {
            Some(col) => mc.axis_of(col)?,
            None => *mc.axes_with_value(event).first()?,
        };
        Some((axis, mc.axes[axis].coords.iter().position(|x| x == event)?))
    }

    fn refused(&self, c: usize, o: &crate::rulec::Outcome) -> bool {
        match &self.m.cases[c].refused_when {
            Some((i, v)) => o.produces.get(*i).cloned().flatten().as_deref() == Some(v.as_str()),
            None => false,
        }
    }

    /// The states the case can really be in now: where it was, and wherever the other side's events take it.
    fn closure(&self, c: usize, st: usize) -> Vec<usize> {
        let case = &self.m.cases[c];
        let mc = self.m.machine(c);
        let mut out = vec![st];
        let mut i = 0;
        while i < out.len() {
            let s = out[i];
            for (axis, coord, _) in &case.external {
                let mut fixed = case.held.clone();
                fixed.push((*axis, *coord));
                for o in mc.outcomes(s, &fixed) {
                    if !self.refused(c, &o) && !out.contains(&o.next) {
                        out.push(o.next);
                    }
                }
            }
            i += 1;
        }
        out
    }

    fn reachable(&self, c: usize) -> Vec<usize> {
        let case = &self.m.cases[c];
        let mc = self.m.machine(c);
        let mut out = vec![mc.initial];
        let mut i = 0;
        while i < out.len() {
            let s = out[i];
            for a in 0..mc.axes.len() {
                if a == mc.state_axis || case.held.iter().any(|(h, _)| *h == a) {
                    continue;
                }
                for ci in 0..mc.axes[a].coords.len() {
                    let mut fixed = case.held.clone();
                    fixed.push((a, ci));
                    for o in mc.outcomes(s, &fixed) {
                        if !self.refused(c, &o) && !out.contains(&o.next) {
                            out.push(o.next);
                        }
                    }
                }
            }
            i += 1;
        }
        out
    }

    fn call(&mut self, s: &TStmt, target: Option<&Target>, callee: &Callee, handlers: &[THandler]) -> Ctl {
        let m = self.m;
        let result_ty = match callee {
            Callee::Task(t) => m.tasks[*t].result.clone(),
            Callee::Rule(r) => Some(Ty::Record(m.rules[*r].outputs)),
        };
        // the ways this call can come out: (label, state of the case after, answer)
        enum Way {
            Ok(Option<usize>),
            Err(String, Option<usize>),
            RetryThenOk(Option<usize>),
            /// a Claude agent's answer whose enum values differ from the enum's in case
            Recased(Option<usize>),
            /// an answer with a state the generated check does not let through
            Unexpected(usize),
            /// an answer that does not have the declared shape
            Malformed,
            /// an answer with a number outside its range, in a case's state the call may lead to
            OutOfRange(Option<usize>),
            /// a Connect answer whose fields are at their zero values, which its JSON leaves out
            Zeros,
            /// a cancellation that comes while the call is out
            Cancelled,
            /// Jev's answer, less sure than the task asks: the task's error, from an answer
            Low(String),
        }
        let mut ways: Vec<(String, Way)> = Vec::new();
        let case = match target {
            Some(Target::Case(c)) => Some(*c),
            _ => None,
        };
        match (case, callee) {
            (Some(c), Callee::Task(t)) => {
                let task = &m.tasks[*t];
                let mc = m.machine(c);
                match &task.machine {
                    Some(TaskMachine::Starts { then, .. }) => {
                        let mut states = vec![mc.initial];
                        for ev in then {
                            let mut next = Vec::new();
                            if let Some(fx) = self.event_fix(c, ev, None) {
                                let mut fixed = m.cases[c].held.clone();
                                fixed.push(fx);
                                for st in &states {
                                    for o in mc.outcomes(*st, &fixed) {
                                        if !self.refused(c, &o) && !next.contains(&o.next) {
                                            next.push(o.next);
                                        }
                                    }
                                }
                            }
                            states = next;
                        }
                        for st in states {
                            ways.push((format!("starts:{}", mc.states[st]), Way::Ok(Some(st))));
                        }
                    }
                    Some(TaskMachine::Sends { event, column }) => {
                        let now = self.cases[c].unwrap_or(mc.initial);
                        if let Some(fx) = self.event_fix(c, event, column.as_deref()) {
                            let mut fixed = m.cases[c].held.clone();
                            fixed.push(fx);
                            let mut seen = BTreeSet::new();
                            for real in self.closure(c, now) {
                                for o in mc.outcomes(real, &fixed) {
                                    if self.refused(c, &o) {
                                        if let Some(err) = &task.refused_as {
                                            if seen.insert(format!("refused@{}", mc.states[real])) {
                                                ways.push((format!("refused:{}", mc.states[real]), Way::Err(err.clone(), Some(real))));
                                            }
                                        }
                                    } else if seen.insert(format!("ok:{}", mc.states[o.next])) {
                                        ways.push((format!("to:{}", mc.states[o.next]), Way::Ok(Some(o.next))));
                                    }
                                }
                            }
                        }
                    }
                    Some(TaskMachine::Observes) => {
                        let seen = match self.cases[c] {
                            Some(now) => self.closure(c, now),
                            // a case this run did not start: any state the machine reaches
                            None => self.reachable(c),
                        };
                        for st in seen {
                            ways.push((format!("sees:{}", mc.states[st]), Way::Ok(Some(st))));
                        }
                    }
                    None => ways.push(("ok".into(), Way::Ok(None))),
                }
            }
            _ => ways.push(("ok".into(), Way::Ok(None))),
        }
        let retriers = retriers(m, callee);
        // a Jev task's error for an answer less sure than it asks
        let floor = match callee {
            Callee::Task(t) => m.tasks[*t].jev().and_then(|j| j.floor.as_ref()).map(|(_, e)| e.clone()),
            Callee::Rule(_) => None,
        };
        for h in handlers {
            let kind = match h.errors.first() {
                Some(HErr::Declared(n)) => n.clone(),
                Some(HErr::Timeout) => "timeout".into(),
                _ => "failure".into(),
            };
            let label = format!("on:{kind}");
            if !ways.iter().any(|(l, _)| *l == label) {
                if floor.as_deref() == Some(kind.as_str()) {
                    ways.push((label, Way::Low(kind)));
                } else {
                    ways.push((label, Way::Err(kind, None)));
                }
            }
        }
        if let Some(e) = &floor {
            // the error nothing at the call takes, or `on failure` takes
            if !ways.iter().any(|(l, _)| *l == format!("on:{e}")) {
                ways.push((format!("less sure:{e}"), Way::Low(e.clone())));
            }
        }
        let caught_all = handlers.iter().any(|h| h.errors.contains(&HErr::Failure));
        if !caught_all {
            ways.push(("unhandled".into(), Way::Err("failure".into(), None)));
        }
        let first_ok = ways.iter().find_map(|(_, w)| match w {
            Way::Ok(st) => Some(*st),
            _ => None,
        });
        if let Callee::Task(t) = callee {
            if m.tasks[*t].retry.is_some() {
                if let Some(st) = first_ok {
                    ways.push(("retried".into(), Way::RetryThenOk(st)));
                }
            }
        }
        if let (Callee::Task(t), Some(ty), Some(st), true) = (callee, &result_ty, first_ok, target.is_some()) {
            if matches!(m.tasks[*t].via(Platform::Temporal), Some(Via::Agent { provider: Provider::Claude, .. })) && render::has_cased_enum(m, ty) {
                ways.push(("recased".into(), Way::Recased(st)));
            }
        }
        if let (Some(c), Some((_, allowed))) = (case, m.monitors.get(&s.site)) {
            let mc = m.machine(c);
            if let Some(st) = (0..mc.states.len()).find(|x| !allowed.contains(&mc.states[*x])) {
                ways.push(("unexpected state".into(), Way::Unexpected(st)));
            }
        }
        // an answer that does not fit is one the workflow reads: not for a task that answers nothing, or anything
        if target.is_some() && !matches!(result_ty, None | Some(Ty::Json)) {
            ways.push(("malformed".into(), Way::Malformed));
        }
        // a number out of its range; not in Jev's answer, whose numbers are how sure it is
        let jev_task = match callee {
            Callee::Task(t) => m.tasks[*t].jev().map(|_| *t),
            Callee::Rule(_) => None,
        };
        if let (Some(ty), Some(st), true, None) = (&result_ty, first_ok, target.is_some(), jev_task) {
            if self.has_outside(ty, m.answer_range(callee)) {
                ways.push(("out of range".into(), Way::OutOfRange(st)));
            }
        }
        let zeros = match callee {
            Callee::Task(t) => m.tasks[*t].connect.clone(),
            Callee::Rule(_) => None,
        };
        if let (Some(_), Some(_), true, None) = (&zeros, first_ok, target.is_some(), case) {
            ways.push(("zero values".into(), Way::Zeros));
        }
        // a workflow that says what to do when it is cancelled can be cancelled at any call, but not in `on cancel`
        if self.m.on_cancel.is_some() && !self.in_on_cancel {
            ways.push(("cancelled".into(), Way::Cancelled));
        }
        let pick = self.choose(&format!("call {}", s.site), ways.len());
        let (label, way) = ways.swap_remove(pick);
        self.labels.insert(format!("{}:{label}", s.site));
        let ok_answer = |ex: &mut Ex, st: Option<usize>| -> Value {
            let mut v = match &result_ty {
                Some(t) => ex.template(t, "value", m.answer_range(callee)),
                None => Value::Null,
            };
            if let (Some(c), Some(st)) = (case, st) {
                let field = m.cases[c].state_field.clone();
                v[field] = json!(m.machine(c).states[st]);
            }
            v
        };
        // an answer, which for a Jev task becomes Jev's response once its value is chosen
        let ok = |v: &Value| match jev_task {
            Some(t) => json!({ "ok": v, "jev": t }),
            None => json!({ "ok": v }),
        };
        match way {
            Way::Ok(st) => {
                let v = ok_answer(self, st);
                self.answers.push(ok(&v));
                self.assign(target, v, st);
                Ctl::Next
            }
            Way::Low(kind) => {
                // not a value the variable may hold already, so that a target that keeps it shows
                let v = self.unlike(result_ty.as_ref().unwrap_or(&Ty::Json), "value", m.answer_range(callee));
                self.answers.push(json!({ "ok": v, "jev": jev_task, "low": true }));
                self.take_error(callee, &kind, &kind, handlers)
            }
            Way::Recased(st) => {
                // every part of the answer there, lists with items, so that every enum value in it is turned
                let t = match callee {
                    Callee::Task(t) => *t,
                    Callee::Rule(_) => unreachable!("only an agent's answer is recased"),
                };
                let v = self.full(result_ty.as_ref().unwrap_or(&Ty::Json), "value", m.answer_range(callee));
                self.answers.push(json!({ "ok": v, "recase": t }));
                self.assign(target, v, st);
                Ctl::Next
            }
            Way::Unexpected(st) => {
                let v = ok_answer(self, Some(st));
                self.answers.push(json!({ "ok": v }));
                self.halt(false)
            }
            Way::Malformed => {
                // protobuf reads {} as a message at its zero values, so a Connect answer that does not fit is a list
                self.answers.push(json!({ "ok": if zeros.is_some() { json!([]) } else { json!({}) } }));
                self.halt(false)
            }
            Way::Zeros => {
                // every part there, lists with items, so that fields are left out at every depth
                let z = zeros.as_ref().expect("a Connect task");
                let v = self.full(result_ty.as_ref().expect("an answer to read"), "value", m.answer_range(callee));
                let wire = crate::apis::sparse(&v, z);
                self.answers.push(json!({ "ok": wire }));
                self.assign(target, crate::apis::fill(&wire, z), None);
                Ctl::Next
            }
            Way::OutOfRange(st) => {
                let mut v = self.outside(result_ty.as_ref().expect("an answer with a range has a type"), m.answer_range(callee)).expect("a number with a range is there");
                if let (Some(c), Some(st)) = (case, st) {
                    let field = m.cases[c].state_field.clone();
                    v[field] = json!(m.machine(c).states[st]);
                }
                self.answers.push(json!({ "ok": v }));
                self.halt(false)
            }
            Way::Cancelled => {
                // every round stops, not only this one
                self.answers.push(json!({ "cancel": true }));
                self.cancelled = true;
                self.stopped = true;
                Ctl::Stop
            }
            Way::RetryThenOk(st) => {
                // one error that the retriers take, then an answer
                let kind = match callee {
                    Callee::Task(t) => match &m.tasks[*t].retry {
                        Some(r) if !r.on.is_empty() && !r.on.iter().any(|x| x == "failure") => r.on[0].clone(),
                        _ => "failure".to_string(),
                    },
                    Callee::Rule(_) => "failure".to_string(),
                };
                self.answers.push(json!({ "error": kind }));
                let v = ok_answer(self, st);
                self.answers.push(ok(&v));
                self.assign(target, v, st);
                Ctl::Next
            }
            Way::Err(kind, real) => {
                // as many times as the retriers would ask for it again, so that it comes through
                let name = error_name(m, callee, &kind);
                let tries = 1 + retriers.iter().find(|(names, _)| matches_error(names, &name)).map(|(_, max)| *max as usize).unwrap_or(0);
                for _ in 0..tries {
                    self.answers.push(json!({ "error": kind }));
                }
                if let (Some(c), Some(r)) = (case, real) {
                    self.cases[c] = Some(r);
                }
                self.take_error(callee, &kind, &name, handlers)
            }
        }
    }

    /// A call's error, `kind` as the `.flow` names it and `name` as Step Functions does: the first
    /// handler that takes it runs; else the round ends, or the run, after `on failure`.
    fn take_error(&mut self, callee: &Callee, kind: &str, name: &str, handlers: &[THandler]) -> Ctl {
        let m = self.m;
        for h in handlers {
            let hit = h.errors.iter().any(|e| match e {
                HErr::Failure => true,
                HErr::Timeout => kind == "timeout",
                HErr::Declared(n) => n == kind || render::asl_error(m, callee, &HErr::Declared(n.clone())).iter().any(|x| x == name),
            });
            if hit {
                return self.block(&h.body);
            }
        }
        if self.par_depth > 0 {
            return self.halt(true);
        }
        self.unhandled()
    }

    /// A task's error that nothing took: `on failure` runs, and the run ends.
    fn unhandled(&mut self) -> Ctl {
        if self.par_depth > 0 {
            return self.halt(true);
        }
        if !self.in_on_failure && !self.in_on_cancel {
            if let Some(block) = self.m.on_failure.clone() {
                self.in_on_failure = true;
                self.labels.insert("on failure".into());
                self.block(&block);
            }
        }
        self.stopped = true;
        Ctl::Stop
    }

    fn assign(&mut self, target: Option<&Target>, v: Value, st: Option<usize>) {
        match target {
            Some(Target::Let(x)) => {
                self.vars.insert(x.clone(), v);
            }
            Some(Target::Case(c)) => {
                self.vars.insert(self.m.cases[*c].name.clone(), v);
                if let Some(st) = st {
                    self.cases[*c] = Some(st);
                }
            }
            None => {}
        }
    }
}

fn error_name(m: &Model, callee: &Callee, kind: &str) -> String {
    match kind {
        "timeout" => "States.Timeout".into(),
        "failure" => crate::interp::asl_failure(m, callee).into(),
        other => render::asl_error(m, callee, &HErr::Declared(other.to_string())).into_iter().next().unwrap_or_else(|| other.to_string()),
    }
}

fn retriers(m: &Model, callee: &Callee) -> Vec<(Vec<String>, u32)> {
    let v = match callee {
        Callee::Rule(_) => crate::asl::rule_retriers(),
        Callee::Task(t) => match &m.tasks[*t].retry {
            Some(r) => crate::asl::retriers(m, callee, &m.tasks[*t], r),
            None => json!([]),
        },
    };
    v.as_array()
        .unwrap()
        .iter()
        .map(|r| (r["ErrorEquals"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect(), r["MaxAttempts"].as_u64().unwrap_or(3) as u32))
        .collect()
}
