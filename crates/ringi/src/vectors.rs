//! The vectors (DESIGN 10.4): transitions of the ticket machine, each with what it was given (the
//! ticket before, the event, the part of the organization, the time) and what it answered, for the
//! code a request is generated to, and a server, to answer the same.
//!
//! Every vector is a transition the reference interpreter made ([`scenario::step`]). What is given
//! next is picked from the ticket the machine answered with (who its stage waits for, which clocks
//! it set, the stages it passed), and where an edge is looked for (the largest number a field
//! takes, the most rows), the machine is asked value by value. Nothing here walks a route, counts a
//! deadline or holds a range of its own: a second walk of the machine would come to disagree with
//! the first, and the vectors would then say what no machine does.
//!
//! They start from the scenarios of the request, which hold a requester, a form the request takes
//! and the events of one way through the route:
//!
//! - the rows of each scenario, as they ran;
//! - at each stage a scenario comes to: the approvals and the rejections one by one (where a quorum
//!   is reached, and where it no longer can be), what the requester may and may not do, a send-back
//!   and what comes of it (to a stage that waits for a later one too, and the approvals after it),
//!   a pause, the approval before it taken back, what an agent and a flow answer, what an
//!   administrator does, and the clocks one after another with nobody answering; where a stage
//!   waits for the one the ticket is at, the approval of the one an administrator adds or hands
//!   it over to, and of the one it goes up to when it is late, with the approvals after them;
//! - what people write on the ticket (DESIGN 7.2), each once for the request: at the first stage a
//!   scenario comes to, a comment by the one the stage waits for and by the requester, one with
//!   nothing in it, the longest a comment holds and one character past it, an approval and a
//!   send-back each with a comment and an approval with one too long, and a comment at the very
//!   time a clock rings; a comment on a ticket paused and ended; on a ticket sent back, by the one
//!   who sent it back and by the requester, and on one the requester took back themselves; on a
//!   ticket made in another's name, and, sent back, the requester's; the first scenario's request
//!   made with a comment, and with one too long;
//! - where the request asks a reason with a rejection and a send-back (DESIGN 7.2), each of them
//!   given with one wherever it is given above, and, once for the request, at the first stage a
//!   scenario comes to, each given with none, with an empty comment and with white space alone
//!   (U+0085 alone among it), and with a character alone that is no white space though the
//!   languages' own functions may take it for some (U+FEFF, U+001C);
//! - each event a person or a flow gives, made on the revision of the ticket before the first
//!   transition of a scenario that moves it on (to one that has not ended, where a scenario has
//!   one), and an approval that says no revision; a comment, which says none, and an approval with
//!   a comment too long, made on the revision before;
//! - a request sent back and made again until the request allows no more;
//! - what a resubmit and an edit make empty (DESIGN 7.3), each once for the request: on the ticket
//!   a scenario first sends back, a resubmit that makes each field the requester gives empty, one
//!   that names a field the form does not have, one it computes, one of the approvers', and one it
//!   gives a value too, and one that gives each choice each of its other values, which drops the
//!   fields the form then does not show; at the first stage a scenario comes to, the same of an
//!   edit by the one the stage waits for, where the approvers give or edit a field;
//! - the same request made before closed days, and its clocks;
//! - each person of the organization as the requester, the proxies on the first and the last day
//!   of each and the days around them, a request taken back and discarded by the one who made it
//!   in another's name, the approvals of the proxies of approvers and what one does in the seat
//!   (a send-back, a rejection, a pause and its end, an edit, an approval taken back), a stage
//!   done with a proxy's approval (`same approver approves once`), and a ticket that lives through
//!   a change of version;
//! - the edges of the numbers a requester writes, of what the form works out of them, and of the
//!   number of rows: the last value taken and the first refused;
//! - each field the form asks for on a condition (shown on one with no `optional`, or `required
//!   when`), left empty in the form of the first scenario that fills it in;
//! - the rows of the examples, laid over a scenario's form.

use crate::ast::{Expr, ExprKind, Field, Reasoned, TicketLine, TypeExpr};
use crate::engine::{COMMENT_MOST, Event, Machine, PartState, Phase, Status, Submit, Ticket, What};
use crate::org::{Org, ProxyKind};
use crate::scenario::{self, Input, Ran, Stepped, TICKET};
use ritsu_base::json::Json;
use ritsu_ports::{Value, Values};
use std::collections::HashSet;

const DAY: i64 = 86_400;

/// What a rejection and a send-back are given with where the request asks a reason with them.
const REASON: &str = "The reason for it";

/// A comment of `n` characters, each a character of four bytes in UTF-8 and two units in UTF-16:
/// what holds the count of a comment to the characters, in whichever language counts it.
fn long(n: usize) -> String {
    "𠮷".repeat(n)
}

/// One vector: where it comes from, in words, and the transition.
#[derive(Clone, Debug)]
pub struct Vector {
    pub from: String,
    pub stepped: Stepped,
}

impl Vector {
    /// The line of the JSON Lines: the request, where the vector comes from, and the transition
    /// ([`Stepped::to_json`]).
    pub fn to_json(&self, m: &Machine) -> Json {
        let mut o = vec![("request".to_string(), Json::str(m.s.file.name.ascii())), ("from".to_string(), Json::str(&self.from))];
        if let Json::Obj(pairs) = self.stepped.to_json(&m.names) {
            o.extend(pairs);
        }
        Json::Obj(o)
    }

    /// The vector in one line, for a person: `scenario a, line 12: approve by e1101 at 2026-11-16
    /// 15:00 -> second, in progress`.
    pub fn listed(&self, m: &Machine) -> String {
        let s = &self.stepped;
        let by = s.input.by().map(|b| format!(" by {b}")).unwrap_or_default();
        let answer = match &s.answer {
            Ok((t, _)) => match t.stage_name() {
                Some(st) => format!("{st}, {}", t.status.word()),
                None => t.status.word().to_string(),
            },
            Err(r) => format!("refused ({})", r.kind),
        };
        format!("{}: {}{by} at {} -> {answer}", self.from, s.input.word(), scenario::local(m, s.now))
    }
}

/// What the vectors of a request start from: the request a scenario makes.
struct Seed {
    scenario: String,
    requester: String,
    by: Option<String>,
    form: Values,
    at: i64,
}

/// A place of the form a requester writes a number at: a field, or a field of the first row of a
/// `rows` field.
struct Spot<'f> {
    rows: Option<&'f Field>,
    f: &'f Field,
}

impl Spot<'_> {
    fn name(&self) -> String {
        match self.rows {
            Some(r) => format!("{}.{}", r.name.text, self.f.name.text),
            None => self.f.name.text.clone(),
        }
    }

    fn get(&self, form: &Values) -> Option<i128> {
        let v = match self.rows {
            None => value(form, self.f.name.ascii()),
            Some(r) => match value(form, r.name.ascii()) {
                Some(Value::List(rows)) => rows.first().and_then(|row| value(row, self.f.name.ascii())),
                _ => None,
            },
        };
        match v {
            Some(Value::Int(k)) => Some(*k),
            _ => None,
        }
    }

    fn put(&self, form: &Values, k: i128) -> Values {
        let mut out = form.clone();
        match self.rows {
            None => put(&mut out, self.f.name.ascii(), Value::Int(k)),
            Some(r) => {
                if let Some((_, Value::List(rows))) = out.iter_mut().find(|(n, _)| n == r.name.ascii())
                    && let Some(row) = rows.first_mut()
                {
                    put(row, self.f.name.ascii(), Value::Int(k));
                }
            }
        }
        out
    }

    /// The value as the language writes it: `1_000_000JPY`.
    fn shown(&self, k: i128) -> String {
        crate::taken::shown(self.f, &Value::Int(k))
    }

    /// The ends the field writes, counted in its step: where an edge is looked for first.
    fn written(&self) -> (Option<i128>, Option<i128>) {
        let TypeExpr::Num(u) = &self.f.ty else { return (None, None) };
        let st = crate::lower::words::step(u);
        let (lo, hi) = crate::lower::words::ends(self.f);
        let steps = |x: Option<ritsu_units::Rat>| x.map(|x| x.div(st)).filter(|x| x.is_int()).map(|x| x.num);
        (steps(lo), steps(hi))
    }
}

fn value<'v>(vals: &'v Values, name: &str) -> Option<&'v Value> {
    vals.iter().find(|(n, _)| n == name).map(|(_, v)| v)
}

fn put(vals: &mut Values, name: &str, v: Value) {
    match vals.iter_mut().find(|(n, _)| n == name) {
        Some(x) => x.1 = v,
        None => vals.push((name.to_string(), v)),
    }
}

/// The field a vector names in `clear` and gives a value too, with the value the ticket `t` holds
/// of it: the first the event makes empty alone (`emptied`) where the ticket holds a value, so that
/// only `clear` naming a field given a value refuses it (one the form asks for would be refused
/// empty too); else the first of `fields` the ticket holds a value of.
fn both(t: &Ticket, emptied: &[&Field], fields: &[&Field]) -> Option<(String, Value)> {
    let held = |f: &&Field| Some((f.name.ascii().to_string(), t.value(f.name.ascii())?.clone()));
    emptied.iter().find_map(held).or_else(|| fields.iter().find_map(held))
}

/// The ones the ticket's stage waits for: the approvers of its open parts who have not answered,
/// in the order the ticket holds them.
fn waiting(t: &Ticket) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if t.status != Status::InProgress {
        return out;
    }
    for p in t.stage().map(|s| s.parts.as_slice()).unwrap_or_default() {
        if p.state != PartState::Open {
            continue;
        }
        for a in &p.approvers {
            if !p.answers.iter().any(|x| x.seat == *a) && !out.contains(a) {
                out.push(a.clone());
            }
        }
    }
    out
}

/// The first clock the ticket has set: its event, and its time.
fn clock(t: &Ticket) -> Option<(Event, i64)> {
    let (w, _, at) = *t.timers().first()?;
    let ev = match w {
        "remind" => Event::Remind,
        "late" => Event::Late,
        "resubmit late" => Event::ResubmitLate,
        _ => Event::PauseEnds,
    };
    Some((ev, at))
}

/// `want`, or a minute before the ticket's first clock when that comes sooner: a time a person can
/// act at without a clock's event coming first.
fn before_clocks(t: &Ticket, want: i64) -> i64 {
    clock(t).map_or(want, |(_, at)| want.min(at - 60))
}

/// The last stage before the one the ticket is at that people answered: one a part of which waits
/// for a later stage too, where it started.
fn passed(t: &Ticket) -> Option<&crate::engine::Stage> {
    t.stages[..t.at?].iter().rev().find(|s| s.phase == Phase::Answered || (s.phase == Phase::Deferred && s.started.is_some()))
}

/// The first stage before the one the ticket is at a part of which waits for a later stage
/// (`same approver approves once, at the last stage`).
fn waits(t: &Ticket) -> Option<&crate::engine::Stage> {
    t.deferred(t.at?).first().map(|(a, _)| &t.stages[*a])
}

struct Maker<'x, 's, 'a> {
    m: &'x Machine<'s, 'a>,
    org: &'x Org,
    out: Vec<Vector>,
    seen: HashSet<String>,
    /// What the request does not have (a pause, a retraction, a send-back to a stage): the machine
    /// said so once in the scenario, and is not asked again.
    absent: HashSet<&'static str>,
    /// The events given on a revision the ticket has moved on from, by their words: each once for
    /// the request.
    stale: HashSet<&'static str>,
    /// The comments given, by where they are given: each once for the request.
    said: HashSet<&'static str>,
    /// The resubmits and the edits that make fields empty, by the event: each once for the request.
    emptied: HashSet<&'static str>,
    /// The approvers' fields shown or asked for on a condition, given a value in an edit, by the
    /// field and whether the edit was taken: each once for the request.
    conditioned: HashSet<String>,
}

impl Maker<'_, '_, '_> {
    /// A transition written down, once.
    fn keep(&mut self, from: &str, st: Stepped) {
        if self.seen.insert(st.to_json(&self.m.names).compact()) {
            self.out.push(Vector { from: from.to_string(), stepped: st });
        }
    }

    /// The reason a rejection or a send-back is given with where the request asks one with it
    /// (`comment required for`, DESIGN 7.2), so that the ticket moves on as it would without the
    /// line; none for any other input.
    fn reason(&self, input: &Input) -> Option<String> {
        let op = match input {
            Input::Event(Event::Reject { .. }) => Reasoned::Reject,
            Input::Event(Event::SendBack { .. }) => Reasoned::SendBack,
            _ => return None,
        };
        self.m.s.file.ticket.as_ref().is_some_and(|t| t.reasoned().contains(&op)).then(|| REASON.to_string())
    }

    /// One transition asked of the machine and written down: the ticket after it, when it is
    /// taken. Nothing is written where no version of the organization is in effect at the time.
    fn give(&mut self, from: &str, before: Option<&Ticket>, input: Input, now: i64) -> Option<Ticket> {
        let reason = self.reason(&input);
        let st = scenario::step_with(self.m, self.org, before, input, reason, now).ok()?;
        let after = st.answer.as_ref().ok().map(|(t, _)| t.clone());
        self.keep(from, st);
        after
    }

    fn event(&mut self, from: &str, t: &Ticket, ev: Event, now: i64) -> Option<Ticket> {
        self.give(from, Some(t), Input::Event(ev), now)
    }

    /// A person's event given with what they wrote, written down: the ticket after it, when it is
    /// taken.
    fn said(&mut self, from: &str, t: &Ticket, ev: Event, comment: &str, now: i64) -> Option<Ticket> {
        let st = scenario::step_with(self.m, self.org, Some(t), Input::Event(ev), Some(comment.to_string()), now).ok()?;
        let after = st.answer.as_ref().ok().map(|(t, _)| t.clone());
        self.keep(from, st);
        after
    }

    /// [`Self::said`], the first time comments are given where `what` says, for the request.
    fn said_once(&mut self, what: &'static str, from: &str, t: &Ticket, ev: Event, comment: &str, now: i64) {
        if self.said.insert(what) {
            self.said(from, t, ev, comment, now);
        }
    }

    /// What people write on the ticket `t`, which a scenario left at its first stage at `now`
    /// (DESIGN 7.2): a comment by the one the stage waits for and by the requester, one with
    /// nothing in it, the longest a comment holds and one character past it, and an approval and a
    /// send-back each with a comment, and an approval with one too long.
    fn comments(&mut self, pre: &str, t: &Ticket, now: i64) {
        if !self.said.insert("at a stage") {
            return;
        }
        let from = format!("{pre}: comments");
        let rq = t.requester.clone();
        let who = waiting(t);
        let by = who.first().cloned().unwrap_or_else(|| rq.clone());
        self.said(&from, t, Event::Comment { by: by.clone() }, "A question about the request", now);
        self.said(&from, t, Event::Comment { by: rq.clone() }, "An answer", now);
        self.said(&from, t, Event::Comment { by: by.clone() }, "", now);
        self.said(&from, t, Event::Comment { by: by.clone() }, &long(COMMENT_MOST), now);
        self.said(&from, t, Event::Comment { by: by.clone() }, &long(COMMENT_MOST + 1), now);
        if let Some(a) = who.first() {
            self.said(&from, t, Event::Approve { by: a.clone() }, "Approved as written", now);
            self.said(&from, t, Event::SendBack { by: a.clone(), to: None }, "Please correct the request", now);
            self.said(&from, t, Event::Approve { by: a.clone() }, &long(COMMENT_MOST + 1), now);
        }
    }

    /// Where the request asks a reason with a rejection and a send-back (DESIGN 7.2), once for the
    /// request: each of them by the one the stage of the ticket `t` waits for, given with no comment,
    /// with an empty one and with white space alone, which the machine refuses (`comment`) before
    /// it asks who gives it; and with a character alone that is no white space, a reason the machine
    /// takes, where a language's own `trim` or `strip` takes one off (U+FEFF) or the other (U+001C).
    /// Each given with a reason is in the other vectors ([`Self::reason`]).
    fn reasons(&mut self, pre: &str, t: &Ticket, now: i64) {
        let asked = self.m.s.file.ticket.as_ref().map(|x| x.reasoned()).unwrap_or_default();
        let Some(by) = waiting(t).first().cloned() else { return };
        if asked.is_empty() || !self.said.insert("no reason") {
            return;
        }
        let (none, some) = (format!("{pre}: a reason asked, and none given"), format!("{pre}: a reason asked, given in a character that is no white space"));
        for op in asked {
            let ev = match op {
                Reasoned::Reject => Event::Reject { by: by.clone() },
                Reasoned::SendBack => Event::SendBack { by: by.clone(), to: None },
            };
            for (from, c) in [(&none, None), (&none, Some("")), (&none, Some(" \u{3000}\n")), (&none, Some("\u{85}")), (&some, Some("\u{feff}")), (&some, Some("\u{1c}"))] {
                if let Ok(st) = scenario::step_with(self.m, self.org, Some(t), Input::Event(ev.clone()), c.map(str::to_string), now) {
                    self.keep(from, st);
                }
            }
        }
    }

    /// An event of a kind the request may not have (`what`): asked until the machine says the
    /// request has none (a refusal of the kind `event`).
    fn once(&mut self, what: &'static str, from: &str, t: &Ticket, ev: Event, now: i64) -> Option<Ticket> {
        if self.absent.contains(what) {
            return None;
        }
        let input = Input::Event(ev);
        let reason = self.reason(&input);
        let st = scenario::step_with(self.m, self.org, Some(t), input, reason, now).ok()?;
        if st.answer.as_ref().is_err_and(|r| r.kind == "event") {
            self.absent.insert(what);
        }
        let after = st.answer.as_ref().ok().map(|(t, _)| t.clone());
        self.keep(from, st);
        after
    }

    fn request(seed: &Seed, form: Values) -> Input {
        Input::Submit(Submit { id: TICKET.to_string(), requester: seed.requester.clone(), by: seed.by.clone(), form, attempt: crate::engine::attempt(1) })
    }

    fn submit(&mut self, from: &str, seed: &Seed, form: Values, at: i64) -> Option<Ticket> {
        self.give(from, None, Self::request(seed, form), at)
    }

    /// Whether the machine takes the seed's request with the form: asked, and not written down.
    fn taken(&self, seed: &Seed, form: &Values) -> bool {
        scenario::step(self.m, self.org, None, Self::request(seed, form.clone()), seed.at).is_ok_and(|s| s.answer.is_ok())
    }

    /// The time of day of `at`, on the day `day`.
    fn on(&self, day: i64, at: i64) -> i64 {
        self.m.instant(day, 0) + (at - self.m.instant(self.m.day(at), 0))
    }

    fn closed(&self, day: i64) -> bool {
        self.m.calendars.first().is_some_and(|(_, c)| c.closed.contains(&day))
    }

    /// The last day before `day` the request's calendar is open on.
    fn open_before(&self, day: i64) -> i64 {
        (1..=31).map(|k| day - k).find(|d| !self.closed(*d)).unwrap_or(day - 1)
    }

    /// The first day from `day` on the request's calendar is open on.
    fn open_from(&self, day: i64) -> i64 {
        (0..=31).map(|k| day + k).find(|d| !self.closed(*d)).unwrap_or(day)
    }

    /// The clocks of the ticket that ring by `until`, given one after another at their times, at
    /// most `most` of them: the ticket as they leave it.
    fn rings(&mut self, from: &str, t: &Ticket, until: i64, most: usize) -> Ticket {
        let mut cur = t.clone();
        for _ in 0..most {
            if cur.status.ended() {
                break;
            }
            let Some((ev, at)) = clock(&cur).filter(|(_, at)| *at <= until) else { break };
            match self.event(from, &cur, ev, at) {
                Some(n) => cur = n,
                None => break,
            }
        }
        cur
    }

    /// The ones the stage waits for answer one by one, until the ticket leaves the stage: where
    /// the answers come to a quorum, and where they no longer can.
    fn answers(&mut self, from: &str, t: &Ticket, now: i64, yes: bool) {
        let mut cur = t.clone();
        let mut refused: Vec<String> = Vec::new();
        for _ in 0..32 {
            let Some(by) = waiting(&cur).into_iter().find(|a| !refused.contains(a)) else { break };
            let ev = if yes { Event::Approve { by: by.clone() } } else { Event::Reject { by: by.clone() } };
            match self.event(from, &cur, ev, now) {
                Some(n) => {
                    let moved = n.stage_name() != cur.stage_name() || n.status != cur.status;
                    cur = n;
                    if moved {
                        break;
                    }
                }
                None => refused.push(by),
            }
        }
    }

    /// Every stage approved in turn, as far as people answer: with `proxies`, by the proxy of each
    /// approver who has one that day. The ticket the first approval a proxy gave was given to, and
    /// the proxy, where one answered.
    fn through(&mut self, from: &str, t: &Ticket, now: i64, proxies: bool) -> Option<(Ticket, String)> {
        let mut cur = t.clone();
        let mut refused: Vec<String> = Vec::new();
        let day = self.m.day(now);
        let mut by_proxy = None;
        for _ in 0..40 {
            let Some(a) = waiting(&cur).into_iter().find(|a| !refused.contains(a)) else { break };
            let proxy = self.org.proxies.iter().find(|x| proxies && x.kind == ProxyKind::Approve && x.person == a && self.m.proxy_on(x, day)).map(|x| x.proxy.clone());
            let before = cur.clone();
            match self.event(from, &cur, Event::Approve { by: proxy.clone().unwrap_or_else(|| a.clone()) }, now) {
                Some(n) => {
                    if n.stage_name() != cur.stage_name() {
                        refused.clear();
                    }
                    if by_proxy.is_none() {
                        by_proxy = proxy.map(|p| (before, p));
                    }
                    cur = n;
                }
                None => refused.push(a),
            }
        }
        by_proxy
    }

    /// What a resubmit makes empty on the ticket `back`, which is with the requester (DESIGN 7.3):
    /// each field the requester gives (one the form asks for on every request is refused, as one not
    /// filled in), a field the form does not have, one it computes, one of the approvers', and one
    /// given a value too ([`both`]); and each other value of each choice the requester gives, which
    /// drops the fields the form then does not show where the ticket holds a value of them, and, once
    /// for each choice, the first of those values again with a field it drops given its value, which
    /// is refused as a value of a field the form does not show (what the event gives is not dropped).
    fn resubmits_emptied(&mut self, from: &str, back: &Ticket, now: i64) {
        let rq = back.requester.clone();
        let again = |form: Values, clear: Vec<String>| Event::Resubmit { by: rq.clone(), form, clear, attempt: crate::engine::attempt(back.round + 1) };
        let fields = self.m.s.fields();
        let mine: Vec<&Field> = fields.iter().filter(|f| f.computed.is_none() && f.approvers_only.is_none()).collect();
        let mut emptied = Vec::new();
        for f in &mine {
            if self.event(from, back, again(Values::new(), vec![f.name.ascii().to_string()]), now).is_some() {
                emptied.push(*f);
            }
        }
        self.event(from, back, again(Values::new(), vec!["no_such_field".to_string()]), now);
        for f in [fields.iter().find(|f| f.computed.is_some()), fields.iter().find(|f| f.approvers_only.is_some())].into_iter().flatten() {
            self.event(from, back, again(Values::new(), vec![f.name.ascii().to_string()]), now);
        }
        if let Some((n, v)) = both(back, &emptied, &mine) {
            self.event(from, back, again(vec![(n.clone(), v)], vec![n]), now);
        }
        for f in fields.iter().filter(|f| f.ty == TypeExpr::Choice && f.computed.is_none() && f.approvers_only.is_none()) {
            let held = back.value(f.name.ascii()).cloned();
            let mut given = false;
            for o in &f.options {
                let v = Value::Enum(o.name.ascii().to_string());
                if held.as_ref() == Some(&v) {
                    continue;
                }
                let Some(after) = self.event(from, back, again(vec![(f.name.ascii().to_string(), v.clone())], Vec::new()), now) else { continue };
                // a value given for a field the choice hides is refused, not dropped
                let dropped = mine.iter().map(|g| g.name.ascii()).find(|g| back.value(g).is_some() && after.value(g).is_none());
                if let (false, Some(g), Some(w)) = (given, dropped, dropped.and_then(|g| back.value(g)).cloned()) {
                    self.event(from, back, again(vec![(f.name.ascii().to_string(), v), (g.to_string(), w)], Vec::new()), now);
                    given = true;
                }
            }
        }
    }

    /// What an edit by `by`, an approver of the stage the ticket `t` is at, makes empty (DESIGN
    /// 7.3), where the approvers give a field or `approvers edit` names one: each field they give
    /// or may edit, one of the requester's they may not, a field the form does not have, and one
    /// given a value too ([`both`]).
    fn edits_emptied(&mut self, from: &str, t: &Ticket, by: &str, now: i64) {
        let editable = self.m.line(|l| if let TicketLine::ApproversEdit { fields, .. } = l { Some(fields.iter().map(|w| w.text.clone()).collect::<Vec<_>>()) } else { None }).unwrap_or_default();
        let fields = self.m.s.fields();
        let theirs: Vec<&Field> = fields.iter().filter(|f| f.computed.is_none() && (f.approvers_only.is_some() || editable.contains(&f.name.text))).collect();
        if theirs.is_empty() {
            return;
        }
        let edit = |fields: Values, clear: Vec<String>| Event::Edit { by: by.to_string(), fields, clear };
        let mut emptied = Vec::new();
        for f in &theirs {
            if self.event(from, t, edit(Values::new(), vec![f.name.ascii().to_string()]), now).is_some() {
                emptied.push(*f);
            }
        }
        if let Some(f) = fields.iter().find(|f| f.computed.is_none() && f.approvers_only.is_none() && !editable.contains(&f.name.text)) {
            self.event(from, t, edit(Values::new(), vec![f.name.ascii().to_string()]), now);
        }
        self.event(from, t, edit(Values::new(), vec!["no_such_field".to_string()]), now);
        if let Some((n, v)) = both(t, &emptied, &theirs) {
            self.event(from, t, edit(vec![(n.clone(), v)], vec![n]), now);
        }
    }

    /// An edit by `by`, an approver of the stage the ticket `t` is at, that gives a value to a field
    /// of the approvers' the form shows or asks for on a condition (DESIGN 7.3): taken where the
    /// form shows the field for the ticket, refused where it does not. The value is the one the
    /// ticket holds, else a text for a text field. Each field once each way for the request.
    fn edits_conditioned(&mut self, from: &str, t: &Ticket, by: &str, now: i64) {
        let editable = self.m.line(|l| if let TicketLine::ApproversEdit { fields, .. } = l { Some(fields.iter().map(|w| w.text.clone()).collect::<Vec<_>>()) } else { None }).unwrap_or_default();
        let given: Vec<(String, Value)> = self
            .m
            .s
            .fields()
            .iter()
            .filter(|f| f.computed.is_none() && (f.shown_when.is_some() || f.required_when.is_some()) && (f.approvers_only.is_some() || editable.contains(&f.name.text)))
            .filter_map(|f| {
                let v = t.value(f.name.ascii()).cloned().or_else(|| matches!(f.ty, TypeExpr::Text | TypeExpr::LongText).then(|| Value::Str("A note".to_string())))?;
                Some((f.name.ascii().to_string(), v))
            })
            .collect();
        for (n, v) in given {
            let ev = Event::Edit { by: by.to_string(), fields: vec![(n.clone(), v)], clear: Vec::new() };
            let Ok(st) = scenario::step(self.m, self.org, Some(t), Input::Event(ev), now) else { continue };
            if self.conditioned.insert(format!("{n} {}", st.answer.is_ok())) {
                self.keep(from, st);
            }
        }
    }

    /// Sent back to the requester and made again, until the request allows no more.
    fn limit(&mut self, from: &str, t: &Ticket, now: i64) {
        let (mut cur, mut now) = (t.clone(), now);
        for _ in 0..12 {
            let Some(by) = waiting(&cur).first().cloned() else { break };
            let Some(back) = self.event(from, &cur, Event::SendBack { by, to: None }, now) else { break };
            if back.status != Status::WithRequester {
                break;
            }
            now = before_clocks(&back, now + DAY);
            let Some(again) = self.event(from, &back, Event::Resubmit { by: cur.requester.clone(), form: Values::new(), clear: Vec::new(), attempt: crate::engine::attempt(back.round + 1) }, now) else { break };
            cur = again;
        }
    }

    /// What can happen to the ticket `t`, which a scenario left at a stage at `now`. `first` for
    /// the first stage of a scenario, where what is asked once a scenario is asked.
    fn at_stage(&mut self, pre: &str, t: &Ticket, now: i64, first: bool) {
        let rq = t.requester.clone();
        let who = waiting(t);
        self.answers(&format!("{pre}: approvals"), t, now, true);
        self.answers(&format!("{pre}: rejections"), t, now, false);
        // what the requester may do, and may not; taken back to the requester by themselves, nobody
        // sent it back, and their comment is told to nobody for it
        let from = format!("{pre}: the requester");
        self.event(&from, t, Event::Approve { by: rq.clone() }, now);
        if let Some(back) = self.event(&from, t, Event::Withdraw { by: rq.clone() }, now).filter(|b| b.status == Status::WithRequester) {
            self.said_once("withdrawn", &from, &back, Event::Comment { by: rq.clone() }, "I will correct it", now);
        }
        if let Some(by) = who.first() {
            // back to the requester, and what comes of it; back to a stage it passed. Once for the
            // request, the one who sent it back writes to the requester, and the requester asks
            // them back (DESIGN 7.4)
            let from = format!("{pre}: sent back");
            if let Some(back) = self.event(&from, t, Event::SendBack { by: by.clone(), to: None }, now) {
                self.event(&from, &back, Event::Resubmit { by: rq.clone(), form: Values::new(), clear: Vec::new(), attempt: crate::engine::attempt(back.round + 1) }, before_clocks(&back, now + DAY));
                if back.status == Status::WithRequester && self.emptied.insert("resubmit") {
                    self.resubmits_emptied(&format!("{pre}: made again, with fields made empty"), &back, before_clocks(&back, now + DAY));
                }
                self.event(&from, &back, Event::Discard { by: rq.clone() }, now);
                if self.said.insert("sent back") {
                    self.said(&from, &back, Event::Comment { by: by.clone() }, "Please correct it", now);
                    self.said(&from, &back, Event::Comment { by: rq.clone() }, "Which part, please?", now);
                }
                self.rings(&from, &back, i64::MAX, 1);
            }
            if let Some(to) = passed(t).map(|s| s.name.clone()) {
                self.once("send back to a stage", &from, t, Event::SendBack { by: by.clone(), to: Some(to) }, now);
            }
            // back to a stage that waits for a later one: it is answered there, and what comes of it
            if let Some(to) = waits(t).map(|s| s.name.clone())
                && let Some(back) = self.once("send back to a stage", &from, t, Event::SendBack { by: by.clone(), to: Some(to) }, now)
            {
                self.through(&from, &back, now, false);
            }
            // fields an approver makes empty
            if self.emptied.insert("edit") {
                self.edits_emptied(&format!("{pre}: edited, with fields made empty"), t, by, now);
            }
            self.edits_conditioned(&format!("{pre}: edited, a field shown on a condition"), t, by, now);
            // a pause, ended by the one who began it, and by its clock
            let from = format!("{pre}: paused");
            if let Some(paused) = self.once("pause", &from, t, Event::Pause { by: by.clone() }, now) {
                self.event(&from, &paused, Event::Resume { by: by.clone() }, before_clocks(&paused, now + DAY));
                self.said_once("paused", &from, &paused, Event::Comment { by: rq.clone() }, "When is it decided?", now);
                self.rings(&from, &paused, i64::MAX, 1);
            }
        }
        // the approval before the stage, taken back
        let last = passed(t).and_then(|s| s.parts.iter().flat_map(|p| &p.answers).rfind(|a| a.yes)).map(|a| a.by.clone());
        if let Some(by) = last {
            self.once("retract", &format!("{pre}: retracted"), t, Event::Retract { by }, now);
        }
        // what an agent answers, and a flow
        let parts = t.stage().map(|s| s.parts.as_slice()).unwrap_or_default();
        let from = format!("{pre}: flows");
        if parts.iter().any(|p| matches!(p.state, PartState::Agent(_))) {
            self.event(&from, t, Event::AgentAnswered { ok: true, reason: String::new() }, now);
            self.event(&from, t, Event::AgentAnswered { ok: false, reason: "a doubt".to_string() }, now);
            self.event(&from, t, Event::FlowFailed, now);
        } else if who.is_empty() {
            self.event(&from, t, Event::NoteReady { outputs: Values::new() }, now);
            self.event(&from, t, Event::FlowFailed, now);
        }
        // a stage before this one waits for the approval given here (`same approver approves once,
        // at the last stage`): what comes of the stage being answered by another
        let awaited = t.at.is_some_and(|i| t.deferred(i).iter().any(|(a, k)| t.stages[*a].parts[*k].deferred().is_some_and(|(_, to)| Some(to) == t.stage_name())));
        // what an administrator does: an approver added, a place handed over; and, where a stage
        // waits for this one, the approval of the one added or handed to, and the ones after it
        if first || awaited || parts.iter().any(|p| p.state == PartState::Stopped) {
            let from = format!("{pre}: an administrator");
            let other = self.org.in_effect(self.m.day(now)).and_then(|v| v.people.iter().map(|p| &p.id).find(|id| **id != rq && !who.contains(id)).cloned());
            if let Some(other) = other {
                let added = self.event(&from, t, Event::AddApprover { by: None, approver: other.clone(), part: None }, now);
                let handed = who.first().and_then(|a| self.event(&from, t, Event::HandOver { by: None, from: a.clone(), to: other.clone() }, now));
                for after in [added, handed].into_iter().flatten().filter(|_| awaited) {
                    if let Some(n) = self.event(&from, &after, Event::Approve { by: other.clone() }, now) {
                        self.through(&from, &n, now, false);
                    }
                }
                // the same, by the administrator who does it (`by`, the person the history keeps),
                // once for the request
                let admin = self.org.in_effect(self.m.day(now)).and_then(|v| v.people.iter().map(|p| &p.id).find(|id| **id != rq && **id != other && !who.contains(id)).cloned());
                if let Some(admin) = admin.filter(|_| self.said.insert("by an administrator")) {
                    let from = format!("{pre}: an administrator, by name");
                    self.event(&from, t, Event::AddApprover { by: Some(admin.clone()), approver: other.clone(), part: None }, now);
                    if let Some(a) = who.first() {
                        self.event(&from, t, Event::HandOver { by: Some(admin), from: a.clone(), to: other.clone() }, now);
                    }
                }
            }
        }
        // the clocks one after another, with nobody answering
        let from = format!("{pre}: clocks");
        self.rings(&from, t, i64::MAX, 12);
        if awaited {
            // late, the stage goes up, and the one it went up to approves it: the stage that waited
            // starts then
            let from = format!("{pre}: late, and approved where it went up to");
            let mut cur = t.clone();
            for _ in 0..3 {
                let Some((ev, at)) = clock(&cur) else { break };
                let late = matches!(ev, Event::Late);
                let Some(n) = self.event(&from, &cur, ev, at) else { break };
                cur = n;
                if late {
                    if cur.status == Status::InProgress {
                        self.through(&from, &cur, before_clocks(&cur, at + 3600), false);
                    }
                    break;
                }
            }
        }
        if first {
            // a clock given a minute early, and an approval and a comment at the very time a clock
            // rings
            if let Some((ev, at)) = clock(t) {
                self.event(&from, t, ev, at - 60);
                if let Some(by) = who.first() {
                    self.event(&from, t, Event::Approve { by: by.clone() }, at);
                    self.said_once("as a clock rings", &from, t, Event::Comment { by: by.clone() }, "Late, sorry", at);
                }
            }
            self.comments(pre, t, now);
            self.reasons(pre, t, now);
            self.limit(&format!("{pre}: sent back to the limit"), t, now);
        }
    }

    /// Each event a person or a flow gives, made on the ticket `before` and given to `after`, the
    /// ticket a transition moved on to another revision (DESIGN 7.1, 7.2): the machine refuses it
    /// (`stale`) before it asks who gives it and in what state the ticket is. Each kind once for the
    /// request, and an approval that says no revision. A person's event is by the one the ticket
    /// waited for before (else waits for now, else the requester), the requester's by the
    /// requester.
    fn moved_on(&mut self, from: &str, before: &Ticket, after: &Ticket, now: i64) {
        let rq = after.requester.clone();
        let by = waiting(before).into_iter().chain(waiting(after)).next().unwrap_or_else(|| rq.clone());
        let other = self.org.in_effect(self.m.day(now)).and_then(|v| v.people.iter().map(|p| &p.id).find(|id| **id != rq && **id != by).cloned()).unwrap_or_else(|| by.clone());
        let events = [
            Event::Approve { by: by.clone() },
            Event::Reject { by: by.clone() },
            Event::SendBack { by: by.clone(), to: None },
            Event::Retract { by: by.clone() },
            Event::Pause { by: by.clone() },
            Event::Resume { by: by.clone() },
            Event::Edit { by: by.clone(), fields: Values::new(), clear: Vec::new() },
            Event::Withdraw { by: rq.clone() },
            Event::Discard { by: rq.clone() },
            Event::Resubmit { by: rq.clone(), form: Values::new(), clear: Vec::new(), attempt: crate::engine::attempt(after.round + 1) },
            Event::AddApprover { by: None, approver: other.clone(), part: None },
            Event::HandOver { by: None, from: by.clone(), to: other },
            Event::AgentAnswered { ok: true, reason: String::new() },
            Event::NoteReady { outputs: Values::new() },
            Event::FlowFailed,
        ];
        let now = before_clocks(after, now);
        let was = before.revision();
        let given: Vec<(Event, Option<crate::engine::Revision>, &'static str)> = events.into_iter().map(|ev| (ev.clone(), Some(was), ev.word())).chain([(Event::Approve { by: by.clone() }, None, "approve, no revision")]).collect();
        for (ev, seen, word) in given {
            if !self.stale.insert(word) {
                continue;
            }
            let label = if seen.is_some() { format!("{from}: made on the revision before") } else { format!("{from}: no revision") };
            if let Ok(st) = scenario::step_seen(self.m, self.org, Some(after), Input::Event(ev), None, seen, now) {
                self.keep(&label, st);
            }
        }
        // a comment says no revision, and is taken however the ticket moved on; an approval with a
        // comment too long, made on the revision before, is stale before its comment is read
        if self.stale.insert("comment") {
            if let Ok(st) = scenario::step_with(self.m, self.org, Some(after), Input::Event(Event::Comment { by: by.clone() }), Some("Seen".to_string()), now) {
                self.keep(&format!("{from}: a comment, which says no revision"), st);
            }
            if let Ok(st) = scenario::step_seen(self.m, self.org, Some(after), Input::Event(Event::Approve { by: by.clone() }), Some(long(COMMENT_MOST + 1)), Some(was), now) {
                self.keep(&format!("{from}: made on the revision before, with a comment too long"), st);
            }
        }
    }

    /// The seed's request made on the day before the next closed day, and on the day before the
    /// longest run of closed days of the year after it: the clocks of each, one after another.
    fn days(&mut self, seed: &Seed) {
        let Some((_, cal)) = self.m.calendars.first() else { return };
        let day = self.m.day(seed.at);
        let mut picks: Vec<(i64, i64)> = Vec::new();
        if let Some(c) = cal.closed.range(day + 1..).next() {
            picks.push((self.open_before(*c), *c));
        }
        // the longest run of closed days, the earliest of the longest
        let (mut best, mut run): (Option<(i64, i64)>, Option<(i64, i64)>) = (None, None);
        for d in cal.closed.range(day + 1..=(day + 370).min(cal.data.1)) {
            run = match run {
                Some((start, len)) if start + len == *d => Some((start, len + 1)),
                _ => Some((*d, 1)),
            };
            if run.is_some_and(|(_, len)| best.is_none_or(|(_, b)| len > b)) {
                best = run;
            }
        }
        if let Some((start, _)) = best {
            picks.push((self.open_before(start), start));
        }
        picks.dedup();
        for (d, closed) in picks.into_iter().filter(|(d, _)| *d != day) {
            let from = format!("scenario {}, made on {} (closed from {})", seed.scenario, crate::lex::date_text(d), crate::lex::date_text(closed));
            let at = self.on(d, seed.at);
            if let Some(t) = self.submit(&from, seed, seed.form.clone(), at) {
                self.rings(&from, &t, i64::MAX, 12);
            }
        }
    }

    /// The seed's request made with a comment, and with one a character past what a comment holds
    /// (DESIGN 7.2): what the requester writes with `submit` is read by `start`, before the form.
    fn made_with_comments(&mut self, seed: &Seed) {
        let from = format!("scenario {}, made with a comment", seed.scenario);
        for comment in ["Please decide by Friday".to_string(), long(COMMENT_MOST + 1)] {
            if let Ok(st) = scenario::step_with(self.m, self.org, None, Self::request(seed, seed.form.clone()), Some(comment), seed.at) {
                self.keep(&from, st);
            }
        }
    }

    /// Each person of the organization as the requester of the seed's request: the ones who may
    /// not submit it first, 30 people at the most.
    fn people(&mut self, seed: &Seed) {
        let Some(v) = self.org.in_effect(self.m.day(seed.at)) else { return };
        let line = crate::org::submit::of(self.m.s.file);
        let mut ids: Vec<&str> = Vec::new();
        if let Some(l) = &line {
            ids.extend(v.people.iter().filter(|p| !l.may(self.org, v, &p.id)).take(10).map(|p| p.id.as_str()));
        }
        for p in &v.people {
            if ids.len() < 30 && !ids.contains(&p.id.as_str()) {
                ids.push(&p.id);
            }
        }
        let order: Vec<String> = v.people.iter().filter(|p| ids.contains(&p.id.as_str())).map(|p| p.id.clone()).collect();
        for id in order {
            let sub = Submit { id: TICKET.to_string(), requester: id.clone(), by: None, form: seed.form.clone(), attempt: crate::engine::attempt(1) };
            self.give(&format!("requester {id}"), None, Input::Submit(sub), seed.at);
        }
    }

    /// The vectors written since `mark`, taken off: what a try that came to nothing wrote.
    fn drop_since(&mut self, mark: usize) {
        for v in self.out.split_off(mark) {
            self.seen.remove(&v.stepped.to_json(&self.m.names).compact());
        }
    }

    /// The proxies of the organization's data for the request: a request made in a person's name
    /// on the first and the last day of the proxy and the days around them, and taken back (where
    /// the request has a `withdraw` line) and discarded by the one who made it; one made by
    /// somebody who is no proxy; and a ticket
    /// answered by the proxies of its approvers, with what a proxy does in the seat too
    /// ([`Self::in_the_seat`]) and, with `same approver approves once`, a stage done with a proxy's
    /// approval ([`Self::once_by_proxy`]). The seed of a proxy of an approver is the first whose
    /// ticket the proxy answers a stage of.
    fn proxies(&mut self, seeds: &[Seed]) {
        let Some(seed) = seeds.first() else { return };
        let file = self.m.s.file;
        let covers = |x: &crate::org::Proxy| x.covers(&file.name.text) || x.covers(file.name.ascii());
        let rows: Vec<crate::org::Proxy> = self.org.proxies.iter().filter(|x| covers(x)).cloned().collect();
        for x in rows.iter().filter(|x| x.kind == ProxyKind::Submit).take(3) {
            let from = format!("proxy {} for {}", x.proxy, x.person);
            let mut days = vec![x.from - 1, x.from];
            days.extend(x.to.into_iter().flat_map(|t| [t, t + 1]));
            for d in days {
                let sub = Submit { id: TICKET.to_string(), requester: x.person.clone(), by: Some(x.proxy.clone()), form: seed.form.clone(), attempt: crate::engine::attempt(1) };
                let at = self.on(d, seed.at);
                let made = self.give(&from, None, Input::Submit(sub), at);
                // a comment on a request made in another's name: it goes to both of them. Sent back
                // to them, the requester's goes to the one who made it and to the one who sent it
                // back (DESIGN 7.4). The one who made it takes it back, and discards it once it is
                // back with the requester, for the requester
                if let Some(t) = made.filter(|_| d == x.from)
                    && let Some(a) = waiting(&t).first()
                    && self.said.insert("made in another's name")
                {
                    self.said(&format!("{from}: a comment"), &t, Event::Comment { by: a.clone() }, "Who is it for?", at);
                    if self.has(|l| matches!(l, TicketLine::Withdraw { .. })) {
                        self.event(&format!("{from}: taken back by the one who made it"), &t, Event::Withdraw { by: x.proxy.clone() }, at);
                    }
                    let back_from = format!("{from}: sent back");
                    if let Some(back) = self.event(&back_from, &t, Event::SendBack { by: a.clone(), to: None }, at).filter(|b| b.status == Status::WithRequester) {
                        self.said(&back_from, &back, Event::Comment { by: x.person.clone() }, "Which part, please?", at);
                        self.event(&format!("{from}: sent back, and discarded by the one who made it"), &back, Event::Discard { by: x.proxy.clone() }, at);
                    }
                }
            }
        }
        let day = self.m.day(seed.at);
        let nobody = self.org.in_effect(day).and_then(|v| v.people.iter().map(|p| &p.id).find(|id| **id != seed.requester && !rows.iter().any(|x| x.kind == ProxyKind::Submit && x.person == seed.requester && x.proxy == **id)).cloned());
        if let Some(by) = nobody {
            let sub = Submit { id: TICKET.to_string(), requester: seed.requester.clone(), by: Some(by.clone()), form: seed.form.clone(), attempt: crate::engine::attempt(1) };
            self.give(&format!("proxy {by} for {}", seed.requester), None, Input::Submit(sub), seed.at);
        }
        let mut sent = false;
        for x in rows.iter().filter(|x| x.kind == ProxyKind::Approve).take(3) {
            let from = format!("proxy {} for {}", x.proxy, x.person);
            // written where the proxy comes to answer: a route the person answers no stage of says
            // nothing of the proxy, and the next seed is tried
            for seed in seeds {
                let at = self.on(x.from, seed.at);
                let mark = self.out.len();
                match self.submit(&from, seed, seed.form.clone(), at).and_then(|t| self.through(&from, &t, at, true)) {
                    None => {
                        self.drop_since(mark);
                        continue;
                    }
                    // sent back by the proxy, once for the request: the ticket holds the one who did
                    // it (DESIGN 7.1)
                    Some((t, proxy)) if !sent => {
                        sent = true;
                        self.event(&format!("{from}: sent back by the proxy"), &t, Event::SendBack { by: proxy.clone(), to: None }, at);
                        self.in_the_seat(&from, &t, &proxy, at);
                    }
                    Some(_) => {}
                }
                break;
            }
            self.once_by_proxy(seeds, x);
        }
    }

    /// What a proxy of an approver does in the seat they answer for, on the ticket `t` their first
    /// approval was given to (DESIGN 5.8, 7.4): a rejection, and, where the request has the line
    /// for it, a pause and its end, an edit of a field the approvers may edit (with the value the
    /// ticket holds), and their approval given and taken back (where the ticket goes on after it).
    /// Each line of the history says whom the proxy did it for.
    fn in_the_seat(&mut self, from: &str, t: &Ticket, proxy: &str, at: i64) {
        let by = || proxy.to_string();
        self.event(&format!("{from}: rejected by the proxy"), t, Event::Reject { by: by() }, at);
        let paused_from = format!("{from}: paused by the proxy");
        if self.has(|l| matches!(l, TicketLine::Pause { .. }))
            && let Some(paused) = self.event(&paused_from, t, Event::Pause { by: by() }, at)
        {
            self.event(&paused_from, &paused, Event::Resume { by: by() }, at);
        }
        let editable = self.m.line(|l| if let TicketLine::ApproversEdit { fields, .. } = l { Some(fields.iter().map(|w| w.text.clone()).collect::<Vec<_>>()) } else { None }).unwrap_or_default();
        let field = self.m.s.fields().iter().filter(|f| editable.contains(&f.name.text)).find_map(|f| Some((f.name.ascii().to_string(), t.value(f.name.ascii())?.clone())));
        if let Some(field) = field {
            self.event(&format!("{from}: edited by the proxy"), t, Event::Edit { by: by(), fields: vec![field], clear: Vec::new() }, at);
        }
        let back_from = format!("{from}: approved and taken back by the proxy");
        if self.has(|l| matches!(l, TicketLine::Retract { .. }))
            && let Some(n) = self.event(&back_from, t, Event::Approve { by: by() }, at).filter(|n| n.status == Status::InProgress)
        {
            self.event(&back_from, &n, Event::Retract { by: by() }, at);
        }
    }

    /// Whether the request has a line of the `ticket` block that `is` says.
    fn has(&self, is: impl Fn(&TicketLine) -> bool) -> bool {
        self.m.line(|l| is(l).then_some(())).is_some()
    }

    /// `same approver approves once` with the approval a proxy gave (DESIGN 5.8, 5.12): the first
    /// stage of a seed's ticket handed over to the one the proxy `x` stands in for and approved by
    /// the proxy in their seat, where the stage after it, which that person alone answers, is done
    /// with that approval. Written for the first seed it is so for, and only where the approval
    /// does the stage right after it (`approves once`, not `, at the last stage`).
    fn once_by_proxy(&mut self, seeds: &[Seed], x: &crate::org::Proxy) {
        if crate::org::listing::once(self.m.s.file) != crate::org::listing::Once::Before {
            return;
        }
        let from = format!("proxy {} for {}: a stage handed to {}", x.proxy, x.person, x.person);
        for seed in seeds.iter().filter(|s| s.requester != x.proxy && s.requester != x.person) {
            let at = self.on(x.from, seed.at);
            let mark = self.out.len();
            if self.handed_and_done(&from, seed, x, at) {
                return;
            }
            self.drop_since(mark);
        }
    }

    /// [`Self::once_by_proxy`] for the seed: whether the proxy's approval did the stage after it.
    fn handed_and_done(&mut self, from: &str, seed: &Seed, x: &crate::org::Proxy, at: i64) -> bool {
        let Some(t) = self.submit(from, seed, seed.form.clone(), at) else { return false };
        let Some(a) = waiting(&t).first().cloned() else { return false };
        let handed = if a == x.person { Some(t) } else { self.event(from, &t, Event::HandOver { by: None, from: a, to: x.person.clone() }, at) };
        let Some(handed) = handed else { return false };
        let Ok(st) = scenario::step(self.m, self.org, Some(&handed), Input::Event(Event::Approve { by: x.proxy.clone() }), at) else { return false };
        let done = st.answer.as_ref().is_ok_and(|(_, fx)| fx.iter().any(|e| matches!(&e.what, What::Record(r) if r.did == "approved" && r.once_with.is_some() && r.proxy_of.as_deref() == Some(x.person.as_str()))));
        self.keep(from, st);
        done
    }

    /// A ticket that lives through a change of the organization's version: made on the last open
    /// day of the one and answered on the first of the next; and sent back in the one and made
    /// again in the next, by the seed's requester and by people who may submit the request in the
    /// one and not in the next.
    fn versions(&mut self, seed: &Seed) {
        let day = self.m.day(seed.at);
        let Some(next) = self.org.versions.iter().map(|v| v.from).filter(|d| *d > day).min() else { return };
        let (before, after) = (self.on(self.open_before(next), seed.at), self.on(self.open_from(next), seed.at));
        let from = "across the versions of the organization";
        if let Some(t) = self.submit(from, seed, seed.form.clone(), before) {
            let t = self.rings(from, &t, after, 12);
            let _ = self.through(from, &t, after, false);
        }
        let mut who = vec![seed.requester.clone()];
        if let (Some(l), Some(a), Some(b)) = (crate::org::submit::of(self.m.s.file), self.org.in_effect(self.m.day(before)), self.org.in_effect(self.m.day(after))) {
            who.extend(a.people.iter().filter(|p| l.may(self.org, a, &p.id) && !l.may(self.org, b, &p.id)).take(2).map(|p| p.id.clone()));
        }
        for p in who {
            let sub = Submit { id: TICKET.to_string(), requester: p.clone(), by: None, form: seed.form.clone(), attempt: crate::engine::attempt(1) };
            let Some(t) = self.give(from, None, Input::Submit(sub), before) else { continue };
            let Some(by) = waiting(&t).first().cloned() else { continue };
            let Some(back) = self.event(from, &t, Event::SendBack { by, to: None }, before) else { continue };
            let back = self.rings(from, &back, after, 12);
            self.event(from, &back, Event::Resubmit { by: p, form: Values::new(), clear: Vec::new(), attempt: crate::engine::attempt(back.round + 1) }, after);
        }
    }

    /// The last value taken and the first refused, going from `k` (which the form takes) by
    /// `dir`: the value the field writes as its end first, then further and further away until one
    /// is refused, and between the last taken and it.
    fn edge(&self, seed: &Seed, sp: &Spot, form: &Values, k: i128, dir: i128, guess: Option<i128>) -> Option<(i128, i128)> {
        let ok = |k: i128| self.taken(seed, &sp.put(form, k));
        if let Some(g) = guess.filter(|g| (g - k) * dir >= 0)
            && ok(g)
            && !ok(g + dir)
        {
            return Some((g, g + dir));
        }
        if !ok(k) {
            return None;
        }
        let (mut last, mut far) = (k, 1i128);
        let refused = loop {
            let x = k + dir * far;
            if !ok(x) {
                break x;
            }
            last = x;
            far *= 2;
            if far > 1 << 62 {
                return None;
            }
        };
        let (mut a, mut b) = (last, refused);
        while (b - a).abs() > 1 {
            let mid = a + (b - a) / 2;
            if ok(mid) {
                a = mid;
            } else {
                b = mid;
            }
        }
        Some((a, b))
    }

    /// The edges of the form: of each number a requester writes, going up and going down, each
    /// kept at its last value taken while the next is looked for (so that what the form works out
    /// of them comes to its own ends); then of each `rows` field, a row more.
    fn edges(&mut self, seed: &Seed) {
        let s = self.m.s;
        let mut spots: Vec<Spot> = Vec::new();
        for f in s.fields().iter().filter(|f| f.approvers_only.is_none()) {
            match &f.ty {
                TypeExpr::Num(_) if f.computed.is_none() => spots.push(Spot { rows: None, f }),
                TypeExpr::Rows => spots.extend(f.children.iter().filter(|c| c.computed.is_none() && matches!(c.ty, TypeExpr::Num(_))).map(|c| Spot { rows: Some(f), f: c })),
                _ => {}
            }
        }
        let (mut up, mut low) = (seed.form.clone(), seed.form.clone());
        for sp in &spots {
            let Some(k) = sp.get(&seed.form) else { continue };
            let (lo, hi) = sp.written();
            for (dir, form, guess, word) in [(1, &mut up, hi, "going up"), (-1, &mut low, lo, "going down")] {
                if let Some((a, b)) = self.edge(seed, sp, form, k, dir, guess) {
                    for x in [a, b] {
                        let from = format!("edge of {}, {word}: {}", sp.name(), sp.shown(x));
                        self.submit(&from, seed, sp.put(form, x), seed.at);
                    }
                    *form = sp.put(form, a);
                }
            }
        }
        for rf in s.fields().iter().filter(|f| f.ty == TypeExpr::Rows && f.approvers_only.is_none()) {
            self.grow(seed, rf, &up, &low);
            self.count(seed, rf, &up, &low);
        }
    }

    /// Rows added to the form with its numbers at their largest (`up`), until one is refused: then
    /// the last row with its numbers at their smallest (the first row of `low`), and one number of
    /// it as large as the form takes. Where what the form works out of the rows has an end, the
    /// two forms written are just inside it and just outside.
    fn grow(&mut self, seed: &Seed, rf: &Field, up: &Values, low: &Values) {
        let name = rf.name.ascii();
        let rows_of = |form: &Values| match value(form, name) {
            Some(Value::List(r)) => r.clone(),
            _ => Vec::new(),
        };
        let (big, small) = (rows_of(up), rows_of(low));
        let (Some(big0), Some(small0)) = (big.first().cloned(), small.first().cloned()) else { return };
        let with = |rows: Vec<Values>| {
            let mut f = up.clone();
            put(&mut f, name, Value::List(rows));
            f
        };
        let pre = format!("edge of the rows of {}", rf.name.text);
        let mut rows = big;
        let mut refused = false;
        for _ in 0..64 {
            let mut more = rows.clone();
            more.push(big0.clone());
            if !self.taken(seed, &with(more.clone())) {
                refused = true;
                break;
            }
            rows = more;
        }
        if !refused {
            return;
        }
        let mut last = small0.clone();
        let and = |last: &Values| {
            let mut more = rows.clone();
            more.push(last.clone());
            with(more)
        };
        let n = rows.len();
        if !self.taken(seed, &and(&last)) {
            self.submit(&format!("{pre}: {n} rows"), seed, with(rows.clone()), seed.at);
            self.submit(&format!("{pre}: {} rows", n + 1), seed, and(&last), seed.at);
            return;
        }
        for c in rf.children.iter().filter(|c| c.computed.is_none() && matches!(c.ty, TypeExpr::Num(_))) {
            let (Some(Value::Int(lo)), Some(Value::Int(hi))) = (value(&small0, c.name.ascii()), value(&big0, c.name.ascii())) else { continue };
            let at = |k: i128| {
                let mut l = last.clone();
                put(&mut l, c.name.ascii(), Value::Int(k));
                l
            };
            if self.taken(seed, &and(&at(*hi))) {
                last = at(*hi);
                continue;
            }
            let (mut a, mut b) = (*lo, *hi);
            while b - a > 1 {
                let mid = a + (b - a) / 2;
                if self.taken(seed, &and(&at(mid))) {
                    a = mid;
                } else {
                    b = mid;
                }
            }
            for x in [a, b] {
                let from = format!("{pre}: {} rows, {} {} in the last", n + 1, c.name.text, crate::taken::shown(c, &Value::Int(x)));
                self.submit(&from, seed, and(&at(x)), seed.at);
            }
            return;
        }
    }

    /// The most rows and the fewest the form takes: the number the field writes first, then a row
    /// more (and a row less) at a time. The rows are copies of one row, and which row decides what
    /// refuses first: with small numbers a sum of the rows falls short of its range before the
    /// rows are too few, with large ones it passes its range before they are too many. So five
    /// rows are tried (the form's first row with its numbers at their smallest, as the scenario
    /// writes them, at their largest, and halfway between), and the one the machine takes the most
    /// copies of is written for the most rows, the one it takes the fewest of for the fewest:
    /// among equals, the smaller numbers for the most and the larger for the fewest, which leave
    /// the sums farthest from the end the next row goes toward.
    fn count(&mut self, seed: &Seed, rf: &Field, up: &Values, low: &Values) {
        let name = rf.name.ascii();
        let rows_of = |form: &Values| match value(form, name) {
            Some(Value::List(r)) => r.clone(),
            _ => Vec::new(),
        };
        let n0 = rows_of(low).len();
        let (Some(small), Some(written_row), Some(big)) = (rows_of(low).first().cloned(), rows_of(&seed.form).first().cloned(), rows_of(up).first().cloned()) else { return };
        let between = |a: &Values, b: &Values| -> Values {
            a.iter()
                .map(|(k, v)| match (v, value(b, k)) {
                    (Value::Int(x), Some(Value::Int(y))) => (k.clone(), Value::Int(x + (y - x) / 2)),
                    _ => (k.clone(), v.clone()),
                })
                .collect()
        };
        let mut tried: Vec<Values> = Vec::new();
        for row in [small.clone(), between(&small, &written_row), written_row.clone(), between(&written_row, &big), big] {
            if !tried.contains(&row) {
                tried.push(row);
            }
        }
        let with = |row: &Values, n: usize| {
            let mut f = low.clone();
            put(&mut f, name, Value::List(vec![row.clone(); n]));
            f
        };
        let pre = format!("edge of the number of rows of {}", rf.name.text);
        let said = |n: usize| if n == 1 { format!("{pre}: 1 row") } else { format!("{pre}: {n} rows") };
        let written = rf.count.as_ref().and_then(|b| crate::taken::count_ends(b).1).filter(|h| h.is_int() && h.num >= n0 as i128 && h.num <= 10_000).map(|h| h.num as usize);
        let mut most: Option<(usize, &Values)> = None;
        for row in &tried {
            let h = match written {
                Some(h) if self.taken(seed, &with(row, h)) && !self.taken(seed, &with(row, h + 1)) => Some(h),
                _ => (n0..n0 + 64).find(|n| self.taken(seed, &with(row, *n)) && !self.taken(seed, &with(row, n + 1))),
            };
            if let Some(h) = h.filter(|h| most.is_none_or(|(m, _)| *h > m)) {
                most = Some((h, row));
            }
        }
        if let Some((h, row)) = most {
            self.submit(&said(h), seed, with(row, h), seed.at);
            self.submit(&said(h + 1), seed, with(row, h + 1), seed.at);
        }
        let mut fewest: Option<(usize, &Values)> = None;
        for row in tried.iter().rev() {
            let l = (1..=n0).rev().find(|n| self.taken(seed, &with(row, *n)) && !self.taken(seed, &with(row, n - 1)));
            if let Some(l) = l.filter(|l| fewest.is_none_or(|(m, _)| *l < m)) {
                fewest = Some((l, row));
            }
        }
        if let Some((l, row)) = fewest {
            self.submit(&said(l), seed, with(row, l), seed.at);
            self.submit(&said(l - 1), seed, with(row, l - 1), seed.at);
        }
    }

    /// The rows that bring a field computed as `sum(rows.field)` to `v`: copies of the form's
    /// first row, the field at the end it writes in each but the last.
    fn summed(&self, form: &Values, computed: &Expr, v: &Value) -> Option<(String, Value)> {
        let ExprKind::Sum(p) = &computed.kind else { return None };
        let [rows, child] = p.as_slice() else { return None };
        let rf = self.m.s.fields().iter().find(|f| f.name.text == rows.text || f.name.ascii() == rows.text)?;
        let cf = rf.children.iter().find(|c| c.name.text == child.text || c.name.ascii() == child.text).filter(|c| c.computed.is_none())?;
        let (Value::Int(total), Some(Value::List(given))) = (v, value(form, rf.name.ascii())) else { return None };
        let first = given.first()?.clone();
        let top = Spot { rows: Some(rf), f: cf }.written().1.filter(|h| *h > 0).unwrap_or(*total).max(1);
        let n = ((*total + top - 1) / top).max(1);
        if n > 1_000 {
            return None;
        }
        let rows = (0..n).map(|i| {
            let mut r = first.clone();
            put(&mut r, cf.name.ascii(), Value::Int(if i + 1 < n { top } else { *total - top * (n - 1) }));
            r
        });
        Some((rf.name.ascii().to_string(), Value::List(rows.collect())))
    }

    /// Each field the form asks for on a condition, left empty: one shown on a condition with no
    /// `optional`, which the form asks for while it shows it, and one with `required when`; each in
    /// the form of the first scenario that fills it in.
    fn left_empty(&mut self, seeds: &[Seed]) {
        let s = self.m.s;
        for f in s.fields().iter().filter(|f| f.computed.is_none() && f.approvers_only.is_none() && ((f.shown_when.is_some() && f.optional.is_none()) || f.required_when.is_some())) {
            let name = f.name.ascii();
            let Some(seed) = seeds.iter().find(|sd| value(&sd.form, name).is_some_and(|v| *v != Value::None)) else { continue };
            let form: Values = seed.form.iter().filter(|(n, _)| n != name).cloned().collect();
            self.submit(&format!("scenario {}, with {} left empty", seed.scenario, f.name.text), seed, form, seed.at);
        }
    }

    /// The rows of the examples: each laid over the form of the first scenario the machine then
    /// takes and whose form comes to the row's values. A row no scenario's form can be brought to
    /// (an input a rule works out, a field the scenarios leave empty) is not written.
    fn examples(&mut self, seeds: &[Seed]) {
        let s = self.m.s;
        let Some(ex) = &s.file.examples else { return };
        for row in &ex.rows {
            'seeds: for seed in seeds {
                let mut form = seed.form.clone();
                let mut want: Values = Vec::new();
                for (col, (e, _)) in ex.inputs.iter().zip(&row.values) {
                    let [name] = col.as_slice() else { break 'seeds };
                    let Some(f) = s.fields().iter().find(|f| f.name.text == name.text || f.name.ascii() == name.text) else { break 'seeds };
                    let Ok(v) = crate::taken::read(f, e) else { break 'seeds };
                    match &f.computed {
                        None => put(&mut form, f.name.ascii(), v.clone()),
                        Some(c) => {
                            let Some((rows, list)) = self.summed(&form, c, &v) else { break 'seeds };
                            put(&mut form, &rows, list);
                        }
                    }
                    want.push((f.name.ascii().to_string(), v));
                }
                let Ok(st) = scenario::step(self.m, self.org, None, Self::request(seed, form), seed.at) else { continue };
                if st.answer.as_ref().is_ok_and(|(t, _)| want.iter().all(|(n, v)| t.value(n) == Some(v))) {
                    self.keep(&format!("examples, line {}", row.line), st);
                    break;
                }
            }
        }
    }
}

/// The vectors of the request of the machine `m`, from its scenarios as they ran (`rans`, each
/// gone as written).
pub fn make(m: &Machine, org: &Org, rans: &[Ran]) -> Vec<Vector> {
    let mut mk = Maker { m, org, out: Vec::new(), seen: HashSet::new(), absent: HashSet::new(), stale: HashSet::new(), said: HashSet::new(), emptied: HashSet::new(), conditioned: HashSet::new() };
    let mut seeds: Vec<Seed> = Vec::new();
    for ran in rans {
        let pre = format!("scenario {}", ran.name);
        mk.absent.clear();
        for row in &ran.rows {
            mk.keep(&format!("{pre}, line {}", row.line), row.stepped.clone());
        }
        // a comment on the ticket the scenario ended
        if let Some((t, last)) = ran.ticket().filter(|t| t.status.ended()).zip(ran.rows.last()) {
            mk.said_once("ended", &format!("{pre}, after it ended"), t, Event::Comment { by: t.requester.clone() }, "Thank you", last.stepped.now);
        }
        if let Some(Stepped { input: Input::Submit(sub), now, answer: Ok(_), .. }) = ran.rows.first().map(|r| &r.stepped) {
            seeds.push(Seed { scenario: ran.name.clone(), requester: sub.requester.clone(), by: sub.by.clone(), form: sub.form.clone(), at: *now });
        }
        // each stage the scenario comes to, the first time in a round
        let mut came: Vec<(u32, String)> = Vec::new();
        for row in &ran.rows {
            let Some(t) = row.ticket().filter(|t| t.status == Status::InProgress) else { continue };
            let Some(stage) = t.stage_name() else { continue };
            let key = (t.round, stage.to_string());
            if came.contains(&key) {
                continue;
            }
            let first = came.is_empty();
            came.push(key);
            mk.at_stage(&format!("{pre}, after line {}", row.line), t, row.stepped.now, first);
        }
    }
    // the first ticket a scenario moves on to another revision, one that has not ended where there
    // is one, and what was made on the revision before it
    let moved = |ended: bool| {
        rans.iter().find_map(|ran| ran.rows.iter().find_map(|r| Some((ran, r, r.stepped.before.as_ref()?, r.ticket()?)).filter(|(_, _, b, a)| (ended || !a.status.ended()) && a.revision() != b.revision())))
    };
    if let Some((ran, row, before, after)) = moved(false).or_else(|| moved(true)) {
        mk.moved_on(&format!("scenario {}, after line {}", ran.name, row.line), before, after, row.stepped.now);
    }
    if let Some(seed) = seeds.first() {
        mk.days(seed);
        mk.made_with_comments(seed);
        mk.people(seed);
        mk.proxies(&seeds);
        mk.versions(seed);
        mk.edges(seed);
    }
    mk.left_empty(&seeds);
    mk.examples(&seeds);
    mk.out
}
