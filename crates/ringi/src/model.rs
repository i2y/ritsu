//! The reference interpreter as the Lean model of the ticket machine asks it (`proofs/RingiCore`,
//! DESIGN 11.3). The model is the machine's code, `lower/ticket.kn`, written in Lean. The code that
//! joins a request to that machine, its glue ([`crate::lower::glue`]), gives the machine the
//! request's data (`spec()`) and the functions it calls (`clocks`, `open_after`, `on_lines`,
//! `reform`, `given`, `formed`, `vals`). The model reads the data from a file this writes
//! ([`Model::spec`]), and asks the answers of the functions one at a time, as the test of
//! `ritsu-model` runs a vector through it: each answer is made here by the reference interpreter
//! ([`Asking::answer`]), from the vector's ticket, event and part of the organization.
//!
//! The questions and their answers, each a JSON value:
//!
//! - `{"clocks": {"stage", "started", "held", "up"}}`: the reminder and the deadline of a stage
//!   ([`Machine::clocks`]), `{"ok": {"remind", "due"}}` or `{"err": <refusal>}`.
//! - `{"open_after": {"key", "day", "n"}}`: the `n`th business day after a day in the calendar a
//!   pause (`pause`) or the requester's turn (`resubmit`) counts in, as the glue's `open_after`
//!   reads it.
//! - `{"on": {"ending", "ref", "form"}}`: each `on` line of the ending, in the order written, with
//!   whether it is done and its arguments, as the glue's `on_lines` gives them.
//! - `{"reform": {"form", "given", "clear", "approver"}}`: a form given again (`resubmit`) or edited
//!   (`edit`), as the glue's `reform` answers it.
//! - `{"prepare": {"given"}}`: the form of a request made, its stages and the values a flow is given
//!   (the glue's `given`, `formed` and `vals` as `start` calls them).
//! - `{"vals": {"form"}}`: the values of a form a flow is given.
//!
//! A day is a number of days from 1970-01-01, a time seconds since 1970-01-01T00:00:00Z. A form, a
//! stage and a list of values are written as the vectors write them. Where the reference has no
//! function of the glue's, the answer is put together from the ones it has, each said where it is.

use crate::ast::{ActionKind, AtTop, Ending, File, Group, How, Late, NamedTable, Spec, TicketLine, WhenEmpty};
use crate::check::{Options, Run};
use crate::engine::state::{time_json, values_json};
use crate::engine::{Effects, Held, Machine, Refusal, Stage, Status, Ticket, What};
use crate::lower::words;
use crate::names::Scope;
use crate::org::Org;
use crate::org::listing;
use crate::suite::Suite;
use crate::taken::{self, Giver};
use crate::vectors::Vector;
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text};
use ritsu_ports::{Value, Values};
use std::collections::BTreeMap;

/// The machine of the request checked at `path` with the languages of `suite` joined, the
/// organization's data it reads and its vectors, made as `ringi vectors` makes them (each scenario
/// run first), given to `f`. Err when the file does not pass its check, makes no machine, has no
/// scenario, or has a scenario that stops.
pub fn with<R>(path: &str, suite: Suite, f: impl FnOnce(&Model) -> R) -> Result<R, Text> {
    let run = Run::with(suite, Options { unlisted: true, unrun: true, ..Options::default() });
    let o = run.file(path).map_err(|e| {
        let e = e.to_string();
        tr!("`{path}` を読めません: {e}", "cannot read `{path}`: {e}")
    })?;
    if o.has_errors() {
        return Err(Text::new(crate::check::render(&o, Lang::Ja), crate::check::render(&o, Lang::En)));
    }
    crate::engine::with(&o, &run, |m, org| {
        let all = &m.s.file.scenarios;
        if all.is_empty() {
            return Err(tr!("`{path}` にシナリオがありません。ベクタは、シナリオの申請者とフォームから作ります", "`{path}` has no scenario: the vectors are made from the requester and the form of a scenario"));
        }
        let rans: Vec<_> = all.iter().map(|sc| crate::scenario::run(m, org, sc)).collect();
        if let Some(stop) = rans.iter().find_map(|r| r.stop.as_ref()) {
            let d = crate::scenario::diag(m, stop);
            return Err(Text::new(d.render(Lang::Ja), d.render(Lang::En)));
        }
        let vectors = crate::vectors::make(m, org, &rans);
        // each `on` line in a request of its own, with a machine of its own: the reference does the
        // lines of an ending at once, and the glue gives them one by one
        let copies = on_copies(m.s.file);
        let scopes: Vec<Scope> = copies.iter().map(|(_, f)| Scope { file: f, path: m.s.path, src: m.s.src, got: m.s.got.clone() }).collect();
        let lines = copies.iter().zip(&scopes).map(|((l, _), s)| Line { endings: l.endings.clone(), action: l.action, first: l.first.clone(), transfer: l.transfer.clone(), m: Machine::new(s, m.suite, m.lowered, m.usable) }).collect();
        Ok(f(&Model { m, org, vectors, lines }))
    })?
}

/// What an `on` line is: the endings it is of, what it does, its book or flow (by its alias as
/// written) and its transfer (none for a flow).
struct OnLine {
    endings: Vec<Ending>,
    action: ActionKind,
    first: String,
    transfer: String,
}

/// An `on` line with the machine of a copy of the request that has it alone among its `on` lines.
struct Line<'x> {
    endings: Vec<Ending>,
    action: ActionKind,
    first: String,
    transfer: String,
    m: Machine<'x, 'x>,
}

/// Each `on` line of the request, in the order written, with a copy of the request whose `ticket`
/// block keeps it and no other `on` line.
fn on_copies(f: &File) -> Vec<(OnLine, File)> {
    let Some(tk) = &f.ticket else { return Vec::new() };
    let mut out = Vec::new();
    for (k, l) in tk.lines.iter().enumerate() {
        let TicketLine::On { endings, action, call, .. } = l else { continue };
        let mut copy = f.clone();
        if let Some(t) = &mut copy.ticket {
            t.lines = t.lines.iter().enumerate().filter(|(j, x)| *j == k || !matches!(x, TicketLine::On { .. })).map(|(_, x)| x.clone()).collect();
        }
        let first = call.target[0].text.clone();
        let transfer = call.target.get(1).map(|w| w.text.clone()).unwrap_or_default();
        out.push((OnLine { endings: endings.iter().map(|(e, _)| *e).collect(), action: *action, first, transfer }, copy));
    }
    out
}

/// The machine of a request, its vectors, and the `on` lines the model asks of one by one.
pub struct Model<'x, 's, 'a> {
    pub m: &'x Machine<'s, 'a>,
    pub org: &'x Org,
    pub vectors: Vec<Vector>,
    lines: Vec<Line<'x>>,
}

/// A text as the glue writes it in kaname (its `quoted`): a `"` as `'`, a line's end as a space.
fn kn(s: &str) -> Json {
    Json::str(s.replace('"', "'").replace('\n', " "))
}

/// What the request has that the glue does not lower yet, as the glue says it.
fn not_yet(what: String) -> Text {
    Text::new(format!("申請の種類の {what} は、まだ kaname に下ろしません"), format!("the glue does not lower {what} yet"))
}

/// A variant of kaname's with fields, as its JSON writes it: `{"<value>": {<fields>}}`.
fn variant(name: &str, fields: Vec<(&str, Json)>) -> Json {
    Json::obj([(name, Json::obj(fields))])
}

fn ok(j: Json) -> Json {
    Json::obj([("ok", j)])
}

fn err(r: &Refusal) -> Json {
    Json::obj([("err", r.to_json())])
}

/// The ways a table of `when late escalate by <table>` sends a late stage to, for each key a part
/// of it can have, as the glue's `sends` writes them: `{"key", "to"}`, each way by its ASCII name.
fn sends(s: &Scope, file: &File, t: &NamedTable, keys: &[String]) -> Vec<Json> {
    let mut all: Vec<String> = Vec::new();
    for k in keys {
        if !all.contains(k) {
            all.push(k.clone());
        }
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < all.len() {
        let key = all[i].clone();
        let to = listing::table_sends(s, file, t, &key);
        for w in &to {
            if !all.contains(w) {
                all.push(w.clone());
            }
        }
        if !to.is_empty() {
            out.push(Json::obj([("key", kn(&words::ascii_of_who(s, &key))), ("to", Json::arr(to.iter().map(|w| kn(&words::ascii_of_who(s, w)))))]));
        }
        i += 1;
    }
    out
}

/// A group of `submitted by`, as the spec writes it.
fn group(g: &Group) -> Json {
    let within = |w: &Option<crate::ast::Word>| w.as_ref().map(|t| kn(&t.text)).unwrap_or(Json::Null);
    match g {
        Group::Everyone { within: w, .. } => variant("everyone", vec![("within", within(w))]),
        Group::Team(t) => variant("team", vec![("id", kn(&t.text))]),
        Group::Position { position, higher, within: w } => variant("position", vec![("position", kn(&position.text)), ("higher", Json::Bool(*higher)), ("within", within(w))]),
        Group::Person(id, _) => variant("person", vec![("id", kn(id))]),
    }
}

/// A ticket with nothing in it, of the request of the machine: what a question about a request
/// not made yet is answered on, and what the contract's shape is read through.
fn bare(m: &Machine) -> Ticket {
    let f = m.s.file;
    Ticket {
        id: String::new(),
        request: f.name.ascii().to_string(),
        version: f.version,
        requester: String::new(),
        made_by: None,
        sent_back_by: None,
        status: Status::InProgress,
        round: 1,
        attempt: String::new(),
        serial: 0,
        form: Values::new(),
        stages: Vec::new(),
        at: None,
        backs: 0,
        edits: 0,
        pause_left: 0,
        paused_at: None,
        pause_ends: None,
        resubmit_due: None,
        holds: Vec::new(),
        notes: Vec::new(),
        ended: None,
    }
}

/// The ASCII name of the field the name names (its name as written, or its ASCII name): `act.rs`'s
/// `ascii_of`, which it keeps to itself.
fn ascii_of(m: &Machine, n: &str) -> String {
    m.s.fields().iter().find(|f| f.name.ascii() == n || f.name.text == n).map(|f| f.name.ascii().to_string()).unwrap_or_else(|| n.to_string())
}

/// The values of a form given again or edited: the values `changed` laid over the ones the ticket
/// held (`was`), the fields `clear` names made empty, then each field the form does not show for
/// them dropped. `act.rs`'s `laid`, which it keeps to itself.
fn laid(m: &Machine, was: &Values, changed: &Values, clear: &[String]) -> Values {
    let mut given = was.clone();
    let mut kept = Vec::new();
    for (n, v) in changed.iter().filter(|(_, v)| *v != Value::None) {
        let ascii = ascii_of(m, n);
        match given.iter_mut().find(|(k, _)| *k == ascii) {
            Some(slot) => slot.1 = v.clone(),
            None => given.push((ascii.clone(), v.clone())),
        }
        kept.push(ascii);
    }
    let clear: Vec<String> = clear.iter().map(|n| ascii_of(m, n)).collect();
    given.retain(|(n, _)| !clear.contains(n));
    m.unshown(given, &kept)
}

/// The fields that held a value before an event and hold none after it, by their ASCII names, in
/// the order the form declares them. `act.rs`'s `lost`, which it keeps to itself.
fn lost(m: &Machine, was: &Values, now: &Values) -> Vec<String> {
    let has = |vals: &Values, n: &str| vals.iter().any(|(k, v)| k == n && *v != Value::None);
    m.s.fields().iter().filter(|f| f.computed.is_none()).map(|f| f.name.ascii().to_string()).filter(|n| has(was, n) && !has(now, n)).collect()
}

/// The fields `approvers edit` names, as written.
fn editable(m: &Machine) -> Vec<String> {
    m.line(|l| if let TicketLine::ApproversEdit { fields, .. } = l { Some(fields.iter().map(|w| w.text.clone()).collect()) } else { None }).unwrap_or_default()
}

/// The stage of a request, as the request writes it, by the name the ticket gives it.
fn written_stage(m: &Machine, name: &str) -> String {
    m.s.stages().unwrap_or_default().iter().find(|n| n.ascii() == name || n.text == name).map(|n| n.text.clone()).unwrap_or_else(|| name.to_string())
}

/// The JSON with the keys of each object in the order of their names: two values that differ only
/// in the order of their keys come to the same.
pub fn sorted(j: &Json) -> Json {
    match j {
        Json::Obj(o) => {
            let mut o: Vec<(String, Json)> = o.iter().map(|(k, v)| (k.clone(), sorted(v))).collect();
            o.sort_by(|a, b| a.0.cmp(&b.0));
            Json::Obj(o)
        }
        Json::Arr(a) => Json::Arr(a.iter().map(sorted).collect()),
        x => x.clone(),
    }
}

/// What a form or a set of values given is looked up by: its JSON, the keys in name order and a key
/// of null left out (a value of none is not given).
fn key(j: &Json) -> String {
    fn bare(j: &Json) -> Json {
        match j {
            Json::Obj(o) => Json::Obj(o.iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k.clone(), bare(v))).collect()),
            Json::Arr(a) => Json::Arr(a.iter().map(bare).collect()),
            x => x.clone(),
        }
    }
    sorted(&bare(j)).compact()
}

/// The ending of an `on` line by the name the machine gives it.
fn ending_of(w: &str) -> Option<Ending> {
    Some(match w {
        "submitted" => Ending::Submitted,
        "approved" => Ending::Approved,
        "sent_back" => Ending::SentBack,
        "rejected" => Ending::Rejected,
        "withdrawn" => Ending::Withdrawn,
        "expired" => Ending::Expired,
        _ => return None,
    })
}

/// An action of an `on` line by the name the machine gives it.
fn action_word(a: ActionKind) -> &'static str {
    match a {
        ActionKind::Hold => "hold",
        ActionKind::Post => "post",
        ActionKind::Void => "release",
        ActionKind::Launch => "launch",
    }
}

fn field<'j>(j: &'j Json, k: &str) -> Result<&'j Json, String> {
    j.get(k).ok_or_else(|| format!("no `{k}` in {}", j.compact()))
}

fn text<'j>(j: &'j Json, k: &str) -> Result<&'j str, String> {
    field(j, k)?.as_str().ok_or_else(|| format!("`{k}` is no text in {}", j.compact()))
}

fn int(j: &Json, k: &str) -> Result<i128, String> {
    field(j, k)?.as_int().ok_or_else(|| format!("`{k}` is no integer in {}", j.compact()))
}

impl<'x, 's, 'a> Model<'x, 's, 'a> {
    /// A way to choose an approver, as the glue's `how` writes it.
    fn how(&self, how: &How, written: &str) -> Result<Json, Text> {
        let (m, s) = (self.m, self.m.s);
        Ok(match how {
            How::ManagerOf => Json::str("manager_of"),
            How::Team(t) => variant("team", vec![("id", kn(&t.text))]),
            How::Person(id, _) => variant("person", vec![("id", kn(id))]),
            How::HeadOf(kind) => variant("head_of", vec![("team_kind", kn(m.names.team_kind(&kind.text)))]),
            How::Nearest(pos) => variant("nearest", vec![("position", kn(&pos.text))]),
            How::Lookup { path, by } if path.len() == 3 => {
                let shown = format!("{}.{}", path[0].text, path[1].text);
                let by_ascii = s.field(&by.text).map(|f| f.name.ascii().to_string()).unwrap_or_else(|| by.text.clone());
                variant(
                    "lookup",
                    vec![
                        ("master", kn(m.names.master(&shown))),
                        ("field", kn(m.names.master_field(&shown, &path[2].text))),
                        ("by", kn(&by_ascii)),
                        ("shown", kn(&shown)),
                        ("column", kn(&path[2].text)),
                        ("by_name", kn(&by.text)),
                    ],
                )
            }
            How::Lookup { .. } => return Err(not_yet(format!("the way to choose `{written}`, a lookup of no master's field"))),
            How::ChosenBy { previous, field } => {
                let ascii = s.field(&field.text).map(|f| f.name.ascii().to_string()).unwrap_or_else(|| field.text.clone());
                variant("chosen_by", vec![("field", kn(&ascii)), ("shown", kn(&field.text)), ("previous", Json::Bool(*previous))])
            }
            How::Agent { flow, otherwise } => variant("agent", vec![("flow", kn(m.names.used(&flow.text))), ("otherwise", kn(&words::ascii_of_who(s, &otherwise.text)))]),
        })
    }

    /// The request as the machine reads it, the glue's `spec()` written as kaname writes its JSON
    /// (a record an object, none `null`, a value of an enum with no fields a text, one with fields
    /// `{"<value>": {<fields>}}`, a date `YYYY-MM-DD`), read from what the glue reads it from; with
    /// `submitters`, the `submitted by` line the glue's `submitted_by` reads (its groups, the file's
    /// name and the line as written, which its refusal says), null for none. Err with what of the
    /// request the glue does not lower yet.
    pub fn spec(&self) -> Result<Json, Text> {
        let (m, s) = (self.m, self.m.s);
        let f = s.file;
        let day = |d: Option<i64>| d.map(|d| Json::str(crate::lex::date_text(d))).unwrap_or(Json::Null);
        let (from, through) = (m.usable.and_then(|u| u.from.as_ref().map(|e| e.day)), m.usable.and_then(|u| u.through.as_ref().map(|e| e.day)));
        let once = match listing::once(f) {
            listing::Once::No => "off",
            listing::Once::Before => "before",
            listing::Once::Last => "last",
        };
        let (mut pause, mut back, mut resubmit, mut withdraw, mut retract) = (Json::Null, Json::Null, Json::Null, Json::Null, false);
        for l in f.ticket.iter().flat_map(|t| &t.lines) {
            match l {
                TicketLine::Pause { days, .. } => pause = Json::int(*days),
                TicketLine::SendBack { to_requester, times, .. } => back = Json::obj([("to_requester", Json::Bool(*to_requester)), ("times", Json::int(*times))]),
                TicketLine::Resubmit { days, .. } => resubmit = Json::int(*days),
                TicketLine::Withdraw { until, .. } => withdraw = Json::obj([("until", until.as_ref().map(|w| kn(m.names.stage(&w.text))).unwrap_or(Json::Null))]),
                TicketLine::Retract { .. } => retract = true,
                _ => {}
            }
        }
        let order: Vec<Json> = s.stages().unwrap_or_default().iter().map(|n| kn(m.names.stage(&n.text))).collect();
        // the ways to choose: each once by its ASCII name, the request's own first
        let mut ways = Vec::new();
        let mut seen: Vec<(String, Json)> = Vec::new();
        for w in s.who_all() {
            let how = self.how(&w.who.how, &w.who.name.text)?;
            let ascii = w.who.name.ascii().to_string();
            if let Some((_, h)) = seen.iter().find(|(a, _)| *a == ascii) {
                if *h != how {
                    return Err(not_yet(format!("the way to choose `{}`, declared twice in two ways", w.who.name.text)));
                }
                continue;
            }
            seen.push((ascii.clone(), how.clone()));
            ways.push(Json::obj([("name", kn(&ascii)), ("written", kn(&w.who.name.text)), ("how", how)]));
        }
        // each stage of the route once by its name and value, as the reference finds it (the first)
        let mut found: Vec<(String, String)> = Vec::new();
        let mut occs = Vec::new();
        for o in m.paths.iter().flatten() {
            let stage = m.names.stage(&o.stage).to_string();
            let who = |w: &str| kn(&words::ascii_of_who(s, w));
            let value = words::spec_value(s, o.spec).map(|x| x.0).unwrap_or_default();
            let (shape, names, keys): (Json, Vec<Json>, Vec<String>) = match o.spec {
                Spec::Who(ws) => (Json::str("who"), ws.iter().map(|w| who(&w.text)).collect(), ws.iter().map(|w| w.text.clone()).collect()),
                Spec::Quorum(q, w, _) => {
                    let q = match q {
                        crate::ast::Quorum::All => Json::str("all"),
                        crate::ast::Quorum::AtLeast(n) => variant("at_least", vec![("n", Json::int(*n))]),
                        crate::ast::Quorum::Percent(p) => variant("percent", vec![("p", Json::int(*p))]),
                    };
                    (variant("quorum", vec![("q", q)]), vec![who(&w.text)], vec![w.text.clone()])
                }
                Spec::RuleOutput(p) => (Json::str("rule_output"), Vec::new(), crate::org::resolve::rule_ways(s, p)),
                Spec::Circulate(w) => (variant("circulate", vec![("way", who(&w.text))]), Vec::new(), Vec::new()),
                Spec::Note(w) => (variant("note", vec![("flow", kn(m.names.used(&w.text)))]), Vec::new(), Vec::new()),
                Spec::Empty | Spec::Route(_) => continue,
            };
            if found.iter().any(|(st, v)| *st == stage && *v == value) {
                continue;
            }
            let (empty, top_way) = match &o.empty {
                WhenEmpty::Stay => (Json::str("stay"), None),
                WhenEmpty::Skip => (Json::str("skip"), None),
                WhenEmpty::Escalate(top) => {
                    let (t, way) = match top {
                        None => (Json::Null, None),
                        Some(AtTop::Stay) => (Json::str("stay"), None),
                        Some(AtTop::Skip) => (Json::str("skip"), None),
                        Some(AtTop::To(w)) => (variant("to", vec![("way", who(&w.text))]), Some(w.text.clone())),
                    };
                    (variant("escalate", vec![("top", t)]), way)
                }
            };
            let late = match o.late {
                None => Json::Null,
                Some(l) => {
                    let mut keys = keys.clone();
                    keys.extend(top_way);
                    let act = |a: &Late| -> Json {
                        match a {
                            Late::Expire => Json::str("expire"),
                            Late::Stay => Json::str("stay"),
                            Late::Escalate(None) => variant("escalate", vec![("table", Json::Bool(false)), ("sends", Json::Arr(Vec::new()))]),
                            Late::Escalate(Some(tw)) => {
                                let sent = l.file.tables.iter().find(|x| x.name.text == tw.text).map(|t| sends(s, l.file, t, &keys)).unwrap_or_default();
                                variant("escalate", vec![("table", Json::Bool(true)), ("sends", Json::Arr(sent))])
                            }
                        }
                    };
                    Json::obj([("first", act(l.first)), ("after", l.then.map(act).unwrap_or(Json::Null))])
                }
            };
            occs.push(Json::obj([("stage", kn(&stage)), ("value", kn(&value)), ("shape", shape), ("ways", Json::Arr(names)), ("empty", empty), ("late", late)]));
            found.push((stage, value));
        }
        // the flows, by the names the JSON takes and as written, which the history names
        let flows: Vec<Json> = s.uses_of(crate::ast::UseKind::Flow).filter_map(|u| u.alias.as_ref()).map(|a| Json::obj([("name", kn(a.ascii())), ("shown", kn(&a.text))])).collect();
        // the stages as the request writes them, where that is not the name the ticket gives them
        let named: Vec<Json> =
            s.stages().unwrap_or_default().iter().filter(|n| m.names.stage(&n.text) != n.text).map(|n| Json::obj([("name", kn(m.names.stage(&n.text))), ("written", kn(&n.text))])).collect();
        // the offsets of the clocks that are not the request's: every stage's, a stage's own where it
        // is not every stage's, a pause's and the requester's turn's
        let entry = |timer: &str, stage: Option<&str>, o: i32| Json::obj([("timer", kn(timer)), ("stage", stage.map(kn).unwrap_or(Json::Null)), ("offset", Json::int(o))]);
        let mut offsets = Vec::new();
        for timer in ["remind", "late"] {
            let every = m.clock_offset(timer, None);
            if every != m.offset {
                offsets.push(entry(timer, None, every));
            }
            for n in s.stages().unwrap_or_default() {
                let own = m.clock_offset(timer, Some(&n.text));
                if own != every {
                    offsets.push(entry(timer, Some(m.names.stage(&n.text)), own));
                }
            }
        }
        for timer in ["pause ends", "resubmit late"] {
            let o = m.clock_offset(timer, None);
            if o != m.offset {
                offsets.push(entry(timer, None, o));
            }
        }
        let reasoned: Vec<Json> = f.ticket.iter().flat_map(|t| t.reasoned()).map(|o| kn(o.event())).collect();
        let submitters = match crate::org::submit::of(f) {
            None => Json::Null,
            Some(l) => {
                let file = std::path::Path::new(s.path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                Json::obj([("groups", Json::arr(l.groups.iter().map(group))), ("except", Json::arr(l.except.iter().map(group))), ("file", kn(&file)), ("shown", kn(&l.shown()))])
            }
        };
        Ok(Json::obj([
            ("request", kn(f.name.ascii())),
            ("version", Json::int(f.version)),
            ("offset", Json::int(m.offset)),
            ("usable_from", day(from)),
            ("usable_to", day(through)),
            ("self_approval", Json::Bool(listing::self_approval(f))),
            ("once", Json::str(once)),
            ("pause", pause),
            ("back", back),
            ("resubmit", resubmit),
            ("withdraw", withdraw),
            ("retract", Json::Bool(retract)),
            ("order", Json::Arr(order)),
            ("ways", Json::Arr(ways)),
            ("occs", Json::Arr(occs)),
            ("flows", Json::Arr(flows)),
            ("named", Json::Arr(named)),
            ("offsets", Json::Arr(offsets)),
            ("comment_required", Json::Arr(reasoned)),
            ("submitters", submitters),
        ]))
    }

    /// A vector as the model is given it: its line of `ringi vectors` without the answer (`request`,
    /// `from`, `now`, `ticket`, `event`, `organization`).
    pub fn line(&self, v: &Vector) -> Json {
        match v.to_json(self.m) {
            Json::Obj(o) => Json::Obj(o.into_iter().filter(|(k, _)| k != "answer").collect()),
            other => other,
        }
    }

    /// What the reference answered the vector: the `answer` of its line.
    pub fn answer(&self, v: &Vector) -> Json {
        v.to_json(self.m).get("answer").cloned().unwrap_or(Json::Null)
    }

    /// The questions of the model about the vector `v`, to be answered as they come.
    pub fn asking<'v>(&'v self, v: &'v Vector) -> Asking<'v, 'x, 's, 'a> {
        let mut forms = BTreeMap::new();
        if let Some(t) = &v.stepped.before {
            forms.insert(key(&values_json(&t.form)), t.form.clone());
        }
        let mut givens = BTreeMap::new();
        let given = match &v.stepped.input {
            crate::scenario::Input::Submit(sub) => Some(&sub.form),
            crate::scenario::Input::Event(crate::engine::Event::Resubmit { form, .. }) => Some(form),
            crate::scenario::Input::Event(crate::engine::Event::Edit { fields, .. }) => Some(fields),
            _ => None,
        };
        if let Some(g) = given {
            givens.insert(key(&values_json(g)), g.clone());
        }
        Asking { model: self, v, forms, givens }
    }

    /// The form, the stages and a list of values as the vectors write them: laid on a ticket (the
    /// list as the outputs of a note) and taken back out of its JSON in the contract's shape
    /// ([`crate::lower::shape`]), so that nothing here writes the contract's words of its own.
    fn shaped(&self, form: &Values, stages: &[Stage], vals: &Values) -> (Json, Json, Json) {
        let mut t = bare(self.m);
        t.form = form.clone();
        t.stages = stages.to_vec();
        t.notes = vec![(String::new(), vals.clone())];
        let j = crate::lower::shape::stepped(&Json::obj([("ticket", t.to_json(&self.m.names))]));
        let t = j.get("ticket").cloned().unwrap_or(Json::Null);
        let get = |k: &str| t.get(k).cloned().unwrap_or(Json::Null);
        let vals = t.get("notes").and_then(Json::as_arr).and_then(|n| n.first()).and_then(|n| n.get("outputs")).cloned().unwrap_or(Json::Arr(Vec::new()));
        (get("form"), get("stages"), vals)
    }
}

/// The questions of the model about one vector, answered by the reference interpreter on the
/// vector's part of the organization. The forms the model names are the ones it was given or
/// answered: the ticket's of the vector, and each answered to `reform` and `prepare`; the values
/// given, the event's.
pub struct Asking<'v, 'x, 's, 'a> {
    model: &'v Model<'x, 's, 'a>,
    v: &'v Vector,
    forms: BTreeMap<String, Values>,
    givens: BTreeMap<String, Values>,
}

impl Asking<'_, '_, '_, '_> {
    /// The answer to a question of the model. Err for a question that is not one, or names a form
    /// or values given that this vector has not met.
    pub fn answer(&mut self, q: &Json) -> Result<Json, String> {
        let Some([(kind, a)]) = q.as_obj() else { return Err(format!("a question is an object of one key: {}", q.compact())) };
        match kind.as_str() {
            "clocks" => self.clocks(a),
            "open_after" => self.open_after(a),
            "on" => self.on(a),
            "reform" => self.reform(a),
            "prepare" => self.prepare(a),
            "vals" => self.vals(a),
            _ => Err(format!("no such question: {}", q.compact())),
        }
    }

    fn form(&self, a: &Json) -> Result<Values, String> {
        let j = field(a, "form")?;
        self.forms.get(&key(j)).cloned().ok_or_else(|| format!("a form the test has not met: {}", j.compact()))
    }

    fn given(&self, a: &Json) -> Result<Values, String> {
        let j = field(a, "given")?;
        self.givens.get(&key(j)).cloned().ok_or_else(|| format!("values given the test has not met: {}", j.compact()))
    }

    /// The reminder and the deadline of a stage: [`Machine::clocks`], the stage by the name the
    /// ticket gives it.
    fn clocks(&self, a: &Json) -> Result<Json, String> {
        let m = self.model.m;
        let stage = written_stage(m, text(a, "stage")?);
        let started = i64::try_from(int(a, "started")?).map_err(|e| e.to_string())?;
        let held = u32::try_from(int(a, "held")?.max(0)).unwrap_or(u32::MAX);
        let up = u8::try_from(int(a, "up")?.clamp(0, 255)).unwrap_or(u8::MAX);
        Ok(match m.clocks(&stage, started, held, up) {
            Ok((remind, due)) => ok(Json::obj([("remind", time_json(remind)), ("due", time_json(due))])),
            Err(r) => err(&r),
        })
    }

    /// The `n`th business day after a day, as the glue's `open_after` reads it: refused (`unread`)
    /// for a key whose line the request does not have, and (`data`) where the line's calendar is
    /// not read; the day itself for none; else [`Machine::add_open`] in the calendar of the line.
    fn open_after(&self, a: &Json) -> Result<Json, String> {
        let m = self.model.m;
        let k = text(a, "key")?;
        let day = i64::try_from(int(a, "day")?).map_err(|e| e.to_string())?;
        let n = int(a, "n")?;
        let has = match k {
            "pause" => m.line(|l| matches!(l, TicketLine::Pause { .. }).then_some(())).is_some(),
            "resubmit" => m.line(|l| matches!(l, TicketLine::Resubmit { .. }).then_some(())).is_some(),
            _ => false,
        };
        if !has {
            return Ok(err(&Refusal::new("unread", tr!("この申請の種類は、`{k}` の営業日を数えません", "the request counts no business days for `{k}`"))));
        }
        let cal = if k == "pause" { m.pause_calendar() } else { m.resubmit_calendar() };
        let read = match cal {
            Some(c) => m.calendars.iter().any(|(x, _)| x == c),
            None => !m.calendars.is_empty(),
        };
        if !read {
            return Ok(err(&Refusal::new("data", tr!("営業日を数えるカレンダーがありません", "the request has no calendar to count business days in"))));
        }
        if n == 0 {
            return Ok(ok(Json::int(day)));
        }
        let n = u32::try_from(n).map_err(|e| e.to_string())?;
        Ok(match m.add_open(cal, day, n) {
            Ok(d) => ok(Json::int(d)),
            Err(r) => err(&r),
        })
    }

    /// The ticket a question about the `on` lines is answered on: the vector's (a ticket with nothing
    /// in it for a request made), with the ID, the round, the try, the requester and the one who
    /// made it in their name of `r`, and the form `form`.
    fn ticket(&self, r: &Json, form: Values) -> Result<Ticket, String> {
        let mut t = self.v.stepped.before.clone().unwrap_or_else(|| bare(self.model.m));
        t.id = text(r, "id")?.to_string();
        t.round = u32::try_from(int(r, "round")?).map_err(|e| e.to_string())?;
        t.attempt = text(r, "attempt")?.to_string();
        t.requester = text(r, "requester")?.to_string();
        t.made_by = match field(r, "made_by")? {
            Json::Str(s) => Some(s.clone()),
            Json::Null => None,
            x => return Err(format!("`made_by` is no text: {}", x.compact())),
        };
        t.form = form;
        Ok(t)
    }

    /// Each `on` line of the ending, in the order written, as the glue's `on_lines` gives it: its
    /// action, its book (or flow) and transfer, whether it is done, and its arguments. Each is asked
    /// of the machine of a copy of the request with that line alone ([`Machine::on`]), on the
    /// ticket with the hold of its round put on it for a `post` and a `void`, which the reference
    /// does only while the hold is open. No thing to do is `when` false; one is `when` true with its
    /// arguments; a refusal is both `when` and `args`, as the machine stops at either.
    fn on(&self, a: &Json) -> Result<Json, String> {
        let m = self.model.m;
        let w = text(a, "ending")?;
        let ending = ending_of(w).ok_or_else(|| format!("no such ending: `{w}`"))?;
        let base = self.ticket(field(a, "ref")?, self.form(a)?)?;
        let part = &self.v.stepped.part;
        let mut out = Vec::new();
        for l in self.model.lines.iter().filter(|l| l.endings.contains(&ending)) {
            let mut t = base.clone();
            t.holds = if matches!(l.action, ActionKind::Post | ActionKind::Void) { vec![Held { book: l.first.clone(), transfer: l.transfer.clone(), round: t.round }] } else { Vec::new() };
            let mut fx = Effects::default();
            let (when, args) = match l.m.on(ending, &mut t, part, &mut fx) {
                Err(r) => (err(&r), err(&r)),
                Ok(()) => {
                    let done = fx.done(&mut t);
                    match done.as_slice() {
                        [] => (ok(Json::Bool(false)), ok(Json::Arr(Vec::new()))),
                        [e] => match &e.what {
                            What::Book { args, .. } | What::Launch { args, .. } => (ok(Json::Bool(true)), ok(self.model.shaped(&Values::new(), &[], args).2)),
                            other => return Err(format!("an `on` line did what no `on` line does: {other:?}")),
                        },
                        more => return Err(format!("an `on` line did {} things", more.len())),
                    }
                }
            };
            out.push(Json::obj([("action", Json::str(action_word(l.action))), ("book", kn(m.names.used(&l.first))), ("transfer", kn(&l.transfer)), ("when", when), ("args", args)]));
        }
        Ok(Json::Arr(out))
    }

    /// A form given again (`approver` false) or edited (true), as the reference's `resubmit` and
    /// `edit` take it: the values given and the fields made empty held to the form and to who gives
    /// them, laid over what the ticket held and the form made of them; with the values a flow is
    /// given, the fields given (`named`, in the order the form declares them) and the fields that
    /// lost their value (`cleared`).
    fn reform(&mut self, a: &Json) -> Result<Json, String> {
        let form = self.form(a)?;
        let given = self.given(a)?;
        let clear: Vec<String> = field(a, "clear")?.as_arr().ok_or("`clear` is no list")?.iter().map(|c| c.as_str().map(str::to_string).ok_or_else(|| format!("a name of `clear` is no text: {}", c.compact()))).collect::<Result<_, _>>()?;
        let approver = field(a, "approver")?.as_bool().ok_or("`approver` is no truth")?;
        let (m, part) = (self.model.m, &self.v.stepped.part);
        let (giver, editable) = if approver { (Giver::Approver, editable(m)) } else { (Giver::Requester, Vec::new()) };
        if let Some(said) = taken::given(m.s, &given, giver, &editable, &m.when(part)).or_else(|| taken::cleared(m.s, &clear, &given, giver, &editable)) {
            return Ok(err(&Refusal::form(said)));
        }
        let was = m.given_of(&form);
        let (now, stages) = match m.formed(&laid(m, &was, &given, &clear), part) {
            Ok(x) => x,
            Err(r) => return Ok(err(&r)),
        };
        let cleared = lost(m, &was, &now);
        let named = m.in_form_order(given.iter().filter(|(_, v)| *v != Value::None).map(|(n, v)| (ascii_of(m, n), v.clone())).collect());
        let (f, st, vals) = self.model.shaped(&now, &stages, &now);
        self.forms.insert(key(&f), now);
        Ok(ok(Json::obj([("form", f), ("stages", st), ("vals", vals), ("named", Json::arr(named.into_iter().map(|(n, _)| Json::str(n)))), ("cleared", Json::arr(cleared.into_iter().map(Json::str)))])))
    }

    /// The form of a request made, as the reference's `start` makes it: the values given held to
    /// the form ([`taken::given`]), then the form and the stages of them ([`Machine::formed`]); with
    /// the values a flow is given.
    fn prepare(&mut self, a: &Json) -> Result<Json, String> {
        let given = self.given(a)?;
        let (m, part) = (self.model.m, &self.v.stepped.part);
        if let Some(said) = taken::given(m.s, &given, Giver::Requester, &[], &m.when(part)) {
            return Ok(err(&Refusal::form(said)));
        }
        Ok(match m.formed(&given, part) {
            Err(r) => err(&r),
            Ok((form, stages)) => {
                let (f, st, vals) = self.model.shaped(&form, &stages, &form);
                self.forms.insert(key(&f), form);
                ok(Json::obj([("form", f), ("stages", st), ("vals", vals)]))
            }
        })
    }

    /// The values of a form a flow is given: the form as the ticket holds it, what the reference
    /// gives with `request_id` before it (`flow_args`).
    fn vals(&self, a: &Json) -> Result<Json, String> {
        let form = self.form(a)?;
        Ok(self.model.shaped(&form, &[], &form).2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Event, Timer};
    use crate::scenario::Input;
    use std::collections::BTreeSet;
    use std::rc::Rc;
    use std::sync::Arc;

    /// Every language joined, as the crate's tests join them (`tests/common`).
    fn joined() -> Suite {
        let rulec = Rc::new(rulec::ports::Engine::with_dates(Arc::new(koyomi::ports::Engine)));
        let koyomi = Rc::new(koyomi::ports::Engine);
        let chobo = Rc::new(chobo::ports::Engine);
        let dandori = Rc::new(dandori::ports::Engine::default());
        let gates = ritsu_ports::GatePorts { rules: rulec.clone(), dates: koyomi.clone(), books: chobo.clone(), flows: dandori.clone(), items: dandori.clone() };
        let sekisho = Rc::new(sekisho::ports::Engine::new(gates.into()));
        Suite { rules: Some(rulec), dates: Some(koyomi), books: Some(chobo), flows: Some(dandori), gates: Some(sekisho), programs: Some(Rc::new(kaname::ports::Engine)) }
    }

    const EXAMPLES: [&str; 10] = [
        "examples/account.ringi",
        "examples/account.ja.ringi",
        "examples/contract.ringi",
        "examples/contract.ja.ringi",
        "examples/expense.ringi",
        "examples/expense.ja.ringi",
        "examples/leave.ringi",
        "examples/leave.ja.ringi",
        "examples/purchase.ringi",
        "examples/purchase.ja.ringi",
    ];

    const ENDINGS: [Ending; 6] = [Ending::Submitted, Ending::Approved, Ending::SentBack, Ending::Rejected, Ending::Withdrawn, Ending::Expired];

    fn ending_word(e: Ending) -> &'static str {
        match e {
            Ending::Submitted => "submitted",
            Ending::Approved => "approved",
            Ending::SentBack => "sent_back",
            Ending::Rejected => "rejected",
            Ending::Withdrawn => "withdrawn",
            Ending::Expired => "expired",
        }
    }

    fn of<R>(path: &str, f: impl FnOnce(&Model) -> R) -> R {
        with(path, joined(), f).unwrap_or_else(|e| panic!("{path}: {}", e.en))
    }

    fn q(kind: &str, pairs: Vec<(&str, Json)>) -> Json {
        Json::obj([(kind, Json::obj(pairs))])
    }

    /// The `ref` of a ticket, as the machine gives it to `on_lines`.
    fn ref_of(t: &Ticket) -> Json {
        Json::obj([("id", Json::str(&t.id)), ("round", Json::int(t.round)), ("attempt", Json::str(&t.attempt)), ("requester", Json::str(&t.requester)), ("made_by", Json::opt_str(t.made_by.clone()))])
    }

    // the shapes the spec is written in: each a value of kaname's JSON
    type Fits = fn(&Json) -> bool;

    fn txt(j: &Json) -> bool {
        j.as_str().is_some()
    }

    fn num(j: &Json) -> bool {
        j.as_int().is_some()
    }

    fn truth(j: &Json) -> bool {
        j.as_bool().is_some()
    }

    fn txts(j: &Json) -> bool {
        list(j, txt)
    }

    fn opt(j: &Json, f: Fits) -> bool {
        j.is_null() || f(j)
    }

    fn list(j: &Json, f: Fits) -> bool {
        j.as_arr().is_some_and(|a| a.iter().all(f))
    }

    /// An object of exactly the keys, in that order, each of its shape.
    fn rec(j: &Json, keys: &[(&str, Fits)]) -> bool {
        j.as_obj().is_some_and(|o| o.len() == keys.len() && o.iter().zip(keys).all(|((k, v), (want, f))| k == want && f(v)))
    }

    /// A value of an enum: a text of a value with no fields, or an object of one value with fields.
    fn one_of(j: &Json, plain: &[&str], with: &[(&str, Fits)]) -> bool {
        match j {
            Json::Str(s) => plain.contains(&s.as_str()),
            Json::Obj(o) if o.len() == 1 => with.iter().any(|(k, f)| o[0].0 == *k && f(&o[0].1)),
            _ => false,
        }
    }

    fn how(j: &Json) -> bool {
        one_of(j, &["manager_of"], &[
            ("head_of", |x| rec(x, &[("team_kind", txt)])),
            ("nearest", |x| rec(x, &[("position", txt)])),
            ("team", |x| rec(x, &[("id", txt)])),
            ("person", |x| rec(x, &[("id", txt)])),
            ("lookup", |x| rec(x, &[("master", txt), ("field", txt), ("by", txt), ("shown", txt), ("column", txt), ("by_name", txt)])),
            ("chosen_by", |x| rec(x, &[("field", txt), ("shown", txt), ("previous", truth)])),
            ("agent", |x| rec(x, &[("flow", txt), ("otherwise", txt)])),
        ])
    }

    fn quorum(j: &Json) -> bool {
        one_of(j, &["all"], &[("at_least", |x| rec(x, &[("n", num)])), ("percent", |x| rec(x, &[("p", num)]))])
    }

    fn shape(j: &Json) -> bool {
        one_of(j, &["who", "rule_output"], &[("quorum", |x| rec(x, &[("q", quorum)])), ("circulate", |x| rec(x, &[("way", txt)])), ("note", |x| rec(x, &[("flow", txt)]))])
    }

    fn top(j: &Json) -> bool {
        j.is_null() || one_of(j, &["stay", "skip"], &[("to", |x| rec(x, &[("way", txt)]))])
    }

    fn empty(j: &Json) -> bool {
        one_of(j, &["stay", "skip"], &[("escalate", |x| rec(x, &[("top", top)]))])
    }

    fn send(j: &Json) -> bool {
        rec(j, &[("key", txt), ("to", txts)])
    }

    fn act(j: &Json) -> bool {
        one_of(j, &["expire", "stay"], &[("escalate", |x| rec(x, &[("table", truth), ("sends", |s| list(s, send))]))])
    }

    fn late(j: &Json) -> bool {
        j.is_null() || rec(j, &[("first", act), ("after", |a| opt(a, act))])
    }

    fn occ(j: &Json) -> bool {
        rec(j, &[("stage", txt), ("value", txt), ("shape", shape), ("ways", txts), ("empty", empty), ("late", late)])
    }

    fn group(j: &Json) -> bool {
        one_of(j, &[], &[
            ("everyone", |x| rec(x, &[("within", |w| opt(w, txt))])),
            ("team", |x| rec(x, &[("id", txt)])),
            ("position", |x| rec(x, &[("position", txt), ("higher", truth), ("within", |w| opt(w, txt))])),
            ("person", |x| rec(x, &[("id", txt)])),
        ])
    }

    fn day(j: &Json) -> bool {
        j.as_str().is_some_and(|d| ritsu_ports::day_of(d).is_some())
    }

    fn spec_fits(j: &Json) -> bool {
        rec(j, &[
            ("request", txt),
            ("version", num),
            ("offset", num),
            ("usable_from", |d| opt(d, day)),
            ("usable_to", |d| opt(d, day)),
            ("self_approval", truth),
            ("once", |o| one_of(o, &["off", "before", "last"], &[])),
            ("pause", |p| opt(p, num)),
            ("back", |b| opt(b, |b| rec(b, &[("to_requester", truth), ("times", num)]))),
            ("resubmit", |r| opt(r, num)),
            ("withdraw", |w| opt(w, |w| rec(w, &[("until", |u| opt(u, txt))]))),
            ("retract", truth),
            ("order", txts),
            ("ways", |w| list(w, |w| rec(w, &[("name", txt), ("written", txt), ("how", how)]))),
            ("occs", |o| list(o, occ)),
            ("flows", |f| list(f, |f| rec(f, &[("name", txt), ("shown", txt)]))),
            ("named", |n| list(n, |n| rec(n, &[("name", txt), ("written", txt)]))),
            ("offsets", |o| list(o, |o| rec(o, &[("timer", txt), ("stage", |s| opt(s, txt)), ("offset", num)]))),
            ("comment_required", txts),
            ("submitters", |s| opt(s, |s| rec(s, &[("groups", |g| list(g, group)), ("except", |g| list(g, group)), ("file", txt), ("shown", txt)]))),
        ])
    }

    #[test]
    fn the_spec_is_written_in_the_shapes_the_glue_gives() {
        for path in EXAMPLES {
            of(path, |m| {
                let s = m.spec().unwrap_or_else(|e| panic!("{path}: {}", e.en));
                assert!(spec_fits(&s), "{path}: {}", s.pretty());
                assert!(!s.get("occs").and_then(Json::as_arr).unwrap_or_default().is_empty(), "{path}: no stage");
            });
        }
        of("examples/leave.ringi", |m| {
            let s = m.spec().unwrap();
            let got = |k: &str| s.get(k).map(Json::compact).unwrap_or_default();
            assert_eq!(got("request"), r#""leave""#);
            assert_eq!(got("version"), "1");
            assert_eq!(got("self_approval"), "false");
            assert_eq!(got("once"), r#""before""#);
            assert_eq!(got("pause"), "null");
            assert_eq!(got("back"), r#"{"to_requester":true,"times":2}"#);
            assert_eq!(got("resubmit"), "5");
            assert_eq!(got("withdraw"), r#"{"until":null}"#);
            assert_eq!(got("retract"), "false");
            assert_eq!(got("comment_required"), r#"["reject","send_back"]"#);
            assert_eq!(got("named"), "[]");
            assert_eq!(got("submitters"), r#"{"groups":[{"everyone":{"within":null}}],"except":[{"team":{"id":"board"}}],"file":"leave.ringi","shown":"everyone except team board"}"#);
        });
        // a request in Japanese names its stages as it writes them, and the ticket by the ASCII names
        of("examples/expense.ja.ringi", |m| {
            let s = m.spec().unwrap();
            assert_eq!(s.get("named").and_then(Json::as_arr).and_then(|n| n.first()).map(Json::compact).as_deref(), Some(r#"{"name":"precheck","written":"確かめ"}"#), "{}", s.pretty());
        });
    }

    #[test]
    fn the_clocks_answered_are_the_ones_the_tickets_set() {
        let mut checked = 0;
        for path in EXAMPLES {
            of(path, |m| {
                for v in &m.vectors {
                    let Ok((t, fx)) = &v.stepped.answer else { continue };
                    let Some(st) = t.stage() else { continue };
                    let Some(started) = st.started else { continue };
                    let set: Vec<(Timer, i64)> = fx
                        .iter()
                        .filter_map(|e| match &e.what {
                            What::SetTimer { timer: timer @ (Timer::Remind | Timer::Late), stage: Some(s), at } if *s == st.name => Some((*timer, *at)),
                            _ => None,
                        })
                        .collect();
                    if set.is_empty() {
                        continue;
                    }
                    let ask = q("clocks", vec![("stage", Json::str(m.m.names.stage(&st.name))), ("started", Json::int(started)), ("held", Json::int(st.held)), ("up", Json::int(st.up))]);
                    let a = m.asking(v).answer(&ask).unwrap();
                    let got = a.get("ok").unwrap_or_else(|| panic!("{path}: {}: {}", v.from, a.compact()));
                    let read = |k: &str| got.get(k).and_then(Json::as_int).map(|x| x as i64);
                    // set as the machine sets them: none before the time, the reminder before the deadline
                    let (remind, due) = Machine::set_at(read("remind"), read("due"), v.stepped.now);
                    for (timer, at) in set {
                        let want = if timer == Timer::Remind { remind } else { due };
                        assert_eq!(want, Some(at), "{path}: {}: {timer:?} of `{}`", v.from, st.name);
                        checked += 1;
                    }
                }
            });
        }
        assert!(checked >= 100, "the clocks checked are fewer than they were: {checked}");
    }

    /// The `on` lines the model was given, done as the machine's code does them (`on_line` of
    /// `lower/ticket.kn`): a post and a void only while the hold of the round is open, a line not
    /// done where its condition is false, and the first refusal stopping the ending. What it comes
    /// to: the holds left (each book, transfer and round) and what is to do, or the refusal.
    #[allow(clippy::type_complexity)]
    fn flowed(round: u32, mut holds: Vec<(String, String, u32)>, lines: &Json) -> Result<(Vec<(String, String, u32)>, Vec<Json>), Json> {
        let mut made = Vec::new();
        for l in lines.as_arr().unwrap() {
            let (action, book, transfer) = (l.get("action").and_then(Json::as_str).unwrap(), l.get("book").and_then(Json::as_str).unwrap(), l.get("transfer").and_then(Json::as_str).unwrap());
            let open = holds.iter().position(|h| h.0 == book && h.1 == transfer && h.2 == round);
            if matches!(action, "post" | "release") && open.is_none() {
                continue;
            }
            let when = l.get("when").unwrap();
            if let Some(e) = when.get("err") {
                return Err(e.clone());
            }
            if when.get("ok") != Some(&Json::Bool(true)) {
                continue;
            }
            let args = l.get("args").unwrap();
            if let Some(e) = args.get("err") {
                return Err(e.clone());
            }
            let did = Json::obj([("action", Json::str(action)), ("book", Json::str(book)), ("transfer", Json::str(transfer)), ("args", args.get("ok").unwrap().clone())]);
            match (action, open) {
                ("hold", _) => holds.push((book.to_string(), transfer.to_string(), round)),
                ("post" | "release", Some(k)) => {
                    holds.remove(k);
                }
                _ => {}
            }
            made.push(did);
        }
        Ok((holds, made))
    }

    #[test]
    fn the_on_lines_one_by_one_do_what_the_machine_does() {
        let (mut checked, mut did) = (0, BTreeSet::new());
        for path in EXAMPLES {
            of(path, |m| {
                let n = &m.m.names;
                let holds = |t: &Ticket| t.holds.iter().map(|h| (n.used(&h.book).to_string(), h.transfer.clone(), h.round)).collect::<Vec<_>>();
                let mut seen = BTreeSet::new();
                for v in &m.vectors {
                    let Ok((t, _)) = &v.stepped.answer else { continue };
                    for e in ENDINGS {
                        if !seen.insert(format!("{e:?} {} {} {} {} {:?} {}", t.id, t.round, t.attempt, t.requester, t.holds, key(&values_json(&t.form)))) {
                            continue;
                        }
                        // the reference, every line of the ending at once
                        let mut after = t.clone();
                        let mut fx = Effects::default();
                        let want = match m.m.on(e, &mut after, &v.stepped.part, &mut fx) {
                            Err(r) => Err(r.to_json()),
                            Ok(()) => {
                                let made = fx
                                    .done(&mut after)
                                    .iter()
                                    .map(|x| {
                                        let (action, book, transfer, args) = match &x.what {
                                            What::Book { op, book, transfer, args } => (if *op == "void" { "release" } else { op }, book.as_str(), transfer.as_str(), args),
                                            What::Launch { flow, args } => ("launch", flow.as_str(), "", args),
                                            other => panic!("{path}: an `on` line did {other:?}"),
                                        };
                                        Json::obj([("action", Json::str(action)), ("book", Json::str(n.used(book))), ("transfer", Json::str(transfer)), ("args", m.shaped(&Values::new(), &[], args).2)])
                                    })
                                    .collect::<Vec<_>>();
                                Ok((holds(&after), made))
                            }
                        };
                        // the model's lines, each asked of the reference alone, done as the machine does them
                        let mut asking = m.asking(v);
                        asking.forms.insert(key(&values_json(&t.form)), t.form.clone());
                        let lines = asking.answer(&q("on", vec![("ending", Json::str(ending_word(e))), ("ref", ref_of(t)), ("form", values_json(&t.form))])).unwrap();
                        let got = flowed(t.round, holds(t), &lines);
                        assert_eq!(got, want, "{path}: {}: on {}: {}", v.from, ending_word(e), lines.compact());
                        if let Ok((_, made)) = &want {
                            did.extend(made.iter().filter_map(|x| x.get("action").and_then(Json::as_str).map(str::to_string)));
                        }
                        checked += 1;
                    }
                }
            });
        }
        assert_eq!(did.into_iter().collect::<Vec<_>>(), ["hold", "launch", "post", "release"]);
        assert!(checked >= 500, "the endings checked are fewer than they were: {checked}");
    }

    #[test]
    fn the_forms_answered_are_the_ones_the_tickets_hold() {
        let (mut made, mut again, mut edited, mut flows) = (0, 0, 0, 0);
        for path in EXAMPLES {
            of(path, |m| {
                let stages = |j: &Json| j.as_arr().unwrap_or_default().iter().map(|s| (s.get("name").cloned(), s.get("value").cloned())).collect::<Vec<_>>();
                let tick = |t: &Ticket| stages(&m.shaped(&t.form, &t.stages, &Values::new()).1);
                for v in &m.vectors {
                    let Ok((t, fx)) = &v.stepped.answer else { continue };
                    let form = values_json(&t.form);
                    let mut asking = m.asking(v);
                    let record = fx.iter().find_map(|e| if let What::Record(r) = &e.what { Some(r) } else { None });
                    match &v.stepped.input {
                        Input::Submit(sub) => {
                            let a = asking.answer(&q("prepare", vec![("given", values_json(&sub.form))])).unwrap();
                            let ok = a.get("ok").unwrap_or_else(|| panic!("{path}: {}: {}", v.from, a.compact()));
                            assert_eq!(ok.get("form"), Some(&form), "{path}: {}", v.from);
                            assert_eq!(stages(ok.get("stages").unwrap()), tick(t), "{path}: {}", v.from);
                            made += 1;
                        }
                        Input::Event(Event::Resubmit { form: given, clear, .. }) => {
                            let a = asking.answer(&q("reform", vec![("form", values_json(&v.stepped.before.as_ref().unwrap().form)), ("given", values_json(given)), ("clear", Json::arr(clear.iter().map(Json::str))), ("approver", Json::Bool(false))])).unwrap();
                            let ok = a.get("ok").unwrap_or_else(|| panic!("{path}: {}: {}", v.from, a.compact()));
                            assert_eq!(ok.get("form"), Some(&form), "{path}: {}", v.from);
                            assert_eq!(stages(ok.get("stages").unwrap()), tick(t), "{path}: {}", v.from);
                            let r = record.unwrap();
                            assert_eq!(ok.get("cleared"), Some(&Json::arr(r.cleared.iter().map(Json::str))), "{path}: {}", v.from);
                            again += 1;
                        }
                        Input::Event(Event::Edit { fields, clear, .. }) => {
                            let a = asking.answer(&q("reform", vec![("form", values_json(&v.stepped.before.as_ref().unwrap().form)), ("given", values_json(fields)), ("clear", Json::arr(clear.iter().map(Json::str))), ("approver", Json::Bool(true))])).unwrap();
                            let ok = a.get("ok").unwrap_or_else(|| panic!("{path}: {}: {}", v.from, a.compact()));
                            assert_eq!(ok.get("form"), Some(&form), "{path}: {}", v.from);
                            let r = record.unwrap();
                            assert_eq!(ok.get("named"), Some(&Json::arr(r.to.iter().map(Json::str))), "{path}: {}", v.from);
                            assert_eq!(ok.get("cleared"), Some(&Json::arr(r.cleared.iter().map(Json::str))), "{path}: {}", v.from);
                            edited += 1;
                        }
                        _ => {}
                    }
                    // what an agent and a flow summing up are given: `vals` with the ticket's ID before it
                    for e in fx {
                        let (What::AskAgent { args, .. } | What::AskNote { args, .. }) = &e.what else { continue };
                        asking.forms.insert(key(&form), t.form.clone());
                        let vals = asking.answer(&q("vals", vec![("form", form.clone())])).unwrap();
                        assert_eq!(m.shaped(&Values::new(), &[], &args[1..].to_vec()).2, vals, "{path}: {}", v.from);
                        flows += 1;
                    }
                }
            });
        }
        assert!(made >= 50 && again >= 10 && edited >= 5 && flows >= 5, "fewer than they were: {made} made, {again} made again, {edited} edited, {flows} flows asked");
    }

    #[test]
    fn open_after_counts_as_the_glue_does() {
        of("examples/leave.ringi", |m| {
            let v = &m.vectors[0];
            let mut asking = m.asking(v);
            let day = m.m.day(v.stepped.now);
            let ask = |k: &str, n: i128| q("open_after", vec![("key", Json::str(k)), ("day", Json::int(day)), ("n", Json::int(n))]);
            assert_eq!(asking.answer(&ask("resubmit", 0)).unwrap(), ok(Json::int(day)));
            let five = asking.answer(&ask("resubmit", 5)).unwrap();
            assert_eq!(five, ok(Json::int(m.m.add_open(m.m.resubmit_calendar(), day, 5).unwrap())));
            assert!(five.get("ok").and_then(Json::as_int).unwrap() >= i128::from(day) + 5);
            // leave has no pause: its days are counted for nothing, the day itself none either
            for n in [0, 1] {
                let a = asking.answer(&ask("pause", n)).unwrap();
                assert_eq!(a.get("err").and_then(|e| e.get("refused")).and_then(Json::as_str), Some("unread"), "{}", a.compact());
            }
        });
    }

    /// Each question answered for leave, as the model asks it; with `RINGI_MODEL_DUMP=<directory>`,
    /// written there with the spec (`leave.spec.json`, `leave.answers.jsonl`), for the Lean side to
    /// read before it asks any.
    #[test]
    fn each_question_is_answered_for_leave() {
        of("examples/leave.ringi", |m| {
            let spec = m.spec().unwrap();
            let mut said: Vec<Json> = Vec::new();
            let mut ask = |asking: &mut Asking, from: &str, question: Json| {
                let a = asking.answer(&question).unwrap_or_else(|e| panic!("{from}: {e}"));
                said.push(Json::obj([("from", Json::str(from)), ("q", question), ("a", a.clone())]));
                a
            };
            // a request made: its form, what a flow is given, its first stage's clocks, its `on` lines
            let v = m.vectors.iter().find(|v| matches!(v.stepped.input, Input::Submit(_)) && v.stepped.answer.is_ok()).unwrap();
            let (Input::Submit(sub), Ok((t, _))) = (&v.stepped.input, &v.stepped.answer) else { unreachable!() };
            let mut asking = m.asking(v);
            let prepared = ask(&mut asking, &v.from, q("prepare", vec![("given", values_json(&sub.form))]));
            let form = prepared.get("ok").and_then(|o| o.get("form")).cloned().unwrap();
            ask(&mut asking, &v.from, q("vals", vec![("form", form.clone())]));
            let st = t.stage().unwrap();
            ask(&mut asking, &v.from, q("clocks", vec![("stage", Json::str(m.m.names.stage(&st.name))), ("started", Json::int(st.started.unwrap())), ("held", Json::int(st.held)), ("up", Json::int(st.up))]));
            let day = m.m.day(v.stepped.now);
            for (k, n) in [("resubmit", 0), ("resubmit", 5), ("pause", 1)] {
                ask(&mut asking, &v.from, q("open_after", vec![("key", Json::str(k)), ("day", Json::int(day)), ("n", Json::int(n))]));
            }
            for e in ENDINGS {
                ask(&mut asking, &v.from, q("on", vec![("ending", Json::str(ending_word(e))), ("ref", ref_of(t)), ("form", form.clone())]));
            }
            // a form given again, and one an approver edits
            for want in ["resubmit", "edit"] {
                let v = m.vectors.iter().find(|v| matches!(&v.stepped.input, Input::Event(e) if e.word() == want)).unwrap();
                let before = v.stepped.before.as_ref().unwrap();
                let (given, clear) = match &v.stepped.input {
                    Input::Event(Event::Resubmit { form, clear, .. }) => (form, clear),
                    Input::Event(Event::Edit { fields, clear, .. }) => (fields, clear),
                    _ => unreachable!(),
                };
                let mut asking = m.asking(v);
                ask(&mut asking, &v.from, q("reform", vec![("form", values_json(&before.form)), ("given", values_json(given)), ("clear", Json::arr(clear.iter().map(Json::str))), ("approver", Json::Bool(want == "edit"))]));
            }
            let kinds: BTreeSet<String> = said.iter().filter_map(|x| x.get("q").and_then(Json::as_obj).and_then(|o| o.first()).map(|(k, _)| k.clone())).collect();
            assert_eq!(kinds.into_iter().collect::<Vec<_>>(), ["clocks", "on", "open_after", "prepare", "reform", "vals"]);
            if let Some(dir) = std::env::var_os("RINGI_MODEL_DUMP") {
                let dir = std::path::PathBuf::from(dir);
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(dir.join("leave.spec.json"), spec.pretty() + "\n").unwrap();
                std::fs::write(dir.join("leave.answers.jsonl"), said.iter().map(|x| x.compact() + "\n").collect::<String>()).unwrap();
            }
        });
    }
}
