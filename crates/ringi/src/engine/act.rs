//! Each event of a ticket (DESIGN 7.2, 7.3): what a person does, what a clock rings, what a flow
//! answers. An event the ticket does not take is refused, and the ticket is as it was.
//!
//! Who answers (DESIGN 5.8): an answer counts in the person's own right in every part of the stage
//! they are an approver of; else as the answer of the first approver they are a proxy of that day
//! who has yet to answer, in one part. A person answers a part once, and a seat is answered once:
//! an approver and their proxy are one vote. The one who made the request does not answer it, their
//! own seat or another's, unless the request allows self approval.

use super::stages::Solved;
use super::{Answer, Effects, Event, Machine, Notice, OrgPart, Part, PartState, Phase, Record, Refusal, Sole, Status, Ticket, Timer, What};
use crate::ast::{Ending, How, Late, Reasoned, Spec, TicketLine};
use crate::org::ProxyKind;
use crate::org::listing::{self, LateTo, Once};
use crate::org::resolve::Solver;
use crate::taken::{self, Giver};
use ritsu_ports::{Value, Values};

/// A `reject` or a `send back` the request asks a reason with (`comment required for`, DESIGN
/// 7.2), given with none: what the person wrote (`comment`, as [`super::said`] keeps it) empty, or
/// white space alone, is refused (`comment`), before who gives it is asked, as the machine's code
/// refuses it (`reasoned` of `lower/ticket.kn`).
pub(super) fn reasoned(s: &crate::names::Scope, ev: &Event, comment: Option<&str>) -> Result<(), Refusal> {
    let op = match ev {
        Event::Reject { .. } => Reasoned::Reject,
        Event::SendBack { .. } => Reasoned::SendBack,
        _ => return Ok(()),
    };
    let asked = s.file.ticket.as_ref().is_some_and(|t| t.reasoned().contains(&op));
    if !asked || comment.is_some_and(|c| !c.trim().is_empty()) {
        return Ok(());
    }
    let e = ev.word();
    Err(Refusal::new("comment", tr!("この申請の `{e}` には理由が要ります。コメントに書いてください", "a `{e}` of this request needs a reason: write it in the comment")))
}

impl Machine<'_, '_> {
    pub(crate) fn act(&self, t: &mut Ticket, ev: &Event, comment: Option<&str>, part: &OrgPart, now: i64, fx: &mut Effects) -> Result<(), Refusal> {
        match ev {
            Event::Approve { by } => self.answer(t, by, true, part, now, fx),
            Event::Reject { by } => self.answer(t, by, false, part, now, fx),
            Event::SendBack { by, to } => self.send_back(t, by, to.as_deref(), part, now, fx),
            Event::Retract { by } => self.retract(t, by, part, now, fx),
            Event::Pause { by } => self.pause(t, by, part, now, fx),
            Event::Resume { by } => self.resume(t, Some(by), part, now, fx),
            Event::PauseEnds => self.resume(t, None, part, now, fx),
            Event::Withdraw { by } => self.withdraw(t, by, part, now, fx),
            Event::Discard { by } => self.discard(t, by, part, now, fx),
            Event::Resubmit { by, form, clear, attempt } => self.resubmit(t, by, form, clear, attempt, part, now, fx),
            Event::Edit { by, fields, clear } => self.edit(t, by, fields, clear, part, fx),
            Event::AddApprover { by, approver, part: which } => self.add_approver(t, by.as_deref(), approver, which.as_deref(), part, fx),
            Event::HandOver { by, from, to } => self.hand_over(t, by.as_deref(), from, to, part, fx),
            Event::Comment { by } => self.comment(t, by, comment, part, fx),
            Event::Remind => self.remind(t, part, now, fx),
            Event::Late => self.late(t, part, now, fx),
            Event::ResubmitLate => self.resubmit_late(t, part, now, fx),
            Event::AgentAnswered { ok, reason } => self.agent(t, *ok, reason, false, part, now, fx),
            Event::FlowFailed => self.agent(t, false, "", true, part, now, fx),
            Event::NoteReady { outputs } => self.note_ready(t, outputs, part, now, fx),
        }
    }

    /// The stage the ticket is at, when its state is one of `states`; else the event is refused.
    fn stage_in(&self, t: &Ticket, ev: &str, states: &[Status]) -> Result<usize, Refusal> {
        match t.at {
            Some(i) if states.contains(&t.status) => Ok(i),
            _ => {
                let st = t.status.word();
                Err(Refusal::new("state", tr!("チケットは `{st}` です。`{ev}` は受け付けません", "the ticket is `{st}`, and takes no `{ev}`")))
            }
        }
    }

    /// A time as a refusal says it, in the UTC offset `offset`: `2026-11-20 09:00`.
    pub(crate) fn time_in(at: i64, offset: i32) -> String {
        let local = at + i64::from(offset) * 60;
        format!("{} {}", crate::lex::date_text(local.div_euclid(86_400)), crate::lex::time_text((local.rem_euclid(86_400) / 60) as u32))
    }

    /// A clock (of the stage `stage`, as written) that is set and has rung; else the event is
    /// refused, with the time it is set for and the time it is, in the offset the clock is read in.
    fn rung(&self, set: Option<i64>, what: &str, stage: Option<&str>, now: i64) -> Result<(), Refusal> {
        match set {
            None => Err(Refusal::new("clock", tr!("`{what}` の時計は置かれていません", "no clock is set for `{what}`"))),
            Some(at) if now < at => {
                let offset = self.clock_offset(what, stage);
                let (a, n) = (Self::time_in(at, offset), Self::time_in(now, offset));
                Err(Refusal::new("clock", tr!("`{what}` の時計は {a} に鳴ります。今は {n} です", "the clock of `{what}` is set for {a}, and it is {n}")))
            }
            Some(_) => Ok(()),
        }
    }

    /// The person is in the organization and did not make the request (where self approval is not
    /// allowed): one who may answer a stage of it.
    fn may_answer(&self, t: &Ticket, part: &OrgPart, by: &str) -> Result<(), Refusal> {
        let v = part.v();
        if v.person(by).is_none() {
            let d = v.day();
            return Err(Refusal::new("person", tr!("{by} は、{d} からの組織にいません", "{by} is not in the organization from {d}")));
        }
        let rq = Self::requester(t);
        if self.solver(&rq, part).left_out(by) {
            let who = v.who(by);
            return Err(Refusal::new("person", tr!("{}は、この申請を出した人なので、答えられません（`self approval not allowed`）", "{} made the request, and does not answer it (`self approval not allowed`)", crate::org::ja_word(&who.ja); who.en)));
        }
        Ok(())
    }

    fn proxy_of(&self, part: &OrgPart, by: &str, person: &str) -> bool {
        part.org.proxies.iter().any(|x| x.kind == ProxyKind::Approve && x.person == person && x.proxy == by && self.proxy_on(x, part.day))
    }

    /// The seats an answer of `by` takes at the stage `i` (DESIGN 5.8): each part, and the approver
    /// it counts as.
    fn seats(&self, t: &Ticket, i: usize, part: &OrgPart, by: &str) -> Result<Vec<(usize, String)>, Refusal> {
        self.may_answer(t, part, by)?;
        let st = &t.stages[i];
        let free = |p: &Part, seat: &str| p.state == PartState::Open && !p.answers.iter().any(|a| a.by == by || a.seat == seat) && p.approvers.iter().any(|a| a == seat);
        let own: Vec<(usize, String)> = st.parts.iter().enumerate().filter(|(_, p)| free(p, by)).map(|(j, _)| (j, by.to_string())).collect();
        if !own.is_empty() {
            return Ok(own);
        }
        for (j, p) in st.parts.iter().enumerate() {
            if let Some(a) = p.approvers.iter().find(|a| free(p, a) && self.proxy_of(part, by, a)) {
                return Ok(vec![(j, a.clone())]);
            }
        }
        let (who, stage) = (part.v().who(by), &st.name);
        if st.parts.iter().any(|p| p.answers.iter().any(|a| a.by == by || a.seat == by)) {
            return Err(Refusal::new("person", tr!("{}は、ステップ `{stage}` にもう答えています", "{} has answered the stage `{stage}` already", crate::org::ja_word(&who.ja); who.en)));
        }
        Err(Refusal::new("person", tr!("{}は、ステップ `{stage}` の承認者でも、その代理でもありません", "{} is not an approver of the stage `{stage}`, nor a proxy of one", crate::org::ja_word(&who.ja); who.en)))
    }

    /// An approver of the stage `i`, or a proxy of one: who may pause it and edit the request there.
    /// Not one whose part waits for a later stage: they answer there.
    fn approver(&self, t: &Ticket, i: usize, part: &OrgPart, by: &str) -> Result<(), Refusal> {
        self.may_answer(t, part, by)?;
        let st = &t.stages[i];
        if st.parts.iter().filter(|p| !matches!(p.state, PartState::Deferred(_))).any(|p| p.approvers.iter().any(|a| a == by || self.proxy_of(part, by, a))) {
            return Ok(());
        }
        let (who, stage) = (part.v().who(by), &st.name);
        Err(Refusal::new("person", tr!("{}は、ステップ `{stage}` の承認者でも、その代理でもありません", "{} is not an approver of the stage `{stage}`, nor a proxy of one", crate::org::ja_word(&who.ja); who.en)))
    }

    /// The requester, the one who made the request for them, or a proxy who may submit for them
    /// that day.
    fn theirs_to_do(&self, t: &Ticket, part: &OrgPart, by: &str, ev: &str) -> Result<(), Refusal> {
        let v = part.v();
        if v.person(by).is_none() {
            let d = v.day();
            return Err(Refusal::new("person", tr!("{by} は、{d} からの組織にいません", "{by} is not in the organization from {d}")));
        }
        if by == t.requester || t.made_by.as_deref() == Some(by) || part.org.proxies.iter().any(|x| x.kind == ProxyKind::Submit && x.person == t.requester && x.proxy == by && self.proxy_on(x, part.day)) {
            return Ok(());
        }
        let who = part.v().who(by);
        Err(Refusal::new("person", tr!("`{ev}` は申請者がします。{}は、この申請の申請者でも、その代理でもありません", "`{ev}` is the requester's to do, and {} is neither the requester nor a proxy of theirs", crate::org::ja_word(&who.ja); who.en)))
    }

    /// Whom one on the requester's side acts for (DESIGN 5.8, 7.4): the requester, when the one who
    /// made the request for them or a proxy of theirs does it.
    fn for_requester(t: &Ticket, by: &str) -> Option<String> {
        (by != t.requester).then(|| t.requester.clone())
    }

    /// Whom one acts for at the stage `i` as an approver does (DESIGN 5.8, 7.4): nobody where they
    /// are an approver of it, else the first approver they are a proxy of that day, as
    /// [`Machine::approver`] reads the stage.
    fn acting_for(&self, t: &Ticket, i: usize, part: &OrgPart, by: &str) -> Option<String> {
        let parts = || t.stages[i].parts.iter().filter(|p| !matches!(p.state, PartState::Deferred(_)));
        if parts().any(|p| p.approvers.iter().any(|a| a == by)) {
            return None;
        }
        parts().flat_map(|p| p.approvers.iter()).find(|a| self.proxy_of(part, by, a)).cloned()
    }

    /// The ticket ends (DESIGN 7.1): its clocks are cancelled, what its `on` lines say of the
    /// ending is done, and the requester is told. `proxy_of` is whom `by` ended it for.
    #[allow(clippy::too_many_arguments)]
    fn end(&self, t: &mut Ticket, status: Status, ending: Ending, did: &'static str, by: Option<&str>, proxy_of: Option<String>, note: Option<String>, part: &OrgPart, now: i64, fx: &mut Effects) -> Result<(), Refusal> {
        let round = t.round;
        self.cancel_clocks(t, fx);
        if t.pause_ends.take().is_some() {
            fx.put(round, What::CancelTimer { timer: Timer::PauseEnds, stage: t.stage_name().map(str::to_string) });
        }
        if t.resubmit_due.take().is_some() {
            fx.put(round, What::CancelTimer { timer: Timer::ResubmitLate, stage: None });
        }
        let stage = t.stage_name().map(str::to_string);
        fx.record(round, Record { did, stage: stage.clone(), by: by.map(str::to_string), proxy_of, note, ..Record::default() });
        (t.status, t.at, t.paused_at, t.sent_back_by) = (status, None, None, None);
        t.ended = Some(super::state::Ended { at: now, stage, by: by.map(str::to_string) });
        self.on(ending, t, part, fx)?;
        fx.put(round, What::Notify { to: Self::theirs(t), notice: Notice::Decided, stage: None });
        Ok(())
    }

    fn answer(&self, t: &mut Ticket, by: &str, yes: bool, part: &OrgPart, now: i64, fx: &mut Effects) -> Result<(), Refusal> {
        let i = self.stage_in(t, if yes { "approve" } else { "reject" }, &[Status::InProgress])?;
        let seats = self.seats(t, i, part, by)?;
        let (round, name, org) = (t.round, t.stages[i].name.clone(), t.stages[i].org);
        for (j, seat) in &seats {
            t.stages[i].parts[*j].answers.push(Answer { by: by.to_string(), seat: seat.clone(), yes, at: now });
        }
        let proxy_of = seats.first().map(|(_, s)| s.clone()).filter(|s| s != by);
        fx.record(round, Record { did: if yes { "approved" } else { "rejected" }, stage: Some(name.clone()), by: Some(by.to_string()), proxy_of: proxy_of.clone(), org, ..Record::default() });
        if !yes {
            // rejected, when a part can no longer come to the approvals it needs
            if t.stages[i].parts.iter().any(|p| p.state == PartState::Open && p.approvers.len().saturating_sub(p.no()) < p.needed) {
                return self.end(t, Status::Rejected, Ending::Rejected, "ticket rejected", Some(by), proxy_of, None, part, now, fx);
            }
            return Ok(());
        }
        for p in t.stages[i].parts.iter_mut() {
            if p.state == PartState::Open && p.yes() >= p.needed {
                p.state = PartState::Passed;
            }
        }
        if !t.stages[i].parts.iter().all(|p| p.done()) {
            return Ok(());
        }
        // the approver of the next stage, whom this stage's approver chooses
        if let Some(next) = t.stages.get(i + 1) {
            let o = self.occ_of(next)?;
            for (way, _, f) in self.ways(o, next) {
                if let Some(How::ChosenBy { previous: true, field }) = Solver::who(f, &way).map(|w| &w.how)
                    && self.chosen(t, &field.text).is_none()
                {
                    let (fname, nn) = (&field.text, &next.name);
                    let r = Refusal::new("form", tr!("`{fname}` が空です。次のステップ `{nn}` は、そこに指名した人が答えます。先に `edit` で入れてください", "`{fname}` is empty, and the next stage `{nn}` is answered by the person chosen in it: fill it in with `edit` first"));
                    return Err(self.at(r, fname));
                }
            }
        }
        self.pass(t, i, part, now, fx)
    }

    /// The stage `i` has every approval it needs: the ticket moves on. The parts that waited for it
    /// are done with the approval given here (`same approver approves once, at the last stage`);
    /// a part of its own that waits for a later stage leaves it waiting.
    fn pass(&self, t: &mut Ticket, i: usize, part: &OrgPart, now: i64, fx: &mut Effects) -> Result<(), Refusal> {
        self.cancel_clocks(t, fx);
        if listing::once(self.s.file) == Once::Last {
            self.answered(t, i, fx);
        }
        t.stages[i].phase = if t.stages[i].parts.iter().any(|p| matches!(p.state, PartState::Deferred(_))) { Phase::Deferred } else { Phase::Answered };
        self.advance(t, i + 1, false, part, now, fx)
    }

    /// How many times the ticket may go back to the requester, send-backs and withdrawals together:
    /// what `send back … at most N times` says, and none without the line.
    fn backs(&self) -> Option<(bool, u32)> {
        self.line(|l| if let TicketLine::SendBack { to_requester, times, .. } = l { Some((*to_requester, *times)) } else { None })
    }

    /// The business days of the pause the ticket is in are taken from what it has left: those of
    /// the pause's calendar after the day it was paused on, through the day it is now, each day as
    /// that calendar reads it (DESIGN 7.5).
    fn unpause(&self, t: &mut Ticket, now: i64, whole: bool) -> Result<u32, Refusal> {
        let Some(at) = t.paused_at.take() else { return Ok(0) };
        let cal = self.pause_calendar();
        let used = if whole { t.pause_left } else { self.open_between(cal, self.day_in(at, cal), self.day_in(now, cal))?.min(t.pause_left) };
        t.pause_left -= used;
        Ok(used)
    }

    /// The ticket goes to the requester: sent back by `by` (`sent`), an approver or a proxy of the
    /// approver `proxy_of`, or withdrawn by the requester's side. The one who sent it back is the
    /// ticket's while it is with the requester (DESIGN 7.1).
    #[allow(clippy::too_many_arguments)]
    fn to_requester(&self, t: &mut Ticket, did: &'static str, by: &str, sent: bool, proxy_of: Option<String>, tell: Vec<String>, part: &OrgPart, now: i64, fx: &mut Effects) -> Result<(), Refusal> {
        let round = t.round;
        self.cancel_clocks(t, fx);
        if t.pause_ends.take().is_some() {
            fx.put(round, What::CancelTimer { timer: Timer::PauseEnds, stage: t.stage_name().map(str::to_string) });
        }
        self.unpause(t, now, false)?;
        fx.record(round, Record { did, stage: t.stage_name().map(str::to_string), by: Some(by.to_string()), proxy_of, ..Record::default() });
        t.backs += 1;
        (t.status, t.at, t.sent_back_by) = (Status::WithRequester, None, sent.then(|| by.to_string()));
        self.on(Ending::SentBack, t, part, fx)?;
        if let Some(days) = self.line(|l| if let TicketLine::Resubmit { days, .. } = l { Some(*days) } else { None }) {
            // counted from the day it goes back where the line's calendar is, to the end of a day there
            let cal = self.resubmit_calendar();
            let due = Self::instant_in(self.add_open(cal, self.day_in(now, cal), days)?, 1440, self.offset_in(cal));
            t.resubmit_due = Some(due);
            fx.put(round, What::SetTimer { timer: Timer::ResubmitLate, stage: None, at: due });
        }
        if !tell.is_empty() {
            fx.put(round, What::Notify { to: tell, notice: Notice::Returned, stage: None });
        }
        Ok(())
    }

    fn send_back(&self, t: &mut Ticket, by: &str, to: Option<&str>, part: &OrgPart, now: i64, fx: &mut Effects) -> Result<(), Refusal> {
        let i = self.stage_in(t, "send back", &[Status::InProgress])?;
        let seats = self.seats(t, i, part, by)?;
        let Some((to_requester, times)) = self.backs() else {
            return Err(Refusal::new("event", tr!("この申請の種類には `send back` の行がありません", "the request has no `send back` line")));
        };
        if t.backs >= times {
            return Err(Refusal::new("limit", tr!("合わせて {times} 回までです（`send back … at most {times} times`）。承認か却下を選んでください", "the ticket has gone back {times} times already (`send back … at most {times} times`): approve it or reject it")));
        }
        // a proxy sends it back in the seat they answer for
        let proxy_of = seats.first().map(|(_, s)| s.clone()).filter(|s| s != by);
        let Some(k) = to else { return self.to_requester(t, "sent back", by, true, proxy_of, Self::theirs(t), part, now, fx) };
        if to_requester {
            return Err(Refusal::new("event", tr!("この申請の種類は、申請者へだけ戻せます（`send back to the requester`）", "the request goes back to the requester alone (`send back to the requester`)")));
        }
        let cur = t.stages[i].name.clone();
        let Some(j) = t.stages[..i].iter().position(|s| s.name == k) else {
            return Err(Refusal::new("event", tr!("`{k}` は、このチケットの `{cur}` より前のステップではありません", "`{k}` is no stage of the ticket before `{cur}`")));
        };
        // a stage that waits for a later one takes it too: sent back to, it is answered there
        if !matches!(t.stages[j].phase, Phase::Answered | Phase::Once | Phase::Deferred) {
            return Err(Refusal::new("event", tr!("ステップ `{k}` には、このチケットでだれも答えていません", "nobody answered the stage `{k}` on this ticket")));
        }
        let (round, org) = (t.round, t.stages[i].org);
        self.cancel_clocks(t, fx);
        t.backs += 1;
        fx.record(round, Record { did: "sent back", stage: Some(cur), by: Some(by.to_string()), proxy_of, org, to: vec![k.to_string()], ..Record::default() });
        self.unwind(t, j, None);
        self.advance(t, j, true, part, now, fx)
    }

    fn withdraw(&self, t: &mut Ticket, by: &str, part: &OrgPart, now: i64, fx: &mut Effects) -> Result<(), Refusal> {
        let i = self.stage_in(t, "withdraw", &[Status::InProgress, Status::Paused])?;
        self.theirs_to_do(t, part, by, "withdraw")?;
        let Some(until) = self.line(|l| if let TicketLine::Withdraw { until, .. } = l { Some(until.as_ref()) } else { None }) else {
            return Err(Refusal::new("event", tr!("この申請の種類には `withdraw` の行がありません", "the request has no `withdraw` line")));
        };
        if let Some(u) = until {
            // until that stage answers: the ticket is at it, or before it
            let order: Vec<String> = self.s.stages().unwrap_or_default().iter().map(|n| n.text.clone()).collect();
            let pos = |n: &str| order.iter().position(|x| x == n);
            let (cur, un) = (&t.stages[i].name, &u.text);
            // a stage that waits for a later one has not answered: the request is withdrawn until
            // the approval it waits for is given
            let waits = cur == un || t.deferred(i).iter().any(|(a, _)| t.stages[*a].name == *un);
            // whether the ticket has come past it: the stage it is at is after it in the route, or
            // one it came to before is (a stage that waited comes again after later ones)
            let past = |n: &String| matches!((pos(n), pos(un)), (Some(a), Some(b)) if a > b);
            let before = t.stages[..i].iter().any(|s| s.name == *un || past(&s.name));
            if (past(cur) || before) && !waits {
                return Err(Refusal::new("state", tr!("取り下げられるのは `{un}` が答えるまでです（`withdraw until {un}`）。チケットは `{cur}` にあります", "the request is withdrawn until `{un}` answers (`withdraw until {un}`), and the ticket is at `{cur}`")));
            }
        }
        // a ticket that has gone back as often as it may cannot be made again: it ends
        if t.backs >= self.backs().map_or(0, |(_, n)| n) {
            self.unpause(t, now, false)?;
            return self.end(t, Status::Withdrawn, Ending::Withdrawn, "ticket withdrawn", Some(by), Self::for_requester(t, by), None, part, now, fx);
        }
        let tell = self.waited(&t.stages[i], part);
        self.to_requester(t, "withdrawn", by, false, Self::for_requester(t, by), tell, part, now, fx)
    }

    fn discard(&self, t: &mut Ticket, by: &str, part: &OrgPart, now: i64, fx: &mut Effects) -> Result<(), Refusal> {
        if t.status != Status::WithRequester {
            return self.stage_in(t, "discard", &[]).map(|_| ());
        }
        self.theirs_to_do(t, part, by, "discard")?;
        self.end(t, Status::Withdrawn, Ending::Withdrawn, "ticket withdrawn", Some(by), Self::for_requester(t, by), None, part, now, fx)
    }

    /// The ASCII name of the field the name names (its name as written, or its ASCII name).
    fn ascii_of(&self, n: &str) -> String {
        self.s.fields().iter().find(|f| f.name.ascii() == n || f.name.text == n).map(|f| f.name.ascii().to_string()).unwrap_or_else(|| n.to_string())
    }

    /// The values of a form given again or edited (DESIGN 7.3), by the lowered names of the fields:
    /// the values `changed` laid over the ones the ticket held (`was`, what was given), the fields
    /// `clear` names made empty, then each field the form does not show for them dropped
    /// ([`Machine::unshown`]). A value of none is not given: the ticket keeps the one it holds, as
    /// `null` in the JSON keeps it (DESIGN 12.4); `clear` is what makes a field empty.
    fn laid(&self, was: &Values, changed: &Values, clear: &[String]) -> Values {
        let mut given = was.clone();
        let mut kept = Vec::new();
        for (n, v) in changed.iter().filter(|(_, v)| *v != Value::None) {
            let ascii = self.ascii_of(n);
            match given.iter_mut().find(|(k, _)| *k == ascii) {
                Some(slot) => slot.1 = v.clone(),
                None => given.push((ascii.clone(), v.clone())),
            }
            kept.push(ascii);
        }
        let clear: Vec<String> = clear.iter().map(|n| self.ascii_of(n)).collect();
        given.retain(|(n, _)| !clear.contains(n));
        self.unshown(given, &kept)
    }

    /// The values with the value of each field the form does not show for them dropped (DESIGN
    /// 7.3), but the fields `kept` (the ones the event gives a value, which the form refuses where
    /// it does not show them: [`taken::missing`]): every field hidden at once, then again for what
    /// is left, until none is (a condition may read a field dropped, or a computation that reads
    /// one; values only go, so it ends within as many rounds as there are fields). Where the form's
    /// computations cannot be worked out for the values (one reads a field that is empty, one is
    /// outside its range), nothing more is dropped, and the form refuses them ([`Machine::formed`]).
    pub(crate) fn unshown(&self, mut vals: Values, kept: &[String]) -> Values {
        let Some(ff) = self.lowered.files.iter().find(|f| f.part == crate::lower::Part::Form) else { return vals };
        let named = |want: &str| ff.renamed.iter().find(|(_, w)| w == want).map(|(g, _)| g.clone()).unwrap_or_else(|| want.to_string());
        let hidable: Vec<String> = self.s.fields().iter().filter(|f| f.shown_when.is_some() && f.computed.is_none()).map(|f| f.name.ascii().to_string()).filter(|n| !kept.contains(n)).collect();
        while let Ok(Some(outs)) = self.fs(|| super::eval::form_outs(self.s, self.suite, self.lowered, &vals)) {
            let hidden = |n: &String| outs.iter().any(|(o, v)| *o == named(&format!("{n}_shown")) && *v == Value::Bool(false));
            let dropped: Vec<&String> = hidable.iter().filter(|n| vals.iter().any(|(k, v)| k == *n && *v != Value::None) && hidden(n)).collect();
            if dropped.is_empty() {
                break;
            }
            vals.retain(|(n, _)| !dropped.contains(&n));
        }
        vals
    }

    /// The fields that held a value before an event and hold none after it (made empty, or dropped
    /// as the form no longer shows them), by their ASCII names, in the order the form declares
    /// them: what the line of the history says (`cleared`, DESIGN 7.4).
    fn lost(&self, was: &Values, now: &Values) -> Vec<String> {
        let has = |vals: &Values, n: &str| vals.iter().any(|(k, v)| k == n && *v != Value::None);
        self.s.fields().iter().filter(|f| f.computed.is_none()).map(|f| f.name.ascii().to_string()).filter(|n| has(was, n) && !has(now, n)).collect()
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    fn resubmit(&self, t: &mut Ticket, by: &str, form: &Values, clear: &[String], attempt: &str, part: &OrgPart, now: i64, fx: &mut Effects) -> Result<(), Refusal> {
        if t.status != Status::WithRequester {
            return self.stage_in(t, "resubmit", &[]).map(|_| ());
        }
        self.theirs_to_do(t, part, by, "resubmit")?;
        self.usable_on(part.day)?;
        if let Some(said) = taken::given(self.s, form, Giver::Requester, &[], &self.when(part)).or_else(|| taken::cleared(self.s, clear, form, Giver::Requester, &[])) {
            return Err(Refusal::form(said));
        }
        let was = self.given_of(&t.form);
        let (form, stages) = self.formed(&self.laid(&was, form, clear), part)?;
        let cleared = self.lost(&was, &form);
        if t.resubmit_due.take().is_some() {
            fx.put(t.round, What::CancelTimer { timer: Timer::ResubmitLate, stage: None });
        }
        t.round += 1;
        (t.form, t.stages, t.sent_back_by, t.attempt) = (form, stages, None, attempt.to_string());
        fx.record(t.round, Record { did: "resubmitted", by: Some(by.to_string()), proxy_of: Self::for_requester(t, by), org: Some(part.v().from), cleared, ..Record::default() });
        self.on(Ending::Submitted, t, part, fx)?;
        self.advance(t, 0, false, part, now, fx)
    }

    fn edit(&self, t: &mut Ticket, by: &str, fields: &Values, clear: &[String], part: &OrgPart, fx: &mut Effects) -> Result<(), Refusal> {
        let i = self.stage_in(t, "edit", &[Status::InProgress])?;
        self.approver(t, i, part, by)?;
        let editable: Vec<String> = self.line(|l| if let TicketLine::ApproversEdit { fields, .. } = l { Some(fields.iter().map(|w| w.text.clone()).collect()) } else { None }).unwrap_or_default();
        if let Some(said) = taken::given(self.s, fields, Giver::Approver, &editable, &self.when(part)).or_else(|| taken::cleared(self.s, clear, fields, Giver::Approver, &editable)) {
            return Err(Refusal::form(said));
        }
        let was = self.given_of(&t.form);
        let (form, stages) = self.formed(&self.laid(&was, fields, clear), part)?;
        let cleared = self.lost(&was, &form);
        t.form = form;
        // the route after this stage is decided again
        let order: Vec<String> = self.s.stages().unwrap_or_default().iter().map(|n| n.text.clone()).collect();
        let pos = |n: &str| order.iter().position(|x| x == n);
        // the last of them the ticket came to: a stage that waited comes again after later ones
        let cur = t.stages[..=i].iter().map(|s| pos(&s.name)).max().flatten();
        t.stages.truncate(i + 1);
        t.stages.extend(stages.into_iter().filter(|s| pos(&s.name) > cur));
        t.edits += 1;
        let stage = t.stages[i].name.clone();
        let proxy_of = self.acting_for(t, i, part, by);
        // the fields edited (none is not given), in the order the form declares them, whatever
        // order they came in
        let to: Vec<String> = self.in_form_order(fields.iter().filter(|(_, v)| *v != Value::None).cloned().collect()).into_iter().map(|(n, _)| n).collect();
        fx.record(t.round, Record { did: "edited", stage: Some(stage.clone()), by: Some(by.to_string()), proxy_of, to, cleared, ..Record::default() });
        // an agent that has yet to answer the stage checks the form as it is now: it is asked again
        // (its answer to the form as it was is made on the revision before, and refused)
        let asked: Vec<String> = t.stages[i].parts.iter().filter_map(|p| if let PartState::Agent(f) = &p.state { Some(f.clone()) } else { None }).collect();
        for flow in asked {
            fx.put(t.round, What::AskAgent { flow: flow.clone(), stage: stage.clone(), args: self.flow_args(t) });
            fx.record(t.round, Record { did: "agent asked", stage: Some(stage.clone()), org: Some(part.v().from), note: Some(flow), ..Record::default() });
        }
        Ok(())
    }

    fn add_approver(&self, t: &mut Ticket, by: Option<&str>, approver: &str, which: Option<&str>, part: &OrgPart, fx: &mut Effects) -> Result<(), Refusal> {
        let i = self.stage_in(t, "add approver", &[Status::InProgress, Status::Paused])?;
        self.may_answer(t, part, approver)?;
        let (round, paused) = (t.round, t.status == Status::Paused);
        let st = &mut t.stages[i];
        let stage = st.name.clone();
        let waits = |p: &Part| matches!(p.state, PartState::Open | PartState::Stopped);
        let j = match which {
            Some(w) => st.parts.iter().position(|p| p.way == w && waits(p)),
            None => st.parts.iter().position(|p| p.state == PartState::Stopped).or_else(|| st.parts.iter().position(waits)),
        };
        let Some(j) = j else {
            return Err(Refusal::new("event", tr!("ステップ `{stage}` に、承認者を足せる所がありません", "the stage `{stage}` has no part that waits for an approver")));
        };
        let p = &mut st.parts[j];
        if p.approvers.iter().any(|a| a == approver) {
            return Err(Refusal::new("event", tr!("{approver} は、もうステップ `{stage}` の承認者です", "{approver} is an approver of the stage `{stage}` already")));
        }
        p.approvers.push(approver.to_string());
        if p.state == PartState::Stopped && p.approvers.len() >= p.needed.max(1) {
            p.state = PartState::Open;
        }
        p.needed = p.needed.max(1);
        p.sole = if p.approvers.len() == 1 && p.needed == 1 { Sole::By(approver.to_string()) } else { Sole::Other };
        fx.record(round, Record { did: "approver added", stage: Some(stage.clone()), by: by.map(str::to_string), to: vec![approver.to_string()], ..Record::default() });
        if !paused {
            fx.put(round, What::Notify { to: vec![approver.to_string()], notice: Notice::Waiting, stage: Some(stage) });
        }
        Ok(())
    }

    fn hand_over(&self, t: &mut Ticket, by: Option<&str>, from: &str, to: &str, part: &OrgPart, fx: &mut Effects) -> Result<(), Refusal> {
        let i = self.stage_in(t, "hand over", &[Status::InProgress, Status::Paused])?;
        self.may_answer(t, part, to)?;
        let (round, paused) = (t.round, t.status == Status::Paused);
        let st = &mut t.stages[i];
        let stage = st.name.clone();
        let mut done = false;
        for p in st.parts.iter_mut().filter(|p| p.state == PartState::Open) {
            if !p.approvers.iter().any(|a| a == from) || p.answers.iter().any(|a| a.seat == from) || p.approvers.iter().any(|a| a == to) {
                continue;
            }
            for a in p.approvers.iter_mut().filter(|a| *a == from) {
                *a = to.to_string();
            }
            if p.sole == Sole::By(from.to_string()) {
                p.sole = Sole::By(to.to_string());
            }
            done = true;
        }
        if !done {
            return Err(Refusal::new("event", tr!("{from} は、ステップ `{stage}` でまだ答えていない承認者ではありません（または {to} がもう承認者です）", "{from} is no approver of the stage `{stage}` who has yet to answer (or {to} is one already)")));
        }
        fx.record(round, Record { did: "handed over", stage: Some(stage.clone()), by: by.map(str::to_string), to: vec![from.to_string(), to.to_string()], ..Record::default() });
        if !paused {
            fx.put(round, What::Notify { to: vec![to.to_string()], notice: Notice::Waiting, stage: Some(stage) });
        }
        Ok(())
    }

    /// A comment (DESIGN 7.2): a line of the history, and a notice to the requester, the one who
    /// made the request for them, the one who sent it back to the requester while it is with them,
    /// and the ones the stage waits for, past the one who wrote it. The ticket is as it was: its
    /// state, its clocks and its revision do not move. Who may write one is the gate's to say
    /// (DESIGN 12.4).
    fn comment(&self, t: &Ticket, by: &str, text: Option<&str>, part: &OrgPart, fx: &mut Effects) -> Result<(), Refusal> {
        let Some(text) = text else { return Err(Refusal::new("comment", tr!("`comment` の文が空です", "the text of `comment` is empty"))) };
        let stage = t.stage_name().map(str::to_string);
        fx.record(t.round, Record { did: "commented", stage: stage.clone(), by: Some(by.to_string()), comment: Some(text.to_string()), ..Record::default() });
        let mut to = Self::theirs(t);
        for x in t.sent_back_by.iter().cloned().chain(t.stage().map(|st| self.waited(st, part)).unwrap_or_default()) {
            if !to.contains(&x) {
                to.push(x);
            }
        }
        to.retain(|x| x != by);
        if !to.is_empty() {
            fx.put(t.round, What::Notify { to, notice: Notice::Comment, stage });
        }
        Ok(())
    }

    fn retract(&self, t: &mut Ticket, by: &str, part: &OrgPart, now: i64, fx: &mut Effects) -> Result<(), Refusal> {
        let i = self.stage_in(t, "retract", &[Status::InProgress])?;
        if self.line(|l| matches!(l, TicketLine::Retract { .. }).then_some(())).is_none() {
            return Err(Refusal::new("event", tr!("この申請の種類には `retract approval` の行がありません", "the request has no `retract approval` line")));
        }
        let cur = t.stages[i].name.clone();
        if t.stages[i].parts.iter().any(|p| !p.answers.is_empty() || matches!(p.state, PartState::Checked(_))) {
            return Err(Refusal::new("state", tr!("ステップ `{cur}` に、もう答えがあります。前の承認は取り消せません", "the stage `{cur}` has an answer already, and the approval before it is not retracted")));
        }
        // the stage the approval was given at: the last one before this that people answered, past
        // the ones that approval did, the ones nobody answers and the circulations
        let mut j = i;
        let j = loop {
            let none = || Refusal::new("state", tr!("`{cur}` の前に、取り消せる承認がありません", "no approval before `{cur}` is there to retract"));
            if j == 0 {
                return Err(none());
            }
            j -= 1;
            match t.stages[j].phase {
                Phase::Answered => break j,
                // a stage a part of which waits for a later one: answered there, where it started
                Phase::Deferred if t.stages[j].started.is_some() => break j,
                Phase::Skipped | Phase::Once | Phase::Circulated | Phase::Deferred => continue,
                _ => return Err(none()),
            }
        };
        let back = t.stages[j].name.clone();
        // the seat the approval was given in: a proxy takes back the approval they gave for another
        let Some(seat) = t.stages[j].parts.iter().flat_map(|p| p.answers.iter()).find(|a| a.by == by && a.yes).map(|a| a.seat.clone()) else {
            let who = part.v().who(by);
            return Err(Refusal::new("person", tr!("{}は、ステップ `{back}` を承認していません", "{} did not approve the stage `{back}`", crate::org::ja_word(&who.ja); who.en)));
        };
        let proxy_of = (seat != by).then_some(seat);
        let round = t.round;
        self.cancel_clocks(t, fx);
        for p in t.stages[j].parts.iter_mut() {
            p.answers.retain(|a| a.by != by);
            if p.state == PartState::Passed && p.yes() < p.needed {
                p.state = PartState::Open;
            }
        }
        t.stages[j].phase = Phase::Open;
        self.unwind(t, j + 1, Some(j));
        t.at = Some(j);
        fx.record(round, Record { did: "retracted", stage: Some(back.clone()), by: Some(by.to_string()), proxy_of, ..Record::default() });
        // its clocks are the ones it had: the deadline does not move
        self.set_clocks(t, j, false, now, fx)?;
        let to = self.waited(&t.stages[j], part);
        if !to.is_empty() {
            fx.put(round, What::Notify { to, notice: Notice::Waiting, stage: Some(back) });
        }
        Ok(())
    }

    fn pause(&self, t: &mut Ticket, by: &str, part: &OrgPart, now: i64, fx: &mut Effects) -> Result<(), Refusal> {
        let i = self.stage_in(t, "pause", &[Status::InProgress])?;
        self.approver(t, i, part, by)?;
        let Some(days) = self.line(|l| if let TicketLine::Pause { days, .. } = l { Some(*days) } else { None }) else {
            return Err(Refusal::new("event", tr!("この申請の種類には `pause` の行がありません", "the request has no `pause` line")));
        };
        if t.pause_left == 0 {
            return Err(Refusal::new("limit", tr!("保留は合わせて{days}営業日までで、もう残っていません（`pause at most {days} business days`）", "the ticket has been paused for the {days} business days it may be (`pause at most {days} business days`)")));
        }
        // to the end of the last business day it may take, counted from the day it is where the
        // pause's calendar is
        let cal = self.pause_calendar();
        let ends = Self::instant_in(self.add_open(cal, self.day_in(now, cal), t.pause_left)?, 1440, self.offset_in(cal));
        let (round, stage, proxy_of) = (t.round, t.stages[i].name.clone(), self.acting_for(t, i, part, by));
        self.cancel_clocks(t, fx);
        (t.status, t.paused_at, t.pause_ends) = (Status::Paused, Some(now), Some(ends));
        fx.record(round, Record { did: "paused", stage: Some(stage.clone()), by: Some(by.to_string()), proxy_of, ..Record::default() });
        fx.put(round, What::Notify { to: Self::theirs(t), notice: Notice::Paused, stage: Some(stage.clone()) });
        fx.put(round, What::SetTimer { timer: Timer::PauseEnds, stage: Some(stage), at: ends });
        Ok(())
    }

    /// `resume` by an approver, or the clock of the pause (`by` None): the stage's clocks are set
    /// again, later by the business days it was paused for.
    fn resume(&self, t: &mut Ticket, by: Option<&str>, part: &OrgPart, now: i64, fx: &mut Effects) -> Result<(), Refusal> {
        let ev = if by.is_some() { "resume" } else { "pause ends" };
        let i = self.stage_in(t, ev, &[Status::Paused])?;
        match by {
            Some(b) => self.approver(t, i, part, b)?,
            None => self.rung(t.pause_ends, ev, t.stage_name(), now)?,
        }
        let (round, stage) = (t.round, t.stages[i].name.clone());
        let proxy_of = by.and_then(|b| self.acting_for(t, i, part, b));
        let used = self.unpause(t, now, by.is_none())?;
        if t.pause_ends.take().is_some() && by.is_some() {
            fx.put(round, What::CancelTimer { timer: Timer::PauseEnds, stage: Some(stage.clone()) });
        }
        t.stages[i].held += used;
        t.status = Status::InProgress;
        fx.record(round, Record { did: "resumed", stage: Some(stage.clone()), by: by.map(str::to_string), proxy_of, note: Some(format!("{used} business days")), ..Record::default() });
        self.set_clocks(t, i, false, now, fx)?;
        let mut to = Self::theirs(t);
        to.extend(self.waited(&t.stages[i], part));
        fx.put(round, What::Notify { to, notice: Notice::Resumed, stage: Some(stage) });
        Ok(())
    }

    fn remind(&self, t: &mut Ticket, part: &OrgPart, now: i64, fx: &mut Effects) -> Result<(), Refusal> {
        let i = self.stage_in(t, "remind", &[Status::InProgress])?;
        self.rung(t.stages[i].remind, "remind", Some(&t.stages[i].name), now)?;
        (t.stages[i].remind, t.stages[i].reminded) = (None, true);
        let (to, stage) = (self.waited(&t.stages[i], part), t.stages[i].name.clone());
        fx.record(t.round, Record { did: "reminded", stage: Some(stage.clone()), to: to.clone(), ..Record::default() });
        if !to.is_empty() {
            fx.put(t.round, What::Notify { to, notice: Notice::Reminder, stage: Some(stage) });
        }
        Ok(())
    }

    fn resubmit_late(&self, t: &mut Ticket, part: &OrgPart, now: i64, fx: &mut Effects) -> Result<(), Refusal> {
        if t.status != Status::WithRequester {
            return self.stage_in(t, "resubmit late", &[]).map(|_| ());
        }
        self.rung(t.resubmit_due, "resubmit late", None, now)?;
        t.resubmit_due = None;
        self.end(t, Status::Expired, Ending::Expired, "ticket expired", None, None, Some("resubmit late".into()), part, now, fx)
    }

    /// The deadline of the ticket's stage passed (DESIGN 5.10, 7.3).
    fn late(&self, t: &mut Ticket, part: &OrgPart, now: i64, fx: &mut Effects) -> Result<(), Refusal> {
        let i = self.stage_in(t, "late", &[Status::InProgress])?;
        self.rung(t.stages[i].due, "late", Some(&t.stages[i].name), now)?;
        let o = self.occ_of(&t.stages[i])?;
        let (round, name, up) = (t.round, t.stages[i].name.clone(), t.stages[i].up);
        t.stages[i].due = None;
        if t.stages[i].remind.take().is_some() {
            fx.put(round, What::CancelTimer { timer: Timer::Remind, stage: Some(name.clone()) });
        }
        // a summary that does not come: the ticket goes on without it
        if let Spec::Note(_) = o.spec {
            fx.record(round, Record { did: "note missing", stage: Some(name), note: Some("late".into()), ..Record::default() });
            t.stages[i].phase = Phase::Noted;
            return self.advance(t, i + 1, false, part, now, fx);
        }
        let expire = |m: &Self, t: &mut Ticket, fx: &mut Effects, note: String| m.end(t, Status::Expired, Ending::Expired, "ticket expired", None, None, Some(note), part, now, fx);
        let then = o.late.and_then(|l| l.then);
        let action = match (up, o.late) {
            (0, Some(l)) => Some(l.first),
            (1, Some(_)) => then,
            _ => None,
        };
        let by = match action {
            None | Some(Late::Expire) => return expire(self, t, fx, "late".into()),
            Some(Late::Stay) => {
                fx.record(round, Record { did: "late", stage: Some(name), note: Some("it waits on".into()), ..Record::default() });
                return Ok(());
            }
            Some(Late::Escalate(by)) => by,
        };
        let l = o.late.expect("a stage that goes up has a `when late` line");
        let rq = Self::requester(t);
        let sv = self.solver(&rq, part);
        // where each part that still waits goes up to
        let mut moves: Vec<(usize, Vec<String>, Option<String>, Option<String>)> = Vec::new();
        let mut nowhere = false;
        let ways = self.ways(o, &t.stages[i]);
        for (j, p) in t.stages[i].parts.iter().enumerate().filter(|(_, p)| !p.done()) {
            let got = match (&p.state, by) {
                // an agent that has not answered: with no table, the way after `else` answers
                (PartState::Agent(_), None) => match ways.get(j).map(|(way, _, f)| self.solve(&sv, f, way, t, part)) {
                    Some(Solved::People(people, team)) => vec![LateTo::People { people, way: None, team }],
                    Some(Solved::Nobody { .. }) => vec![LateTo::Nowhere(ritsu_base::text::Text::default())],
                    None => Vec::new(),
                },
                _ => listing::late_to(&sv, l.file, o.file, by.as_ref(), Some(&p.key), &p.approvers, p.team.as_deref()),
            };
            let to = got.iter().find_map(|g| if let LateTo::People { people, way, team } = g { Some((people.clone(), way.clone(), team.clone())) } else { None });
            match to {
                Some((people, way, team)) => moves.push((j, people, way, team)),
                None => nowhere = true,
            }
        }
        if nowhere {
            // it cannot go up: what follows `escalate` is done at this deadline. Why it cannot is
            // the check's to say (W507) and `simulate`'s; the history says that it could not
            if up == 0 && matches!(then, Some(Late::Stay)) {
                fx.record(round, Record { did: "late", stage: Some(name), note: Some("nowhere to go up; it waits on".into()), ..Record::default() });
                return Ok(());
            }
            return expire(self, t, fx, "late; nowhere to go up".into());
        }
        // going up starts no stage: its deadline is counted from the day the stage started, which
        // the days the request is used on cover (DESIGN 9.6), and `clocks` refuses what they do not
        let st = &mut t.stages[i];
        let mut to: Vec<String> = Vec::new();
        for (j, people, way, team) in moves {
            let p = &mut st.parts[j];
            p.needed = if team.is_some() { p.needed.clamp(1, people.len().max(1)) } else { 1 };
            // the one it goes up to alone answers it, an agent's part too: they answer in place of
            // the agent, and a stage right after it that they alone answer is done with that
            // approval (DESIGN 5.12)
            p.sole = if people.len() == 1 && p.needed == 1 { Sole::By(people[0].clone()) } else { Sole::Other };
            p.key = way.unwrap_or_else(|| p.key.clone());
            p.answers.clear();
            p.state = PartState::Open;
            to.extend(people.iter().filter(|x| !to.contains(x)).cloned().collect::<Vec<_>>());
            (p.approvers, p.team) = (people, team);
        }
        (st.up, st.reminded) = (st.up + 1, false);
        let (started, held, up) = (st.started.unwrap_or(now), st.held, st.up);
        // the ones it went up to have a deadline and a reminder of their own, counted from the
        // time the deadline passed (not from `now`: an event given late lengthens neither), the
        // reminder not yet rung
        let (remind, due) = self.clocks(&name, started, held, up)?;
        let st = &mut t.stages[i];
        (st.remind, st.due) = Self::set_at(remind, due, now);
        fx.record(round, Record { did: "went up", stage: Some(name.clone()), to: to.clone(), org: Some(part.v().from), note: Some("late".into()), ..Record::default() });
        let tell = self.waited(&t.stages[i], part);
        if !tell.is_empty() {
            fx.put(round, What::Notify { to: tell, notice: Notice::WentUp, stage: Some(name.clone()) });
        }
        if let Some(at) = t.stages[i].remind {
            fx.put(round, What::SetTimer { timer: Timer::Remind, stage: Some(name.clone()), at });
        }
        if let Some(at) = t.stages[i].due {
            fx.put(round, What::SetTimer { timer: Timer::Late, stage: Some(name), at });
        }
        Ok(())
    }

    /// The stage `i` is done by a flow's answer while the ticket is paused: nobody is left for the
    /// pause to wait for, and it ends with the stage, the business days it took counted.
    fn lift(&self, t: &mut Ticket, i: usize, now: i64, fx: &mut Effects) -> Result<(), Refusal> {
        if t.status != Status::Paused {
            return Ok(());
        }
        let (round, stage) = (t.round, t.stages[i].name.clone());
        let used = self.unpause(t, now, false)?;
        if t.pause_ends.take().is_some() {
            fx.put(round, What::CancelTimer { timer: Timer::PauseEnds, stage: Some(stage.clone()) });
        }
        fx.record(round, Record { did: "resumed", stage: Some(stage.clone()), note: Some(format!("{used} business days")), ..Record::default() });
        fx.put(round, What::Notify { to: Self::theirs(t), notice: Notice::Resumed, stage: Some(stage) });
        // the pause cancelled the stage's clocks: none is set
        (t.stages[i].remind, t.stages[i].due) = (None, None);
        t.status = Status::InProgress;
        Ok(())
    }

    /// An agent's flow answered (`ok`, or a doubt with its reason), or failed, which is a doubt; a
    /// summary's flow that failed leaves the ticket to go on without it. A flow answers when it is
    /// done, so a paused ticket takes its answer too.
    #[allow(clippy::too_many_arguments)]
    fn agent(&self, t: &mut Ticket, ok: bool, reason: &str, failed: bool, part: &OrgPart, now: i64, fx: &mut Effects) -> Result<(), Refusal> {
        let ev = if failed { "flow failed" } else { "agent answered" };
        // a flow answers when it is done, a paused ticket too
        let i = self.stage_in(t, ev, &[Status::InProgress, Status::Paused])?;
        let o = self.occ_of(&t.stages[i])?;
        let (round, name) = (t.round, t.stages[i].name.clone());
        if let Spec::Note(_) = o.spec {
            if !failed {
                return Err(Refusal::new("event", tr!("ステップ `{name}` は要約です。`note ready` を受け付けます", "the stage `{name}` is a summary, and takes `note ready`")));
            }
            fx.record(round, Record { did: "note missing", stage: Some(name), note: Some("flow failed".into()), ..Record::default() });
            self.lift(t, i, now, fx)?;
            self.cancel_clocks(t, fx);
            t.stages[i].phase = Phase::Noted;
            return self.advance(t, i + 1, false, part, now, fx);
        }
        let Some(j) = t.stages[i].parts.iter().position(|p| matches!(p.state, PartState::Agent(_))) else {
            return Err(Refusal::new("state", tr!("ステップ `{name}` は、エージェントの答えを待っていません", "the stage `{name}` waits for no agent's answer")));
        };
        let PartState::Agent(flow) = t.stages[i].parts[j].state.clone() else { unreachable!() };
        // the flow in the note, and the reason the agent gave as the comment of the line
        let (note, comment) = (Some(flow.clone()), (!reason.is_empty()).then(|| reason.to_string()));
        if ok {
            t.stages[i].parts[j].state = PartState::Checked(flow);
            fx.record(round, Record { did: "agent ok", stage: Some(name), note, comment, ..Record::default() });
        } else {
            fx.record(round, Record { did: if failed { "agent failed" } else { "agent doubt" }, stage: Some(name.clone()), note, comment, ..Record::default() });
            // the way after `else` answers
            let rq = Self::requester(t);
            let sv = self.solver(&rq, part);
            let ways = self.ways(o, &t.stages[i]);
            let Some((way, q, f)) = ways.get(j) else { return Err(Refusal::new("unread", tr!("ステップ `{name}` の決め方がありません", "the stage `{name}` has no such way"))) };
            let mut np = self.part_of(&sv, o, &t.stages[i], way, *q, f, t, part, false, fx);
            // `, at the last stage`: the one who answers it alone answers a stage ahead too, so it
            // waits for the last of them, as a part does when its stage starts (not at a stage the
            // ticket was sent back to). They see the agent's doubt there (DESIGN 5.12)
            if listing::once(self.s.file) == Once::Last && !t.stages[i].again {
                if let (PartState::Open, Sole::By(x)) = (&np.state, &np.sole) {
                    if let Some(k) = self.last_of(&sv, t, i + 1, x, part) {
                        let to = t.stages[k].name.clone();
                        fx.record(round, Record { did: "deferred", stage: Some(name.clone()), org: Some(part.v().from), once_with: Some(to.clone()), to: vec![x.clone()], ..Record::default() });
                        np.state = PartState::Deferred(to);
                    }
                }
            }
            let to: Vec<String> = if np.state == PartState::Open { np.approvers.clone() } else { Vec::new() };
            let stopped = np.state == PartState::Stopped;
            t.stages[i].parts[j] = np;
            if !to.is_empty() {
                fx.put(round, What::Notify { to, notice: Notice::Waiting, stage: Some(name.clone()) });
            }
            if stopped {
                fx.put(round, What::Notify { to: Vec::new(), notice: Notice::Stopped, stage: Some(name) });
            }
        }
        if t.stages[i].parts.iter().all(|p| p.done()) {
            self.lift(t, i, now, fx)?;
            return self.pass(t, i, part, now, fx);
        }
        Ok(())
    }

    fn note_ready(&self, t: &mut Ticket, outputs: &Values, part: &OrgPart, now: i64, fx: &mut Effects) -> Result<(), Refusal> {
        let i = self.stage_in(t, "note ready", &[Status::InProgress, Status::Paused])?;
        let o = self.occ_of(&t.stages[i])?;
        let name = t.stages[i].name.clone();
        if !matches!(o.spec, Spec::Note(_)) {
            return Err(Refusal::new("state", tr!("ステップ `{name}` は要約ではありません", "the stage `{name}` is no summary")));
        }
        t.notes.push((name.clone(), outputs.clone()));
        fx.record(t.round, Record { did: "note attached", stage: Some(name), ..Record::default() });
        self.lift(t, i, now, fx)?;
        self.cancel_clocks(t, fx);
        t.stages[i].phase = Phase::Noted;
        self.advance(t, i + 1, false, part, now, fx)
    }
}
