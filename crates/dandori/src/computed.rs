//! The answers rulec, koyomi and chobo give a run in place of the scenario's (ritsu's DESIGN 7.9,
//! `ritsu run`): a rule's outputs, as rulec's evaluator computes them from the inputs; a date of a
//! dates file, as koyomi's interpreter computes it; and what an operation of a book comes to, as
//! chobo's reference interpreter answers it on a book that keeps what was done, and whose holds
//! expire as the run's time goes by. The other calls are answered by the scenario, in order, as
//! `dandori run` answers them.
//!
//! Each answer is written as a scenario writes one, and the reference interpreter takes it as it
//! takes a scenario's (crate::interp::Answers): so the run is the one `dandori run` makes of the
//! scenario with these answers in it, which [`Ran::replay`] is. What each language answers is the
//! language's own: dandori only hands it the values as the code dandori writes hands them —
//! a day as the calendar's offset reads a time, a book's arguments by the transfer's parameters,
//! a rule's answer as its Connect service writes it — and reads the answer back the same way.
//!
//! Time: the run starts at the scenario's `now`, and goes on by each `wait` (to the moment of a
//! `wait until`), each wait before a retry, and the timeout of each try that timed out. A try that
//! is answered takes no time: the scenario does not say how long it took, and none is the least it
//! may have. The books are told the time as it goes by. `now` itself reads the scenario's moment
//! throughout, as the reference interpreter reads it (crate::interp::NOW_VAR).

use crate::interp::{Answers, Passing};
use crate::model::*;
use crate::render::{self, View};
use crate::rulec::RType;
use ritsu_base::text::Text;
use ritsu_ports::{Balance, BookCall, BookOutcome, Books, DateFacts, DateValue, Dates, Ledger, RuleError, Rules};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

/// Who answered a try of a call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who {
    Rulec,
    Koyomi,
    Chobo,
    Scenario,
}

impl Who {
    pub fn word(self) -> &'static str {
        match self {
            Who::Rulec => "rulec",
            Who::Koyomi => "koyomi",
            Who::Chobo => "chobo",
            Who::Scenario => "scenario",
        }
    }
}

/// What happened in a run, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A try of a call, answered: who answered it, the call's name (`reserve`, `terms.payment`),
    /// the arguments as dandori hands them, and the answer, as a scenario writes one.
    Answer { who: Who, call: String, args: Map<String, Value>, answer: Value },
    /// Time went by: how many seconds, why, the moment a `wait until` was given, and the moment
    /// after.
    Pass { seconds: f64, why: Passing, until: Option<String>, after: String },
    /// A hold expired as the time went by: the book (the flow's name for it), the transfer, the
    /// values of the hold's key.
    Expired { book: String, transfer: String, key: Vec<String> },
}

/// A book as a run leaves it.
#[derive(Clone, Debug, PartialEq)]
pub struct BookEnd {
    /// the flow's name for the book, and its file as the flow reaches it
    pub name: String,
    pub file: PathBuf,
    /// what the scenario had done to the book before the run (`books` in the scenario), each
    /// operation as it was written, with what it came to (`done`, `done_before`)
    pub before: Vec<(Value, String)>,
    /// every account a move has touched, with its balance; every hold, with its state
    pub accounts: Vec<(String, Vec<String>, Balance)>,
    pub holds: Vec<(String, Vec<String>, String)>,
}

/// A run, its rules, dates and books computed.
#[derive(Clone, Debug)]
pub struct Ran {
    /// the trace, as `dandori run` prints it
    pub trace: Value,
    /// the scenario with every answer in it, in order: `dandori run` on it makes this run again
    pub replay: Value,
    pub events: Vec<Event>,
    pub books: Vec<BookEnd>,
    /// the moment the run starts at, how long its time went on, and the moment it ends at
    pub start: String,
    pub elapsed: f64,
    pub end: String,
}

/// Why a run did not come to an end.
#[derive(Clone, Debug, PartialEq)]
pub enum Stopped {
    /// The scenario cannot be run as it is written (a book it names that the flow does not use, an
    /// operation the book refuses before the run, a `now` that is not a time).
    Scenario(Text),
    /// The run cannot go on: the scenario has no answer for a call, or a language cannot answer one.
    Run(String),
}

/// The languages a run asks, and what it keeps between the calls: the books, and its time.
pub struct Computed {
    rules: Rc<dyn Rules>,
    dates: Rc<dyn Dates>,
    /// each book of the flow's (by its place in the model), opened at the start
    ledgers: Vec<Box<dyn Ledger>>,
    names: Vec<String>,
    /// what koyomi knows of each dates file, asked once
    date_facts: BTreeMap<PathBuf, DateFacts>,
    /// the moment the run starts at, in milliseconds since 1970-01-01 in UTC, and the milliseconds
    /// gone by since
    start_ms: i64,
    elapsed_ms: u64,
    events: Vec<Event>,
}

/// Run the flow on the scenario, with a rule's outputs computed by `rules` (rulec), a date by
/// `dates` (koyomi), and an operation of a book by `books` (chobo), on books that start as the
/// scenario's `books` leaves them; every other call is answered by the scenario.
pub fn run(m: &Model, sc: &Value, view: View, rules: Rc<dyn Rules>, dates: Rc<dyn Dates>, books: Rc<dyn Books>) -> Result<Ran, Stopped> {
    let start = sc.get("now").and_then(|v| v.as_str()).unwrap_or(render::SCENARIO_NOW).to_string();
    let Some(start_ms) = moment_ms(&start) else {
        return Err(Stopped::Scenario(ritsu_base::tr!(
            "シナリオの `now` の `{start}` は時刻（`2026-03-31T15:30:00Z` の形）ではありません",
            "the scenario's `now`, `{start}`, is not a time (`2026-03-31T15:30:00Z`)"
        )));
    };
    let mut ledgers = Vec::new();
    for bu in &m.books {
        let l = books.open(&bu.path).map_err(|said| Stopped::Run(format!("chobo cannot open the book {}: {}", bu.path.display(), said_en(&said))))?;
        ledgers.push(l);
    }
    let names = m.books.iter().map(|b| b.name.clone()).collect();
    let mut c = Computed { rules, dates, ledgers, names, date_facts: BTreeMap::new(), start_ms, elapsed_ms: 0, events: Vec::new() };
    let before = c.before(m, sc).map_err(Stopped::Scenario)?;
    let (trace, _) = crate::interp::run_answered(m, sc, view, &mut c).map_err(Stopped::Run)?;
    let answers: Vec<Value> = c.events.iter().filter_map(|e| if let Event::Answer { answer, .. } = e { Some(answer.clone()) } else { None }).collect();
    let mut replay = sc.clone();
    if let Some(o) = replay.as_object_mut() {
        o.insert("answers".into(), Value::Array(answers));
    }
    let ends = m
        .books
        .iter()
        .zip(&c.ledgers)
        .zip(before)
        .map(|((bu, l), before)| BookEnd { name: bu.name.clone(), file: bu.path.clone(), before, accounts: l.accounts(), holds: l.holds() })
        .collect();
    let end = moment_text(c.start_ms + c.elapsed_ms as i64);
    Ok(Ran { trace, replay, events: c.events, books: ends, start, elapsed: c.elapsed_ms as f64 / 1000.0, end })
}

impl Computed {
    /// What the scenario has done to each book before the run: its `books`, by the flow's name for
    /// the book, each a list of operations written as `chobo run`'s scenario writes one (`op`,
    /// `kind`, `args`, and `amounts` for a post of part of a hold). Each must be done, now or before.
    fn before(&mut self, m: &Model, sc: &Value) -> Result<Vec<Vec<(Value, String)>>, Text> {
        let mut out: Vec<Vec<(Value, String)>> = m.books.iter().map(|_| Vec::new()).collect();
        let Some(given) = sc.get("books").filter(|b| !b.is_null()) else { return Ok(out) };
        let Some(given) = given.as_object() else {
            return Err(ritsu_base::tr!("シナリオの `books` は、帳簿の名前ごとの操作の並びです", "the scenario's `books` is the operations done to each book, by the book's name"));
        };
        for (name, ops) in given {
            let Some(b) = m.books.iter().position(|bu| bu.name == *name) else {
                return Err(ritsu_base::tr!("シナリオの `books` の `{name}` を、フローは使いません", "the flow uses no book `{name}`, which the scenario's `books` names"));
            };
            let Some(ops) = ops.as_array() else {
                return Err(ritsu_base::tr!("シナリオの `books` の `{name}` は、操作の並びです", "`{name}` in the scenario's `books` is a list of operations"));
            };
            for (i, op) in ops.iter().enumerate() {
                let n = i + 1;
                let call = written_call(&m.books[b], op).map_err(|t| ritsu_base::tr!("帳簿 `{name}` の {n} 番目の操作: {}", "operation {n} of the book `{name}`: {}", t.ja; t.en))?;
                let result = match self.ledgers[b].apply(&call) {
                    Ok(BookOutcome::Done) => "done",
                    Ok(BookOutcome::DoneBefore) => "done_before",
                    Ok(BookOutcome::Refused(r)) => {
                        return Err(ritsu_base::tr!(
                            "帳簿 `{name}` は、走らせる前の {n} 番目の操作を `{r}` で断ります",
                            "the book `{name}` refuses operation {n}, before the run, with `{r}`"
                        ))
                    }
                    Err(t) => return Err(ritsu_base::tr!("帳簿 `{name}` の {n} 番目の操作: {}", "operation {n} of the book `{name}`: {}", t.ja; t.en)),
                };
                out[b].push((op.clone(), result.to_string()));
            }
        }
        Ok(out)
    }

    /// The moment now, on the run's clock.
    fn now_ms(&self) -> i64 {
        self.start_ms + self.elapsed_ms as i64
    }

    /// Let time go by, and tell the books: the holds that expire on the way are events after it.
    fn go_by(&mut self, seconds: f64, why: Passing, until: Option<String>) {
        let ms = (seconds * 1000.0).round().max(0.0) as u64;
        let before = self.elapsed_ms / 1000;
        self.elapsed_ms += ms;
        let after = self.elapsed_ms / 1000;
        self.events.push(Event::Pass { seconds, why, until, after: moment_text(self.now_ms()) });
        if after > before {
            for (l, name) in self.ledgers.iter_mut().zip(&self.names) {
                for (transfer, key) in l.pass(after - before) {
                    self.events.push(Event::Expired { book: name.clone(), transfer, key });
                }
            }
        }
    }

    /// A rule's outputs for the arguments, as rulec's evaluator computes them, refused as the
    /// generated code refuses an input at its door; read as the call reads the rule's answer (as its
    /// service writes it, for a rule called at its Connect service).
    fn rule(&mut self, m: &Model, r: usize, args: &Map<String, Value>) -> Result<Value, String> {
        let ru = &m.rules[r];
        let mut inputs = Vec::new();
        for c in &ru.info.inputs {
            let v = args.get(&c.name).unwrap_or(&Value::Null);
            match to_rule_value(&c.ty, v) {
                Some(x) => inputs.push((c.name.clone(), x)),
                None => return Ok(failure(&format!("the input `{}` of the rule {} is not a value of its type: {v}", c.name, ru.name))),
            }
        }
        match self.rules.eval(&ru.info.path, &inputs) {
            Ok(outs) => {
                let record = Value::Object(outs.iter().map(|(n, v)| (n.clone(), from_rule_value(v))).collect());
                let answer = match &ru.connect {
                    Some(c) => render::rule_wire(c, &record),
                    None => record,
                };
                Ok(json!({ "ok": answer }))
            }
            Err(RuleError::Input(t)) => Ok(failure(&t.en)),
            Err(RuleError::Contradiction(t)) => Err(format!("rulec gives the rule {} no answer: {}", ru.name, t.en)),
            Err(RuleError::Unread(said)) => Err(format!("rulec cannot read the rule {}: {}", ru.name, said_en(&said))),
        }
    }

    /// A date of a dates file for the arguments, as koyomi's interpreter computes it: the day, and
    /// for a date that says a time, that time of the day at the calendar's offset, in UTC.
    fn date(&mut self, m: &Model, r: usize, d: &DateCall, args: &Map<String, Value>) -> Result<Value, String> {
        let path = m.rules[r].info.path.clone();
        if !self.date_facts.contains_key(&path) {
            let f = self.dates.facts(&path).map_err(|said| format!("koyomi cannot read the dates file {}: {}", path.display(), said_en(&said)))?;
            self.date_facts.insert(path.clone(), f);
        }
        let facts = &self.date_facts[&path];
        let offset = d.offset.unwrap_or(0);
        let mut inputs = Vec::new();
        for (name, _, day) in &d.params {
            let v = args.get(name).unwrap_or(&Value::Null);
            let n = if *day { v.as_str().and_then(|s| day_given(s, offset)) } else { v.as_i64() };
            match n {
                Some(n) => inputs.push((name.clone(), n)),
                None => return Ok(failure(&format!("the input `{name}` of the date {} is not a value of its type: {v}", m.rules[r].name))),
            }
        }
        // koyomi computes every date of the file for one input: the inputs this date does not read
        // take the least value of their range, where every date of a file that passes koyomi's
        // check computes (koyomi's E202 to E204 say otherwise), so they change nothing it answers
        for i in &facts.inputs {
            if !inputs.iter().any(|(n, _)| *n == i.name) {
                inputs.push((i.name.clone(), i.min));
            }
        }
        let at = facts.functions.iter().find(|f| f.name == d.date).and_then(|f| f.at);
        match self.dates.eval(&path, &inputs) {
            Ok(values) => match values.iter().find(|(n, _)| *n == d.date) {
                Some((_, DateValue::Day(day))) => {
                    let mut o = Map::new();
                    o.insert("day".into(), json!(date_text(*day)));
                    if let (true, Some(mins)) = (d.at, at) {
                        o.insert("at".into(), json!(time_of_day(*day, mins, offset)));
                    }
                    Ok(json!({ "ok": Value::Object(o) }))
                }
                Some((_, DateValue::Stopped(t))) => Ok(failure(&t.en)),
                None => Err(format!("koyomi gives no date {} of {}", d.date, path.display())),
            },
            // an input outside the range the file declares, which koyomi's code refuses
            Err(said) => Ok(failure(&said_en(&said))),
        }
    }

    /// An operation of a book, as chobo's reference interpreter does it on the book the run keeps:
    /// done, now or before; or refused, with the reason, which comes back as the error the task
    /// declares by that name, else as a failure (`Dandori.Failure.<reason>`, as the code dandori
    /// writes names it; on Step Functions the book's Lambda function fails by the reason itself).
    fn book(&mut self, m: &Model, view: View, t: usize, b: &BookOp, args: &Map<String, Value>) -> Result<Value, String> {
        let task = &m.tasks[t];
        let (written, _) = render::book_call(m, task, b, args);
        let bu = &m.books[b.book];
        let call = written_call(bu, &json!({ "op": written["op"], "kind": written["transfer"], "args": written["args"], "amounts": written.get("amounts").cloned().unwrap_or(Value::Null) }))
            .map_err(|e| format!("the operation {}.{}.{} of task {} does not fit the book: {}", bu.name, b.transfer, b.op, task.name, e.en))?;
        match self.ledgers[b.book].apply(&call) {
            Ok(BookOutcome::Done) => Ok(json!({ "ok": { "result": "done" } })),
            Ok(BookOutcome::DoneBefore) => Ok(json!({ "ok": { "result": "done_before" } })),
            Ok(BookOutcome::Refused(reason)) => {
                let kind = if task.error(&reason).is_some() || view == View::Asl { reason.clone() } else { format!("Dandori.Failure.{reason}") };
                Ok(json!({ "error": kind, "cause": reason }))
            }
            Err(e) => Err(format!("chobo does not take the operation {}.{}.{} of task {}: {}", bu.name, b.transfer, b.op, task.name, e.en)),
        }
    }
}

impl Answers for Computed {
    fn answer(&mut self, m: &Model, view: View, callee: &Callee, args: &Map<String, Value>) -> Option<Result<Value, String>> {
        let (who, name, got) = match callee {
            Callee::Rule(r) => match &m.rules[*r].kind {
                RuleKind::Rule => (Who::Rulec, m.rules[*r].name.clone(), self.rule(m, *r, args)),
                RuleKind::Date(d) => (Who::Koyomi, m.rules[*r].name.clone(), self.date(m, *r, d, args)),
                // a hold's life is followed by a case, and nothing calls it
                RuleKind::Hold { .. } => return None,
            },
            Callee::Task(t) => match m.tasks[*t].book() {
                Some(b) => (Who::Chobo, m.tasks[*t].name.clone(), self.book(m, view, *t, b, args)),
                None => return None,
            },
        };
        if let Ok(answer) = &got {
            self.events.push(Event::Answer { who, call: name, args: args.clone(), answer: answer.clone() });
        }
        Some(got)
    }

    fn scripted(&mut self, m: &Model, callee: &Callee, args: &Map<String, Value>, answer: &Value) {
        let call = match callee {
            Callee::Task(t) => m.tasks[*t].name.clone(),
            Callee::Rule(r) => m.rules[*r].name.clone(),
        };
        self.events.push(Event::Answer { who: Who::Scenario, call, args: args.clone(), answer: answer.clone() });
    }

    fn pass(&mut self, seconds: f64, why: Passing) {
        self.go_by(seconds, why, None);
    }

    fn wait_until(&mut self, at: &Value) {
        let given = at.as_str().unwrap_or_default().to_string();
        // a value that is not a time does not come here: the check holds `wait until` to timestamps
        let Some(target) = moment_ms(&given) else { return };
        let ms = (target - self.now_ms()).max(0);
        self.go_by(ms as f64 / 1000.0, Passing::Wait, Some(given));
    }
}

/// A failure of a call, with its cause: what the language says of it.
fn failure(cause: &str) -> Value {
    json!({ "error": "failure", "cause": cause })
}

/// What a port says, in English, one thing after another.
fn said_en(said: &[ritsu_ports::Said]) -> String {
    said.iter().map(|s| s.message.en.clone()).collect::<Vec<_>>().join("; ")
}

/// An operation of a book written as `chobo run`'s scenario writes one, as chobo's port takes it:
/// an amount as its whole number, a string as itself.
fn written_call(bu: &BookUse, op: &Value) -> Result<BookCall, Text> {
    let Some(opname) = op.get("op").and_then(|v| v.as_str()) else {
        return Err(ritsu_base::tr!("`op`（do、hold、post、void）がありません", "it has no `op` (do, hold, post or void)"));
    };
    let Some(kind) = op.get("kind").and_then(|v| v.as_str()) else {
        return Err(ritsu_base::tr!("`kind`（振替の名前）がありません", "it has no `kind` (the transfer's name)"));
    };
    let Some(t) = bu.transfer(kind) else {
        return Err(ritsu_base::tr!("帳簿に振替 `{kind}` はありません", "the book has no transfer `{kind}`"));
    };
    let value = |name: &str, v: &Value| -> Result<Result<i128, String>, Text> {
        let Some(p) = t.params.iter().find(|p| p.name == name) else {
            return Err(ritsu_base::tr!("振替 `{kind}` に引数 `{name}` はありません", "the transfer `{kind}` takes no `{name}`"));
        };
        match (&p.unit, v) {
            (Some(_), Value::Number(n)) if n.as_i64().is_some() => Ok(Ok(n.as_i64().unwrap_or_default() as i128)),
            (Some(_), _) => Err(ritsu_base::tr!("`{name}` は額（整数）です", "`{name}` is an amount, a whole number")),
            (None, Value::String(s)) => Ok(Err(s.clone())),
            (None, _) => Err(ritsu_base::tr!("`{name}` は文字列です", "`{name}` is a string")),
        }
    };
    let mut args = Vec::new();
    for (name, v) in op.get("args").and_then(|a| a.as_object()).into_iter().flatten() {
        args.push((name.clone(), value(name, v)?));
    }
    let amounts = match op.get("amounts") {
        None | Some(Value::Null) => None,
        Some(Value::Object(a)) => {
            let mut out = Vec::new();
            for (name, v) in a {
                match value(name, v)? {
                    Ok(n) => out.push((name.clone(), n)),
                    Err(_) => return Err(ritsu_base::tr!("`amounts` の `{name}` は額です", "`{name}` in `amounts` is an amount")),
                }
            }
            Some(out)
        }
        Some(_) => return Err(ritsu_base::tr!("`amounts` は、額の名前ごとのオブジェクトです", "`amounts` is an object of amounts by name")),
    };
    Ok(BookCall { transfer: t.name.clone(), op: opname.to_string(), args, amounts })
}

/// A value dandori hands a rule, as rulec's evaluator takes it: None for one not of the input's type.
fn to_rule_value(ty: &RType, v: &Value) -> Option<ritsu_ports::Value> {
    use ritsu_ports::Value as P;
    match (ty, v) {
        (RType::Bool, Value::Bool(b)) => Some(P::Bool(*b)),
        (RType::Str, Value::String(s)) => Some(P::Str(s.clone())),
        (RType::Date, Value::String(s)) if render::is_date(s) => Some(P::Date(s.clone())),
        (RType::Enum(_), Value::String(s)) => Some(P::Enum(s.clone())),
        (RType::Num { .. }, Value::Number(n)) => n.as_i64().map(|n| P::Int(n as i128)).or_else(|| n.as_f64().filter(|f| f.fract() == 0.0).map(|f| P::Int(f as i128))),
        _ => None,
    }
}

/// A value rulec answers, as dandori reads it: a number as the integer on the wire, a day as
/// `YYYY-MM-DD`, an enum's value by the rule's name of it.
fn from_rule_value(v: &ritsu_ports::Value) -> Value {
    use ritsu_ports::Value as P;
    match v {
        P::Bool(b) => json!(b),
        P::Int(n) => i64::try_from(*n).map(|n| json!(n)).unwrap_or_else(|_| json!(*n as f64)),
        P::Str(s) | P::Date(s) | P::Enum(s) => json!(s),
        P::List(rows) => Value::Array(rows.iter().map(|row| Value::Object(row.iter().map(|(k, x)| (k.clone(), from_rule_value(x))).collect())).collect()),
        P::None => Value::Null,
    }
}

// ── days and moments ───────────────────────────────────────────────────────

/// The days since 1970-01-01 of a day of the proleptic Gregorian calendar (the day count koyomi
/// and rulec keep).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// The day of a day count, as (year, month, day).
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { yoe + era * 400 + 1 } else { yoe + era * 400 }, m, d)
}

/// `2026-05-08` as its day count.
fn day_of(s: &str) -> Option<i64> {
    if !render::is_date(s) {
        return None;
    }
    let (y, m, d) = (s[0..4].parse().ok()?, s[5..7].parse().ok()?, s[8..10].parse().ok()?);
    if !(1..=12).contains(&m) || d < 1 || d > 31 {
        return None;
    }
    let n = days_from_civil(y, m, d);
    // a day the month does not have (`2026-02-30`) comes back as another
    (civil_from_days(n) == (y, m, d)).then_some(n)
}

/// A day count as `YYYY-MM-DD`.
fn date_text(n: i64) -> String {
    let (y, m, d) = civil_from_days(n);
    format!("{y:04}-{m:02}-{d:02}")
}

/// A day handed a date's input: `YYYY-MM-DD` as it is, or a time as the day it falls on at the
/// calendar's offset (minutes east of UTC), as the code dandori writes reads it (`day` in
/// `rules.ts`).
fn day_given(s: &str, offset: i32) -> Option<i64> {
    if s.len() <= 10 {
        return day_of(s);
    }
    let ms = moment_ms(s)? + offset as i64 * 60_000;
    Some(ms.div_euclid(86_400_000))
}

/// The time a date gives: the day at `minutes` after midnight (1440 for the end of the day) at
/// the calendar's offset, in UTC, as koyomi's code writes it (`2026-05-08T09:00:00Z`).
fn time_of_day(day: i64, minutes: u32, offset: i32) -> String {
    let utc = day * 1440 + minutes as i64 - offset as i64;
    moment_text(utc * 60_000)
}

/// `2026-03-31T15:30:00Z`, or with a fraction of a second, as milliseconds since 1970-01-01 in UTC.
pub fn moment_ms(s: &str) -> Option<i64> {
    if !render::is_timestamp(s) {
        return None;
    }
    let day = day_of(&s[0..10])?;
    let (h, mi, se): (i64, i64, i64) = (s[11..13].parse().ok()?, s[14..16].parse().ok()?, s[17..19].parse().ok()?);
    if h > 23 || mi > 59 || se > 60 {
        return None;
    }
    let frac = s[19..].trim_end_matches('Z').trim_start_matches('.');
    let ms = if frac.is_empty() { 0 } else { format!("{frac:0<3}")[..3].parse::<i64>().ok()? };
    Some(day * 86_400_000 + ((h * 60 + mi) * 60 + se) * 1000 + ms)
}

/// Milliseconds since 1970-01-01 as a time in UTC: to the second, with the milliseconds when there
/// are any.
pub fn moment_text(ms: i64) -> String {
    let day = ms.div_euclid(86_400_000);
    let rest = ms.rem_euclid(86_400_000);
    let (h, mi, se, milli) = (rest / 3_600_000, rest / 60_000 % 60, rest / 1000 % 60, rest % 1000);
    let frac = if milli == 0 { String::new() } else { format!(".{milli:03}") };
    format!("{}T{h:02}:{mi:02}:{se:02}{frac}Z", date_text(day))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_and_moments_read_back() {
        assert_eq!(day_of("1970-01-01"), Some(0));
        assert_eq!(day_of("2026-03-31"), Some(20543));
        assert_eq!(date_text(20543), "2026-03-31");
        assert_eq!(day_of("2026-02-29"), None);
        assert_eq!(day_of("2028-02-29").map(date_text).as_deref(), Some("2028-02-29"));
        assert_eq!(moment_ms("2026-03-31T15:30:00Z"), Some(20543 * 86_400_000 + 55_800_000));
        assert_eq!(moment_text(20543 * 86_400_000 + 55_800_000), "2026-03-31T15:30:00Z");
        assert_eq!(moment_text(moment_ms("2026-03-31T15:30:00.5Z").unwrap()), "2026-03-31T15:30:00.500Z");
        // half past midnight in Tokyo is the next day there, and the same day in UTC
        assert_eq!(day_given("2026-03-31T15:30:00Z", 540).map(date_text).as_deref(), Some("2026-04-01"));
        assert_eq!(day_given("2026-03-31T15:30:00Z", 0).map(date_text).as_deref(), Some("2026-03-31"));
        assert_eq!(day_given("2026-03-31", 540).map(date_text).as_deref(), Some("2026-03-31"));
        // 09:00 in Tokyo is midnight in UTC; the end of a day is the next one's midnight
        assert_eq!(time_of_day(day_of("2026-05-08").unwrap(), 540, 540), "2026-05-08T00:00:00Z");
        assert_eq!(time_of_day(day_of("2026-05-08").unwrap(), 1440, 0), "2026-05-09T00:00:00Z");
    }
}
