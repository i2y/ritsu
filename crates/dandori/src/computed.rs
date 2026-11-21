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
//! Time: the run's clock (crate::interp::Clock) starts at the scenario's `now`, and goes on by each
//! `wait` (to the moment of a `wait until`), each wait before a retry, and the timeout of each try
//! that timed out. A try that is answered takes no time: the scenario does not say how long it
//! took, and none is the least it may have. `now` reads the clock when the statement that reads it
//! runs, and the books are told the time as it goes by.
//!
//! What a scenario says is computed: `"rules": "rulec"`, `"dates": "koyomi"`, and its `books` (what
//! was done to each book before the run), which `ritsu dandori scenarios` writes ([`Takes`]).
//! `ritsu run` computes every call of the three; `dandori run` computes those the scenario says.

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
    /// operation as it was written, with what it came to (`done`, `done_before`, and `passed` for
    /// time that went by)
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
    /// the scenario with every answer in it, in order, and nothing left to compute (no `rules`,
    /// `dates` or `books`): `dandori run` on it makes this run again
    pub replay: Value,
    pub events: Vec<Event>,
    pub books: Vec<BookEnd>,
    /// the moment the run starts at, how long its time went on, and the moment it ends at
    pub start: String,
    pub elapsed: f64,
    pub end: String,
    /// the answers of the scenario the run did not take: an error of the scenario's, which
    /// `ritsu run` and `dandori run` say once the run is shown (DESIGN 36)
    pub unused: Option<crate::interp::LeftOver>,
    /// everything the run went through, in order: what `ritsu dandori scenarios` holds a
    /// scenario's `covers` to, and what the page of the flow draws (DESIGN 36)
    pub visits: Vec<crate::interp::Visit>,
}

impl Ran {
    /// Whether the run computed a call of a rule, a date or a book.
    pub fn computes(&self) -> bool {
        self.events.iter().any(|e| matches!(e, Event::Answer { who, .. } if *who != Who::Scenario))
    }
}

/// Why a run did not come to an end.
#[derive(Clone, Debug, PartialEq)]
pub enum Stopped {
    /// The scenario cannot be run as it is written (a book it names that the flow does not use, an
    /// operation the book refuses before the run, a `now` that is not a time, an answer of its own
    /// for a call `ritsu run` computes).
    Scenario(Text),
    /// The run cannot go on: the scenario has no answer for a call, or a language cannot answer one.
    Run(Text),
}

/// Which calls a run computes in place of the scenario's answers: the calls of local rules
/// (rulec), of dates (koyomi), and the operations of books (chobo).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Takes {
    pub rules: bool,
    pub dates: bool,
    pub books: bool,
}

impl Takes {
    /// every call of the three: what `ritsu run` computes
    pub const ALL: Takes = Takes { rules: true, dates: true, books: true };

    /// What the scenario says is computed: `"rules": "rulec"`, `"dates": "koyomi"`, and `books`, the
    /// operations done to each book before the run.
    pub fn said(sc: &Value) -> Takes {
        Takes {
            rules: sc.get("rules").and_then(|v| v.as_str()) == Some(RULES_COMPUTED),
            dates: sc.get("dates").and_then(|v| v.as_str()) == Some(DATES_COMPUTED),
            books: sc.get("books").is_some_and(|b| b.is_object()),
        }
    }

    pub fn any(self) -> bool {
        self.rules || self.dates || self.books
    }

    /// The kinds of call a scenario a generator wrote (it says what it `covers`) answers itself,
    /// for it does not say they are computed: `ritsu dandori scenarios` wrote such scenarios before
    /// it left the rules to rulec, and the dates and the books to koyomi and chobo. None for a
    /// scenario written by hand.
    pub fn answered(sc: &Value) -> Takes {
        if sc.get("covers").is_none() {
            return Takes::default();
        }
        let said = Takes::said(sc);
        Takes { rules: !said.rules, dates: !said.dates, books: !said.books }
    }
}

/// The value of a scenario's `rules` that says the calls of rules are not in its `answers`:
/// rulec computes them. `ritsu dandori scenarios` writes it.
pub const RULES_COMPUTED: &str = "rulec";

/// The value of a scenario's `dates` that says the calls of dates are not in its `answers`: koyomi
/// computes them. `ritsu dandori scenarios` writes it.
pub const DATES_COMPUTED: &str = "koyomi";

/// The languages a run asks, and what it keeps between the calls: the books, and its time.
pub struct Computed {
    rules: Rc<dyn Rules>,
    dates: Rc<dyn Dates>,
    /// the calls this side answers
    takes: Takes,
    /// the kinds of call the scenario answers itself, which a run that computes them stops at
    /// (`Takes::answered`): `ritsu run`'s
    answered: Takes,
    /// why the run stopped where the scenario cannot be run: the scenario's to say
    said: Option<Text>,
    /// each book of the flow's (by its place in the model), opened at the start, when this side
    /// does the operations of books
    ledgers: Vec<Box<dyn Ledger>>,
    names: Vec<String>,
    /// what was done to each book before the run, as written, with what it came to
    before: Vec<Vec<(Value, String)>>,
    /// what koyomi knows of each dates file, asked once
    date_facts: BTreeMap<PathBuf, DateFacts>,
    /// the moment the run starts at and the moment its clock shows, in milliseconds since
    /// 1970-01-01 in UTC
    start_ms: i64,
    at_ms: i64,
    events: Vec<Event>,
}

/// Run the flow on the scenario, as `ritsu run` runs it: a rule's outputs computed by `rules`
/// (rulec), a date by `dates` (koyomi), and an operation of a book by `books` (chobo), on books that
/// start as the scenario's `books` leaves them; every other call is answered by the scenario. A
/// scenario a generator wrote that answers a kind of these calls itself stops at the first.
pub fn run(m: &Model, sc: &Value, view: View, rules: Rc<dyn Rules>, dates: Rc<dyn Dates>, books: Rc<dyn Books>) -> Result<Ran, Stopped> {
    run_taking(m, sc, view, rules, dates, books, Takes::ALL, Takes::answered(sc))
}

/// Run the flow on the scenario, as `dandori run` runs it: the calls the scenario says are computed
/// (`Takes::said`) computed by the ports, and every other call answered by the scenario. A scenario
/// that says all three is run as `ritsu run` runs it.
pub fn run_said(m: &Model, sc: &Value, view: View, rules: Rc<dyn Rules>, dates: Rc<dyn Dates>, books: Rc<dyn Books>) -> Result<Ran, Stopped> {
    run_taking(m, sc, view, rules, dates, books, Takes::said(sc), Takes::default())
}

/// The ports `dandori run` runs `sc` through (`run_said`), as the program that runs dandori handed
/// them over: all three (`ritsu dandori run`), or those it has where the scenario says some of its
/// calls are computed (a program that joins rulec only, whose `dandori scenarios` writes `"rules":
/// "rulec"`), a port it does not have answering with what to run instead. None where the
/// scenario's answers answer every call: the reference interpreter runs it (`interp::run_moving`).
pub fn said_ports(sc: &Value) -> Option<(Rc<dyn Rules>, Rc<dyn Dates>, Rc<dyn Books>)> {
    let held = crate::sources::held()?;
    (held.ports().is_some() || Takes::said(sc).any()).then(|| held.or_unread())
}

#[allow(clippy::too_many_arguments)]
fn run_taking(m: &Model, sc: &Value, view: View, rules: Rc<dyn Rules>, dates: Rc<dyn Dates>, books: Rc<dyn Books>, takes: Takes, answered: Takes) -> Result<Ran, Stopped> {
    let clock = crate::interp::Clock::of(sc, true).map_err(Stopped::Scenario)?;
    let start = clock.start().to_string();
    let mut c = Computed::new(m, sc, rules, dates, books, takes, answered)?;
    let crate::interp::Answered { trace, visits, unused, .. } = match crate::interp::run_answered(m, sc, view, &mut c) {
        Ok(ran) => ran,
        // a scenario that cannot be run stops where it is first read wrong, and it is the scenario
        // that is said
        Err(e) => return Err(c.said.take().map_or(Stopped::Run(e), Stopped::Scenario)),
    };
    let answers: Vec<Value> = c.events.iter().filter_map(|e| if let Event::Answer { answer, .. } = e { Some(answer.clone()) } else { None }).collect();
    let mut replay = sc.clone();
    if let Some(o) = replay.as_object_mut() {
        o.insert("answers".into(), Value::Array(answers));
        // every answer is in it: nothing is left to compute
        for k in ["rules", "dates", "books"] {
            o.remove(k);
        }
    }
    let ends = c.ends(m);
    let end = moment_text(c.at_ms);
    Ok(Ran { trace, replay, events: c.events, books: ends, start, elapsed: (c.at_ms - c.start_ms) as f64 / 1000.0, end, unused, visits })
}

impl Computed {
    /// The languages a run of the scenario asks, the calls of `takes` answered by them: the books
    /// opened and done to as the scenario's `books` says (when it does the books), and the clock at
    /// the scenario's start.
    pub fn new(m: &Model, sc: &Value, rules: Rc<dyn Rules>, dates: Rc<dyn Dates>, books: Rc<dyn Books>, takes: Takes, answered: Takes) -> Result<Computed, Stopped> {
        let clock = crate::interp::Clock::of(sc, true).map_err(Stopped::Scenario)?;
        let start_ms = clock.at_ms();
        let mut ledgers = Vec::new();
        if takes.books {
            for bu in &m.books {
                let l = books.open(&bu.path).map_err(|said| {
                    let why = said_text(&said);
                    Stopped::Run(ritsu_base::tr!("chobo が帳簿 {} を開けません: {}", "chobo cannot open the book {}: {}", bu.path.display(), why.ja; bu.path.display(), why.en))
                })?;
                ledgers.push(l);
            }
        }
        let names = m.books.iter().map(|b| b.name.clone()).collect();
        let before = m.books.iter().map(|_| Vec::new()).collect();
        let mut c = Computed { rules, dates, takes, answered, said: None, ledgers, names, before, date_facts: BTreeMap::new(), start_ms, at_ms: start_ms, events: Vec::new() };
        if takes.books {
            c.done_before_run(m, sc).map_err(Stopped::Scenario)?;
        }
        Ok(c)
    }

    /// What the scenario has done to each book before the run: its `books`, by the flow's name for
    /// the book, each a list of operations written as `chobo run`'s scenario writes one (`op`,
    /// `kind`, `args`, and `amounts` for a post of part of a hold), and of the time that went by
    /// between them (`pass`). Each operation must be done, now or before.
    fn done_before_run(&mut self, m: &Model, sc: &Value) -> Result<(), Text> {
        let Some(given) = sc.get("books").filter(|b| !b.is_null()) else { return Ok(()) };
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
            for op in ops {
                self.do_before(m, b, op)?;
            }
        }
        Ok(())
    }

    /// Do one operation to the book `b` before the run, as the scenario's `books` writes one: done,
    /// now or before, or said. A `pass` is time that went by before the run, as `chobo run`'s
    /// scenario writes it (`{"op": "pass", "duration": "7 days"}`): a hold whose expiry it reaches
    /// has expired when the run starts.
    pub fn do_before(&mut self, m: &Model, b: usize, op: &Value) -> Result<(), Text> {
        let name = &m.books[b].name;
        let n = self.before[b].len() + 1;
        match op.get("op").and_then(|v| v.as_str()) {
            Some("pass") => {
                let Some(secs) = op.get("duration").and_then(|d| d.as_str()).and_then(duration_secs) else {
                    return Err(ritsu_base::tr!(
                        "帳簿 `{name}` の {n} 番目の操作: `pass` には `\"duration\": \"7 days\"` のような長さが要ります（単位は seconds、minutes、hours、days）",
                        "operation {n} of the book `{name}`: a `pass` needs a duration like `\"duration\": \"7 days\"` (in seconds, minutes, hours or days)"
                    ));
                };
                self.ledgers[b].pass(secs);
                self.before[b].push((op.clone(), "passed".to_string()));
                return Ok(());
            }
            Some("together") => {
                return Err(ritsu_base::tr!(
                    "帳簿 `{name}` の {n} 番目の操作: 走らせる前の操作には `together` を書けません。操作を一つずつ順に書いてください",
                    "operation {n} of the book `{name}`: what was done before the run is written one operation after another, without `together`"
                ))
            }
            _ => {}
        }
        let call = written_call(&m.books[b], op).map_err(|t| ritsu_base::tr!("帳簿 `{name}` の {n} 番目の操作: {}", "operation {n} of the book `{name}`: {}", t.ja; t.en))?;
        let result = match self.ledgers[b].apply(&call) {
            Ok(BookOutcome::Done) => "done",
            Ok(BookOutcome::DoneBefore) => "done_before",
            Ok(BookOutcome::Refused(r)) => {
                return Err(ritsu_base::tr!(
                    "帳簿 `{name}` は、走らせる前の {n} 番目の操作を `{r}` で拒否します",
                    "the book `{name}` refuses operation {n}, before the run, with `{r}`"
                ))
            }
            Err(t) => return Err(ritsu_base::tr!("帳簿 `{name}` の {n} 番目の操作: {}", "operation {n} of the book `{name}`: {}", t.ja; t.en)),
        };
        self.before[b].push((op.clone(), result.to_string()));
        Ok(())
    }

    /// Start the run's time at another moment, in milliseconds since 1970-01-01 in UTC, before
    /// anything is done in it (a generator that chooses when its run starts).
    pub fn start_at(&mut self, ms: i64) {
        self.start_ms = ms;
        self.at_ms = ms;
    }

    /// The book `b` as the run has left it so far (when this side does the books).
    pub fn ledger(&self, b: usize) -> Option<&dyn Ledger> {
        self.ledgers.get(b).map(|l| &**l)
    }

    /// What happened so far, in order.
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// The books as the run leaves them (when this side does the books).
    fn ends(&self, m: &Model) -> Vec<BookEnd> {
        m.books
            .iter()
            .zip(&self.ledgers)
            .zip(&self.before)
            .map(|((bu, l), before)| BookEnd { name: bu.name.clone(), file: bu.path.clone(), before: before.clone(), accounts: l.accounts(), holds: l.holds() })
            .collect()
    }

    /// The run stops at a call the scenario answers itself, which this side computes.
    fn answered_here(&mut self, why: Text) -> Result<Value, Text> {
        self.said = Some(why.clone());
        Err(why)
    }

    /// A rule's outputs for the arguments, as rulec's evaluator computes them (`rule_answer`). A
    /// scenario `ritsu dandori scenarios` wrote before it left the rules to rulec answers the
    /// calls of rules itself, and a run that read it would go on one answer off from here: it stops
    /// at the first call of a rule instead, and says so.
    fn rule(&mut self, m: &Model, r: usize, args: &Map<String, Value>) -> Result<Value, Text> {
        if self.answered.rules {
            return self.answered_here(answered_rule(&m.rules[r].name));
        }
        rule_answer(&*self.rules, m, r, args)
    }

    /// What koyomi knows of the dates file of the date `r` is kept, asked once; its path.
    fn ask_facts(&mut self, m: &Model, r: usize) -> Result<PathBuf, Text> {
        let path = m.rules[r].info.path.clone();
        if !self.date_facts.contains_key(&path) {
            let f = self.dates.facts(&path).map_err(|said| {
                let why = said_text(&said);
                ritsu_base::tr!("koyomi が日付のファイル {} を読めません: {}", "koyomi cannot read the dates file {}: {}", path.display(), why.ja; path.display(), why.en)
            })?;
            self.date_facts.insert(path.clone(), f);
        }
        Ok(path)
    }

    /// The range each input of the date `r` takes, by the input's name: days since 1970-01-01 for
    /// a day, else whole numbers, both ends in. None when koyomi cannot read the file.
    pub fn date_ranges(&mut self, m: &Model, r: usize) -> Option<Vec<(String, i64, i64)>> {
        let path = self.ask_facts(m, r).ok()?;
        Some(self.date_facts[&path].inputs.iter().map(|i| (i.name.clone(), i.min, i.max)).collect())
    }

    /// A date of a dates file for the arguments, as koyomi's interpreter computes it: the day, and
    /// for a date that says a time, that time of the day at the calendar's offset, in UTC. A
    /// scenario of the old form, which answers the calls of dates itself, stops here.
    fn date(&mut self, m: &Model, r: usize, d: &DateCall, args: &Map<String, Value>) -> Result<Value, Text> {
        if self.answered.dates {
            return self.answered_here(answered_date(&m.rules[r].name));
        }
        let path = self.ask_facts(m, r)?;
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
                None => Err(ritsu_base::tr!("koyomi が {} の日付 {} を返しません", "koyomi gives no date {} of {}", path.display(), d.date; d.date, path.display())),
            },
            // an input outside the range the file declares, which koyomi's code refuses
            Err(said) => Ok(failure(&said_en(&said))),
        }
    }

    /// An operation of a book, as chobo's reference interpreter does it on the book the run keeps:
    /// done, now or before; or refused, with the reason, which comes back as the error the task
    /// declares by that name, else as a failure (`Dandori.Failure.<reason>`, as the code dandori
    /// writes names it; on Step Functions the book's Lambda function fails by the reason itself). A
    /// scenario of the old form, which answers the operations of books itself, stops here.
    fn book(&mut self, m: &Model, view: View, t: usize, b: &BookOp, args: &Map<String, Value>) -> Result<Value, Text> {
        let task = &m.tasks[t];
        let bu = &m.books[b.book];
        if self.answered.books {
            return self.answered_here(answered_book(&task.name, &bu.name));
        }
        let call = book_call_of(m, task, b, args).map_err(|e| {
            ritsu_base::tr!(
                "タスク {} の操作 {}.{}.{} が帳簿に合いません: {}",
                "the operation {}.{}.{} of task {} does not fit the book: {}",
                task.name, bu.name, b.transfer, b.op, e.ja;
                bu.name, b.transfer, b.op, task.name, e.en
            )
        })?;
        match self.ledgers[b.book].apply(&call) {
            Ok(BookOutcome::Done) => Ok(json!({ "ok": { "result": "done" } })),
            Ok(BookOutcome::DoneBefore) => Ok(json!({ "ok": { "result": "done_before" } })),
            Ok(BookOutcome::Refused(reason)) => {
                let kind = if task.error(&reason).is_some() || view == View::Asl { reason.clone() } else { format!("Dandori.Failure.{reason}") };
                Ok(json!({ "error": kind, "cause": reason }))
            }
            Err(e) => Err(ritsu_base::tr!(
                "chobo がタスク {} の操作 {}.{}.{} を受け付けません: {}",
                "chobo does not take the operation {}.{}.{} of task {}: {}",
                task.name, bu.name, b.transfer, b.op, e.ja;
                bu.name, b.transfer, b.op, task.name, e.en
            )),
        }
    }
}

impl Answers for Computed {
    fn answer(&mut self, m: &Model, view: View, callee: &Callee, args: &Map<String, Value>) -> Option<Result<Value, Text>> {
        let (who, name, got) = match callee {
            Callee::Rule(r) => match &m.rules[*r].kind {
                RuleKind::Rule if self.takes.rules => (Who::Rulec, m.rules[*r].name.clone(), self.rule(m, *r, args)),
                RuleKind::Date(d) if self.takes.dates => (Who::Koyomi, m.rules[*r].name.clone(), self.date(m, *r, d, args)),
                // a hold's life is followed by a case, and nothing calls it
                _ => return None,
            },
            Callee::Task(t) => match m.tasks[*t].book() {
                Some(b) if self.takes.books => (Who::Chobo, m.tasks[*t].name.clone(), self.book(m, view, *t, b, args)),
                _ => return None,
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

    /// Time went by: the books are told, and the holds that expire on the way are events after it.
    fn pass(&mut self, seconds: f64, why: Passing, until: Option<&str>, after_ms: i64) {
        let before = (self.at_ms - self.start_ms).div_euclid(1000);
        self.at_ms = after_ms;
        let after = (self.at_ms - self.start_ms).div_euclid(1000);
        self.events.push(Event::Pass { seconds, why, until: until.map(str::to_string), after: moment_text(self.at_ms) });
        if after > before {
            for (l, name) in self.ledgers.iter_mut().zip(&self.names) {
                for (transfer, key) in l.pass((after - before) as u64) {
                    self.events.push(Event::Expired { book: name.clone(), transfer, key });
                }
            }
        }
    }

    fn cannot_run(&mut self, why: &Text) {
        self.said = Some(why.clone());
    }
}

/// The operation a task of a book does with these arguments, as chobo's port takes it: the
/// arguments as the code dandori writes hands them to chobo's client (`render::book_call`).
pub fn book_call_of(m: &Model, task: &TaskDef, b: &BookOp, args: &Map<String, Value>) -> Result<BookCall, Text> {
    let (written, _) = render::book_call(m, task, b, args);
    written_call(&m.books[b.book], &json!({ "op": written["op"], "kind": written["transfer"], "args": written["args"], "amounts": written.get("amounts").cloned().unwrap_or(Value::Null) }))
}

/// What a call of a rule comes to, as `ritsu run` answers it and `ritsu dandori scenarios` writes
/// the runs it chooses with (§15.209 of rulec's DESIGN, dandori's DESIGN): rulec's outputs for the
/// arguments, as the call reads the rule's answer — as its service writes it, for a rule called at
/// its Connect service — and a failure where the generated code refuses an input at its door. Err
/// where rulec gives no answer at all, with why in both languages.
pub fn rule_answer(rules: &dyn Rules, m: &Model, r: usize, args: &Map<String, Value>) -> Result<Value, Text> {
    Ok(match rule_outputs(rules, m, r, args)? {
        Ok(record) => match &m.rules[r].connect {
            Some(c) => json!({ "ok": render::rule_wire(c, &record) }),
            None => json!({ "ok": record }),
        },
        Err(cause) => failure(&cause),
    })
}

/// [`rule_answer`] before it is written as an answer: the record of the rule's outputs as the flow
/// reads them, or why the generated code refuses the call at its door.
pub fn rule_outputs(rules: &dyn Rules, m: &Model, r: usize, args: &Map<String, Value>) -> Result<Result<Value, String>, Text> {
    let ru = &m.rules[r];
    let mut inputs = Vec::new();
    for c in &ru.info.inputs {
        let v = args.get(&c.name).unwrap_or(&Value::Null);
        match to_rule_value(&c.ty, v) {
            Some(x) => inputs.push((c.name.clone(), x)),
            None => return Ok(Err(format!("the input `{}` of the rule {} is not a value of its type: {v}", c.name, ru.name))),
        }
    }
    match rules.eval(&ru.info.path, &inputs) {
        Ok(outs) => Ok(Ok(Value::Object(outs.iter().map(|(n, v)| (n.clone(), from_rule_value(v))).collect()))),
        Err(RuleError::Input(t)) => Ok(Err(t.en)),
        Err(RuleError::Contradiction(t)) => Err(ritsu_base::tr!("rulec が規則 {} の結果を返しません: {}", "rulec gives the rule {} no answer: {}", ru.name, t.ja; ru.name, t.en)),
        Err(RuleError::Unread(said)) => {
            let why = said_text(&said);
            Err(ritsu_base::tr!("rulec が規則 {} を読めません: {}", "rulec cannot read the rule {}: {}", ru.name, why.ja; ru.name, why.en))
        }
    }
}

/// The inputs of the vectors rulec makes of the rule `r` (`Rules::vector_inputs`), each as a call of
/// the flow gives them, as `rule_outputs` takes them: a number as the integer on the wire, a day as
/// `YYYY-MM-DD`, an enum's value by the rule's name of it. None where rulec makes none.
pub fn rule_vectors(rules: &dyn Rules, m: &Model, r: usize) -> Vec<Map<String, Value>> {
    rules.vector_inputs(&m.rules[r].info.path).unwrap_or_default().iter().map(|v| v.iter().map(|(n, x)| (n.clone(), from_rule_value(x))).collect()).collect()
}

/// What a run says when it stops at a call of a rule that the scenario answers itself.
fn answered_rule(rule: &str) -> Text {
    ritsu_base::tr!(
        "規則 {rule} の呼び出しの結果を、シナリオが自分で持っています（`dandori run` が読む形で、`ritsu dandori scenarios` が規則を rulec に任せる前に書いたものです）。`ritsu run` は規則を rulec で計算するので、このまま進むと、それより後の結果が一つずつずれます。`ritsu dandori scenarios` でシナリオを書き直すか、このシナリオは `ritsu dandori run` で流してください",
        "it answers the call of the rule {rule} itself, as `dandori run` reads a scenario: `ritsu dandori scenarios` wrote it before it left the rules to rulec. `ritsu run` computes the rule with rulec, so every answer after it would be read one call off. Write the scenarios again with `ritsu dandori scenarios`, or run this one with `ritsu dandori run`"
    )
}

/// What a run says when it stops at a call of a date that the scenario answers itself.
fn answered_date(date: &str) -> Text {
    ritsu_base::tr!(
        "日付 {date} の呼び出しの結果を、シナリオが自分で持っています（`dandori run` が読む形で、`ritsu dandori scenarios` が日付を koyomi に任せる前に書いたものです。シナリオに `\"dates\": \"koyomi\"` がありません）。`ritsu run` は日付を koyomi で計算するので、このまま進むと、それより後の結果が一つずつずれます。`ritsu dandori scenarios` でシナリオを書き直すか、このシナリオは `ritsu dandori run` で流してください",
        "it answers the call of the date {date} itself, as `dandori run` reads a scenario: `ritsu dandori scenarios` wrote it before it left the dates to koyomi (it has no `\"dates\": \"koyomi\"`). `ritsu run` computes the date with koyomi, so every answer after it would be read one call off. Write the scenarios again with `ritsu dandori scenarios`, or run this one with `ritsu dandori run`"
    )
}

/// What a run says when it stops at an operation of a book that the scenario answers itself.
fn answered_book(task: &str, book: &str) -> Text {
    ritsu_base::tr!(
        "帳簿 {book} の操作（タスク {task}）の結果を、シナリオが自分で持っています（`dandori run` が読む形で、`ritsu dandori scenarios` が帳簿を chobo に任せる前に書いたものです。シナリオに `books` がありません）。`ritsu run` は帳簿の操作を chobo でするので、このまま進むと、それより後の結果が一つずつずれます。`ritsu dandori scenarios` でシナリオを書き直すか、このシナリオは `ritsu dandori run` で流してください",
        "it answers the operation of the book {book} (the task {task}) itself, as `dandori run` reads a scenario: `ritsu dandori scenarios` wrote it before it left the books to chobo (it has no `books`). `ritsu run` does the operation with chobo, so every answer after it would be read one call off. Write the scenarios again with `ritsu dandori scenarios`, or run this one with `ritsu dandori run`"
    )
}

/// A failure of a call, with its cause: what the language says of it.
fn failure(cause: &str) -> Value {
    json!({ "error": "failure", "cause": cause })
}

/// What a port says, in English, one thing after another: the cause of a failure the run's trace
/// writes, as a platform's run writes one.
fn said_en(said: &[ritsu_ports::Said]) -> String {
    said.iter().map(|s| s.message.en.clone()).collect::<Vec<_>>().join("; ")
}

/// What a port says, one thing after another, in both languages: why a run stops.
fn said_text(said: &[ritsu_ports::Said]) -> Text {
    Text::join(&said.iter().map(|s| s.message.clone()).collect::<Vec<_>>(), "。", "; ")
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
        (RType::Enum { .. }, Value::String(s)) => Some(P::Enum(s.clone())),
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
pub(crate) fn day_of(s: &str) -> Option<i64> {
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
pub(crate) fn time_of_day(day: i64, minutes: u32, offset: i32) -> String {
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

/// A duration as `chobo run`'s scenario writes one (`31 minutes`, `7 days`), in seconds.
pub fn duration_secs(s: &str) -> Option<u64> {
    let (n, unit) = s.trim().split_once(' ')?;
    let per = match unit.trim() {
        "second" | "seconds" => 1,
        "minute" | "minutes" => 60,
        "hour" | "hours" => 3600,
        "day" | "days" => 86_400,
        _ => return None,
    };
    n.parse::<u64>().ok()?.checked_mul(per)
}

/// Seconds as `chobo run`'s scenario writes a duration: in the largest unit that divides them.
pub fn duration_text(secs: u64) -> String {
    for (per, one, many) in [(86_400, "day", "days"), (3600, "hour", "hours"), (60, "minute", "minutes")] {
        if secs > 0 && secs % per == 0 {
            let n = secs / per;
            return format!("{n} {}", if n == 1 { one } else { many });
        }
    }
    format!("{secs} {}", if secs == 1 { "second" } else { "seconds" })
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
    fn durations_read_and_write_as_chobo_writes_them() {
        assert_eq!(duration_secs("30 days"), Some(2_592_000));
        assert_eq!(duration_secs("1 second"), Some(1));
        assert_eq!(duration_secs(" 90 minutes "), Some(5400));
        assert_eq!(duration_secs("a month"), None);
        assert_eq!(duration_secs("3 weeks"), None);
        assert_eq!(duration_text(2_592_000), "30 days");
        assert_eq!(duration_text(3600), "1 hour");
        assert_eq!(duration_text(90), "90 seconds");
        assert_eq!(duration_text(1), "1 second");
    }

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
