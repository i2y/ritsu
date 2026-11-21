//! The ticket machine (DESIGN 7, 10.1): the reference interpreter of a request. A ticket is one
//! request on its way; [`Machine::start`] makes one from a `submit`, and [`Machine::next`] moves one
//! by one event. Both are given the time of the event, the part of the organization the ticket
//! needs ([`OrgPart`]), the values of the form and what the person wrote with the event (its
//! comment), read nothing else, and answer the ticket as it is then with the list of things to do
//! ([`Effect`]), or why the event is not taken ([`Refusal`]).
//!
//! What the request computes is asked of the languages its parts are lowered to, through the
//! ports: the form and the route of rulec's rules ([`eval`]), the deadlines of koyomi's dates
//! ([`timers`]). Who answers a stage is solved as the check solves it ([`crate::org::resolve`],
//! [`crate::org::listing`]), on the part of the organization the event comes with. The check of the
//! request ([`crate::check::Outcome`]) is made once, and a machine is built on it ([`with`]).
//!
//! - [`state`]: a ticket. [`event`]: the events. [`effects`]: the things to do, and their order.
//! - [`part`]: the part of the organization an event is given with.
//! - [`stages`]: how a stage starts (its approvers, a stage with nobody, `same approver approves
//!   once`), and how the ticket moves on. [`act`]: each event. [`on`]: what a ticket does when it is
//!   submitted and when it ends (`on approved post …`).

mod act;
pub mod effects;
pub mod eval;
pub mod event;
mod on;
pub mod part;
mod stages;
pub mod state;
mod timers;
pub mod wire;
pub mod worked;

pub use effects::{Effect, Effects, Notice, Record, Timer, What};
pub use event::{Event, Submit, attempt};
pub use part::{Need, OrgPart, PartRow};
pub use state::{Answer, Ended, Held, Part, PartState, Phase, Revision, Sole, Stage, Status, Ticket};
pub use wire::Names;

use crate::ast::{FileKind, TicketLine};
use crate::check::{Outcome, Run};
use crate::lower::Lowered;
use crate::names::Scope;
use crate::org::listing::{self, Occ};
use crate::org::resolve::Requester;
use crate::org::{Data, Org, ProxyKind};
use crate::suite::Suite;
use crate::taken::{self, Giver, When};
use ritsu_base::json::Json;
use ritsu_base::text::Text;
use ritsu_ports::{CalendarFacts, Value, Values};
use std::rc::Rc;

/// Why an event is not taken: the ticket stays as it was, and nothing is to do. `kind` tells the
/// refusals apart for whoever calls: `state` (the ticket's state does not take the event), `person`
/// (the one it is by may not do it), `submit` (the requester may not submit the request), `form` (a
/// value the form does not take, or one it asks for), `limit` (a count the request allows is used
/// up), `clock` (the clock has not rung, or one that has rung has not been given: a clock's event
/// comes before whatever else happens at its time or after it), `event` (the event names what the
/// ticket does not have),
/// `data` (data the request reads does not cover the day), `book` (a hold cannot be made of what
/// the ticket holds; a server that is refused a hold by the book refuses the event with this too),
/// `unread` (a lowered file gave no answer). `text` says why in both languages; `at` is the place
/// of the form a refusal names (DESIGN 12.4): a field by its ASCII name, a cell of a row the
/// requester gave as `items[1].price` (the rows counted from 0), none for one that names no field.
#[derive(Clone, Debug, PartialEq)]
pub struct Refusal {
    pub kind: &'static str,
    pub text: Text,
    pub at: Option<String>,
}

impl Refusal {
    pub fn new(kind: &'static str, text: Text) -> Refusal {
        Refusal { kind, text, at: None }
    }

    /// A refusal of a value the form does not take, at the place it names.
    pub fn form(said: taken::Said) -> Refusal {
        Refusal { kind: "form", text: said.text, at: said.at }
    }

    /// The refusal as the contract writes it: its kind, its sentence in English (`text`) and in
    /// Japanese (`text_ja`), whatever language the run is in, and the place it names (`at`; null
    /// where it names none).
    pub fn to_json(&self) -> Json {
        Json::obj([("refused", Json::str(self.kind)), ("text", Json::str(&self.text.en)), ("text_ja", Json::str(&self.text.ja)), ("at", Json::opt_str(self.at.clone()))])
    }
}

/// The most characters a comment holds (DESIGN 7.2), counted as a field's `count` counts them. The
/// refusal past it says the length and the limit as the refusal of a value of a master does, here
/// and in the machine's code (`said` of `lower/ticket.kn`).
pub const COMMENT_MOST: usize = 2_000;

/// What a person wrote with an event, as the machine keeps it: none for an empty one, and refused
/// (`form`) past [`COMMENT_MOST`] characters.
fn said(comment: Option<&str>) -> Result<Option<&str>, Refusal> {
    let Some(c) = comment.filter(|c| !c.is_empty()) else { return Ok(None) };
    let n = c.chars().count();
    if n > COMMENT_MOST {
        let g = commas(n);
        return Err(Refusal::new("comment", tr!("コメントは {g} 文字で、上限の 2,000 を超えています", "the comment is {n} characters long, past the limit ({COMMENT_MOST})")));
    }
    Ok(Some(c))
}

/// `n` with `,` between each three figures, as `commas` of `lower/ticket.kn` writes it: `2,001`.
fn commas(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The machine of one request: its checked file, the files it is lowered to, the days it can be
/// used on, and the calendars its days are read in.
pub struct Machine<'s, 'a> {
    pub s: &'s Scope<'a>,
    pub suite: &'s Suite,
    pub lowered: &'s Lowered,
    pub usable: Option<&'s crate::usable::Usable>,
    /// The paths of the route: each row of its table, or its stage lines.
    pub(crate) paths: Vec<Vec<Occ<'s>>>,
    /// The UTC offset the days of the request are read in, in minutes east of UTC: its first
    /// calendar's that says one, else UTC.
    pub offset: i32,
    /// The calendars the request uses (and the shared route it uses), by alias.
    pub calendars: Vec<(String, Rc<CalendarFacts>)>,
    /// The books the request uses, by alias, as chobo reads them: the parameters of the transfers
    /// a `hold`, a `post` and a `void` are given by.
    pub books: Vec<(String, ritsu_ports::BookFacts)>,
    /// The names the JSON takes (DESIGN 3.4).
    pub names: Names,
    files: Rc<dyn ritsu_base::fs::Files>,
}

/// The calendars a request uses, by alias, with the days each closes.
pub fn calendars(s: &Scope, suite: &Suite) -> Vec<(String, Rc<CalendarFacts>)> {
    let Some(dates) = &suite.dates else { return Vec::new() };
    crate::lower::context(s, suite).calendars.iter().filter_map(|c| Some((c.alias.clone(), Rc::new(dates.calendar(c.target.as_ref()?).ok()?)))).collect()
}

/// The UTC offset the days of a request are read in (DESIGN 7.5), in minutes east of UTC, from the
/// calendars it uses ([`calendars`]): the first's that says one, else UTC. The ticket machine's, the
/// generated code's, and what `ringi api` and the description of the kind (`utc_offset_minutes`)
/// say.
pub fn offset(calendars: &[(String, Rc<CalendarFacts>)]) -> i32 {
    calendars.iter().find_map(|(_, c)| c.offset).unwrap_or(0)
}

/// The books a request uses, by alias, as chobo reads them.
pub fn books(s: &Scope, suite: &Suite) -> Vec<(String, ritsu_ports::BookFacts)> {
    let Some(books) = &suite.books else { return Vec::new() };
    let aliases = s.file.uses.iter().filter(|u| u.kind == crate::ast::UseKind::Book).filter_map(|u| u.alias.as_ref());
    aliases.filter_map(|a| Some((a.text.clone(), books.facts(s.other_path(&a.text)?).ok()?))).collect()
}

/// A machine for the checked request `o`, and the data of the organization it reads, given to `f`.
/// Err when the file is no request that passes its check with a route lowered, or reads no data of
/// an organization.
pub fn with<R>(o: &Outcome, run: &Run, f: impl FnOnce(&Machine, &Org) -> R) -> Result<R, Text> {
    let src = ritsu_base::fs::read_to_string(&o.path).map_err(|e| {
        let e = e.to_string();
        tr!("読めません: {e}", "cannot read it: {e}")
    })?;
    with_text(o, run, &src, f)
}

/// [`with`], for a request checked as the text `src` ([`Run::text`]).
pub fn with_text<R>(o: &Outcome, run: &Run, src: &str, f: impl FnOnce(&Machine, &Org) -> R) -> Result<R, Text> {
    let Some(file) = &o.file else { return Err(tr!("検査が通りません", "the file does not pass its check")) };
    if file.kind != FileKind::Request {
        return Err(tr!("チケットを持つのは申請の種類のファイルです", "a ticket is of a request's file"));
    }
    let mut said = Vec::new();
    let got = crate::names::read_uses(file, &o.path, src, run, &mut said);
    let s = Scope { file, path: &o.path, src, got };
    let data = s.org_rc().ok_or_else(|| tr!("申請の種類が組織を読みません", "the request reads no organization"))?;
    let org = match data.as_ref() {
        Data::Org(org) => org,
        Data::NoOrg(none) => return Err(tr!("{}（{}）", "{} ({})", none.what.ja, none.why.ja; none.what.en, none.why.en)),
        Data::Masters(_) => return Err(Text::default()),
    };
    let Some(lowered) = &o.lowered else { return Err(tr!("下ろした経路がありません", "the route is not lowered")) };
    let m = Machine::new(&s, &run.suite, lowered, o.usable.as_ref());
    Ok(f(&m, org))
}

impl<'s, 'a> Machine<'s, 'a> {
    pub fn new(s: &'s Scope<'a>, suite: &'s Suite, lowered: &'s Lowered, usable: Option<&'s crate::usable::Usable>) -> Machine<'s, 'a> {
        let calendars = calendars(s, suite);
        let offset = offset(&calendars);
        Machine { s, suite, lowered, usable, paths: listing::paths(s), offset, calendars, books: books(s, suite), names: Names::of(s), files: eval::overlay(lowered) }
    }

    /// `f`, with the lowered files where the ports read them.
    pub(crate) fn fs<R>(&self, f: impl FnOnce() -> R) -> R {
        ritsu_base::fs::with(self.files.clone(), f)
    }

    /// A line of the ticket's block, by what it is.
    pub(crate) fn line<T>(&self, pick: impl Fn(&'s TicketLine) -> Option<T>) -> Option<T> {
        self.s.file.ticket.as_ref()?.lines.iter().find_map(pick)
    }

    /// The names a proxy's row may name the request by.
    pub(crate) fn proxy_on(&self, x: &crate::org::Proxy, day: i64) -> bool {
        x.on(day, &self.s.file.name.text) || x.on(day, self.s.file.name.ascii())
    }

    pub(crate) fn requester(t: &Ticket) -> Requester {
        Requester { id: t.requester.clone(), by: t.made_by.clone() }
    }

    /// The requester, and the one who made the request in their name: who a notice of the ticket
    /// itself goes to.
    pub(crate) fn theirs(t: &Ticket) -> Vec<String> {
        let mut out = vec![t.requester.clone()];
        out.extend(t.made_by.clone());
        out
    }

    /// What the values of a form are read against, for an event that comes with `part`.
    pub(crate) fn when<'x>(&'x self, part: &'x OrgPart) -> When<'x> {
        When { day: Some(part.day), versions: Some(vec![part.v()]), calendars: &self.calendars, rows: Some(&part.rows) }
    }

    /// The part is of the day the event is on.
    fn on_day(&self, part: &OrgPart, now: i64) -> Result<i64, Refusal> {
        let day = self.day(now);
        if part.day != day {
            let (a, b) = (crate::lex::date_text(part.day), crate::lex::date_text(day));
            return Err(Refusal::new("data", tr!("組織の部分が {a} のものです。イベントは {b} に起きました", "the part of the organization is of {a}, and the event is on {b}")));
        }
        Ok(day)
    }

    /// Why the lowered files gave no answer, as a refusal.
    pub(crate) fn unrouted(&self, e: eval::Unrouted) -> Refusal {
        use eval::Unrouted;
        match e {
            Unrouted::Unjoined(tool) => Refusal::new("unread", tr!("{tool} がつながっていません", "{tool} is not joined")),
            Unrouted::Unread(m) => Refusal::new("unread", tr!("下ろしたファイルを読めません: {}", "a lowered file does not read: {}", m.ja; m.en)),
            Unrouted::Lacks { name, route: true } => self.at(Refusal::new("form", tr!("経路が読む `{name}` がありません", "the route reads `{name}`, which is not filled in")), &name),
            Unrouted::Lacks { name, route: false } => self.at(Refusal::new("form", tr!("フォームの計算が読む `{name}` がありません", "the form's computations read `{name}`, which is not filled in")), &name),
            Unrouted::Date(field, t) => self.at(Refusal::new("form", tr!("`{field}` を数えられません: {}", "`{field}` cannot be counted: {}", t.ja; t.en)), &field),
            Unrouted::Outside(t, at) => Refusal { kind: "form", text: t, at: Some(at) },
            Unrouted::Rule(e, what) => match e {
                ritsu_ports::RuleError::Input(t) => Refusal::new("form", tr!("{}が、値を受け取りません: {}", "{} does not take the values: {}", what.ja, t.ja; what.en, t.en)),
                ritsu_ports::RuleError::Contradiction(t) => Refusal::new("unread", tr!("{}の答えが一つに決まりません: {}", "{} gives no one answer: {}", what.ja, t.ja; what.en, t.en)),
                ritsu_ports::RuleError::Unread(said) => {
                    let first = said.first().map(|x| x.message.clone()).unwrap_or_default();
                    Refusal::new("unread", tr!("{}を下ろしたファイルを読めません: {}", "what {} is lowered to cannot be read: {}", what.ja, first.ja; what.en, first.en))
                }
            },
        }
    }

    /// The refusal `r` at the field named `name` (as written or by its ASCII name), when the form
    /// has one.
    pub(crate) fn at(&self, r: Refusal, name: &str) -> Refusal {
        let at = self.s.fields().iter().find(|f| f.name.ascii() == name || f.name.text == name).map(|f| f.name.ascii().to_string());
        Refusal { at, ..r }
    }

    /// The values a requester gave, of a ticket's form: without what the form computes.
    pub(crate) fn given_of(&self, form: &Values) -> Values {
        form.iter().filter(|(n, _)| !self.s.fields().iter().any(|f| f.name.ascii() == n && f.computed.is_some())).cloned().collect()
    }

    /// The form computed and the route decided for the values given: the values of the ticket (what
    /// was given, then what the form computes), and the stages the route comes to. Refused when the
    /// form does not take them: a field it asks for is empty, a value it works out leaves its range.
    pub(crate) fn formed(&self, given: &Values, part: &OrgPart) -> Result<(Values, Vec<Stage>), Refusal> {
        let given: Values = given.iter().filter(|(_, v)| *v != Value::None).cloned().collect();
        let r = self.fs(|| eval::routed(self.s, self.suite, self.lowered, &given)).map_err(|e| self.unrouted(e))?;
        let ff = self.lowered.files.iter().find(|f| f.part == crate::lower::Part::Form);
        let named = |want: &str| ff.and_then(|f| f.renamed.iter().find(|(_, w)| w == want).map(|(g, _)| g.clone())).unwrap_or_else(|| want.to_string());
        if let Some(t) = taken::missing(self.s, &given, &r.outs, &named) {
            return Err(Refusal::form(t));
        }
        // a date the form counts is held to the range its field writes here: koyomi holds none for
        // a date it computes. A number's range is held at the door of the lowered rule.
        let when = self.when(part);
        for (n, v) in &r.computed {
            if let Some(f) = self.s.fields().iter().find(|f| f.name.ascii() == n && f.ty == crate::ast::TypeExpr::Date)
                && let Some(t) = taken::holds(self.s, f, v, &taken::shown(f, v), &when)
            {
                return Err(Refusal::form(taken::Said::at(t, f.name.ascii().to_string())));
            }
        }
        let mut form = given;
        // a computation with no value is not in the form, as a field not filled in is not
        form.extend(r.computed.into_iter().filter(|(_, v)| *v != Value::None));
        let stages = r.stages.into_iter().filter_map(|(name, v)| Some(Stage::new(&name, &v?))).collect();
        Ok((self.in_form_order(form), stages))
    }

    /// The values of a form in the order the form declares its fields (DESIGN 10.4): what was given,
    /// then the numbers the form works out, then the dates it counts, each in the order written; the
    /// cells of a row in the order its field declares them, a cell with no value left out. The
    /// ticket, and what a flow is given (`ask agent`, `ask note`), hold them so in whatever order the
    /// JSON gave them, as the generated code does.
    pub(crate) fn in_form_order(&self, vals: Values) -> Values {
        let fields = self.s.fields();
        let of = |n: &str| fields.iter().position(|f| f.name.ascii() == n || f.name.text == n);
        let rank = |n: &str| match of(n) {
            Some(i) => match (&fields[i].computed, &fields[i].ty) {
                (None, _) => (0, i),
                (Some(_), crate::ast::TypeExpr::Date) => (2, i),
                (Some(_), _) => (1, i),
            },
            None => (3, 0),
        };
        let mut out: Vec<((u8, usize), (String, Value))> = vals
            .into_iter()
            .map(|(n, v)| {
                let v = match (of(&n), v) {
                    (Some(i), Value::List(rows)) if !fields[i].children.is_empty() => {
                        let f = &fields[i];
                        let cell = |c: &str| f.children.iter().position(|x| x.name.ascii() == c || x.name.text == c).unwrap_or(usize::MAX);
                        Value::List(
                            rows.into_iter()
                                .map(|row| {
                                    let mut row: Values = row.into_iter().filter(|(_, x)| *x != Value::None).collect();
                                    row.sort_by_key(|(c, _)| cell(c));
                                    row
                                })
                                .collect(),
                        )
                    }
                    (_, v) => v,
                };
                (rank(&n), (n, v))
            })
            .collect();
        // a stable sort: a name the form does not declare keeps its place after the others
        out.sort_by_key(|(k, _)| *k);
        out.into_iter().map(|(_, x)| x).collect()
    }

    /// A request is made (DESIGN 7.3): the ticket at its first stage, with what is to do; or why it
    /// is not made. What the requester wrote with it (`comment`) is on the line of the history it
    /// makes.
    pub fn start(&self, sub: &Submit, comment: Option<&str>, part: &OrgPart, now: i64) -> Result<(Ticket, Vec<Effect>), Refusal> {
        let day = self.on_day(part, now)?;
        self.usable_on(day)?;
        let (v, file) = (part.v(), self.s.file);
        let d = v.day();
        let not_in = |id: &str| Refusal::new("person", tr!("{id} は、{d} からの組織にいません", "{id} is not in the organization from {d}"));
        if v.person(&sub.requester).is_none() {
            return Err(not_in(&sub.requester));
        }
        let made_by = sub.by.clone().filter(|b| *b != sub.requester);
        if let Some(by) = &made_by {
            if v.person(by).is_none() {
                return Err(not_in(by));
            }
            if !part.org.proxies.iter().any(|x| x.kind == ProxyKind::Submit && x.person == sub.requester && x.proxy == *by && self.proxy_on(x, day)) {
                let (who, whose, on) = (v.who(by), v.who(&sub.requester), crate::lex::date_text(day));
                return Err(Refusal::new(
                    "person",
                    tr!("{}は、{on} に {}の代わりにこの申請を出せません（`proxies.csv` に `submit` の行がありません）", "{} is no proxy of {} to submit the request on {on} (no `submit` row of `proxies.csv`)", crate::org::ja_word(&who.ja), crate::org::ja_word(&whose.ja); who.en, whose.en),
                ));
            }
        }
        // who may submit: the requester, as the check and `simulate` decide it
        if let Some(l) = crate::org::submit::of(file)
            && !l.may(&part.org, v, &sub.requester)
        {
            return Err(Refusal::new("submit", l.refused(v, &sub.requester, self.s.path)));
        }
        let comment = said(comment)?;
        if let Some(t) = taken::given(self.s, &sub.form, Giver::Requester, &[], &self.when(part)) {
            return Err(Refusal::form(t));
        }
        let (form, stages) = self.formed(&sub.form, part)?;
        let pause = self.line(|l| if let TicketLine::Pause { days, .. } = l { Some(*days) } else { None }).unwrap_or(0);
        let mut t = Ticket {
            id: sub.id.clone(),
            request: file.name.ascii().to_string(),
            version: file.version,
            requester: sub.requester.clone(),
            made_by,
            sent_back_by: None,
            status: Status::InProgress,
            round: 1,
            attempt: sub.attempt.clone(),
            serial: 0,
            form,
            stages,
            at: None,
            backs: 0,
            edits: 0,
            pause_left: pause,
            paused_at: None,
            pause_ends: None,
            resubmit_due: None,
            holds: Vec::new(),
            notes: Vec::new(),
            ended: None,
        };
        let mut fx = Effects::default();
        let by = t.made_by.clone().unwrap_or_else(|| t.requester.clone());
        fx.record(t.round, Record { did: "submitted", by: Some(by), proxy_of: t.made_by.as_ref().map(|_| t.requester.clone()), org: Some(v.from), ..Record::default() });
        self.on(crate::ast::Ending::Submitted, &mut t, part, &mut fx)?;
        self.advance(&mut t, 0, false, part, now, &mut fx)?;
        fx.comment(comment);
        let effects = fx.done(&mut t);
        Ok((t, effects))
    }

    /// One event of a ticket (DESIGN 7.3): the ticket as it is after it, with what is to do; or why
    /// the event is not taken, the ticket as it was. What a person wrote with it (`comment`) is on
    /// the line of the history of what they did; a clock and a flow write none.
    pub fn next(&self, t: &Ticket, ev: &Event, comment: Option<&str>, seen: Option<&Revision>, part: &OrgPart, now: i64) -> Result<(Ticket, Vec<Effect>), Refusal> {
        self.on_day(part, now)?;
        // a clock that has rung comes first: what a person or a flow does at its time or after it
        // waits for the clock's own event, so that a ticket moves the same way however late its
        // clocks are given, and no stage outlives its deadline (DESIGN 7.6). A ticket that has
        // ended has no clock set.
        if !ev.clock()
            && let Some((w, stage, at)) = t.timers().into_iter().find(|(.., at)| *at <= now)
        {
            let (e, a) = (ev.word(), Self::time_in(at, self.clock_offset(w, stage)));
            let of = stage.map(|s| format!(" (`{s}`)")).unwrap_or_default();
            let of_ja = stage.map(|s| format!("（`{s}`）")).unwrap_or_default();
            return Err(Refusal::new("clock", tr!("`{w}` の時計{of_ja}が {a} に鳴っています。`{e}` より先に、その時計のイベントを渡してください", "the clock of `{w}`{of} rang at {a}: its event comes before `{e}`")));
        }
        // then the revision the event was made on, before who does it and the state: one who saw
        // the ticket as it was is to see it again (DESIGN 7.2). A clock's event carries none, and a
        // comment, which moves nothing, none either.
        if ev.revised() {
            stale(t, ev, seen)?;
        }
        if t.status.ended() {
            let (e, st) = (ev.word(), t.status.word());
            return Err(Refusal::new("state", tr!("チケットは `{st}` で終わっています。`{e}` は受け付けません", "the ticket has ended (`{st}`), and takes no `{e}`")));
        }
        let comment = if ev.person() { said(comment)? } else { None };
        act::reasoned(self.s, ev, comment)?;
        let mut t = t.clone();
        let mut fx = Effects::default();
        self.act(&mut t, ev, comment, part, now, &mut fx)?;
        fx.comment(comment);
        let effects = fx.done(&mut t);
        Ok((t, effects))
    }
}

/// Whether an event was made on the ticket's revision (DESIGN 7.2): else it is refused (`stale`),
/// and the one who made it is to see the ticket again.
fn stale(t: &Ticket, ev: &Event, seen: Option<&Revision>) -> Result<(), Refusal> {
    let e = ev.word();
    let now = t.revision();
    match seen {
        Some(s) if *s == now => Ok(()),
        Some(s) => {
            let (was, is) = (s.text(), now.text());
            Err(Refusal::new(
                "stale",
                tr!("`{e}` はチケットの {}のときのものですが、チケットは今 {}です。チケットを見せ直してください", "`{e}` was made on the ticket at {}, and it is at {} now: the ticket is to be shown again", was.ja, is.ja; was.en, is.en),
            ))
        }
        None => Err(Refusal::new("stale", tr!("`{e}` が、どの版のチケットに対するものかを言っていません", "`{e}` does not say which revision of the ticket it was made on"))),
    }
}
