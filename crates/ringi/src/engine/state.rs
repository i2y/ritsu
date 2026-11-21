//! A ticket (DESIGN 7.1): its state, the stages of its route with the approvers and the answers
//! of each, its clocks, its open holds. Everything `next` reads of a ticket is here; it is plain
//! data, written to JSON for the vectors (DESIGN 10.4).

use ritsu_base::json::Json;
use ritsu_ports::{Value, Values};

/// The states of a ticket (DESIGN 7.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    InProgress,
    Paused,
    WithRequester,
    Approved,
    Rejected,
    Withdrawn,
    Expired,
}

impl Status {
    /// The state as a scenario and the JSON write it.
    pub fn word(self) -> &'static str {
        match self {
            Status::InProgress => "in progress",
            Status::Paused => "paused",
            Status::WithRequester => "with requester",
            Status::Approved => "approved",
            Status::Rejected => "rejected",
            Status::Withdrawn => "withdrawn",
            Status::Expired => "expired",
        }
    }

    /// Whether the ticket has ended.
    pub fn ended(self) -> bool {
        matches!(self, Status::Approved | Status::Rejected | Status::Withdrawn | Status::Expired)
    }
}

/// An answer to a part of a stage: who gave it, whose answer it counts as (the approver a proxy
/// answered for; the same person otherwise), and whether it approves.
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    pub by: String,
    pub seat: String,
    pub yes: bool,
    pub at: i64,
}

/// Where a part of a stage stands.
#[derive(Clone, Debug, PartialEq)]
pub enum PartState {
    /// It waits for its approvers.
    Open,
    /// It has the approvals it needs.
    Passed,
    /// Nobody answers it, and the stage says to skip it.
    Skipped,
    /// Nobody answers it, and it waits for an administrator to add an approver.
    Stopped,
    /// It waits for the agent's flow to answer.
    Agent(String),
    /// The agent's flow answered `ok`.
    Checked(String),
    /// Done with the approval of another stage (`same approver approves once`): that stage. The
    /// one right before it; with `, at the last stage`, the later stage the part waited for.
    Once(String),
    /// It waits for a later stage, which its one approver alone answers too (`same approver
    /// approves once, at the last stage`): that stage. The approval given there does it.
    Deferred(String),
}

/// What a part comes to, as `same approver approves once` reads it (DESIGN 5.12): the same three
/// the check reads ([`crate::org::listing::Out`]).
#[derive(Clone, Debug, PartialEq)]
pub enum Sole {
    Pass,
    By(String),
    Other,
}

/// One part of a stage: `dept_head` and `accounting` of `dept_head and accounting` are two.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    /// The way to choose, as the stage names it.
    pub way: String,
    /// The way a table of `when late escalate by <table>` reads for the ones who answer now.
    pub key: String,
    pub approvers: Vec<String>,
    /// The team, when the ones who answer are a team's members.
    pub team: Option<String>,
    /// How many approvals pass it.
    pub needed: usize,
    pub answers: Vec<Answer>,
    pub state: PartState,
    pub sole: Sole,
}

impl Part {
    pub fn yes(&self) -> usize {
        self.answers.iter().filter(|a| a.yes).count()
    }

    pub fn no(&self) -> usize {
        self.answers.iter().filter(|a| !a.yes).count()
    }

    /// Whether the part no longer keeps the stage: passed, skipped, checked by the agent, done with
    /// another stage's approval, or waiting for a later stage's.
    pub fn done(&self) -> bool {
        matches!(self.state, PartState::Passed | PartState::Skipped | PartState::Checked(_) | PartState::Once(_) | PartState::Deferred(_))
    }

    /// The one person it waits for a later stage's approval of, and that stage.
    pub fn deferred(&self) -> Option<(&str, &str)> {
        match (&self.state, &self.sole) {
            (PartState::Deferred(to), Sole::By(x)) => Some((x.as_str(), to.as_str())),
            _ => None,
        }
    }
}

/// Where a stage of the route stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The ticket has not come to it.
    Ahead,
    /// The ticket is at it.
    Open,
    /// Answered: every part has what it needs.
    Answered,
    /// Nobody answers any part of it, and it is skipped.
    Skipped,
    /// A circulation, sent.
    Circulated,
    /// A summary, asked for and attached, or passed without it.
    Noted,
    /// Every part done with another stage's approval, or skipped.
    Once,
    /// The ticket has moved on from it, and a part of it waits for a later stage's approval
    /// (`same approver approves once, at the last stage`).
    Deferred,
}

impl Phase {
    pub fn word(self) -> &'static str {
        match self {
            Phase::Ahead => "ahead",
            Phase::Open => "open",
            Phase::Answered => "answered",
            Phase::Skipped => "skipped",
            Phase::Circulated => "circulated",
            Phase::Noted => "noted",
            Phase::Once => "once",
            Phase::Deferred => "deferred",
        }
    }
}

/// A stage of the route a ticket takes this round.
#[derive(Clone, Debug, PartialEq)]
pub struct Stage {
    pub name: String,
    /// The value of the stage's output in the lowered route: which approvers the stage names.
    pub value: String,
    pub phase: Phase,
    /// When it started: its clocks are counted from this time's day.
    pub started: Option<i64>,
    /// The business days its clocks were stopped for (`pause`).
    pub held: u32,
    /// The clocks set and not yet rung: the reminder, the deadline.
    pub remind: Option<i64>,
    /// Whether its reminder has rung, since it started or last went up: a reminder that has not is
    /// set again when the stage comes back to it, where it comes before the deadline (DESIGN 7.5).
    pub reminded: bool,
    pub due: Option<i64>,
    /// How many times it went up for being late.
    pub up: u8,
    /// The version of the organization its approvers were chosen in, by its day.
    pub org: Option<i64>,
    /// Sent back to: `same approver approves once` does not do it.
    pub again: bool,
    pub parts: Vec<Part>,
}

impl Stage {
    pub fn new(name: &str, value: &str) -> Stage {
        Stage { name: name.to_string(), value: value.to_string(), phase: Phase::Ahead, started: None, held: 0, remind: None, reminded: false, due: None, up: 0, org: None, again: false, parts: Vec::new() }
    }

    /// The stage as it is before the ticket comes to it.
    pub fn reset(&mut self) {
        *self = Stage::new(&self.name, &self.value);
    }
}

/// A hold a ticket keeps open: the book, the transfer, the round it was made in.
#[derive(Clone, Debug, PartialEq)]
pub struct Held {
    pub book: String,
    pub transfer: String,
    pub round: u32,
}

/// How a ticket ended: when, at which stage, and by whom (nobody for a clock, and for the last
/// approval, which the stage's answers hold).
#[derive(Clone, Debug, PartialEq)]
pub struct Ended {
    pub at: i64,
    pub stage: Option<String>,
    pub by: Option<String>,
}

/// The revision of a ticket (DESIGN 7.1): its round, the place in its stages of the stage it is at
/// (none while it is with the requester and once it has ended), and how many times an approver's
/// edit has gone through. A person's event and a flow's answer are given with the revision of the
/// ticket they were made on (what a screen or a tool showed, what the transition that asked the flow
/// gave): one made on another revision is refused (`stale`). It is worked out from the ticket
/// ([`Ticket::revision`]), never kept apart: another's answer at the same stage, going up, an
/// approver added and a place handed over leave it as it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Revision {
    pub round: u32,
    pub stage: Option<usize>,
    pub edits: u32,
}

impl Revision {
    pub fn to_json(&self) -> Json {
        Json::obj([("round", Json::int(self.round)), ("stage", self.stage.map(|i| Json::int(i as u32)).unwrap_or(Json::Null)), ("edits", Json::int(self.edits))])
    }

    /// The revision as a refusal says it: `round 1, stage 2 of the ticket, 1 edit`,
    /// `1 回目、2 番目のステップ、直し 1 回`.
    pub fn text(&self) -> ritsu_base::text::Text {
        let r = self.round;
        let (stage_ja, stage_en) = match self.stage {
            Some(i) => (format!("{} 番目のステップ", i + 1), format!("stage {} of the ticket", i + 1)),
            None => ("ステップの外".to_string(), "at no stage".to_string()),
        };
        let edits = match self.edits {
            1 => "1 edit".to_string(),
            e => format!("{e} edits"),
        };
        ritsu_base::text::Text::new(format!("{r} 回目、{stage_ja}、直し {} 回", self.edits), format!("round {r}, {stage_en}, {edits}"))
    }
}

/// A ticket.
#[derive(Clone, Debug, PartialEq)]
pub struct Ticket {
    pub id: String,
    /// The request and its version.
    pub request: String,
    pub version: u32,
    pub requester: String,
    /// The one who made it in the requester's name, when a proxy did.
    pub made_by: Option<String>,
    /// The one who sent it back to the requester, while it is with them (DESIGN 7.1): the person
    /// who did it, a proxy's own ID where a proxy did. None at any other time, and while it is with
    /// the requester because they took it back themselves.
    pub sent_back_by: Option<String>,
    pub status: Status,
    pub round: u32,
    /// The try that started the round (DESIGN 12.4): the `attempt` of its `submit` or `resubmit`,
    /// which its holds are keyed by (`ticket.attempt`).
    pub attempt: String,
    /// How many things to do it has given so far: the next one's serial is this and one.
    pub serial: u32,
    /// The values of the form, by their lowered names: what was given, and what the form computes.
    pub form: Values,
    pub stages: Vec<Stage>,
    /// The stage it is at, while it is in progress or paused.
    pub at: Option<usize>,
    /// The send-backs and the withdrawals so far.
    pub backs: u32,
    /// The approvers' edits that have gone through so far.
    pub edits: u32,
    /// The business days of pause left, and when the pause it is in began.
    pub pause_left: u32,
    pub paused_at: Option<i64>,
    /// The clock of a pause, and of the requester's turn.
    pub pause_ends: Option<i64>,
    pub resubmit_due: Option<i64>,
    pub holds: Vec<Held>,
    /// The summaries attached: the stage, and the outputs of its flow.
    pub notes: Vec<(String, Values)>,
    /// Set once it has ended.
    pub ended: Option<Ended>,
}

impl Ticket {
    /// The ticket's revision, worked out from it: the one place it is made.
    pub fn revision(&self) -> Revision {
        Revision { round: self.round, stage: self.at, edits: self.edits }
    }

    pub fn stage(&self) -> Option<&Stage> {
        self.at.and_then(|i| self.stages.get(i))
    }

    /// Whether the stage `a` is the last of its name on the ticket. A stage whose parts waited for a
    /// later stage and are started after all comes a second time, right where it is started, and
    /// the parts that still wait are the later one's: the one before stands as it was left.
    pub fn live(&self, a: usize) -> bool {
        !self.stages[a + 1..].iter().any(|s| s.name == self.stages[a].name)
    }

    /// The parts that wait for a later stage's approval, among the stages before `i`: each by its
    /// stage and its place in it.
    pub fn deferred(&self, i: usize) -> Vec<(usize, usize)> {
        let i = i.min(self.stages.len());
        (0..i).filter(|a| self.live(*a)).flat_map(|a| self.stages[a].parts.iter().enumerate().filter(|(_, p)| p.deferred().is_some()).map(move |(k, _)| (a, k))).collect()
    }

    /// The stage it is at, by name.
    pub fn stage_name(&self) -> Option<&str> {
        self.stage().map(|s| s.name.as_str())
    }

    /// A value of the form, by its lowered name.
    pub fn value(&self, name: &str) -> Option<&Value> {
        self.form.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    /// The clocks set and not yet rung: each kind, its stage, its time.
    pub fn timers(&self) -> Vec<(&'static str, Option<&str>, i64)> {
        let mut out = Vec::new();
        if let Some(t) = self.pause_ends {
            out.push(("pause ends", self.stage_name(), t));
        }
        if let Some(t) = self.resubmit_due {
            out.push(("resubmit late", None, t));
        }
        if self.status == Status::InProgress
            && let Some(st) = self.stage()
        {
            if let Some(t) = st.remind {
                out.push(("remind", Some(st.name.as_str()), t));
            }
            if let Some(t) = st.due {
                out.push(("late", Some(st.name.as_str()), t));
            }
        }
        out.sort_by_key(|x| x.2);
        out
    }

    /// The ticket as the JSON writes it, its names the ASCII ones (DESIGN 3.4).
    pub fn to_json(&self, n: &super::Names) -> Json {
        Json::obj([
            ("id", Json::str(&self.id)),
            ("request", Json::str(&self.request)),
            ("version", Json::int(self.version)),
            ("requester", Json::str(&self.requester)),
            ("made_by", Json::opt_str(self.made_by.clone())),
            ("sent_back_by", Json::opt_str(self.sent_back_by.clone())),
            ("status", Json::str(self.status.word())),
            ("round", Json::int(self.round)),
            ("attempt", Json::str(&self.attempt)),
            ("serial", Json::int(self.serial)),
            ("revision", self.revision().to_json()),
            ("form", values_json(&self.form)),
            ("stage", Json::opt_str(self.stage_name().map(|s| n.stage(s)))),
            ("stages", Json::arr(self.stages.iter().map(|s| stage_json(s, n)))),
            ("backs", Json::int(self.backs)),
            ("edits", Json::int(self.edits)),
            ("pause_left", Json::int(self.pause_left)),
            ("paused_at", time_json(self.paused_at)),
            ("pause_ends", time_json(self.pause_ends)),
            ("resubmit_due", time_json(self.resubmit_due)),
            ("holds", Json::arr(self.holds.iter().map(|h| Json::obj([("book", Json::str(n.used(&h.book))), ("transfer", Json::str(&h.transfer)), ("round", Json::int(h.round))])))),
            ("notes", Json::arr(self.notes.iter().map(|(s, v)| Json::obj([("stage", Json::str(n.stage(s))), ("outputs", values_json(v))])))),
            ("ended", self.ended.as_ref().map(|e| Json::obj([("at", Json::int(e.at as i128)), ("stage", Json::opt_str(e.stage.as_deref().map(|s| n.stage(s)))), ("by", Json::opt_str(e.by.clone()))])).unwrap_or(Json::Null)),
        ])
    }
}

/// A time (seconds since 1970-01-01T00:00:00Z), or null.
pub fn time_json(t: Option<i64>) -> Json {
    t.map(|t| Json::int(t as i128)).unwrap_or(Json::Null)
}

fn stage_json(s: &Stage, n: &super::Names) -> Json {
    Json::obj([
        ("name", Json::str(n.stage(&s.name))),
        ("value", Json::str(&s.value)),
        ("phase", Json::str(s.phase.word())),
        ("started", time_json(s.started)),
        ("held", Json::int(s.held)),
        ("remind", time_json(s.remind)),
        ("reminded", Json::Bool(s.reminded)),
        ("due", time_json(s.due)),
        ("up", Json::int(s.up as u32)),
        ("org", s.org.map(|d| Json::str(crate::lex::date_text(d))).unwrap_or(Json::Null)),
        ("again", Json::Bool(s.again)),
        ("parts", Json::arr(s.parts.iter().map(|p| part_json(p, n)))),
    ])
}

fn part_json(p: &Part, n: &super::Names) -> Json {
    let (state, with) = match &p.state {
        PartState::Open => ("open", None),
        PartState::Passed => ("passed", None),
        PartState::Skipped => ("skipped", None),
        PartState::Stopped => ("stopped", None),
        PartState::Agent(f) => ("agent", Some(n.used(f))),
        PartState::Checked(f) => ("checked", Some(n.used(f))),
        PartState::Once(st) => ("once", Some(n.stage(st))),
        PartState::Deferred(st) => ("deferred", Some(n.stage(st))),
    };
    let sole = match &p.sole {
        Sole::Pass => Json::str("pass"),
        Sole::Other => Json::str("other"),
        Sole::By(x) => Json::obj([("by", Json::str(x))]),
    };
    Json::obj([
        ("way", Json::str(n.way(&p.way))),
        ("key", Json::str(n.way(&p.key))),
        ("approvers", Json::arr(p.approvers.iter().map(Json::str))),
        ("team", Json::opt_str(p.team.clone())),
        ("needed", Json::int(p.needed as u32)),
        ("answers", Json::arr(p.answers.iter().map(|a| Json::obj([("by", Json::str(&a.by)), ("seat", Json::str(&a.seat)), ("yes", Json::Bool(a.yes)), ("at", Json::int(a.at as i128))])))),
        ("state", Json::str(state)),
        ("with", Json::opt_str(with)),
        ("sole", sole),
    ])
}

/// A value as the JSON writes it: a number as itself (counted in its field's step), a date and an
/// enum's value as strings, rows as a list of objects.
pub fn value_json(v: &Value) -> Json {
    match v {
        Value::Bool(b) => Json::Bool(*b),
        Value::Int(n) => Json::Int(*n),
        Value::Str(s) | Value::Date(s) | Value::Enum(s) => Json::str(s),
        Value::List(rows) => Json::arr(rows.iter().map(values_json)),
        Value::None => Json::Null,
    }
}

pub fn values_json(v: &Values) -> Json {
    Json::obj(v.iter().map(|(n, x)| (n.clone(), value_json(x))))
}
