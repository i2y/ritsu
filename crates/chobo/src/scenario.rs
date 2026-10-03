//! Scenarios and their results, as JSON (PLAN 0.3), and running one in the reference
//! interpreter. `together` is run in every order its callers' operations can interleave,
//! and the result is every distinct way it can come out (DESIGN 2.6).

use ritsu_base::text::{Lang, Text};
use crate::interp::{AccountId, Call, HoldState, Op, Outcome, State, Val};
use crate::model::*;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// The most operations one `together` may hold: past it, the orders to try grow too many.
pub const MAX_TOGETHER: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Call(Call),
    /// seconds, and the duration as written
    Pass(u64, String),
    /// each caller's operations, in order
    Together(Vec<Vec<Call>>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scenario {
    pub name: String,
    pub steps: Vec<Step>,
}

/// `31 minutes` in seconds.
pub fn parse_duration(s: &str) -> Option<u64> {
    let (n, unit) = s.trim().split_once(' ')?;
    let n: u64 = n.parse().ok()?;
    n.checked_mul(crate::syntax::duration(unit.trim())?)
}

/// Seconds in the largest unit that divides them: `31 minutes`, `90 seconds`.
pub fn format_duration(secs: u64) -> String {
    for (per, one, many) in [(86400, "day", "days"), (3600, "hour", "hours"), (60, "minute", "minutes")] {
        if secs > 0 && secs % per == 0 {
            let n = secs / per;
            return format!("{n} {}", if n == 1 { one } else { many });
        }
    }
    format!("{secs} {}", if secs == 1 { "second" } else { "seconds" })
}

pub fn call_from_json(book: &Book, v: &Value) -> Result<Call, String> {
    let op_name = v["op"].as_str().ok_or("a step needs `op`")?;
    let op = Op::parse(op_name).ok_or_else(|| format!("unknown op `{op_name}`"))?;
    let kind_name = v["kind"].as_str().ok_or("a step needs `kind`")?;
    let kind = book.transfer(kind_name).ok_or_else(|| format!("the book has no transfer `{kind_name}`"))?;
    let t = &book.transfers[kind];
    let mut args: Vec<Option<Val>> = vec![None; t.params.len()];
    let given = v["args"].as_object().ok_or("a step needs `args`")?;
    for (name, x) in given {
        let i = t.params.iter().position(|p| &p.name == name).ok_or_else(|| format!("`{kind_name}` has no parameter `{name}`"))?;
        args[i] = Some(match (&t.params[i].ty, x) {
            (Ty::Str, Value::String(s)) => Val::Str(s.clone()),
            (Ty::Amount(_), Value::Number(n)) => Val::Amt(n.as_i64().map(|v| v as i128).ok_or_else(|| format!("`{name}` is not a whole number in range"))?),
            (Ty::Str, _) => return Err(format!("`{name}` is a string")),
            (Ty::Amount(_), _) => return Err(format!("`{name}` is an amount, a whole number")),
        });
    }
    let amounts = match v.get("amounts") {
        None | Some(Value::Null) => None,
        Some(Value::Object(m)) => {
            let mut a = BTreeMap::new();
            for (name, x) in m {
                let i = t.params.iter().position(|p| &p.name == name).ok_or_else(|| format!("`{kind_name}` has no parameter `{name}`"))?;
                let n = x.as_i64().ok_or_else(|| format!("`{name}` is an amount, a whole number"))?;
                a.insert(i, n as i128);
            }
            Some(a)
        }
        Some(_) => return Err("`amounts` is an object".into()),
    };
    let c = Call { op, kind, args, amounts };
    crate::interp::validate(book, &c)?;
    Ok(c)
}

pub fn call_to_json(book: &Book, c: &Call) -> Value {
    let t = &book.transfers[c.kind];
    let mut args = Map::new();
    for (i, p) in t.params.iter().enumerate() {
        if let Some(v) = &c.args[i] {
            args.insert(
                p.name.clone(),
                match v {
                    Val::Str(s) => json!(s),
                    Val::Amt(a) => json!(*a as i64),
                },
            );
        }
    }
    let mut o = Map::new();
    o.insert("op".into(), json!(c.op.name()));
    o.insert("kind".into(), json!(t.name));
    o.insert("args".into(), Value::Object(args));
    if let Some(a) = &c.amounts {
        let m: Map<String, Value> = a.iter().map(|(i, v)| (t.params[*i].name.clone(), json!(*v as i64))).collect();
        o.insert("amounts".into(), Value::Object(m));
    }
    Value::Object(o)
}

pub fn from_json(book: &Book, v: &Value) -> Result<Scenario, String> {
    if let Some(b) = v.get("book").and_then(|b| b.as_str()) {
        if b != book.name {
            return Err(format!("the scenario is for the book `{b}`, not `{}`", book.name));
        }
    }
    let name = v["name"].as_str().unwrap_or("").to_string();
    let mut steps = Vec::new();
    for (i, s) in v["steps"].as_array().ok_or("a scenario needs `steps`")?.iter().enumerate() {
        let at = |e: String| format!("step {}: {e}", i + 1);
        match s["op"].as_str() {
            Some("pass") => {
                let d = s["duration"].as_str().ok_or_else(|| at("`pass` needs `duration`".into()))?;
                let secs = parse_duration(d).ok_or_else(|| at(format!("`{d}` is not a duration like `31 minutes`")))?;
                steps.push(Step::Pass(secs, d.to_string()));
            }
            Some("together") => {
                let callers = s["callers"].as_array().ok_or_else(|| at("`together` needs `callers`".into()))?;
                let mut cs = Vec::new();
                let mut total = 0;
                for c in callers {
                    let ops = c.as_array().ok_or_else(|| at("each caller is a list of operations".into()))?;
                    let mut list = Vec::new();
                    for o in ops {
                        if matches!(o["op"].as_str(), Some("pass") | Some("together")) {
                            return Err(at("`pass` and `together` cannot be inside `together`".into()));
                        }
                        list.push(call_from_json(book, o).map_err(at)?);
                    }
                    total += list.len();
                    cs.push(list);
                }
                if total > MAX_TOGETHER {
                    return Err(at(format!("`together` holds {total} operations; at most {MAX_TOGETHER} can be tried in every order")));
                }
                steps.push(Step::Together(cs));
            }
            _ => steps.push(Step::Call(call_from_json(book, s).map_err(at)?)),
        }
    }
    Ok(Scenario { name, steps })
}

pub fn to_json(book: &Book, s: &Scenario) -> Value {
    let steps: Vec<Value> = s
        .steps
        .iter()
        .map(|st| match st {
            Step::Call(c) => call_to_json(book, c),
            Step::Pass(_, d) => json!({"op": "pass", "duration": d}),
            Step::Together(cs) => {
                json!({"op": "together", "callers": cs.iter().map(|ops| ops.iter().map(|c| call_to_json(book, c)).collect::<Vec<_>>()).collect::<Vec<_>>()})
            }
        })
        .collect();
    json!({"name": s.name, "book": book.name, "steps": steps})
}

#[derive(Clone, Debug, PartialEq)]
pub enum StepOut {
    Call(Outcome),
    /// the holds that expired
    Pass(Vec<(usize, Vec<String>)>),
    Together(Vec<Vec<Outcome>>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub steps: Vec<StepOut>,
    pub state: State,
}

/// Every order the callers' operations can interleave in, each caller's kept in order:
/// a list of callers, one per operation.
pub fn interleavings(lens: &[usize]) -> Vec<Vec<usize>> {
    let total: usize = lens.iter().sum();
    let mut out = Vec::new();
    let mut cur = Vec::new();
    let mut left = lens.to_vec();
    fn go(left: &mut Vec<usize>, cur: &mut Vec<usize>, total: usize, out: &mut Vec<Vec<usize>>) {
        if cur.len() == total {
            out.push(cur.clone());
            return;
        }
        for i in 0..left.len() {
            if left[i] > 0 {
                left[i] -= 1;
                cur.push(i);
                go(left, cur, total, out);
                cur.pop();
                left[i] += 1;
            }
        }
    }
    go(&mut left, &mut cur, total, &mut out);
    out
}

/// Run a scenario from an empty book. Without `together` there is one run; with it, one
/// for each distinct way it can come out.
pub fn run(book: &Book, s: &Scenario) -> Result<Vec<Run>, String> {
    let mut runs = vec![Run { steps: vec![], state: State::default() }];
    for (i, st) in s.steps.iter().enumerate() {
        let at = |e: String| format!("step {}: {e}", i + 1);
        let mut next: Vec<Run> = Vec::new();
        for r in runs {
            match st {
                Step::Call(c) => {
                    let mut r = r;
                    let o = r.state.apply(book, c).map_err(at)?;
                    r.steps.push(StepOut::Call(o));
                    push_new(&mut next, r);
                }
                Step::Pass(secs, _) => {
                    let mut r = r;
                    let e = r.state.pass(*secs);
                    r.steps.push(StepOut::Pass(e));
                    push_new(&mut next, r);
                }
                Step::Together(cs) => {
                    let lens: Vec<usize> = cs.iter().map(|c| c.len()).collect();
                    for order in interleavings(&lens) {
                        let mut state = r.state.clone();
                        let mut outs: Vec<Vec<Option<Outcome>>> = lens.iter().map(|n| vec![None; *n]).collect();
                        let mut pos = vec![0usize; cs.len()];
                        for caller in order {
                            let c = &cs[caller][pos[caller]];
                            let o = state.apply(book, c).map_err(at)?;
                            outs[caller][pos[caller]] = Some(o);
                            pos[caller] += 1;
                        }
                        let mut steps = r.steps.clone();
                        steps.push(StepOut::Together(outs.into_iter().map(|v| v.into_iter().map(|o| o.unwrap()).collect()).collect()));
                        push_new(&mut next, Run { steps, state });
                    }
                }
            }
        }
        runs = next;
    }
    Ok(runs)
}

fn push_new(runs: &mut Vec<Run>, r: Run) {
    if !runs.contains(&r) {
        runs.push(r);
    }
}

pub fn has_together(s: &Scenario) -> bool {
    s.steps.iter().any(|st| matches!(st, Step::Together(_)))
}

/// Every account a `do` or `hold` of the scenario names, whatever came of it, in the order of
/// the account kinds and then of the arguments: what a runner reads back from a database.
pub fn named_accounts(book: &Book, s: &Scenario) -> Vec<AccountId> {
    let mut out: Vec<AccountId> = Vec::new();
    let mut add = |c: &Call| {
        if matches!(c.op, Op::Do | Op::Hold) {
            for id in c.accounts(book) {
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
    };
    for st in &s.steps {
        match st {
            Step::Call(c) => add(c),
            Step::Together(cs) => cs.iter().flatten().for_each(&mut add),
            Step::Pass(..) => {}
        }
    }
    out.sort();
    out
}

fn outcome_json(book: &Book, c: &Call, o: &Outcome) -> Value {
    let mut m = Map::new();
    m.insert("op".into(), json!(c.op.name()));
    m.insert("kind".into(), json!(book.transfers[c.kind].name));
    m.insert("result".into(), json!(o.result()));
    if let Some(r) = o.reason() {
        m.insert("reason".into(), json!(r));
    }
    Value::Object(m)
}

/// One run as PLAN 0.3 writes a result.
pub fn result_json(book: &Book, s: &Scenario, r: &Run) -> Value {
    let steps: Vec<Value> = s
        .steps
        .iter()
        .zip(&r.steps)
        .map(|(st, out)| match (st, out) {
            (Step::Call(c), StepOut::Call(o)) => outcome_json(book, c, o),
            (Step::Pass(..), _) => json!({"op": "pass"}),
            (Step::Together(cs), StepOut::Together(os)) => json!({
                "op": "together",
                "callers": cs.iter().zip(os).map(|(cc, oo)| cc.iter().zip(oo).map(|(c, o)| outcome_json(book, c, o)).collect::<Vec<_>>()).collect::<Vec<_>>(),
            }),
            _ => Value::Null,
        })
        .collect();
    let accounts: Vec<Value> = named_accounts(book, s)
        .iter()
        .map(|id| {
            let b = r.state.balance(id);
            json!({"account": book.accounts[id.kind].name, "args": id.args, "posted": b.posted as i64, "held_in": b.held_in as i64, "held_out": b.held_out as i64})
        })
        .collect();
    let holds: Vec<Value> = r
        .state
        .holds
        .iter()
        .map(|((k, key), h)| json!({"kind": book.transfers[*k].name, "key": key, "state": h.state.name()}))
        .collect();
    json!({"steps": steps, "accounts": accounts, "holds": holds})
}

/// The result of a scenario as PLAN 0.3 writes it: one result, or with `together`, every
/// distinct one, in the order of their text.
pub fn run_json(book: &Book, s: &Scenario) -> Result<Value, String> {
    let runs = run(book, s)?;
    if !has_together(s) {
        return Ok(result_json(book, s, &runs[0]));
    }
    let mut outs: Vec<(String, Value)> = runs.iter().map(|r| result_json(book, s, r)).map(|v| (v.to_string(), v)).collect();
    outs.sort_by(|a, b| a.0.cmp(&b.0));
    outs.dedup_by(|a, b| a.0 == b.0);
    Ok(json!({"outcomes": outs.into_iter().map(|(_, v)| v).collect::<Vec<_>>()}))
}

// ── as a person reads it ──────────────────────────────────────────────────

/// A call as a person reads it: `引当.hold(注文: 注文-3, sku: sku-2, 数: 3)`.
pub fn call_text(book: &Book, c: &Call) -> String {
    let t = &book.transfers[c.kind];
    let mut parts: Vec<String> = Vec::new();
    for (i, p) in t.params.iter().enumerate() {
        if let Some(v) = &c.args[i] {
            parts.push(format!("{}: {}", p.name, v.text()));
        }
    }
    if let Some(a) = &c.amounts {
        for (i, v) in a {
            parts.push(format!("{}: {v}", t.params[*i].name));
        }
    }
    format!("{}.{}({})", t.name, c.op.name(), parts.join(", "))
}

pub fn outcome_text(o: &Outcome) -> Text {
    match o {
        Outcome::Done => tr!("通る", "done"),
        Outcome::DoneBefore => tr!("done_before（前に済んでいる）", "done_before"),
        Outcome::Refused(r) => {
            let reason = &r.reason;
            tr!("{reason} で断られる", "refused: {reason}")
        }
    }
}

pub fn state_text(s: HoldState) -> Text {
    match s {
        HoldState::Held => tr!("押さえ中", "held"),
        HoldState::Posted => tr!("確定", "posted"),
        HoldState::Voided => tr!("取消", "voided"),
        HoldState::Expired => tr!("期限切れ", "expired"),
    }
}

/// How wide a string shows in a terminal: East Asian wide characters take two columns.
pub fn width(s: &str) -> usize {
    s.chars()
        .map(|c| {
            let u = c as u32;
            let wide = matches!(u, 0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6 | 0x20000..=0x3FFFD);
            if wide { 2 } else { 1 }
        })
        .sum()
}

pub fn pad(s: &str, w: usize) -> String {
    format!("{s}{}", " ".repeat(w.saturating_sub(width(s))))
}

pub fn hold_text(book: &Book, k: usize, key: &[String]) -> String {
    format!("{}({})", book.transfers[k].name, key.join(", "))
}

/// The steps, the accounts and the holds of one run, as `chobo run` prints them.
fn run_text(book: &Book, s: &Scenario, r: &Run, lang: Lang, indent: &str) -> String {
    let mut rows: Vec<(String, String)> = Vec::new();
    for (i, (st, out)) in s.steps.iter().zip(&r.steps).enumerate() {
        let n = i + 1;
        match (st, out) {
            (Step::Call(c), StepOut::Call(o)) => rows.push((format!("{n:>3}  {}", call_text(book, c)), outcome_text(o).get(lang).to_string())),
            (Step::Pass(_, d), StepOut::Pass(e)) => {
                let what = if e.is_empty() {
                    String::new()
                } else {
                    let names: Vec<String> = e.iter().map(|(k, key)| hold_text(book, *k, key)).collect();
                    let names = names.join(", ");
                    tr!("{names} が期限切れ", "{names} expired").get(lang).to_string()
                };
                rows.push((format!("{n:>3}  pass {d}"), what));
            }
            (Step::Together(cs), StepOut::Together(os)) => {
                rows.push((format!("{n:>3}  together"), String::new()));
                for (ci, (cc, oo)) in cs.iter().zip(os).enumerate() {
                    for (c, o) in cc.iter().zip(oo) {
                        let who = tr!("呼び出し元 {}", "caller {}", ci + 1);
                        rows.push((format!("       {}: {}", who.get(lang), call_text(book, c)), outcome_text(o).get(lang).to_string()));
                    }
                }
            }
            _ => {}
        }
    }
    let w = rows.iter().map(|(a, _)| width(a)).max().unwrap_or(0);
    let mut out = String::new();
    for (a, b) in &rows {
        if b.is_empty() {
            out.push_str(&format!("{indent}{a}\n"));
        } else {
            out.push_str(&format!("{indent}{}  {b}\n", pad(a, w)));
        }
    }
    let accts = named_accounts(book, s);
    if !accts.is_empty() {
        out.push_str(&format!("{indent}{}\n", tr!("勘定:", "accounts:").get(lang)));
        let names: Vec<String> = accts.iter().map(|id| id.text(book)).collect();
        let w = names.iter().map(|n| width(n)).max().unwrap_or(0);
        for (id, name) in accts.iter().zip(&names) {
            let b = r.state.balance(id);
            let (p, o, i) = (b.posted, b.held_out, b.held_in);
            let line = tr!("確定 {p}、出ていく仮押さえ {o}、入ってくる仮押さえ {i}", "posted {p}, held out {o}, held in {i}");
            out.push_str(&format!("{indent}  {}  {}\n", pad(name, w), line.get(lang)));
        }
    }
    if !r.state.holds.is_empty() {
        out.push_str(&format!("{indent}{}\n", tr!("仮押さえ:", "holds:").get(lang)));
        let names: Vec<String> = r.state.holds.keys().map(|(k, key)| hold_text(book, *k, key)).collect();
        let w = names.iter().map(|n| width(n)).max().unwrap_or(0);
        for (h, name) in r.state.holds.values().zip(&names) {
            out.push_str(&format!("{indent}  {}  {}\n", pad(name, w), state_text(h.state).get(lang)));
        }
    }
    out
}

/// What `chobo run` prints: the steps and what came of them, then the accounts and the holds;
/// with `together`, each way it can come out.
pub fn render(book: &Book, file: &str, s: &Scenario, runs: &[Run], lang: Lang) -> String {
    let n = s.steps.len();
    let mut out = String::new();
    let name = if s.name.is_empty() { String::new() } else { format!(" ({})", s.name) };
    if runs.len() == 1 && !has_together(s) {
        out.push_str(&format!("{file}{name}: {}\n", tr!("{n} ステップ", "{n} steps").get(lang)));
        out.push_str(&run_text(book, s, &runs[0], lang, ""));
        return out;
    }
    let mut texts: Vec<(String, &Run)> = runs.iter().map(|r| (result_json(book, s, r).to_string(), r)).collect();
    texts.sort_by(|a, b| a.0.cmp(&b.0));
    texts.dedup_by(|a, b| a.0 == b.0);
    let k = texts.len();
    out.push_str(&format!("{file}{name}: {}\n", tr!("{n} ステップ、とりうる結果は {k} 通り", "{n} steps, {k} possible outcomes").get(lang)));
    for (i, (_, r)) in texts.iter().enumerate() {
        out.push_str(&format!("{}\n", tr!("結果 {}:", "outcome {}:", i + 1).get(lang)));
        out.push_str(&run_text(book, s, r, lang, "  "));
    }
    out
}

// ── a step at a time ──────────────────────────────────────────────────────

/// One way a scenario comes out, with the state after each of its steps: what `chobo doc` shows
/// a step at a time.
#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
    pub steps: Vec<StepOut>,
    /// the state after each step
    pub states: Vec<State>,
}

impl Trace {
    pub fn run(&self) -> Run {
        Run { steps: self.steps.clone(), state: self.states.last().cloned().unwrap_or_default() }
    }
}

/// Every distinct way a scenario can come out, as `run` finds them, each with the state after
/// every step, in the order `chobo run` prints them (that of their results' text).
pub fn trace(book: &Book, s: &Scenario) -> Result<Vec<Trace>, String> {
    let mut traces = vec![Trace { steps: vec![], states: vec![] }];
    for (i, st) in s.steps.iter().enumerate() {
        let at = |e: String| format!("step {}: {e}", i + 1);
        let mut next: Vec<Trace> = Vec::new();
        for t in &traces {
            let before = t.states.last().cloned().unwrap_or_default();
            let mut ends: Vec<(StepOut, State)> = Vec::new();
            match st {
                Step::Call(c) => {
                    let mut state = before;
                    let o = state.apply(book, c).map_err(at)?;
                    ends.push((StepOut::Call(o), state));
                }
                Step::Pass(secs, _) => {
                    let mut state = before;
                    let e = state.pass(*secs);
                    ends.push((StepOut::Pass(e), state));
                }
                Step::Together(cs) => {
                    let lens: Vec<usize> = cs.iter().map(|c| c.len()).collect();
                    for order in interleavings(&lens) {
                        let mut state = before.clone();
                        let mut outs: Vec<Vec<Option<Outcome>>> = lens.iter().map(|n| vec![None; *n]).collect();
                        let mut pos = vec![0usize; cs.len()];
                        for caller in order {
                            let o = state.apply(book, &cs[caller][pos[caller]]).map_err(at)?;
                            outs[caller][pos[caller]] = Some(o);
                            pos[caller] += 1;
                        }
                        let outs = outs.into_iter().map(|v| v.into_iter().map(|o| o.unwrap()).collect()).collect();
                        ends.push((StepOut::Together(outs), state));
                    }
                }
            }
            for (out, state) in ends {
                let mut t2 = t.clone();
                t2.steps.push(out);
                t2.states.push(state);
                if !next.contains(&t2) {
                    next.push(t2);
                }
            }
        }
        traces = next;
    }
    let mut keyed: Vec<(String, Trace)> = traces.into_iter().map(|t| (result_json(book, s, &t.run()).to_string(), t)).collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    keyed.dedup_by(|a, b| a.0 == b.0);
    Ok(keyed.into_iter().map(|(_, t)| t).collect())
}
