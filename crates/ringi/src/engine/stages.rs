//! How a ticket comes to a stage and moves on from it (DESIGN 5.2, 5.4, 5.12, 7.3).
//!
//! A stage's approvers are chosen when it starts, in the part of the organization the event comes
//! with: each way to choose solved as the check solves it ([`Solver`]), a lookup in the row of the
//! master the form chose, a person the requester or the approver before chose in the field. A way
//! that finds nobody does what the stage's `when empty` says: the stage waits for an administrator,
//! goes up to the first head above, to the way after `then` where nobody is above, or is skipped;
//! each with its line of the history. A stage nobody answers is passed over, and a ticket no stage
//! of which anyone answers is approved as it is submitted.
//!
//! With `same approver approves once`, a part of the stage that one person alone answers is done
//! with the approval of the stage right before it, when that person alone approved it: the decision
//! is [`listing::before_on`], the one the check and `simulate` make, over the stages the ticket came
//! to. A stage done so, and one skipped, has no clock and tells nobody; the ticket goes on to the
//! next in the same move.
//!
//! With `same approver approves once, at the last stage`, such a part waits instead, for the last
//! stage ahead that the same person alone answers a part of, as the stages ahead would start now
//! ([`Machine::last_of`]): the approval given there does it ([`Machine::answered`]). A stage every
//! part of which waits has no clock and tells nobody either. What a part waits for is looked at
//! again each time the ticket moves on ([`Machine::settle`]): where the later stage no longer comes
//! to the person alone, has been answered without them, or has left the route, the part's approver
//! is decided again, and the part is started then, as a second coming of its stage, unless the one
//! who answers it now alone answers a stage ahead too.

use super::{Effects, Machine, Notice, OrgPart, Part, PartState, Phase, Record, Refusal, Sole, Stage, Status, Ticket, What};
use crate::ast::{AtTop, Ending, File, How, Quorum, Spec, WhenEmpty};
use crate::org::ProxyKind;
use crate::org::listing::{self, Before, Occ, Once, Out, Seen};
use crate::org::resolve::{Found, Solver, Up, Why};
use ritsu_ports::Value;

/// What a way to choose comes to for a ticket.
pub(crate) enum Solved {
    /// These people answer; the team, when they are a team's members.
    People(Vec<String>, Option<String>),
    /// Nobody: why (as a line of the history says it: fixed words and IDs, the same for the same
    /// part of the organization), where the stage goes up from, and whether it cannot go up at all
    /// (a person or a team the organization does not have, a row that is not there, nobody chosen).
    Nobody { why: String, up: Up, cannot: bool },
}

/// What became of the parts that wait for a later stage, as the ticket comes to a stage
/// ([`Machine::settle`]).
enum Settled {
    /// Each still has its stage to wait for.
    Clear,
    /// A stage whose parts no longer have one is started, and the ticket waits at it.
    Started,
    /// Such a stage came again, and nobody answers what it was to start with: the ticket goes on.
    Passed,
}

/// Why a way finds nobody, as a line of the history says it: in fixed words, with IDs and no
/// names, so that the same part of the organization gives the same line wherever it is solved.
fn why_note(why: &Why) -> String {
    match why {
        Why::PersonNotIn(id) => format!("{id} is not in the organization"),
        Why::TeamNotIn(id) => format!("the team {id} is not in the organization"),
        Why::HeadEmpty(t) => format!("the head of {t} is empty"),
        Why::OnlyRequester(_) => "it comes to the requester alone".to_string(),
        Why::NoTeamOfKind(k) => format!("no team of the kind {k} is above the requester"),
        Why::NoneAbove => "no head is above the requester".to_string(),
        Why::NoMembers(t) => format!("{t} has no member"),
    }
}

impl<'s, 'a> Machine<'s, 'a> {
    /// The stage of the route a ticket's stage is: by its name and the value of its output.
    pub(crate) fn occ(&self, st: &Stage) -> Option<&Occ<'s>> {
        let all = || self.paths.iter().flatten();
        all()
            .find(|o| o.stage == st.name && crate::lower::words::spec_value(self.s, o.spec).is_some_and(|(x, _)| x == st.value))
            .or_else(|| all().find(|o| o.stage == st.name && matches!(o.spec, Spec::RuleOutput(_))))
    }

    pub(crate) fn occ_of(&self, st: &Stage) -> Result<&Occ<'s>, Refusal> {
        let (n, v) = (&st.name, &st.value);
        self.occ(st).ok_or_else(|| Refusal::new("unread", tr!("ステップ `{n}`（`{v}`）が経路にありません", "the route has no stage `{n}` of `{v}`")))
    }

    /// The ways the stage names: each as the stage names it, with its quorum and the file that
    /// declares it. A rule's output names the way its value is.
    pub(crate) fn ways(&self, o: &Occ<'s>, st: &Stage) -> Vec<(String, Option<Quorum>, &'s File)> {
        match o.spec {
            Spec::Who(ws) => ws.iter().map(|w| (w.text.clone(), None, o.file)).collect(),
            Spec::Quorum(q, w, _) => vec![(w.text.clone(), Some(*q), o.file)],
            Spec::RuleOutput(_) => {
                let all = self.s.who_all();
                let name = all.iter().map(|w| &w.who.name).find(|n| n.ascii() == st.value || n.text == st.value).map(|n| n.text.clone()).unwrap_or_else(|| st.value.clone());
                let f = if Solver::who(o.file, &name).is_some() { o.file } else { self.s.file };
                vec![(name, None, f)]
            }
            _ => Vec::new(),
        }
    }

    /// The value of a field of the ticket's form that names a person or a row: by the field as a
    /// way writes it.
    pub(crate) fn chosen<'t>(&self, t: &'t Ticket, field: &str) -> Option<&'t str> {
        let ascii = self.s.field(field).map(|f| f.name.ascii().to_string()).unwrap_or_else(|| field.to_string());
        match t.value(&ascii) {
            Some(Value::Str(x) | Value::Enum(x)) if !x.is_empty() => Some(x.as_str()),
            _ => None,
        }
    }

    /// The way `way` declared in `f`, solved for the ticket: the way after `else` for an agent's.
    pub(crate) fn solve(&self, sv: &Solver, f: &File, way: &str, t: &Ticket, part: &OrgPart) -> Solved {
        let cannot = |why: String| Solved::Nobody { why, up: Up::Cannot, cannot: true };
        let Some(w) = Solver::who(f, way).or_else(|| Solver::who(self.s.file, way)) else {
            return cannot(format!("the way to choose {way} is not declared"));
        };
        let found = match &w.how {
            How::Lookup { path, by } if path.len() == 3 => {
                let (m, d) = (format!("{}.{}", path[0].text, path[1].text), crate::lex::date_text(part.day));
                let bn = &by.text;
                let Some(key) = self.chosen(t, bn) else { return cannot(format!("{bn} is empty, and no row of {m} is looked up")) };
                let Some(row) = part.row(&m, key) else {
                    return cannot(format!("{m} has no row {key} in effect on {d}"));
                };
                let col = &path[2].text;
                match row.values.iter().find(|(n, _)| n == col).map(|(_, x)| x.as_str()).filter(|x| !x.is_empty()) {
                    Some(id) => sv.person(id),
                    None => return cannot(format!("the row {key} of {m} has no {col}")),
                }
            }
            How::ChosenBy { field, .. } => {
                let fname = &field.text;
                match self.chosen(t, fname) {
                    Some(id) => sv.person(id),
                    None => return cannot(format!("nobody is chosen in {fname}")),
                }
            }
            How::Agent { otherwise, .. } => return self.solve(sv, f, &otherwise.text, t, part),
            _ => sv.way(f, &w.name.text).into_iter().next().map(|c| c.found).unwrap_or(Found::Open),
        };
        match found {
            Found::People(ps) => Solved::People(
                ps,
                match &w.how {
                    How::Team(tm) => Some(tm.text.clone()),
                    _ => None,
                },
            ),
            Found::Nobody(why, up) => {
                let cannot = up == Up::Cannot && matches!(why, Why::PersonNotIn(_) | Why::TeamNotIn(_));
                Solved::Nobody { why: why_note(&why), up, cannot }
            }
            Found::Open => cannot(format!("the organization does not decide the way to choose {way}")),
        }
    }

    /// The solver of a ticket's ways, on the part of the organization an event comes with.
    pub(crate) fn solver<'x>(&'x self, rq: &'x crate::org::resolve::Requester, part: &'x OrgPart) -> Solver<'x, 'a> {
        Solver { s: self.s, org: &part.org, v: part.v(), requester: rq, self_approval: listing::self_approval(self.s.file) }
    }

    /// One part of a stage as it starts: who answers the way `way`, or what the stage's `when empty`
    /// does with nobody, with its line of the history. `asked` is false for an agent's way whose
    /// flow has answered with a doubt: the way after `else` answers then.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn part_of(&self, sv: &Solver, o: &Occ<'s>, st: &Stage, way: &str, q: Option<Quorum>, f: &File, t: &Ticket, part: &OrgPart, ask: bool, fx: &mut Effects) -> Part {
        let who = Solver::who(f, way).or_else(|| Solver::who(self.s.file, way));
        let agent = match who.map(|w| &w.how) {
            Some(How::Agent { flow, .. }) => Some(flow.text.clone()),
            _ => None,
        };
        let chosen = matches!(who.map(|w| &w.how), Some(How::ChosenBy { .. }));
        let mut p = Part { way: way.to_string(), key: way.to_string(), approvers: Vec::new(), team: None, needed: 1, answers: Vec::new(), state: PartState::Open, sole: Sole::Other };
        let rec = |did: &'static str, to: Vec<String>, note: String| Record { did, stage: Some(st.name.clone()), org: Some(part.v().from), to, note: Some(note), ..Record::default() };
        if let (Some(flow), true) = (&agent, ask) {
            p.state = PartState::Agent(flow.clone());
            fx.put(t.round, What::AskAgent { flow: flow.clone(), stage: st.name.clone(), args: self.flow_args(t) });
            fx.record(t.round, rec("agent asked", Vec::new(), flow.clone()));
            return p;
        }
        // one person alone answers it, as `same approver approves once` reads it: not an agent's
        // way, nor a person chosen for the stage
        let sole_of = |ps: &[String], needed: usize| if ps.len() == 1 && needed == 1 && agent.is_none() && !chosen { Sole::By(ps[0].clone()) } else { Sole::Other };
        match self.solve(sv, f, way, t, part) {
            Solved::People(ps, team) => {
                p.needed = q.map_or(1, |q| listing::needed(q, ps.len()));
                p.sole = sole_of(&ps, p.needed);
                if ps.len() < p.needed {
                    let (n, k) = (p.needed, ps.len());
                    p.state = PartState::Stopped;
                    fx.record(t.round, rec("stopped", ps.clone(), format!("{k} of {way} cannot give the {n} approvals the stage needs")));
                }
                (p.approvers, p.team) = (ps, team);
            }
            // the requester has left the organization since the ticket was made: a way that asks
            // the line above them finds nobody, and that is not the top of the organization, which
            // `when empty` is written for. The stage waits for an administrator
            Solved::Nobody { why, .. } if part.v().person(&t.requester).is_none() => {
                let (rq, d) = (&t.requester, part.v().day());
                p.state = PartState::Stopped;
                fx.record(t.round, rec("stopped", Vec::new(), format!("{why}; the requester {rq} is not in the organization from {d}")));
            }
            Solved::Nobody { why, up, cannot } => {
                match &o.empty {
                    WhenEmpty::Stay => {
                        p.state = PartState::Stopped;
                        fx.record(t.round, rec("stopped", Vec::new(), why));
                    }
                    WhenEmpty::Skip => {
                        p.state = PartState::Skipped;
                        fx.record(t.round, rec("skipped", Vec::new(), why));
                    }
                    WhenEmpty::Escalate(_) if cannot => {
                        p.state = PartState::Stopped;
                        fx.record(t.round, rec("stopped", Vec::new(), format!("{why}; it cannot go up")));
                    }
                    WhenEmpty::Escalate(top) => match (sv.up(&up), top) {
                        (Some(h), _) => {
                            p.sole = sole_of(std::slice::from_ref(&h), 1);
                            fx.record(t.round, rec("went up", vec![h.clone()], why));
                            p.approvers = vec![h];
                        }
                        (None, Some(AtTop::Skip)) => {
                            p.state = PartState::Skipped;
                            fx.record(t.round, rec("skipped", Vec::new(), format!("{why}; nobody is above to go up to")));
                        }
                        (None, Some(AtTop::To(w))) => match self.solve(sv, o.empty_in, &w.text, t, part) {
                            Solved::People(ps, team) => {
                                p.sole = sole_of(&ps, 1);
                                p.key = w.text.clone();
                                fx.record(t.round, rec("went to", ps.clone(), format!("{why}; nobody is above to go up to: {}", w.text)));
                                (p.approvers, p.team) = (ps, team);
                            }
                            Solved::Nobody { why: w2, .. } => {
                                p.state = PartState::Stopped;
                                fx.record(t.round, rec("stopped", Vec::new(), format!("{why}; nobody is above to go up to, and {} has nobody: {w2}", w.text)));
                            }
                        },
                        (None, _) => {
                            p.state = PartState::Stopped;
                            fx.record(t.round, rec("stopped", Vec::new(), format!("{why}; nobody is above to go up to")));
                        }
                    },
                }
            }
        }
        p
    }

    /// What a flow the ticket asks is given: the ticket's ID, and the values of its form.
    pub(crate) fn flow_args(&self, t: &Ticket) -> ritsu_ports::Values {
        let mut out = vec![("request_id".to_string(), Value::Str(t.id.clone()))];
        out.extend(t.form.iter().cloned());
        out
    }

    /// The stages before the stage `i`, as `same approver approves once` reads them: what each came
    /// to on this ticket.
    fn seen(t: &Ticket, i: usize) -> Vec<Seen> {
        t.stages[..i]
            .iter()
            .map(|st| match st.phase {
                Phase::Circulated | Phase::Skipped | Phase::Ahead => Seen::Pass,
                Phase::Noted | Phase::Open | Phase::Deferred => Seen::Other,
                Phase::Answered | Phase::Once => {
                    let parts: Vec<(String, Vec<Out>)> = st
                        .parts
                        .iter()
                        .map(|p| {
                            let out = match (&p.state, &p.sole) {
                                (PartState::Skipped, _) | (_, Sole::Pass) => Out::Pass,
                                (_, Sole::By(x)) => Out::By(x.clone()),
                                (_, Sole::Other) => Out::Other,
                            };
                            (p.way.clone(), vec![out])
                        })
                        .collect();
                    match st.phase {
                        Phase::Once => Seen::Once(parts),
                        _ => Seen::Answered(parts),
                    }
                }
            })
            .collect()
    }

    /// The people a stage waits for: the approvers of its open parts who have yet to answer, and
    /// the proxies who may answer for them that day.
    pub(crate) fn waited(&self, st: &Stage, part: &OrgPart) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for p in st.parts.iter().filter(|p| p.state == PartState::Open) {
            for a in p.approvers.iter().filter(|a| !p.answers.iter().any(|x| x.seat == **a)) {
                if !out.contains(a) {
                    out.push(a.clone());
                }
            }
        }
        for a in out.clone() {
            for x in part.org.proxies.iter().filter(|x| x.kind == ProxyKind::Approve && x.person == a && self.proxy_on(x, part.day)) {
                if !out.contains(&x.proxy) {
                    out.push(x.proxy.clone());
                }
            }
        }
        out
    }

    /// The persons who alone answer a part of the stage `i`, were it to start now: how `same
    /// approver approves once, at the last stage` reads a stage ahead. What solving it would put in
    /// the history is not kept.
    fn alone_at(&self, sv: &Solver, t: &Ticket, i: usize, part: &OrgPart) -> Vec<String> {
        let st = &t.stages[i];
        let Some(o) = self.occ(st) else { return Vec::new() };
        let mut unkept = Effects::default();
        let mut out: Vec<String> = Vec::new();
        for (way, q, f) in self.ways(o, st) {
            let p = self.part_of(sv, o, st, &way, q, f, t, part, true, &mut unkept);
            if let (PartState::Open, Sole::By(x)) = (&p.state, &p.sole)
                && !out.contains(x)
            {
                out.push(x.clone());
            }
        }
        out
    }

    /// The last stage at or after `from` that `x` alone answers a part of, as the stages would start
    /// now: the one a part of an earlier stage that `x` alone answers waits for.
    fn last_of(&self, sv: &Solver, t: &Ticket, from: usize, x: &str, part: &OrgPart) -> Option<usize> {
        (from..t.stages.len()).rev().find(|k| self.alone_at(sv, t, *k, part).iter().any(|p| p == x))
    }

    /// The ticket waits at the stage `i`: it starts now, the ones it waits for are told, and its
    /// clocks are set.
    fn wait_at(&self, t: &mut Ticket, i: usize, part: &OrgPart, now: i64, fx: &mut Effects) -> Result<(), Refusal> {
        self.usable_on(part.day)?;
        let (round, name) = (t.round, t.stages[i].name.clone());
        let st = &mut t.stages[i];
        (st.phase, st.started) = (Phase::Open, Some(now));
        t.at = Some(i);
        let to = self.waited(&t.stages[i], part);
        if !to.is_empty() {
            fx.put(round, What::Notify { to, notice: Notice::Waiting, stage: Some(name.clone()) });
        }
        if t.stages[i].parts.iter().any(|p| p.state == PartState::Stopped) {
            fx.put(round, What::Notify { to: Vec::new(), notice: Notice::Stopped, stage: Some(name) });
        }
        self.set_clocks(t, i, true, now, fx)
    }

    /// The parts that wait for a later stage (`same approver approves once, at the last stage`), as
    /// the ticket comes to the stage `i`, or to the end of its route (DESIGN 5.12). A part still
    /// has its stage to wait for where that stage is ahead, and, when it is the one the ticket
    /// comes to, where the part's one approver alone answers a part of it now. A part that has not
    /// is decided again, in the part of the organization the event comes with: it waits on where
    /// the one who answers it now alone answers a stage ahead, for the last of them; else its
    /// stage comes again, here, with the parts of it that waited (the ones that wait on, too, which
    /// are the later coming's from then), its approvers the ones decided now and its clocks
    /// counted from now. One stage at a time, the first of the route: the ticket comes back here
    /// when it is answered.
    fn settle(&self, sv: &Solver, t: &mut Ticket, i: usize, part: &OrgPart, now: i64, fx: &mut Effects) -> Result<Settled, Refusal> {
        let (round, org) = (t.round, Some(part.v().from));
        let i = i.min(t.stages.len());
        let waiting = t.deferred(i);
        if waiting.is_empty() {
            return Ok(Settled::Clear);
        }
        let ahead: Vec<String> = t.stages[i..].iter().map(|s| s.name.clone()).collect();
        let here = if i < t.stages.len() { self.alone_at(sv, t, i, part) } else { Vec::new() };
        let mut stages: Vec<usize> = waiting.iter().map(|(a, _)| *a).collect();
        stages.dedup();
        for a in stages {
            // the parts of the stage that are started now, each by its place
            let mut started: Vec<(usize, Part)> = Vec::new();
            let name = t.stages[a].name.clone();
            for (_, k) in waiting.iter().filter(|(x, _)| *x == a) {
                let Some((x, to)) = t.stages[a].parts[*k].deferred().map(|(x, to)| (x.to_string(), to.to_string())) else { continue };
                let why = match ahead.iter().position(|n| *n == to) {
                    Some(0) if !here.contains(&x) => format!("the stage it waited for does not come to {x} alone"),
                    Some(_) => continue,
                    // (a stage that waits too takes the parts that waited for it with it as it starts)
                    None if t.stages[..i].iter().any(|s| s.name == to) => format!("the stage it waited for was answered without {x}"),
                    None => "the stage it waited for is not on the route".to_string(),
                };
                let st = &t.stages[a];
                let o = self.occ_of(st)?;
                let way = st.parts[*k].way.clone();
                let Some((_, q, f)) = self.ways(o, st).into_iter().find(|(w, ..)| *w == way) else {
                    return Err(Refusal::new("unread", tr!("ステップ `{name}` の決め方 `{way}` がありません", "the stage `{name}` has no way `{way}`")));
                };
                let mut made = Effects::default();
                let np = self.part_of(sv, o, st, &way, q, f, t, part, true, &mut made);
                let later = match (&np.state, &np.sole) {
                    (PartState::Open, Sole::By(y)) => self.last_of(sv, t, i, y, part).map(|k2| (y.clone(), t.stages[k2].name.clone())),
                    _ => None,
                };
                fx.append(made);
                match later {
                    // the one who answers it now alone answers a stage ahead: it waits on, for that
                    Some((y, to2)) => {
                        fx.record(round, Record { did: "deferred", stage: Some(name.clone()), org, once_with: Some(to2.clone()), to: vec![y], note: Some(why), ..Record::default() });
                        let st = &mut t.stages[a];
                        st.parts[*k] = Part { state: PartState::Deferred(to2), ..np };
                        if st.started.is_none() {
                            st.org = org;
                        }
                    }
                    None => {
                        fx.record(round, Record { did: "deferral ended", stage: Some(name.clone()), org, once_with: Some(to), to: np.approvers.clone(), note: Some(why), ..Record::default() });
                        started.push((*k, np));
                    }
                }
            }
            if started.is_empty() {
                continue;
            }
            // the stage comes again: the parts started now, and the ones of it that wait on
            let st = &t.stages[a];
            let mut again = Stage::new(&st.name, &st.value);
            (again.again, again.org) = (true, org);
            for (k, p) in st.parts.iter().enumerate().filter(|(_, p)| p.deferred().is_some()) {
                again.parts.push(match started.iter().position(|(k2, _)| *k2 == k) {
                    Some(n) => started[n].1.clone(),
                    None => p.clone(),
                });
            }
            t.stages.insert(i, again);
            let st = &mut t.stages[i];
            if st.parts.iter().all(|p| p.done()) {
                st.phase = if st.parts.iter().any(|p| matches!(p.state, PartState::Deferred(_))) { Phase::Deferred } else { Phase::Skipped };
                return Ok(Settled::Passed);
            }
            self.wait_at(t, i, part, now, fx)?;
            return Ok(Settled::Started);
        }
        Ok(Settled::Clear)
    }

    /// The stage `i` has every approval it needs (`same approver approves once, at the last
    /// stage`): each part that waited for it is done with the approval given at it in the seat of
    /// the part's one approver, by them or by a proxy of theirs, with its line of the history. A
    /// part the stage was answered without waits on, and [`Machine::settle`] starts it as the
    /// ticket moves on.
    pub(crate) fn answered(&self, t: &mut Ticket, i: usize, fx: &mut Effects) {
        let (round, name) = (t.round, t.stages[i].name.clone());
        for (a, k) in t.deferred(i) {
            let Some((x, to)) = t.stages[a].parts[k].deferred().map(|(x, to)| (x.to_string(), to.to_string())) else { continue };
            if to != name {
                continue;
            }
            let Some(by) = t.stages[i].parts.iter().flat_map(|p| p.answers.iter()).find(|ans| ans.yes && ans.seat == x).map(|ans| ans.by.clone()) else { continue };
            let st = &mut t.stages[a];
            st.parts[k].state = PartState::Once(name.clone());
            fx.record(round, Record { did: "approved", stage: Some(st.name.clone()), proxy_of: (by != x).then_some(x), by: Some(by), org: st.org, once_with: Some(name.clone()), ..Record::default() });
            if !st.parts.iter().any(|p| matches!(p.state, PartState::Deferred(_))) {
                st.phase = if st.started.is_some() { Phase::Answered } else { Phase::Once };
            }
        }
    }

    /// The stages from `from` on are as they were before the ticket came to them: a stage that came
    /// again there is taken off, and the parts before `from` that an approval given there did wait
    /// for it again. So do the ones done at the stage `reopened` (the one an approval is retracted
    /// at), which is open again: they are done when it has every approval it needs again.
    pub(crate) fn unwind(&self, t: &mut Ticket, from: usize, reopened: Option<usize>) {
        let mut undone: Vec<String> = t.stages[from..].iter().map(|s| s.name.clone()).collect();
        undone.extend(reopened.map(|j| t.stages[j].name.clone()));
        let mut k = from;
        while k < t.stages.len() {
            if t.stages[..k].iter().any(|s| s.name == t.stages[k].name) {
                t.stages.remove(k);
            } else {
                t.stages[k].reset();
                k += 1;
            }
        }
        if listing::once(self.s.file) != Once::Last {
            return;
        }
        for st in t.stages[..from].iter_mut() {
            let mut waits = false;
            for p in st.parts.iter_mut() {
                if matches!(&p.state, PartState::Once(to) if undone.contains(to)) {
                    let PartState::Once(to) = std::mem::replace(&mut p.state, PartState::Open) else { unreachable!() };
                    p.state = PartState::Deferred(to);
                    waits = true;
                }
            }
            if waits && matches!(st.phase, Phase::Answered | Phase::Once) {
                st.phase = Phase::Deferred;
            }
        }
    }

    /// The ticket moves on from the stage `from` (DESIGN 7.3): the first stage at or after it that
    /// waits for someone is started, the ones before it that nobody answers, that circulate, and
    /// that another stage's approval does are passed, each with its line of the history; with none
    /// left, the ticket is approved. `again` starts `from` as a stage it was sent back to, which no
    /// other approval does. With `same approver approves once, at the last stage`, a stage whose
    /// parts waited and no longer have their later stage comes again first ([`Machine::settle`]).
    pub(crate) fn advance(&self, t: &mut Ticket, from: usize, again: bool, part: &OrgPart, now: i64, fx: &mut Effects) -> Result<(), Refusal> {
        let rq = Self::requester(t);
        let sv = self.solver(&rq, part);
        let once = listing::once(self.s.file);
        let org = Some(part.v().from);
        let round = t.round;
        t.status = Status::InProgress;
        if let Some(st) = t.stages.get_mut(from).filter(|_| again) {
            st.again = true;
        }
        let mut next = from;
        loop {
            let i = next;
            if once == Once::Last {
                match self.settle(&sv, t, i, part, now, fx)? {
                    Settled::Started => return Ok(()),
                    Settled::Passed => {
                        next = i + 1;
                        continue;
                    }
                    Settled::Clear => {}
                }
            }
            if i >= t.stages.len() {
                break;
            }
            next = i + 1;
            let o = self.occ_of(&t.stages[i])?;
            let name = t.stages[i].name.clone();
            let sent_back_to = t.stages[i].again;
            t.stages[i].reset();
            t.stages[i].again = sent_back_to;
            t.stages[i].org = org;
            match o.spec {
                Spec::Circulate(w) => {
                    let to = match self.solve(&sv, o.file, &w.text, t, part) {
                        Solved::People(ps, _) => ps,
                        Solved::Nobody { .. } => Vec::new(),
                    };
                    fx.record(round, Record { did: "circulated", stage: Some(name.clone()), org, to: to.clone(), ..Record::default() });
                    if !to.is_empty() {
                        fx.put(round, What::Notify { to, notice: Notice::Circulation, stage: Some(name) });
                    }
                    t.stages[i].phase = Phase::Circulated;
                    continue;
                }
                Spec::Note(flow) => {
                    self.usable_on(part.day)?;
                    let st = &mut t.stages[i];
                    // a summary names no way to choose: its one part waits for the flow
                    st.parts = vec![Part { way: String::new(), key: String::new(), approvers: Vec::new(), team: None, needed: 0, answers: Vec::new(), state: PartState::Agent(flow.text.clone()), sole: Sole::Other }];
                    (st.phase, st.started) = (Phase::Open, Some(now));
                    t.at = Some(i);
                    fx.record(round, Record { did: "note asked", stage: Some(name.clone()), note: Some(flow.text.clone()), ..Record::default() });
                    fx.put(round, What::AskNote { flow: flow.text.clone(), stage: name, args: self.flow_args(t) });
                    return self.set_clocks(t, i, true, now, fx);
                }
                Spec::Empty | Spec::Route(_) => {
                    t.stages[i].phase = Phase::Skipped;
                    continue;
                }
                _ => {}
            }
            let mut parts = Vec::new();
            for (way, q, f) in self.ways(o, &t.stages[i]) {
                parts.push(self.part_of(&sv, o, &t.stages[i], &way, q, f, t, part, true, fx));
            }
            // `same approver approves once`: a part one person alone answers, whom the stage right
            // before had as its one approver too. The approval is the one given in their seat at
            // the stage they approved, by them or by a proxy of theirs (DESIGN 5.8)
            if once == Once::Before && !t.stages[i].again {
                let seen = Self::seen(t, i);
                let conds = vec![false; i];
                for p in parts.iter_mut() {
                    let (PartState::Open, Sole::By(x)) = (&p.state, &p.sole) else { continue };
                    if let Before::Sure(k) = listing::before_on(&conds, &seen, i, x) {
                        let origin = &t.stages[listing::origin_on(&conds, &seen, k, x)];
                        let by = origin.parts.iter().flat_map(|q| q.answers.iter()).find(|a| a.yes && a.seat == *x).map_or_else(|| x.clone(), |a| a.by.clone());
                        let with = origin.name.clone();
                        fx.record(round, Record { did: "approved", stage: Some(name.clone()), proxy_of: (by != *x).then(|| x.clone()), by: Some(by), org, once_with: Some(with.clone()), ..Record::default() });
                        p.state = PartState::Once(with);
                    }
                }
            }
            if once == Once::Last {
                // `, at the last stage`: a part one person alone answers, who alone answers a part
                // of a stage ahead too, waits for the last of them. Not at a stage the ticket was
                // sent back to, which is answered there
                for p in parts.iter_mut().filter(|_| !sent_back_to) {
                    let (PartState::Open, Sole::By(x)) = (&p.state, &p.sole) else { continue };
                    if let Some(k) = self.last_of(&sv, t, i + 1, x, part) {
                        let to = t.stages[k].name.clone();
                        fx.record(round, Record { did: "deferred", stage: Some(name.clone()), org, once_with: Some(to.clone()), to: vec![x.clone()], ..Record::default() });
                        p.state = PartState::Deferred(to);
                    }
                }
                // the parts that waited for this stage wait on with it, where it waits too
                for (a, k) in t.deferred(i) {
                    let Some((x, to)) = t.stages[a].parts[k].deferred().map(|(x, to)| (x.to_string(), to.to_string())) else { continue };
                    let Some(on) = parts.iter().find_map(|p| p.deferred().filter(|(y, _)| *y == x && to == name).map(|(_, on)| on.to_string())) else { continue };
                    let st = &mut t.stages[a];
                    fx.record(round, Record { did: "deferred", stage: Some(st.name.clone()), org: st.org, once_with: Some(on.clone()), to: vec![x], note: Some("the stage it waited for waits too".to_string()), ..Record::default() });
                    st.parts[k].state = PartState::Deferred(on);
                }
            }
            let st = &mut t.stages[i];
            st.parts = parts;
            if st.parts.iter().all(|p| p.done()) {
                let has = |f: fn(&PartState) -> bool| st.parts.iter().any(|p| f(&p.state));
                st.phase = if has(|s| matches!(s, PartState::Deferred(_))) {
                    Phase::Deferred
                } else if has(|s| matches!(s, PartState::Once(_))) {
                    Phase::Once
                } else {
                    Phase::Skipped
                };
                continue;
            }
            // it waits for someone
            return self.wait_at(t, i, part, now, fx);
        }
        // no stage is left
        t.at = None;
        t.status = Status::Approved;
        t.ended = Some(super::state::Ended { at: now, stage: None, by: None });
        fx.record(round, Record { did: "ticket approved", ..Record::default() });
        self.on(Ending::Approved, t, part, fx)?;
        fx.put(round, What::Notify { to: Self::theirs(t), notice: Notice::Decided, stage: None });
        Ok(())
    }
}
