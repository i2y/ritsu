//! What a transition gives the server to do (DESIGN 7.4): data, each with its key (the ticket, the
//! round, a serial), so that doing one twice is doing it once. They come in a fixed order: the
//! lines of the history, the clocks cancelled, the holds posted, voided and made, the flows
//! launched and asked, the notices, the clocks set; within a kind, in the order the transition
//! made them.

use super::state::{Ticket, time_json, values_json};
use ritsu_base::json::Json;
use ritsu_ports::Values;

/// A clock of a ticket.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Timer {
    Remind,
    Late,
    ResubmitLate,
    PauseEnds,
}

impl Timer {
    /// The clock by the event it rings as.
    pub fn word(self) -> &'static str {
        match self {
            Timer::Remind => "remind",
            Timer::Late => "late",
            Timer::ResubmitLate => "resubmit late",
            Timer::PauseEnds => "pause ends",
        }
    }
}

/// What a notice tells (DESIGN 7.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    /// A stage waits for them.
    Waiting,
    Reminder,
    /// A late stage, or one with nobody, went up to them.
    WentUp,
    /// The ticket came back to the requester.
    Returned,
    /// The ticket ended: approved, rejected, withdrawn or expired.
    Decided,
    Circulation,
    Paused,
    Resumed,
    /// A stage has nobody to answer it and waits for an administrator.
    Stopped,
    /// Somebody wrote on the ticket.
    Comment,
}

impl Notice {
    pub fn word(self) -> &'static str {
        match self {
            Notice::Waiting => "waiting",
            Notice::Reminder => "reminder",
            Notice::WentUp => "went up",
            Notice::Returned => "returned",
            Notice::Decided => "decided",
            Notice::Circulation => "circulation",
            Notice::Paused => "paused",
            Notice::Resumed => "resumed",
            Notice::Stopped => "stopped",
            Notice::Comment => "comment",
        }
    }
}

/// A line of the history (DESIGN 7.4): what was done, at which stage, by whom and for whom (the
/// approver a proxy answered for), in which version of the organization the stage's approvers were
/// chosen, and, for a stage done with another stage's approval, the stage that approval was given
/// at (for one that waits for a later stage, the stage it waits for). What a person wrote with what
/// they did, and the reason an agent gave, is its comment. A resubmit's and an edit's say the
/// fields that lost their value with it (`cleared`): made empty, or dropped as the form no longer
/// shows them.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Record {
    pub did: &'static str,
    pub stage: Option<String>,
    pub by: Option<String>,
    pub proxy_of: Option<String>,
    pub org: Option<i64>,
    pub once_with: Option<String>,
    /// Who it went to, or what it is about.
    pub to: Vec<String>,
    pub note: Option<String>,
    pub comment: Option<String>,
    /// The fields that lost their value, by their ASCII names, in the order the form declares them.
    pub cleared: Vec<String>,
}

/// One thing to do.
#[derive(Clone, Debug, PartialEq)]
pub enum What {
    Record(Record),
    CancelTimer { timer: Timer, stage: Option<String> },
    /// `hold`, `post` or `void` of a transfer of a book.
    Book { op: &'static str, book: String, transfer: String, args: Values },
    Launch { flow: String, args: Values },
    AskAgent { flow: String, stage: String, args: Values },
    AskNote { flow: String, stage: String, args: Values },
    Notify { to: Vec<String>, notice: Notice, stage: Option<String> },
    SetTimer { timer: Timer, stage: Option<String>, at: i64 },
}

impl What {
    /// Its place in the order of a transition's list.
    fn rank(&self) -> u8 {
        match self {
            What::Record(_) => 0,
            What::CancelTimer { .. } => 1,
            What::Book { op: "post", .. } => 2,
            What::Book { op: "void", .. } => 3,
            What::Book { .. } => 4,
            What::Launch { .. } => 5,
            What::AskAgent { .. } | What::AskNote { .. } => 6,
            What::Notify { .. } => 7,
            What::SetTimer { .. } => 8,
        }
    }
}

/// A thing to do, with its key: the ticket, the round, the serial.
#[derive(Clone, Debug, PartialEq)]
pub struct Effect {
    pub ticket: String,
    pub round: u32,
    pub serial: u32,
    pub what: What,
}

/// The things a transition gives, as it makes them.
#[derive(Default)]
pub struct Effects {
    made: Vec<(u32, What)>,
}

impl Effects {
    /// One more, of the round the ticket is in as it is made.
    pub fn put(&mut self, round: u32, w: What) {
        self.made.push((round, w));
    }

    pub fn record(&mut self, round: u32, r: Record) {
        self.put(round, What::Record(r));
    }

    /// The things `other` holds, after the ones here.
    pub fn append(&mut self, other: Effects) {
        self.made.extend(other.made);
    }

    /// What a person wrote with what they did, on the first line of the history the transition
    /// made: the line of what they did.
    pub fn comment(&mut self, c: Option<&str>) {
        let Some(c) = c else { return };
        if let Some(r) = self.made.iter_mut().find_map(|(_, w)| if let What::Record(r) = w { Some(r) } else { None }) {
            r.comment = Some(c.to_string());
        }
    }

    /// The list, in its order, keyed from the ticket's serial on; the ticket's serial moves past
    /// them.
    pub fn done(mut self, t: &mut Ticket) -> Vec<Effect> {
        self.made.sort_by_key(|(_, w)| w.rank());
        self.made
            .into_iter()
            .map(|(round, what)| {
                t.serial += 1;
                Effect { ticket: t.id.clone(), round, serial: t.serial, what }
            })
            .collect()
    }
}

impl Effect {
    /// The thing to do as the JSON writes it, its names the ASCII ones (DESIGN 3.4).
    pub fn to_json(&self, n: &super::Names) -> Json {
        let stage = |s: &Option<String>| Json::opt_str(s.as_deref().map(|s| n.stage(s)));
        let mut o: Vec<(&str, Json)> = vec![("key", Json::arr([Json::str(&self.ticket), Json::int(self.round), Json::int(self.serial)]))];
        match &self.what {
            What::Record(r) => {
                o.push(("do", Json::str("record")));
                o.push(("did", Json::str(r.did)));
                o.push(("stage", stage(&r.stage)));
                o.push(("by", Json::opt_str(r.by.clone())));
                o.push(("proxy_of", Json::opt_str(r.proxy_of.clone())));
                o.push(("organization", r.org.map(|d| Json::str(crate::lex::date_text(d))).unwrap_or(Json::Null)));
                o.push(("once_with", stage(&r.once_with)));
                // a send-back names the stage it goes to
                o.push(("to", Json::arr(r.to.iter().map(|x| Json::str(if r.did == "sent back" { n.stage(x) } else { x })))));
                o.push(("note", Json::opt_str(r.note.clone())));
                o.push(("comment", Json::opt_str(r.comment.clone())));
                o.push(("cleared", Json::arr(r.cleared.iter().map(Json::str))));
            }
            What::CancelTimer { timer, stage: stage_of } => {
                o.push(("do", Json::str("cancel timer")));
                o.push(("timer", Json::str(timer.word())));
                o.push(("stage", stage(stage_of)));
            }
            What::Book { op, book, transfer, args } => {
                o.push(("do", Json::str(*op)));
                o.push(("book", Json::str(n.used(book))));
                o.push(("transfer", Json::str(transfer)));
                o.push(("args", values_json(args)));
            }
            What::Launch { flow, args } => {
                o.push(("do", Json::str("launch")));
                o.push(("flow", Json::str(n.used(flow))));
                o.push(("args", values_json(args)));
            }
            What::AskAgent { flow, stage: stage_of, args } | What::AskNote { flow, stage: stage_of, args } => {
                o.push(("do", Json::str(if matches!(self.what, What::AskAgent { .. }) { "ask agent" } else { "ask note" })));
                o.push(("flow", Json::str(n.used(flow))));
                o.push(("stage", Json::str(n.stage(stage_of))));
                o.push(("args", values_json(args)));
            }
            What::Notify { to, notice, stage: stage_of } => {
                o.push(("do", Json::str("notify")));
                o.push(("to", Json::arr(to.iter().map(Json::str))));
                o.push(("notice", Json::str(notice.word())));
                o.push(("stage", stage(stage_of)));
            }
            What::SetTimer { timer, stage: stage_of, at } => {
                o.push(("do", Json::str("set timer")));
                o.push(("timer", Json::str(timer.word())));
                o.push(("stage", stage(stage_of)));
                o.push(("at", time_json(Some(*at))));
            }
        }
        Json::obj(o)
    }
}
