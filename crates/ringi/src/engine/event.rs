//! The events of a ticket (DESIGN 7.2): what a person does to it, what a clock rings, what a flow
//! answers. `submit` makes a ticket and is [`Submit`]; every other event moves one, or, a comment,
//! adds a line to its history. What a person writes with an event (its comment) is given beside it,
//! as the revision it was made on is ([`super::Machine::next`]).

use super::state::values_json;
use ritsu_base::json::Json;
use ritsu_ports::Values;

/// A request about to be made: the ticket's ID, the requester, the one who makes it in their name
/// when a proxy does, the values of the form by their lowered names, and the try it is
/// (`attempt`, DESIGN 12.4): the server's name for this one operation, the same when it is sent
/// again, which the holds of the round are keyed by (`ticket.attempt`).
#[derive(Clone, Debug, PartialEq)]
pub struct Submit {
    pub id: String,
    pub requester: String,
    pub by: Option<String>,
    pub form: Values,
    pub attempt: String,
}

/// An event of a ticket.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// The requester, or their proxy, makes it again; the values that change, the fields made
    /// empty (`clear`, DESIGN 7.3), by their lowered names, and the try it is (`attempt`, as
    /// [`Submit`]'s).
    Resubmit { by: String, form: Values, clear: Vec<String>, attempt: String },
    Approve { by: String },
    Reject { by: String },
    /// Back to the requester, or to an earlier stage.
    SendBack { by: String, to: Option<String> },
    Retract { by: String },
    Pause { by: String },
    Resume { by: String },
    Withdraw { by: String },
    Discard { by: String },
    /// An approver fills in or corrects fields, or makes some empty (`clear`).
    Edit { by: String, fields: Values, clear: Vec<String> },
    /// An administrator adds an approver to the stage (to the part of the way `part`, where it has
    /// several), or hands one approver's place to another person.
    AddApprover { by: Option<String>, approver: String, part: Option<String> },
    HandOver { by: Option<String>, from: String, to: String },
    /// A person writes on the ticket and changes nothing else: its text is the comment given with
    /// it.
    Comment { by: String },
    Remind,
    Late,
    ResubmitLate,
    PauseEnds,
    AgentAnswered { ok: bool, reason: String },
    NoteReady { outputs: Values },
    FlowFailed,
}

impl Event {
    /// The event as a scenario writes it.
    pub fn word(&self) -> &'static str {
        match self {
            Event::Resubmit { .. } => "resubmit",
            Event::Approve { .. } => "approve",
            Event::Reject { .. } => "reject",
            Event::SendBack { .. } => "send back",
            Event::Retract { .. } => "retract",
            Event::Pause { .. } => "pause",
            Event::Resume { .. } => "resume",
            Event::Withdraw { .. } => "withdraw",
            Event::Discard { .. } => "discard",
            Event::Edit { .. } => "edit",
            Event::AddApprover { .. } => "add approver",
            Event::HandOver { .. } => "hand over",
            Event::Comment { .. } => "comment",
            Event::Remind => "remind",
            Event::Late => "late",
            Event::ResubmitLate => "resubmit late",
            Event::PauseEnds => "pause ends",
            Event::AgentAnswered { .. } => "agent answered",
            Event::NoteReady { .. } => "note ready",
            Event::FlowFailed => "flow failed",
        }
    }

    /// The one it is by, where a person does it.
    pub fn by(&self) -> Option<&str> {
        match self {
            Event::Resubmit { by, .. } | Event::Approve { by } | Event::Reject { by } | Event::SendBack { by, .. } | Event::Retract { by } | Event::Pause { by } | Event::Resume { by } | Event::Withdraw { by } | Event::Discard { by } | Event::Edit { by, .. } | Event::Comment { by } => Some(by),
            Event::AddApprover { by, .. } | Event::HandOver { by, .. } => by.as_deref(),
            _ => None,
        }
    }

    /// The people it names besides the one it is by.
    pub fn names(&self) -> Vec<String> {
        match self {
            Event::AddApprover { approver, .. } => vec![approver.clone()],
            Event::HandOver { from, to, .. } => vec![from.clone(), to.clone()],
            _ => Vec::new(),
        }
    }

    /// Whether a clock rings it.
    pub fn clock(&self) -> bool {
        matches!(self, Event::Remind | Event::Late | Event::ResubmitLate | Event::PauseEnds)
    }

    /// Whether a person gives it: one that may come with a comment.
    pub fn person(&self) -> bool {
        !self.clock() && !matches!(self, Event::AgentAnswered { .. } | Event::NoteReady { .. } | Event::FlowFailed)
    }

    /// Whether it is made on a revision of the ticket (DESIGN 7.2): every event but a clock's and a
    /// comment, which moves nothing.
    pub fn revised(&self) -> bool {
        !self.clock() && !matches!(self, Event::Comment { .. })
    }

    /// The event as the JSON writes it, the stage and the way it names by their ASCII names
    /// (DESIGN 3.4).
    pub fn to_json(&self, n: &super::Names) -> Json {
        let mut o: Vec<(&str, Json)> = vec![("event", Json::str(self.word())), ("by", Json::opt_str(self.by()))];
        match self {
            Event::Resubmit { form, clear, attempt, .. } => {
                o.push(("form", values_json(form)));
                o.push(("clear", Json::arr(clear.iter().map(Json::str))));
                o.push(("attempt", Json::str(attempt)));
            }
            Event::SendBack { to, .. } => o.push(("to", Json::opt_str(to.as_deref().map(|s| n.stage(s))))),
            Event::Edit { fields, clear, .. } => {
                o.push(("fields", values_json(fields)));
                o.push(("clear", Json::arr(clear.iter().map(Json::str))));
            }
            Event::AddApprover { approver, part, .. } => {
                o.push(("approver", Json::str(approver)));
                o.push(("part", Json::opt_str(part.as_deref().map(|w| n.way(w)))));
            }
            Event::HandOver { from, to, .. } => {
                o.push(("from", Json::str(from)));
                o.push(("to", Json::str(to)));
            }
            Event::AgentAnswered { ok, reason } => {
                o.push(("verdict", Json::str(if *ok { "ok" } else { "doubt" })));
                o.push(("reason", Json::str(reason)));
            }
            Event::NoteReady { outputs } => o.push(("outputs", values_json(outputs))),
            _ => {}
        }
        Json::obj(o)
    }
}

/// The try a scenario, the vectors and the tests give a `submit` or a `resubmit` that starts the
/// round `round`: one name for each round, as a server gives one for each operation it takes.
pub fn attempt(round: u32) -> String {
    format!("attempt-{round}")
}

impl Submit {
    pub fn to_json(&self) -> Json {
        Json::obj([("event", Json::str("submit")), ("id", Json::str(&self.id)), ("requester", Json::str(&self.requester)), ("by", Json::opt_str(self.by.clone())), ("form", values_json(&self.form)), ("attempt", Json::str(&self.attempt))])
    }
}
