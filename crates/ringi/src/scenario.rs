//! Scenarios and `run` (DESIGN 10.2): a scenario's rows given to the ticket machine one by one, on
//! the organization's data and the books the request uses.
//!
//! A scenario holds what people, clocks and flows do, and the stage and the state it expects after
//! each; it holds no answer of a language. The stages are the lowered route's, the deadlines the
//! lowered dates', the balances the book's, each asked through its port as a row is run. A row is
//! run at the time it is written at: the part of the organization is cut for that time
//! ([`Machine::part`]), the books are let that much time pass, and the machine is given the time.
//!
//! No row passes unsaid. A row the ticket does not take, a hold the book refuses, and a row after
//! the ticket has ended are E603; a stage or a state other than the row writes is E602; a clock's
//! row at another time than the clock is set for, a row for a clock that is not set, and a row
//! written after a clock rings with no row for the clock are E604, with the clocks that are set.
//! The scenario stops at the first, and says where the ticket is.

use crate::ast::{Arg, Expr, ExprKind, Scenario, Span, Word};
use crate::engine::{Effect, Event, Machine, Need, OrgPart, PartState, Refusal, Stage, Status, Submit, Ticket, What};
use crate::org::Org;
use crate::taken::{self, When};
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text};
use ritsu_ports::{BookCall, BookOutcome, Ledger, Value, Values};

/// The ID of the ticket a scenario makes.
pub const TICKET: &str = "T-1";

/// What a transition is given: a request to make, or an event of a ticket.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    Submit(Submit),
    Event(Event),
}

impl Input {
    pub fn word(&self) -> &'static str {
        match self {
            Input::Submit(_) => "submit",
            Input::Event(e) => e.word(),
        }
    }

    pub fn by(&self) -> Option<&str> {
        match self {
            Input::Submit(s) => Some(s.by.as_deref().unwrap_or(&s.requester)),
            Input::Event(e) => e.by(),
        }
    }

    /// Whether a person gives it: one that may come with a comment (DESIGN 7.2).
    pub fn person(&self) -> bool {
        match self {
            Input::Submit(_) => true,
            Input::Event(e) => e.person(),
        }
    }

    pub fn to_json(&self, n: &crate::engine::Names) -> Json {
        match self {
            Input::Submit(s) => s.to_json(),
            Input::Event(e) => e.to_json(n),
        }
    }
}

/// One transition as the machine made it: what it was given (the ticket before, the event, the
/// part of the organization, the time) and what it answered. The vectors are these (DESIGN 10.4).
#[derive(Clone, Debug)]
pub struct Stepped {
    pub before: Option<Ticket>,
    pub input: Input,
    /// The revision of the ticket the event was made on (DESIGN 7.2): none for `submit`, a clock's
    /// event and a comment.
    pub seen: Option<crate::engine::Revision>,
    /// What the person wrote with the event (DESIGN 7.2): none for a clock's and a flow's.
    pub comment: Option<String>,
    pub part: OrgPart,
    pub now: i64,
    pub answer: Result<(Ticket, Vec<Effect>), Refusal>,
}

impl Stepped {
    /// The transition as the JSON writes it, in the contract's shape ([`crate::lower::shape`];
    /// DESIGN 10.4): every name in it the ASCII one (DESIGN 3.4), so that a request in Japanese
    /// names and its English draft write the same.
    pub fn to_json(&self, n: &crate::engine::Names) -> Json {
        let answer = match &self.answer {
            Ok((t, fx)) => Json::obj([("ticket", t.to_json(n)), ("effects", Json::arr(fx.iter().map(|e| e.to_json(n))))]),
            Err(r) => r.to_json(),
        };
        let mut event = self.input.to_json(n);
        if let Json::Obj(o) = &mut event {
            if let Some(seen) = &self.seen {
                o.push(("revision".into(), seen.to_json()));
            }
            if self.input.person() {
                o.push(("comment".into(), Json::opt_str(self.comment.clone())));
            }
        }
        crate::lower::shape::stepped(&Json::obj([
            ("now", Json::int(self.now as i128)),
            ("ticket", self.before.as_ref().map(|t| t.to_json(n)).unwrap_or(Json::Null)),
            ("event", event),
            ("organization", self.part.to_json(n)),
            ("answer", answer),
        ]))
    }
}

/// One transition: the part of `org` cut for the time, and the machine's answer. Err when no
/// version of the organization is in effect at the time. An event other than a clock's and a
/// comment is given with the revision of the ticket before it, as a screen that shows the ticket
/// gives it (DESIGN 7.2). Nobody writes a comment with it ([`step_with`]).
pub fn step(m: &Machine, org: &Org, before: Option<&Ticket>, input: Input, now: i64) -> Result<Stepped, Text> {
    step_with(m, org, before, input, None, now)
}

/// [`step`], a person's event given with what they wrote (`comment`).
pub fn step_with(m: &Machine, org: &Org, before: Option<&Ticket>, input: Input, comment: Option<String>, now: i64) -> Result<Stepped, Text> {
    let seen = match &input {
        Input::Event(ev) if ev.revised() => before.map(Ticket::revision),
        _ => None,
    };
    step_seen(m, org, before, input, comment, seen, now)
}

/// [`step`], the event given with the revision `seen`: an older one, for the vectors of an event
/// made on a ticket that has moved on since. An event that says no revision is given none, and a
/// clock's or a flow's no comment.
pub fn step_seen(m: &Machine, org: &Org, before: Option<&Ticket>, input: Input, comment: Option<String>, seen: Option<crate::engine::Revision>, now: i64) -> Result<Stepped, Text> {
    let comment = comment.filter(|_| input.person());
    let seen = seen.filter(|_| matches!(&input, Input::Event(ev) if ev.revised()));
    let need = match (&input, before) {
        (Input::Submit(sub), _) => Need::submit(sub),
        (Input::Event(ev), Some(t)) => Need::event(t, ev),
        (Input::Event(ev), None) => {
            let e = ev.word();
            return Err(tr!("チケットがまだありません。`{e}` の前に `submit` が要ります", "there is no ticket yet: `submit` comes before `{e}`"));
        }
    };
    let Some(part) = m.part(org, &need, now) else {
        let d = crate::lex::date_text(m.day(now));
        return Err(tr!("{d} に効いている組織の版がありません", "no version of the organization is in effect on {d}"));
    };
    let answer = match (&input, before) {
        (Input::Submit(sub), _) => m.start(sub, comment.as_deref(), &part, now),
        (Input::Event(ev), Some(t)) => m.next(t, ev, comment.as_deref(), seen.as_ref(), &part, now),
        (Input::Event(_), None) => unreachable!(),
    };
    Ok(Stepped { before: before.cloned(), input, seen, comment, part, now, answer })
}

/// Why a scenario stops: the code, where in the scenario, what is wrong, and where the ticket is.
#[derive(Clone, Debug)]
pub struct Stop {
    pub code: &'static str,
    pub at: Span,
    pub text: Text,
    pub notes: Vec<Text>,
}

/// What a book did with one thing the ticket gave it.
#[derive(Clone, Debug, PartialEq)]
pub struct Booked {
    pub op: String,
    pub book: String,
    pub transfer: String,
    pub args: Values,
    /// `done`, `done before`, or the reason it was refused with.
    pub outcome: String,
}

/// A row as it ran.
#[derive(Clone, Debug)]
pub struct Row {
    pub line: usize,
    pub stepped: Stepped,
    pub booked: Vec<Booked>,
    /// The holds that expired in the book before the row: the book, the transfer, its key.
    pub expired: Vec<(String, String, Vec<String>)>,
}

impl Row {
    pub fn ticket(&self) -> Option<&Ticket> {
        self.stepped.answer.as_ref().ok().map(|(t, _)| t)
    }

    pub fn effects(&self) -> &[Effect] {
        self.stepped.answer.as_ref().map(|(_, fx)| fx.as_slice()).unwrap_or(&[])
    }
}

/// An account of a book as a scenario leaves it, and a hold.
#[derive(Clone, Debug, PartialEq)]
pub struct BookEnd {
    pub book: String,
    /// Each account: its kind, the values of its parameters, its unit and the places its amounts
    /// count past the point, and its balance (posted, held coming in, held going out).
    pub accounts: Vec<(String, Vec<String>, String, u32, [i128; 3])>,
    /// Each hold: its transfer, its key, its state.
    pub holds: Vec<(String, Vec<String>, String)>,
}

/// A scenario as it ran.
#[derive(Clone, Debug)]
pub struct Ran {
    pub name: String,
    pub line: usize,
    pub rows: Vec<Row>,
    pub stop: Option<Stop>,
    pub books: Vec<BookEnd>,
}

impl Ran {
    /// The ticket as the scenario leaves it.
    pub fn ticket(&self) -> Option<&Ticket> {
        self.rows.iter().rev().find_map(|r| r.ticket())
    }
}

/// The books a scenario runs on: each opened when it is first named.
struct Books<'x, 's, 'a> {
    m: &'x Machine<'s, 'a>,
    open: Vec<(String, Box<dyn Ledger>)>,
}

impl Books<'_, '_, '_> {
    /// The book of the alias, opened; None where chobo is not joined, or the book does not read.
    fn ledger(&mut self, alias: &str) -> Option<&mut Box<dyn Ledger>> {
        if !self.open.iter().any(|(a, _)| a == alias) {
            let books = self.m.suite.books.as_ref()?;
            let led = books.open(self.m.s.other_path(alias)?).ok()?;
            self.open.push((alias.to_string(), led));
        }
        self.open.iter_mut().find(|(a, _)| a == alias).map(|(_, l)| l)
    }

    /// One operation: what the book did (`done`, `done before`, or the reason of a refusal); Err
    /// for a call that does not fit the book.
    fn apply(&mut self, op: &str, book: &str, transfer: &str, args: &Values) -> Result<Booked, Text> {
        let call = BookCall {
            transfer: transfer.to_string(),
            op: op.to_string(),
            args: args
                .iter()
                .map(|(n, v)| {
                    (
                        n.clone(),
                        match v {
                            Value::Int(k) => Ok(*k),
                            Value::Str(s) | Value::Enum(s) | Value::Date(s) => Err(s.clone()),
                            Value::Bool(b) => Err(b.to_string()),
                            Value::List(_) | Value::None => Err(String::new()),
                        },
                    )
                })
                .collect(),
            amounts: None,
        };
        let outcome = match self.ledger(book) {
            None => "not run".to_string(),
            Some(l) => match l.apply(&call)? {
                BookOutcome::Done => "done".to_string(),
                BookOutcome::DoneBefore => "done before".to_string(),
                BookOutcome::Refused(why) => why,
            },
        };
        Ok(Booked { op: op.to_string(), book: book.to_string(), transfer: transfer.to_string(), args: args.clone(), outcome })
    }

    fn pass(&mut self, seconds: u64) -> Vec<(String, String, Vec<String>)> {
        let mut out = Vec::new();
        for (alias, l) in self.open.iter_mut() {
            out.extend(l.pass(seconds).into_iter().map(|(t, k)| (alias.clone(), t, k)));
        }
        out
    }

    fn ends(&self) -> Vec<BookEnd> {
        self.open
            .iter()
            .map(|(alias, l)| {
                let facts = self.m.books.iter().find(|(a, _)| a == alias).map(|(_, f)| f);
                let unit = |account: &str| {
                    let u = facts.and_then(|f| f.accounts.iter().find(|a| a.name == account)).map(|a| a.unit.clone()).unwrap_or_default();
                    let scale = facts.and_then(|f| f.units.iter().find(|x| x.name == u)).map(|x| x.scale).unwrap_or(0);
                    (u, scale)
                };
                BookEnd {
                    book: alias.clone(),
                    accounts: l
                        .accounts()
                        .into_iter()
                        .map(|(a, args, b)| {
                            let (u, scale) = unit(&a);
                            (a, args, u, scale, [b.posted, b.held_in, b.held_out])
                        })
                        .collect(),
                    holds: l.holds(),
                }
            })
            .collect()
    }
}

/// A constant written in a scenario where no field gives its type (a `given` line's argument, an
/// output of `note ready`).
fn constant(e: &Expr) -> Option<Value> {
    Some(match &e.kind {
        ExprKind::Str(s) | ExprKind::Person(s) => Value::Str(s.clone()),
        ExprKind::Team(w) => Value::Str(w.text.clone()),
        ExprKind::Bool(b) => Value::Bool(*b),
        ExprKind::Date(d) => Value::Date(crate::lex::date_text(*d)),
        ExprKind::Path(p) if p.len() == 1 => Value::Enum(p[0].text.clone()),
        ExprKind::None => Value::None,
        ExprKind::Num(_) | ExprKind::Neg(_) => {
            let r = crate::lower::words::constant(e)?.rat();
            if !r.is_int() {
                return None;
            }
            Value::Int(r.num)
        }
        _ => return None,
    })
}

/// The ID of a person an argument names: `person "e9002"`, or the ID as a `by` cell writes it.
fn person(e: &Expr) -> Option<String> {
    match &e.kind {
        ExprKind::Person(id) | ExprKind::Str(id) => Some(id.clone()),
        ExprKind::Path(p) if p.len() == 1 => Some(p[0].text.clone()),
        _ => None,
    }
}

fn word(e: &Expr) -> Option<String> {
    match &e.kind {
        ExprKind::Path(p) if p.len() == 1 => Some(p[0].text.clone()),
        ExprKind::Str(s) => Some(s.clone()),
        _ => None,
    }
}

struct Runner<'x, 's, 'a> {
    m: &'x Machine<'s, 'a>,
    org: &'x Org,
    sc: &'x Scenario,
}

impl Runner<'_, '_, '_> {
    fn stop(&self, code: &'static str, at: Span, text: Text) -> Stop {
        Stop { code, at, text, notes: Vec::new() }
    }

    /// The arguments an event's cell may write, by the event; Err at the first other.
    fn args<'e>(&self, e: &'e crate::ast::Event, names: &[&str]) -> Result<Vec<Option<&'e Arg>>, Stop> {
        let ev = &e.event.text;
        if let Some(a) = e.args.iter().find(|a| !names.contains(&a.name.text.as_str())) {
            let n = &a.name.text;
            let list = if names.is_empty() { "none".to_string() } else { names.iter().map(|x| format!("`{x}`")).collect::<Vec<_>>().join(", ") };
            let list_ja = if names.is_empty() { "ありません".to_string() } else { format!("{} です", names.iter().map(|x| format!("`{x}`")).collect::<Vec<_>>().join("、")) };
            return Err(self.stop("E603", a.name.span, tr!("`{ev}` は `{n}` を取りません。取るのは{list_ja}", "`{ev}` takes no `{n}`: it takes {list}")));
        }
        Ok(names.iter().map(|n| e.args.iter().find(|a| a.name.text == *n)).collect())
    }

    /// The values an event gives the form (`resubmit`, `edit`), read and held as every value of a
    /// form is.
    fn values(&self, e: &crate::ast::Event, part_day: i64, v: &crate::org::Version) -> Result<Values, Stop> {
        let written: Vec<(Word, Expr)> = e.args.iter().map(|a| (a.name.clone(), a.value.clone())).collect();
        let when = When { day: Some(part_day), versions: Some(vec![v]), calendars: &self.m.calendars, rows: None };
        taken::form(self.m.s, self.m.s.src, &written, &when).map_err(|(at, no)| self.stop("E603", at, no.text))
    }

    /// The row as the machine takes it.
    /// `round` is the round of the ticket the row is given to, none before its `submit`.
    fn input(&self, e: &crate::ast::Event, requester: &str, form: &Values, round: Option<u32>, now: i64) -> Result<Input, Stop> {
        let ev = e.event.text.as_str();
        let by = e.by.as_ref().map(|w| w.text.clone());
        let clock = matches!(ev, "remind" | "late" | "resubmit late" | "pause ends");
        let flow = matches!(ev, "agent answered" | "note ready" | "flow failed");
        let need_by = || match &by {
            Some(b) => Ok(b.clone()),
            None => Err(self.stop("E603", e.event.span, tr!("`{ev}` は人がします。`by` のセルに、する人の ID を書いてください", "`{ev}` is a person's to do: write who does it in the `by` cell"))),
        };
        for (cell, w) in [("by", &e.by), ("comment", &e.comment)] {
            if let (true, Some(w)) = (clock || flow, w) {
                let what = if clock { tr!("時計", "a clock") } else { tr!("flow", "a flow") };
                return Err(self.stop("E603", w.span, tr!("`{ev}` は{}のイベントです。`{cell}` のセルは空にします", "`{ev}` is {}'s event: its `{cell}` cell is empty", what.ja; what.en)));
            }
        }
        let day = self.m.day(now);
        let version = || self.org.in_effect(day).ok_or_else(|| self.stop("E603", e.event.span, tr!("{} に効いている組織の版がありません", "no version of the organization is in effect on {}", crate::lex::date_text(day); crate::lex::date_text(day))));
        let stage = |x: &Expr| {
            let w = word(x)?;
            Some(self.m.s.stages().unwrap_or_default().iter().find(|n| n.text == w || n.ascii() == w).map(|n| n.text.clone()).unwrap_or(w))
        };
        let bad = |a: &Arg, want: Text| self.stop("E603", a.value.span, tr!("`{ev}` の `{}` には、{}を書きます", "`{}` of `{ev}` takes {}", a.name.text, want.ja; a.name.text, want.en));
        Ok(match ev {
            "submit" => {
                self.args(e, &[])?;
                Input::Submit(Submit { id: TICKET.to_string(), requester: requester.to_string(), by: by.filter(|b| b != requester), form: form.clone(), attempt: crate::engine::attempt(1) })
            }
            "resubmit" => {
                let (form, clear) = emptied(self.values(e, day, version()?)?);
                Input::Event(Event::Resubmit { by: need_by()?, form, clear, attempt: crate::engine::attempt(round.unwrap_or(0) + 1) })
            }
            "edit" => {
                let (fields, clear) = emptied(self.values(e, day, version()?)?);
                Input::Event(Event::Edit { by: need_by()?, fields, clear })
            }
            "approve" | "reject" | "retract" | "pause" | "resume" | "withdraw" | "discard" | "comment" => {
                self.args(e, &[])?;
                let by = need_by()?;
                Input::Event(match ev {
                    "approve" => Event::Approve { by },
                    "reject" => Event::Reject { by },
                    "retract" => Event::Retract { by },
                    "pause" => Event::Pause { by },
                    "resume" => Event::Resume { by },
                    "withdraw" => Event::Withdraw { by },
                    "comment" => Event::Comment { by },
                    _ => Event::Discard { by },
                })
            }
            "send back" => {
                let a = self.args(e, &["to"])?;
                let to = match a[0] {
                    None => None,
                    Some(x) => Some(stage(&x.value).ok_or_else(|| bad(x, tr!("ステップの名前", "a stage's name")))?),
                };
                Input::Event(Event::SendBack { by: need_by()?, to })
            }
            "add approver" => {
                let a = self.args(e, &["approver", "part"])?;
                let Some(x) = a[0] else {
                    return Err(self.stop("E603", e.event.span, tr!("`add approver` には、足す人（`add approver(approver: person \"e9002\")`）が要ります", "`add approver` needs the person to add (`add approver(approver: person \"e9002\")`)")));
                };
                let approver = person(&x.value).ok_or_else(|| bad(x, tr!("人", "a person")))?;
                let part = match a[1] {
                    None => None,
                    Some(p) => Some(word(&p.value).ok_or_else(|| bad(p, tr!("承認者の決め方の名前", "the name of a way to choose")))?),
                };
                Input::Event(Event::AddApprover { by, approver, part })
            }
            "hand over" => {
                let a = self.args(e, &["from", "to"])?;
                let (Some(f), Some(t)) = (a[0], a[1]) else {
                    return Err(self.stop("E603", e.event.span, tr!("`hand over` には、渡す人と受ける人（`hand over(from: person \"e1101\", to: person \"e1102\")`）が要ります", "`hand over` needs who hands over and who takes over (`hand over(from: person \"e1101\", to: person \"e1102\")`)")));
                };
                let from = person(&f.value).ok_or_else(|| bad(f, tr!("人", "a person")))?;
                let to = person(&t.value).ok_or_else(|| bad(t, tr!("人", "a person")))?;
                Input::Event(Event::HandOver { by, from, to })
            }
            "remind" | "late" | "resubmit late" | "pause ends" | "flow failed" => {
                self.args(e, &[])?;
                Input::Event(match ev {
                    "remind" => Event::Remind,
                    "late" => Event::Late,
                    "resubmit late" => Event::ResubmitLate,
                    "pause ends" => Event::PauseEnds,
                    _ => Event::FlowFailed,
                })
            }
            "agent answered" => {
                let a = self.args(e, &["verdict", "reason"])?;
                let ok = match a[0].map(|x| (x, word(&x.value))) {
                    Some((_, Some(w))) if w == "ok" => true,
                    Some((_, Some(w))) if w == "doubt" => false,
                    Some((x, _)) => return Err(bad(x, tr!("`ok` か `doubt` ", "`ok` or `doubt`"))),
                    None => return Err(self.stop("E603", e.event.span, tr!("`agent answered` には、答え（`agent answered(verdict: ok)`）が要ります", "`agent answered` needs the answer (`agent answered(verdict: ok)`)"))),
                };
                let reason = match a[1] {
                    None => String::new(),
                    Some(x) => match &x.value.kind {
                        ExprKind::Str(s) => s.clone(),
                        _ => return Err(bad(x, tr!("文", "a text"))),
                    },
                };
                Input::Event(Event::AgentAnswered { ok, reason })
            }
            _ => {
                // note ready: the outputs of the summary's flow, as constants
                let mut outputs = Values::new();
                for a in &e.args {
                    outputs.push((a.name.text.clone(), constant(&a.value).ok_or_else(|| bad(a, tr!("定数", "a constant")))?));
                }
                Input::Event(Event::NoteReady { outputs })
            }
        })
    }

    /// Where the ticket is, as a note of a diagnostic says it.
    fn stands(&self, t: Option<&Ticket>) -> Text {
        let Some(t) = t else { return tr!("チケットはまだありません。", "There is no ticket yet.") };
        let st = t.status.word();
        let clocks = clocks_said(self.m, t);
        let at = match t.stage_name() {
            Some(s) => tr!("チケットは `{s}` にあり、状態は `{st}` です。", "The ticket is at `{s}`, and is `{st}`."),
            None => tr!("チケットの状態は `{st}` です。", "The ticket is `{st}`."),
        };
        match (t.status.ended(), clocks.is_empty()) {
            (true, _) => at,
            (false, true) => tr!("{}置いてある時計はありません。", "{} No clock is set.", at.ja; at.en),
            (false, false) => tr!("{}置いてある時計: {}。", "{} The clocks set: {}.", at.ja, clocks.ja; at.en, clocks.en),
        }
    }

    fn run(&self) -> Ran {
        let (m, sc) = (self.m, self.sc);
        let mut ran = Ran { name: sc.name.text.clone(), line: sc.line, rows: Vec::new(), stop: None, books: Vec::new() };
        let mut books = Books { m, open: Vec::new() };
        let mut ticket: Option<Ticket> = None;
        let stop = (|| -> Result<(), Stop> {
            let Some(rq) = &sc.requester else {
                return Err(self.stop("E603", sc.name.span, tr!("シナリオに申請者がありません（`requester <ID>`）", "The scenario names no requester (`requester <id>`)")));
            };
            // what the books hold at the start
            for g in &sc.given {
                let (book, transfer) = (g.target[0].text.clone(), g.target.get(1).map(|w| w.text.clone()).unwrap_or_default());
                let args = self.given(g, &book, &transfer)?;
                let b = books.apply("do", &book, &transfer, &args).map_err(|t| self.stop("E603", g.span, tr!("`given` の振替が帳簿に合いません: {}", "The transfer of `given` does not fit the book: {}", t.ja; t.en)))?;
                if !matches!(b.outcome.as_str(), "done" | "done before" | "not run") {
                    let why = &b.outcome;
                    return Err(self.stop("E603", g.span, tr!("帳簿 `{book}` が `given` の `{transfer}` を拒否しました: {why}", "The book `{book}` refuses the `{transfer}` of `given`: {why}")));
                }
            }
            let mut form: Option<Values> = None;
            let mut prev: Option<i64> = None;
            for e in &sc.events {
                let at = Span { line: e.line, col: e.event.span.col, end: e.event.span.end };
                let now = match (e.at, prev) {
                    (Some((d, min)), _) => m.instant(d, min),
                    (None, Some(p)) => p,
                    (None, None) => return Err(self.stop("E603", at, tr!("最初の行に時刻がありません（`at` のセル）", "The first row has no time (its `at` cell)"))),
                };
                if let Some(p) = prev.filter(|p| now < *p) {
                    let (a, b) = (local(m, now), local(m, p));
                    let mut s = self.stop("E603", at, tr!("この行の時刻（{a}）が、上の行（{b}）より前です", "The row is at {a}, before the row above ({b})"));
                    s.notes.push(self.stands(ticket.as_ref()));
                    return Err(s);
                }
                let ev = e.event.text.as_str();
                // the clocks: a clock's row is at the time the clock is set for, and no row comes
                // after a clock rings without the clock's own row
                if let Some(t) = ticket.as_ref().filter(|t| !t.status.ended()) {
                    let timers = t.timers();
                    let own = timers.iter().position(|(w, ..)| *w == ev);
                    let clock = matches!(ev, "remind" | "late" | "resubmit late" | "pause ends");
                    if clock {
                        match own.map(|i| timers[i]) {
                            None => {
                                let mut s = self.stop("E604", at, tr!("`{ev}` の時計は置かれていません", "No clock is set for `{ev}`"));
                                s.notes.push(self.stands(Some(t)));
                                return Err(s);
                            }
                            Some((_, stage, set)) if set != now => {
                                let (a, b) = (local(m, set), local(m, now));
                                let of = stage.map(|s| format!(" (`{s}`)")).unwrap_or_default();
                                let of_ja = stage.map(|s| format!("（`{s}`）")).unwrap_or_default();
                                let mut s = self.stop("E604", at, tr!("`{ev}` の時計{of_ja}は {a} に鳴ります。この行は {b} です", "The clock of `{ev}`{of} is set for {a}, and the row is at {b}"));
                                s.notes.push(tr!("時計のイベントは、時計が鳴る時刻に書きます。時刻は下ろした日付が決め、シナリオは持ちません。", "A clock's row is written at the time the clock rings: the lowered dates decide the time, and a scenario holds none."));
                                return Err(s);
                            }
                            Some(_) => {}
                        }
                    }
                    // (what is no clock's row waits for a clock that rings at its very time too)
                    if let Some((w, stage, set)) = timers.iter().enumerate().find(|(i, (.., set))| Some(*i) != own.filter(|_| clock) && (*set < now || (!clock && *set == now))).map(|(_, x)| *x) {
                        let (a, b) = (local(m, set), local(m, now));
                        let st = stage.unwrap_or_default();
                        let mut s = if set < now {
                            self.stop("E604", at, tr!("`{w}` の時計が {a} に鳴ります。この行（{b}）より前です", "The clock of `{w}` rings at {a}, before this row ({b})"))
                        } else {
                            self.stop("E604", at, tr!("`{w}` の時計が、この行と同じ {a} に鳴ります。時計の行が先です", "The clock of `{w}` rings at {a}, the time of this row: the clock's row comes first"))
                        };
                        s.notes.push(tr!("時計が鳴ったら、その行を書きます: `| {a} | {w} | | {st} | … |`。", "Write the row of a clock that rings: `| {a} | {w} | | {st} | … |`."));
                        s.notes.push(self.stands(Some(t)));
                        return Err(s);
                    }
                }
                // the values of the form, read on the day the request is made
                if ev == "submit" && form.is_none() {
                    let day = m.day(now);
                    let Some(v) = self.org.in_effect(day) else {
                        let d = crate::lex::date_text(day);
                        return Err(self.stop("E603", at, tr!("{d} に効いている組織の版がありません", "No version of the organization is in effect on {d}")));
                    };
                    let when = When { day: Some(day), versions: Some(vec![v]), calendars: &m.calendars, rows: None };
                    form = Some(taken::form(m.s, m.s.src, &sc.form, &when).map_err(|(span, no)| self.stop("E603", span, no.text))?);
                }
                let input = self.input(e, &rq.text, form.as_ref().unwrap_or(&Values::new()), ticket.as_ref().map(|t| t.round), now)?;
                let expired = books.pass(prev.map_or(0, |p| (now - p).max(0) as u64));
                prev = Some(now);
                if ev == "submit" && ticket.is_some() {
                    let mut s = self.stop("E603", at, tr!("`submit` は、シナリオの最初の行に一度だけ書きます", "`submit` is written once, in the first row of a scenario"));
                    s.notes.push(self.stands(ticket.as_ref()));
                    return Err(s);
                }
                let comment = e.comment.as_ref().map(|w| w.text.clone());
                let stepped = step_with(m, self.org, ticket.as_ref(), input, comment, now).map_err(|t| self.stop("E603", at, t))?;
                let (after, effects) = match &stepped.answer {
                    Ok(x) => x.clone(),
                    Err(r) => {
                        let mut s = self.stop("E603", at, tr!("チケットは、この行の `{ev}` を受け付けません: {}", "The ticket does not take the `{ev}` of this row: {}", r.text.ja; r.text.en));
                        s.notes.push(self.stands(ticket.as_ref()));
                        ran.rows.push(Row { line: e.line, stepped, booked: Vec::new(), expired });
                        return Err(s);
                    }
                };
                // the books: what the ticket gives them is done, and a refusal stops the row
                let mut booked = Vec::new();
                let mut refused = None;
                for fx in &effects {
                    let What::Book { op, book, transfer, args } = &fx.what else { continue };
                    let b = books.apply(op, book, transfer, args).map_err(|t| self.stop("E603", at, tr!("`{op} {book}.{transfer}` が帳簿に合いません: {}", "`{op} {book}.{transfer}` does not fit the book: {}", t.ja; t.en)))?;
                    if !matches!(b.outcome.as_str(), "done" | "done before" | "not run") && refused.is_none() {
                        refused = Some((op.to_string(), book.clone(), transfer.clone(), b.outcome.clone()));
                    }
                    booked.push(b);
                }
                ran.rows.push(Row { line: e.line, stepped, booked, expired });
                if let Some((op, book, transfer, why)) = refused {
                    let mut s = self.stop("E603", at, tr!("帳簿 `{book}` が `{op} {transfer}` を拒否しました（`{why}`）。サーバーは、この行のイベントを受け付けません", "The book `{book}` refuses to {op} `{transfer}` (`{why}`): a server does not take the event of this row"));
                    s.notes.push(self.stands(ticket.as_ref()));
                    return Err(s);
                }
                // what the row expects
                let want_stage = e.stage.as_ref().map(|w| m.s.stages().unwrap_or_default().iter().find(|n| n.text == w.text || n.ascii() == w.text).map(|n| n.text.clone()).unwrap_or_else(|| w.text.clone()));
                let (got_stage, got_status) = (after.stage_name().map(str::to_string), after.status.word());
                if want_stage != got_stage || e.status.text != got_status {
                    let span = if want_stage != got_stage { e.stage.as_ref().map_or(e.status.span, |w| w.span) } else { e.status.span };
                    let said = |stage: &Option<String>, status: &str| match stage {
                        Some(s) => tr!("`{s}`、`{status}`", "`{s}`, `{status}`"),
                        None => Text::same(format!("`{status}`")),
                    };
                    let (want, got) = (said(&want_stage, &e.status.text), said(&got_stage, got_status));
                    let mut s = self.stop("E602", span, tr!("`{ev}` のあと、チケットは {} です。シナリオは {} と書いています", "After `{ev}`, the ticket is {}, and the scenario writes {}", got.ja, want.ja; got.en, want.en));
                    s.notes.push(self.stands(Some(&after)));
                    ticket = Some(after);
                    return Err(s);
                }
                ticket = Some(after);
            }
            Ok(())
        })();
        ran.stop = stop.err();
        ran.books = books.ends();
        ran
    }

    /// The arguments of a `given` line, as the book's transfer takes them: an amount as the whole
    /// number the book keeps, anything else as a text.
    fn given(&self, g: &crate::ast::Call, book: &str, transfer: &str) -> Result<Values, Stop> {
        let facts = self.m.books.iter().find(|(a, _)| a == book).map(|(_, f)| f);
        let tr = facts.and_then(|f| f.transfers.iter().find(|t| t.name == transfer));
        let mut out = Values::new();
        for a in &g.args {
            let n = &a.name.text;
            let w = crate::types::written(self.m.s.src, a.value.span);
            let bad = || self.stop("E603", a.value.span, tr!("`{w}` は、`{book}.{transfer}` の `{n}` に渡せる定数ではありません", "`{w}` is no constant `{n}` of `{book}.{transfer}` takes"));
            let unit = tr.and_then(|t| t.params.iter().find(|p| p.name == *n)).and_then(|p| p.unit.clone());
            let v = match (&unit, &a.value.kind) {
                (Some(u), ExprKind::Num(_) | ExprKind::Neg(_)) => {
                    let scale = facts.and_then(|f| f.units.iter().find(|x| x.name == *u)).map(|x| x.scale).unwrap_or(0);
                    let r = crate::lower::words::constant(&a.value).ok_or_else(bad)?.rat().mul(ritsu_units::Rat::int(10i128.pow(scale)));
                    if !r.is_int() {
                        return Err(bad());
                    }
                    Value::Int(r.num)
                }
                (Some(_), _) => return Err(bad()),
                (None, _) => match constant(&a.value).ok_or_else(bad)? {
                    Value::Int(k) => Value::Str(k.to_string()),
                    Value::Enum(s) | Value::Date(s) => Value::Str(s),
                    Value::Bool(b) => Value::Str(b.to_string()),
                    v => v,
                },
            };
            out.push((n.clone(), v));
        }
        Ok(out)
    }
}

/// A scenario of the request, run on the machine `m` and the data `org`.
pub fn run(m: &Machine, org: &Org, sc: &Scenario) -> Ran {
    Runner { m, org, sc }.run()
}

// ── What `run` prints ──

/// A time as the request reads it: `2026-11-16 09:00`.
pub fn local(m: &Machine, at: i64) -> String {
    let l = at + i64::from(m.offset) * 60;
    format!("{} {}", crate::lex::date_text(l.div_euclid(86_400)), crate::lex::time_text((l.rem_euclid(86_400) / 60) as u32))
}

/// A deadline as it is read: `2026-11-18 end of day` for the 00:00 after it.
fn deadline(m: &Machine, at: i64) -> Text {
    let l = at + i64::from(m.offset) * 60;
    if l.rem_euclid(86_400) == 0 {
        let d = crate::lex::date_text(l.div_euclid(86_400) - 1);
        tr!("{d} の終わり", "{d} end of day")
    } else {
        Text::same(local(m, at))
    }
}

/// The clocks a ticket has set, each with its time: `late 2026-11-19 00:00 (head)`.
fn clocks_said(m: &Machine, t: &Ticket) -> Text {
    let each: Vec<Text> = t
        .timers()
        .iter()
        .map(|(w, stage, at)| {
            let a = local(m, *at);
            match stage {
                Some(s) => tr!("{w} {a}（{s}）", "{w} {a} ({s})"),
                None => Text::same(format!("{w} {a}")),
            }
        })
        .collect();
    Text::join(&each, "、", ", ")
}

/// Who a stage waits for: `e1101`, `one of e1001, e1002`, `2 of e0201, e0202, e0203`, by part
/// where it has several.
fn waits(st: &Stage) -> Text {
    let open: Vec<&crate::engine::Part> = st.parts.iter().filter(|p| !p.done()).collect();
    let one = |p: &crate::engine::Part| match &p.state {
        PartState::Agent(flow) => Text::same(format!("flow {flow}")),
        PartState::Stopped => tr!("だれもいない。管理する人が承認者を足すのを待つ", "nobody, until an administrator adds an approver"),
        _ => {
            let left: Vec<&str> = p.approvers.iter().filter(|a| !p.answers.iter().any(|x| x.seat == **a)).map(String::as_str).collect();
            let need = p.needed.saturating_sub(p.yes());
            match (left.len(), need) {
                (1, _) => Text::same(left[0]),
                (_, 1) => tr!("{} のだれか", "one of {}", left.join("、"); left.join(", ")),
                (_, n) => tr!("{} のうち {n} 人", "{n} of {}", left.join("、"); left.join(", ")),
            }
        }
    };
    match open.as_slice() {
        [] => Text::default(),
        [p] if st.parts.len() == 1 => one(p),
        ps => Text::join(
            &ps.iter()
                .map(|p| {
                    let (w, o) = (&p.way, one(p));
                    tr!("{w}: {}", "{w}: {}", o.ja; o.en)
                })
                .collect::<Vec<_>>(),
            "; ",
            "; ",
        ),
    }
}

/// Where a ticket is after a row: `manager (e1101), due 2026-11-18 end of day`.
fn place(m: &Machine, t: &Ticket) -> Text {
    match t.status {
        Status::InProgress | Status::Paused => {
            let Some(st) = t.stage() else { return Text::same(t.status.word()) };
            let (name, who) = (&st.name, waits(st));
            let mut s = if who.is_empty() { Text::same(name) } else { tr!("{name}（{}）", "{name} ({})", who.ja; who.en) };
            // (a space after a name and a time, none after a bracket and a word)
            let sp = |x: &str| if x.ends_with(|c: char| c.is_ascii_alphanumeric()) { " " } else { "" };
            if t.status == Status::Paused {
                s = tr!("{}{}で保留", "paused at {}", s.ja, sp(&s.ja); s.en);
                if let Some(at) = t.pause_ends {
                    let d = deadline(m, at);
                    s = tr!("{}、{}{}まで", "{}, until {}", s.ja, d.ja, sp(&d.ja); s.en, d.en);
                }
            } else if let Some(at) = st.due {
                let d = deadline(m, at);
                s = tr!("{}、期限は {}", "{}, due {}", s.ja, d.ja; s.en, d.en);
            }
            s
        }
        Status::WithRequester => match t.resubmit_due {
            Some(at) => {
                let d = deadline(m, at);
                tr!("with requester、再申請は {}{}まで", "with requester, resubmit by {}", d.ja, if d.ja.ends_with(|c: char| c.is_ascii_digit()) { " " } else { "" }; d.en)
            }
            None => Text::same("with requester"),
        },
        s => Text::same(s.word()),
    }
}

/// An amount as a book keeps it, written in its unit: `8 day`, `1_200_000 JPY`.
fn amount(n: i128, unit: &str, scale: u32) -> String {
    let r = ritsu_units::Rat::new(n, 10i128.pow(scale));
    let shown = if r.is_int() { crate::lower::words::grouped(r.num) } else { crate::lex::decimal(r) };
    if unit.is_empty() { shown } else { format!("{shown} {unit}") }
}

/// What a row did besides moving the ticket, from the things it gave to do: the stages passed
/// over, the books, the flows. The words of the history (why a stage has nobody, why it went up)
/// are printed as the history keeps them, in whichever language the rest is.
fn things(m: &Machine, row: &Row) -> Vec<Text> {
    let mut out = Vec::new();
    for fx in row.effects() {
        match &fx.what {
            What::Record(r) => {
                let stage = r.stage.clone().unwrap_or_default();
                let (to, to_ja) = (r.to.join(", "), r.to.join("、"));
                let note = r.note.clone().unwrap_or_default();
                let (with, by, of) = (r.once_with.as_deref().unwrap_or_default(), r.by.as_deref().unwrap_or_default(), r.proxy_of.as_deref().unwrap_or_default());
                match r.did {
                    "skipped" => out.push(tr!("{stage} を飛ばした（{note}）", "{stage} skipped ({note})")),
                    "stopped" => out.push(tr!("{stage} にだれもいない（{note}）", "{stage} has nobody ({note})")),
                    "went up" if note == "late" => out.push(tr!("{stage} を {to_ja} へ上げた", "{stage} went up to {to}")),
                    "went up" => out.push(tr!("{stage} を {to_ja} へ上げた（{note}）", "{stage} went up to {to} ({note})")),
                    "went to" => out.push(tr!("{stage} を {to_ja} へ回した（{note}）", "{stage} went to {to} ({note})")),
                    "circulated" => out.push(tr!("{stage} を {to_ja} へ回覧した", "{stage} circulated to {to}")),
                    "approved" if r.once_with.is_some() && r.proxy_of.is_some() => out.push(tr!("{stage} は {with} の承認で済んだ（{of} の代理 {by}）", "{stage} done with the approval of {with} ({by} for {of})")),
                    "approved" if r.once_with.is_some() => out.push(tr!("{stage} は {with} の承認で済んだ（{by}）", "{stage} done with the approval of {with} ({by})")),
                    "deferred" if r.note.is_some() => out.push(tr!("{stage} は {with} の承認を待つ（{to_ja}。{note}）", "{stage} waits for the approval of {with} ({to}; {note})")),
                    "deferred" => out.push(tr!("{stage} は {with} の承認を待つ（{to_ja}）", "{stage} waits for the approval of {with} ({to})")),
                    "deferral ended" if r.to.is_empty() => out.push(tr!("{stage} は {with} を待つのをやめた（{note}）", "{stage} no longer waits for {with} ({note})")),
                    "deferral ended" => out.push(tr!("{stage} は {with} を待つのをやめて始めた（{to_ja}。{note}）", "{stage} no longer waits for {with}, and starts ({to}; {note})")),
                    "approved" | "rejected" | "sent back" if r.proxy_of.is_some() => out.push(tr!("{of} の代理", "for {of}")),
                    "agent asked" | "note asked" => out.push(tr!("flow {note} に頼んだ", "ask flow {note}")),
                    "note missing" => out.push(tr!("{stage} は要約なしで進む（{note}）", "{stage} goes on without its summary ({note})")),
                    "late" => out.push(tr!("{stage} が期限を過ぎた（{note}）", "{stage} is late ({note})")),
                    "reminded" => out.push(tr!("{to_ja} に催促した", "reminded {to}")),
                    "ticket expired" => out.push(tr!("期限切れ（{note}）", "expired ({note})")),
                    _ => {}
                }
                // the fields that lost their value, by their names as written
                if !r.cleared.is_empty() {
                    let named: Vec<&str> = r.cleared.iter().map(|n| m.s.fields().iter().find(|f| f.name.ascii() == n).map(|f| f.name.text.as_str()).unwrap_or(n)).collect();
                    out.push(tr!("空になった: {}", "now empty: {}", named.join("、"); named.join(", ")));
                }
                // what a person wrote (an agent's reason is in the event's cell already)
                if let Some(c) = r.comment.as_deref().filter(|_| !r.did.starts_with("agent")) {
                    out.push(tr!("コメント「{c}」", "comment \"{c}\""));
                }
            }
            What::Launch { flow, .. } => out.push(Text::same(format!("launch {flow}"))),
            _ => {}
        }
    }
    for b in &row.booked {
        let facts = m.books.iter().find(|(a, _)| *a == b.book).map(|(_, f)| f);
        let tr = facts.and_then(|f| f.transfers.iter().find(|t| t.name == b.transfer));
        let amounts: Vec<String> = b
            .args
            .iter()
            .filter_map(|(n, v)| {
                let u = tr?.params.iter().find(|p| p.name == *n)?.unit.clone()?;
                let scale = facts?.units.iter().find(|x| x.name == u).map(|x| x.scale).unwrap_or(0);
                if let Value::Int(k) = v { Some(amount(*k, &u, scale)) } else { None }
            })
            .collect();
        let mut s = format!("{} {}.{}", b.op, b.book, b.transfer);
        if !amounts.is_empty() {
            s.push_str(&format!(" {}", amounts.join(", ")));
        }
        out.push(match b.outcome.as_str() {
            "done" => Text::same(s),
            "not run" => tr!("{s}（chobo がつながっていないので、行っていない）", "{s} (not run: chobo is not joined)"),
            other => Text::same(format!("{s} ({other})")),
        });
    }
    for (book, transfer, key) in &row.expired {
        let k = key.join(", ");
        out.insert(0, tr!("仮押さえ {book}.{transfer}({k}) が期限切れになった", "the hold {book}.{transfer}({k}) expired"));
    }
    out
}

/// The values of a resubmit's or an edit's cell, and the fields it makes empty: the ones it writes
/// `none` (DESIGN 7.3), by their lowered names.
fn emptied(vals: Values) -> (Values, Vec<String>) {
    let clear = vals.iter().filter(|(_, v)| *v == Value::None).map(|(n, _)| n.clone()).collect();
    (vals.into_iter().filter(|(_, v)| *v != Value::None).collect(), clear)
}

/// What an event's cell wrote after its name: `to first`, `reviewer = person "e9002"`, `cover =
/// none` for a field a resubmit or an edit makes empty.
fn carried(m: &Machine, i: &Input) -> Text {
    let vals = |v: &Values, clear: &[String]| {
        let named = |n: &str| m.s.fields().iter().find(|f| f.name.ascii() == n);
        v.iter()
            .map(|(n, x)| match named(n) {
                Some(f) => format!("{} = {}", f.name.text, taken::shown(f, x)),
                None => format!("{n} = {}", taken::plain(x)),
            })
            .chain(clear.iter().map(|n| format!("{} = none", named(n).map(|f| f.name.text.as_str()).unwrap_or(n))))
            .collect::<Vec<_>>()
            .join(", ")
    };
    match i {
        Input::Event(Event::SendBack { to: Some(k), .. }) => tr!(" {k} へ", " to {k}"),
        Input::Event(Event::Resubmit { form, clear, .. }) if !form.is_empty() || !clear.is_empty() => { let v = vals(form, clear); tr!("（{}）", " ({})", v; v) },
        Input::Event(Event::Edit { fields, clear, .. }) => { let v = vals(fields, clear); tr!("（{}）", " ({})", v; v) },
        Input::Event(Event::AddApprover { approver, part: Some(p), .. }) => tr!(" {approver} を {p} に", " {approver} to {p}"),
        Input::Event(Event::AddApprover { approver, .. }) => Text::same(format!(" {approver}")),
        Input::Event(Event::HandOver { from, to, .. }) => tr!(" {from} から {to} へ", " from {from} to {to}"),
        Input::Event(Event::AgentAnswered { ok, reason }) => Text::same(format!(" {}{}", if *ok { "ok" } else { "doubt" }, if reason.is_empty() { String::new() } else { format!(" \"{reason}\"") })),
        _ => Text::default(),
    }
}

/// What `run` prints of a scenario: a line a row, the books at the end, and whether it went as
/// written.
pub fn render(m: &Machine, ran: &Ran, lang: Lang) -> String {
    let mut out = String::new();
    let events: Vec<String> = ran.rows.iter().map(|r| format!("{}{}", r.stepped.input.word(), carried(m, &r.stepped.input).get(lang))).collect();
    let bys: Vec<String> = ran.rows.iter().map(|r| r.stepped.input.by().map(|b| tr!("{b}", "by {b}").get(lang).to_string()).unwrap_or_default()).collect();
    let width = ritsu_base::text::width;
    let (we, wb) = (events.iter().map(|e| width(e)).max().unwrap_or(0), bys.iter().map(|b| width(b)).max().unwrap_or(0));
    for (k, r) in ran.rows.iter().enumerate() {
        let pad = |s: &str, w: usize| format!("{s}{}", " ".repeat(w.saturating_sub(width(s))));
        let head = format!("{}  {}  {}", local(m, r.stepped.now), pad(&events[k], we), pad(&bys[k], wb));
        match &r.stepped.answer {
            Ok((t, _)) => {
                let mut tail = place(m, t).get(lang).to_string();
                for th in things(m, r) {
                    tail.push_str(&format!("; {}", th.get(lang)));
                }
                out.push_str(&format!("{head}  -> {tail}\n"));
            }
            Err(refusal) => out.push_str(&format!("{head}  -> {}: {}\n", tr!("受け付けません", "not taken").get(lang), refusal.text.get(lang))),
        }
    }
    for b in &ran.books {
        let accounts: Vec<String> = b
            .accounts
            .iter()
            .map(|(a, args, unit, scale, [posted, held_in, held_out])| {
                let name = if args.is_empty() { a.clone() } else { format!("{a}({})", args.join(", ")) };
                let mut s = format!("{name} {}", amount(*posted, unit, *scale));
                let held: Vec<Text> = [(held_out, tr!("出る仮押さえ", "held going out")), (held_in, tr!("入る仮押さえ", "held coming in"))]
                    .into_iter()
                    .filter(|(n, _)| **n != 0)
                    .map(|(n, w)| {
                        let a = amount(*n, unit, *scale);
                        tr!("{} {a}", "{a} {}", w.ja; w.en)
                    })
                    .collect();
                if !held.is_empty() {
                    let h = Text::join(&held, "、", ", ");
                    s.push_str(tr!("（{}）", " ({})", h.ja; h.en).get(lang));
                }
                s
            })
            .collect();
        let name = &b.book;
        if !accounts.is_empty() {
            out.push_str(&format!("{}: {}\n", tr!("帳簿 `{name}` の終わりの残高", "the book `{name}` at the end").get(lang), accounts.join(if lang == Lang::Ja { "、" } else { ", " })));
        }
        for (t, key, state) in &b.holds {
            out.push_str(&format!("  {}\n", tr!("仮押さえ `{t}`（{}）: {state}", "the hold `{t}` ({}): {state}", key.join("、"); key.join(", ")).get(lang)));
        }
    }
    let (name, n) = (&ran.name, ran.rows.len());
    let rows = if n == 1 { "1 row".to_string() } else { format!("{n} rows") };
    match (&ran.stop, ran.ticket()) {
        (Some(_), _) => out.push_str(&format!("{}\n", tr!("シナリオ `{name}`: 書いたとおりに進みません", "scenario `{name}`: not as written").get(lang))),
        (None, Some(t)) if !t.status.ended() => {
            let clocks = clocks_said(m, t);
            let st = t.status.word();
            let left = if clocks.is_empty() { tr!("チケットは `{st}` のままで、時計は置いてありません", "the ticket is left `{st}`, with no clock set") } else { tr!("チケットは `{st}` のままです。置いてある時計: {}", "the ticket is left `{st}`; the clocks set: {}", clocks.ja; clocks.en) };
            out.push_str(&format!("{}\n", tr!("シナリオ `{name}`: 書いたとおり（{n} 行）。{}", "scenario `{name}`: as written ({rows}); {}", left.ja; left.en).get(lang)));
        }
        (None, _) => out.push_str(&format!("{}\n", tr!("シナリオ `{name}`: 書いたとおり（{n} 行）", "scenario `{name}`: as written ({rows})").get(lang))),
    }
    out
}

/// A scenario as it ran, for `--format json`: each row with what the machine was given and what it
/// answered, the books at the end, and whether it went as written.
pub fn to_json(m: &Machine, ran: &Ran, lang: Lang) -> Json {
    let rows = ran.rows.iter().map(|r| {
        let mut o = vec![("line", Json::int(r.line as u32)), ("at", Json::str(local(m, r.stepped.now)))];
        o.push(("step", r.stepped.to_json(&m.names)));
        o.push((
            "books",
            Json::arr(r.booked.iter().map(|b| Json::obj([("do", Json::str(&b.op)), ("book", Json::str(m.names.used(&b.book))), ("transfer", Json::str(&b.transfer)), ("args", crate::engine::state::values_json(&b.args)), ("outcome", Json::str(&b.outcome))]))),
        ));
        Json::obj(o)
    });
    let books = ran.books.iter().map(|b| {
        Json::obj([
            ("book", Json::str(m.names.used(&b.book))),
            (
                "accounts",
                Json::arr(b.accounts.iter().map(|(a, args, unit, scale, [posted, held_in, held_out])| {
                    Json::obj([
                        ("account", Json::str(a)),
                        ("args", Json::arr(args.iter().map(Json::str))),
                        ("unit", Json::str(unit)),
                        ("scale", Json::int(*scale)),
                        ("posted", Json::Int(*posted)),
                        ("held_in", Json::Int(*held_in)),
                        ("held_out", Json::Int(*held_out)),
                    ])
                })),
            ),
            ("holds", Json::arr(b.holds.iter().map(|(t, key, state)| Json::obj([("transfer", Json::str(t)), ("key", Json::arr(key.iter().map(Json::str))), ("state", Json::str(state))])))),
        ])
    });
    let stop = match &ran.stop {
        None => Json::Null,
        Some(s) => Json::obj([("code", Json::str(s.code)), ("line", Json::int(s.at.line as u32)), ("column", Json::int(s.at.col as u32)), ("message", Json::str(s.text.get(lang))), ("notes", Json::arr(s.notes.iter().map(|n| Json::str(n.get(lang)))))]),
    };
    let ticket = ran.ticket();
    Json::obj([
        ("scenario", Json::str(&ran.name)),
        ("as_written", Json::Bool(ran.stop.is_none())),
        // the status and the clocks in the words the ticket of a row is written in
        ("status", Json::opt_str(ticket.map(|t| crate::lower::shape::word(t.status.word())))),
        ("stage", Json::opt_str(ticket.and_then(|t| t.stage_name()).map(|s| m.names.stage(s)))),
        ("timers", Json::arr(ticket.into_iter().flat_map(|t| t.timers()).map(|(w, stage, at)| Json::obj([("timer", Json::str(crate::lower::shape::word(w))), ("stage", Json::opt_str(stage.map(|s| m.names.stage(s)))), ("at", Json::str(local(m, at)))])))),
        ("rows", Json::arr(rows)),
        ("books", Json::arr(books)),
        ("stopped", stop),
    ])
}

/// E602, E603 and E604 for the scenarios of a checked request (DESIGN 10.2): each run, and the
/// first row that does not go as written said where it is written.
pub fn check(m: &Machine, org: &Org, out: &mut Vec<crate::diag::Diag>) {
    for sc in &m.s.file.scenarios {
        if let Some(stop) = run(m, org, sc).stop {
            out.push(diag(m, &stop));
        }
    }
}

/// A stop as a diagnostic of the request's file.
pub fn diag(m: &Machine, stop: &Stop) -> crate::diag::Diag {
    let mut d = m.s.err(stop.code, stop.at, upper_first(&stop.text));
    for n in &stop.notes {
        d = d.note(n.clone());
    }
    d
}

fn upper_first(t: &Text) -> Text {
    let mut c = t.en.chars();
    let en = match c.next() {
        Some(f) if !t.en.starts_with('`') => f.to_uppercase().collect::<String>() + c.as_str(),
        _ => t.en.clone(),
    };
    Text::new(t.ja.clone(), en)
}
