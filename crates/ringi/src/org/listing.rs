//! Every version of the organization, every requester, every stage of a request, and what is said
//! of them (DESIGN 5.4, 8.6): a stage with nobody to answer it that waits (E502: `when empty
//! stay`, and `escalate` where nobody is above, unless it says `then skip` or `then <way>`), goes
//! up, elsewhere or is skipped for a hole in the data (W502), a person or a team the version does
//! not have where the stage would go up from them (E503), a quorum a requester's team cannot reach
//! (E504), one person who answers two stages of one ticket (W506), a late stage with nobody above
//! to go up to for a hole in the data (W507), a request no stage of which anyone answers for a
//! requester (W510); and, with no data, that none of it is decided (W500).
//!
//! A hole is what the data can fill: a team whose head is empty, a team with no member, a person
//! or a team the version does not have, and a team with an empty head that the stage passes over
//! going up ([`Hole`]). Where there is none, the shape of the organization sends the stage
//! elsewhere as the request is written (the way comes to the requester alone, nobody is above
//! them, no team of the kind is above them): that is not a warning, and `ok` says it under the
//! line, as how many requesters it comes to in each version, with every case in the JSON's
//! `facts` ([`facts_of`]).
//!
//! The requesters are every person of the version, and each one a submit proxy makes requests for
//! (with the proxy left out too, DESIGN 5.5). The route is read as its paths: each row of a table,
//! or every stage line together (their conditions are not read: two stages that may not both come
//! are compared as if they did). A finding is said once for each version and each cause, at the
//! first line it comes from, with how many requesters it comes to and three of them.
//!
//! An approver an approve proxy adds (DESIGN 5.8) answers beside the one the way chose, when the
//! stage is answered: it does not fill a stage with nobody, and is not counted here.
//!
//! With `same approver approves once` (DESIGN 5.12), the later of two stages one person answers is
//! done with the approval of the stage right before it, when that person alone approved it: W506
//! does not say the two, and says why where it does not do them (a stage between them that others
//! answer, another part of the earlier stage, an agent's stage). Without it, W506 says how to write
//! it where it would do them. `simulate` reads the stages it does the same way ([`before`]).
//!
//! With `submitted by` (DESIGN 5.13), the requesters are the ones who may submit the request
//! ([`super::submit`]); a team or a person it names that a version does not have is E508, a version
//! no one may submit in E509, and `ok` says how many may submit in each version.

use super::resolve::{Case, Found, Requester, Solver, Up, Why, rule_ways};
use super::{Data, Org, ProxyKind, Version};
use crate::ast::*;
use crate::diag::Diag;
use crate::names::Scope;
use ritsu_base::json::Json;
use crate::check::Fact;
use ritsu_base::text::{Lang, Text};
use std::collections::{BTreeMap, BTreeSet};

/// How many requesters a finding names.
const SHOWN: usize = 3;

/// A stage as a path of the route has it.
pub struct Occ<'f> {
    pub stage: String,
    pub spec: &'f Spec,
    /// Where it is said: the row of the table, the stage line, the request's `route` line for a
    /// shared route's stage.
    pub line: usize,
    pub col: usize,
    /// The file its ways to choose are declared in: the request, or the shared route.
    pub file: &'f File,
    pub empty: WhenEmpty,
    /// The file the `when empty` line is written in: the way it goes to where nobody is above
    /// (`then board`) is declared in its `who`.
    pub empty_in: &'f File,
    /// What it does when it is late (`when late …`), its own or `every stage`'s, with the file the
    /// line is written in (where a table it escalates by is).
    pub late: Option<LateLine<'f>>,
    /// Its deadline (`due in …`), its own or `every stage`'s: None for a stage that is never late.
    pub due: Option<DueLine<'f>>,
    /// Whether a condition decides if it comes (a stage line's `when`): the conditions are not
    /// read, so such a stage may come or not.
    pub cond: bool,
}

/// A `when late` line: what is done first, what then, and the file it is written in.
#[derive(Clone, Copy)]
pub struct LateLine<'f> {
    pub first: &'f Late,
    pub then: Option<&'f Late>,
    pub file: &'f File,
    pub line: usize,
}

/// A `due in` line: the business days, the calendar they are counted in where one is named, the
/// time of the day, and where it is written.
#[derive(Clone, Copy)]
pub struct DueLine<'f> {
    pub days: u32,
    pub calendar: Option<&'f Word>,
    pub at: At,
    pub file: &'f File,
    pub line: usize,
}

/// The deadline a stage keeps: its own, else `every stage`'s.
fn due_line<'f>(own: Option<(&'f Deadlines, &'f File)>, every: Option<(&'f Deadlines, &'f File)>) -> Option<DueLine<'f>> {
    let of = |d: Option<(&'f Deadlines, &'f File)>| {
        d.and_then(|(d, file)| {
            d.lines.iter().find_map(|l| match l {
                Deadline::Due { days, calendar, at, line } => Some(DueLine { days: *days, calendar: calendar.as_ref(), at: *at, file, line: *line }),
                _ => None,
            })
        })
    };
    of(own).or_else(|| of(every))
}

/// The `when empty` line a stage keeps, its own or `every stage`'s, with the file it is written in;
/// `when empty stay` where none is written.
fn when_empty<'f>(own: Option<(&'f Deadlines, &'f File)>, every: Option<(&'f Deadlines, &'f File)>, f: &'f File) -> (WhenEmpty, &'f File) {
    let of = |d: Option<(&'f Deadlines, &'f File)>| {
        d.and_then(|(d, file)| {
            d.lines.iter().find_map(|l| match l {
                Deadline::Empty { what, .. } => Some((what.clone(), file)),
                _ => None,
            })
        })
    };
    of(own).or_else(|| of(every)).unwrap_or((WhenEmpty::Stay, f))
}

/// The `when late` line a stage keeps: its own, else `every stage`'s, each with its file; None for
/// a stage with no deadline, which is never late.
fn when_late<'f>(own: Option<(&'f Deadlines, &'f File)>, every: Option<(&'f Deadlines, &'f File)>) -> Option<LateLine<'f>> {
    let of = |d: Option<(&'f Deadlines, &'f File)>| {
        d.and_then(|(d, file)| {
            d.lines.iter().find_map(|l| match l {
                Deadline::Late { first, then, line } => Some(LateLine { first, then: then.as_ref(), file, line: *line }),
                _ => None,
            })
        })
    };
    let due = |d: Option<(&'f Deadlines, &'f File)>| d.is_some_and(|(d, _)| d.lines.iter().any(|l| matches!(l, Deadline::Due { .. })));
    if !due(own) && !due(every) {
        return None;
    }
    of(own).or_else(|| of(every))
}

/// The paths of the route of `f`: each row of a table, or every stage line together. `every` is
/// the `every stage` that holds where a stage writes nothing of its own; `at` puts every stage at
/// a line of the request (a shared route's, at the request's `route` line or row).
fn file_paths<'f>(s: &'f Scope, f: &'f File, every: Option<(&'f Deadlines, &'f File)>, at: Option<(usize, usize)>) -> Vec<Vec<Occ<'f>>> {
    let place = |line: usize, col: usize| at.unwrap_or((line, col));
    match &f.route {
        None => Vec::new(),
        Some(Route::Stages(lines)) => vec![
            lines
                .iter()
                .map(|l| {
                    let (line, col) = place(l.line, l.spec_span.col);
                    let late = when_late(l.deadlines.as_ref().map(|d| (d, f)), every);
                    let (empty, empty_in) = when_empty(l.deadlines.as_ref().map(|d| (d, f)), every, f);
                    Occ { stage: l.name.text.clone(), spec: &l.spec, line, col, file: f, empty, empty_in, late, due: due_line(l.deadlines.as_ref().map(|d| (d, f)), every), cond: l.when.is_some() }
                })
                .collect(),
        ],
        Some(Route::Table(t)) if !t.chooses_route => t
            .rows
            .iter()
            .map(|r| {
                r.stages
                    .iter()
                    .enumerate()
                    .filter_map(|(i, (spec, span))| {
                        let st = t.stages.get(i)?;
                        let (line, col) = place(r.line, span.col);
                        let (empty, empty_in) = when_empty(None, every, f);
                        Some(Occ { stage: st.text.clone(), spec, line, col, file: f, empty, empty_in, late: when_late(None, every), due: due_line(None, every), cond: false })
                    })
                    .collect()
            })
            .collect(),
        Some(Route::Table(t)) => {
            let mut out = Vec::new();
            for r in &t.rows {
                for (spec, span) in &r.stages {
                    let Spec::Route(w) = spec else { continue };
                    let Some(shared) = s.ringi(&w.text, UseKind::Route) else { continue };
                    let ev = every.or(shared.every_stage.as_ref().map(|d| (d, shared)));
                    out.extend(file_paths(s, shared, ev, Some(at.unwrap_or((r.line, span.col)))));
                }
            }
            out
        }
        Some(Route::Shared { alias, line, .. }) => match s.ringi(&alias.text, UseKind::Route) {
            Some(shared) => {
                let ev = every.or(shared.every_stage.as_ref().map(|d| (d, shared)));
                file_paths(s, shared, ev, Some(at.unwrap_or((*line, alias.span.col))))
            }
            None => Vec::new(),
        },
    }
}

/// The paths of the request's route.
pub fn paths<'f>(s: &'f Scope) -> Vec<Vec<Occ<'f>>> {
    file_paths(s, s.file, s.file.every_stage.as_ref().map(|d| (d, s.file)), None)
}

/// Whether self approval is allowed (`self approval allowed`); not, unless the ticket says so.
pub fn self_approval(f: &File) -> bool {
    f.ticket.as_ref().is_some_and(|t| t.lines.iter().any(|l| matches!(l, TicketLine::SelfApproval { allowed: true, .. })))
}

/// What a ticket does with one person who alone answers two or more of its stages (DESIGN 5.12).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Once {
    /// Nothing is written: they answer each stage.
    No,
    /// `same approver approves once`: a stage is done with the approval of the stage right before
    /// it.
    Before,
    /// `same approver approves once, at the last stage`: the earlier stages wait, and are done with
    /// the approval of the last.
    Last,
}

/// What the ticket says of `same approver approves once` (DESIGN 5.12).
pub fn once(f: &File) -> Once {
    match f.ticket.as_ref().and_then(|t| t.lines.iter().find_map(|l| if let TicketLine::SameApprover { last, .. } = l { Some(*last) } else { None })) {
        None => Once::No,
        Some(false) => Once::Before,
        Some(true) => Once::Last,
    }
}

/// What one case of a part of a stage comes to, as `same approver approves once` reads it (DESIGN
/// 5.12): passed over, answered by one person alone, or by others.
#[derive(Clone, Debug, PartialEq)]
pub enum Out {
    /// Nobody approves it: the stage is skipped for lack of an approver.
    Pass,
    /// This one person alone answers it: the one the way comes to, or where the stage goes up to.
    By(String),
    /// Others answer it, or may: a team of several, a quorum of two or more, an agent, a person
    /// chosen when the ticket runs, a stage that waits for an administrator.
    Other,
}

/// What the case `c` of a part of the stage `o` comes to, for the requester of `sv`.
pub fn out_of(sv: &Solver, o: &Occ, quorum: Option<Quorum>, c: &Case) -> Out {
    if c.agent {
        return Out::Other;
    }
    match &c.found {
        Found::Open => Out::Other,
        Found::People(ps) => {
            let n = quorum.map_or(1, |q| needed(q, ps.len()));
            if ps.len() == 1 && n == 1 { Out::By(ps[0].clone()) } else { Out::Other }
        }
        Found::Nobody(why, up) => match &o.empty {
            WhenEmpty::Stay => Out::Other,
            WhenEmpty::Skip => Out::Pass,
            WhenEmpty::Escalate(_) if *up == Up::Cannot && matches!(why, Why::PersonNotIn(_) | Why::TeamNotIn(_)) => Out::Other,
            WhenEmpty::Escalate(top) => match (sv.up(up), top) {
                (Some(h), _) => Out::By(h),
                (None, Some(AtTop::Skip)) => Out::Pass,
                (None, Some(AtTop::To(w))) => match sv.way(o.empty_in, &w.text).as_slice() {
                    [Case { found: Found::People(ps), agent: false, .. }] if ps.len() == 1 => Out::By(ps[0].clone()),
                    _ => Out::Other,
                },
                (None, _) => Out::Other,
            },
        },
    }
}

/// A stage of a path, as `same approver approves once` reads it for a requester.
pub enum Seen {
    /// Nobody approves it: an empty cell, a circulation.
    Pass,
    /// A summary, whose notes come onto the cards after it; a stage the check cannot solve.
    Other,
    /// Each part's way, as the stage names it, with what each of its cases comes to. On a path the
    /// check or `simulate` reads, where whether the stage was done with the approval of a stage
    /// before it is not told: it may have been, so [`origin_on`] goes back through it.
    Parts(Vec<(String, Vec<Out>)>),
    /// The same of a ticket's stage that was answered directly: not done with the approval of a
    /// stage before it, so [`origin_on`] does not go back past it (DESIGN 5.12).
    Answered(Vec<(String, Vec<Out>)>),
    /// The same of a ticket's stage `same approver approves once` itself did: the approval it was
    /// done with is of a stage before it, which [`origin_on`] goes back to.
    Once(Vec<(String, Vec<Out>)>),
}

pub fn seen_of(sv: &Solver, o: &Occ, cases: &[(String, Option<Quorum>, Vec<Case>)]) -> Seen {
    match o.spec {
        Spec::Empty | Spec::Circulate(_) => Seen::Pass,
        Spec::Note(_) | Spec::Route(_) => Seen::Other,
        _ if cases.is_empty() => Seen::Other,
        _ => Seen::Parts(cases.iter().map(|(w, q, cs)| (w.clone(), if cs.is_empty() { vec![Out::Other] } else { cs.iter().map(|c| out_of(sv, o, *q, c)).collect() })).collect()),
    }
}

/// Where `same approver approves once` does a stage that one person alone answers, on a path whose
/// conditions are not read (`simulate` without the form): the stage whose approval does it, sure
/// when every stage between is passed over and it comes, or only where no stage with a condition
/// comes between them (and it comes, and is not skipped).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Before {
    Sure(usize),
    Maybe(usize),
    Not,
}

/// Where `same approver approves once` does the stage `ib` of the path, which `x` alone answers.
pub fn before(path: &[Occ], seen: &[Seen], ib: usize, x: &str) -> Before {
    let conds: Vec<bool> = path.iter().map(|o| o.cond).collect();
    before_on(&conds, seen, ib, x)
}

/// [`before`], over the stages as they are seen and whether a condition decides if each comes:
/// the one decision of `same approver approves once`, which the check, `simulate` and the ticket
/// machine all make here (the machine with the stages a ticket came to, none of them in doubt).
pub fn before_on(conds: &[bool], seen: &[Seen], ib: usize, x: &str) -> Before {
    let mut sure = true;
    let mut k = ib;
    while k > 0 {
        k -= 1;
        let parts = match &seen[k] {
            Seen::Pass => continue,
            Seen::Other if conds[k] => {
                sure = false;
                continue;
            }
            Seen::Other => return Before::Not,
            Seen::Parts(parts) | Seen::Answered(parts) | Seen::Once(parts) => parts,
        };
        let outs = || parts.iter().flat_map(|(_, os)| os.iter());
        if outs().all(|o| *o == Out::Pass) {
            continue;
        }
        if outs().any(|o| !matches!(o, Out::Pass) && !matches!(o, Out::By(p) if p == x)) {
            if conds[k] {
                sure = false;
                continue;
            }
            return Before::Not;
        }
        let skipped = parts.iter().all(|(_, os)| os.contains(&Out::Pass));
        return if sure && !conds[k] && !skipped { Before::Sure(k) } else { Before::Maybe(k) };
    }
    Before::Not
}

/// The stage `x` approves that `same approver approves once` does the stage `k` with, where it is
/// sure to: `k` itself, or the one before it whose approval does it.
pub fn origin(path: &[Occ], seen: &[Seen], k: usize, x: &str) -> usize {
    let conds: Vec<bool> = path.iter().map(|o| o.cond).collect();
    origin_on(&conds, seen, k, x)
}

/// [`origin`], over the stages as they are seen: a stage `same approver approves once` itself did
/// is gone back past; one that was answered is not (DESIGN 5.12).
pub fn origin_on(conds: &[bool], seen: &[Seen], k: usize, x: &str) -> usize {
    match &seen[k] {
        Seen::Parts(_) | Seen::Once(_) => match before_on(conds, seen, k, x) {
            Before::Sure(j) => origin_on(conds, seen, j, x),
            _ => k,
        },
        _ => k,
    }
}

/// Where `same approver approves once, at the last stage` makes the stage `ia` of the path wait,
/// a part of which `x` alone answers: the last stage after it that `x` alone answers a part of. On a
/// path whose conditions are not read, the stages it can be, in the order of the route: from the
/// last one that may come to `x` back to the first that is sure to (it comes whatever the form, no
/// condition deciding it, and a part of it comes to `x` in every case), and whether the last of
/// them is sure, which leaves it alone. None where no stage after `ia` comes to `x` alone.
pub fn later(path: &[Occ], seen: &[Seen], ia: usize, x: &str) -> (Vec<usize>, bool) {
    let by = |o: &Out| matches!(o, Out::By(p) if p == x);
    let mut among = Vec::new();
    for k in (ia + 1..path.len()).rev() {
        let (Seen::Parts(parts) | Seen::Answered(parts) | Seen::Once(parts)) = &seen[k] else { continue };
        if !parts.iter().any(|(_, os)| os.iter().any(by)) {
            continue;
        }
        among.insert(0, k);
        if !path[k].cond && parts.iter().any(|(_, os)| os.iter().all(by)) {
            return (among, true);
        }
    }
    (among, false)
}

/// Why `same approver approves once` does not do the later of two stages one person answers.
enum Break {
    /// A stage between them that others answer, or may.
    Between(usize),
    /// Another part of the earlier stage, which others answer or may (`dept_head and accounting`).
    Parts(String),
    /// An agent answers the later stage first.
    Agent,
}

/// Whether `same approver approves once` does the part `jb` (in its case `kb`) of the stage `ib` of
/// the path, which `x` alone answers, where the stage `ia` comes with its part `ja` in the case `ka`,
/// which `x` alone answers too (DESIGN 5.12): from the stage before `ib` back, every stage is passed
/// over (it does not come, circulates, or is skipped for lack of an approver) until one that `x`
/// alone approves, every part of it; that stage is the one it is done with. A stage line with a
/// condition may not come; `ia` comes.
fn carried(path: &[Occ], seen: &[Seen], cases: &[Vec<(String, Option<Quorum>, Vec<Case>)>], (ia, ja, ka): (usize, usize, usize), (ib, jb, kb): (usize, usize, usize), x: &str) -> Result<usize, Break> {
    if cases[ib][jb].2[kb].agent {
        return Err(Break::Agent);
    }
    let mut k = ib;
    while k > 0 {
        k -= 1;
        let parts = match &seen[k] {
            Seen::Pass => continue,
            Seen::Other => return Err(Break::Between(k)),
            Seen::Parts(parts) | Seen::Answered(parts) | Seen::Once(parts) => parts,
        };
        let mut skipped = true;
        for (j, (w, outs)) in parts.iter().enumerate() {
            let outs: &[Out] = if k == ia && j == ja { &outs[ka..=ka] } else { outs };
            if outs.iter().any(|o| !matches!(o, Out::Pass) && !matches!(o, Out::By(p) if p == x)) {
                return Err(if k == ia { Break::Parts(w.clone()) } else { Break::Between(k) });
            }
            skipped &= outs.contains(&Out::Pass);
        }
        if k == ia || !(skipped || path[k].cond) {
            return Ok(k);
        }
    }
    Err(Break::Between(ia))
}

/// The requesters of a version: every person, and each person a submit proxy in effect on some day
/// of the version makes requests for, with the proxy.
pub fn requesters(org: &Org, v: &Version, request: &File) -> Vec<Requester> {
    let mut out: Vec<Requester> = v.people.iter().map(|p| Requester { id: p.id.clone(), by: None }).collect();
    let (a, b) = org.period(v);
    for x in &org.proxies {
        if x.kind != ProxyKind::Submit || !x.meets(a, b) || !(x.covers(&request.name.text) || x.covers(request.name.ascii())) {
            continue;
        }
        if v.person(&x.person).is_none() || v.person(&x.proxy).is_none() {
            continue;
        }
        let r = Requester { id: x.person.clone(), by: Some(x.proxy.clone()) };
        if !out.contains(&r) {
            out.push(r);
        }
    }
    out
}

/// The ways a stage names, each with its cases, for one requester: the parts of `a and b`, the team
/// of a quorum (with the quorum), the ways a rule's output can name (each a case of its own).
pub fn stage_cases(sv: &Solver, o: &Occ) -> Vec<(String, Option<Quorum>, Vec<Case>)> {
    match o.spec {
        Spec::Who(ws) => ws.iter().map(|w| (w.text.clone(), None, sv.way(o.file, &w.text))).collect(),
        Spec::Quorum(q, w, _) => vec![(w.text.clone(), Some(*q), sv.way(o.file, &w.text))],
        Spec::RuleOutput(p) => {
            let out_name = crate::ast::dotted(p);
            let mut cases = Vec::new();
            for way in rule_ways(sv.s, p) {
                let f = if Solver::who(o.file, &way).is_some() { o.file } else { sv.s.file };
                for mut c in sv.way(f, &way) {
                    let label = match &c.label {
                        Some(l) => tr!("`{out_name}` が `{way}`、{}", "`{out_name}` is `{way}`, {}", l.ja; l.en),
                        None => tr!("`{out_name}` が `{way}`", "`{out_name}` is `{way}`"),
                    };
                    c.label = Some(label);
                    cases.push(c);
                }
            }
            vec![(out_name, None, cases)]
        }
        _ => Vec::new(),
    }
}

/// What in the data makes a stage go elsewhere or be skipped for lack of an approver, or leaves a
/// late stage with nowhere to go up to (DESIGN 5.4, 8.6): what the data can fill. Where there is
/// none, the shape of the organization does it, as the request is written.
#[derive(Clone, Debug, PartialEq)]
pub enum Hole {
    /// The way finds nobody for lack of data: a team whose head is empty, a team with no member, a
    /// person or a team the version does not have (its cause says which).
    Way,
    /// The head of this team is empty, and it is passed over.
    Empty(String),
}

impl Hole {
    /// What tells a finding with this hole from one without.
    fn key(hole: &Option<Hole>) -> String {
        match hole {
            Some(Hole::Empty(t)) => format!(" empty {t}"),
            _ => String::new(),
        }
    }

    /// What a finding says of it, where its cause does not: the team whose head is empty.
    fn note(v: &Version, hole: &Option<Hole>) -> Option<Text> {
        let Some(Hole::Empty(t)) = hole else { return None };
        let t = v.team_named(t);
        Some(tr!("チーム {}の長が空いています。", "The head of the team {} is empty.", super::ja_word(&t.ja); t.en))
    }

    /// What a stage says of nobody above it to go up to: the top of the organization where nothing
    /// is missing; where the data has a gap, only that nobody is above, for the gap is the cause.
    fn none_above(hole: &Option<Hole>) -> Text {
        match hole {
            None => tr!("上に回せる人がいない（組織のいちばん上）", "nobody is above (the top of the organization)"),
            Some(_) => tr!("上に回せる人がいない", "nobody is above"),
        }
    }
}

/// The hole of a way that finds nobody, by its cause; for a way that walks up from the requester
/// and finds no head (`manager of requester`, `nearest …`), a team with an empty head on their way
/// up.
fn way_hole(sv: &Solver, why: &Why) -> Option<Hole> {
    match why {
        Why::PersonNotIn(_) | Why::TeamNotIn(_) | Why::HeadEmpty(_) | Why::NoMembers(_) => Some(Hole::Way),
        Why::NoneAbove => empty_above(sv, &sv.requester.id),
        Why::OnlyRequester(_) | Why::NoTeamOfKind(_) => None,
    }
}

/// A team with an empty head from `p`'s primary team up.
fn empty_above(sv: &Solver, p: &str) -> Option<Hole> {
    let team = sv.v.primary(p)?.team.clone();
    sv.v.up(&team).into_iter().find(|t| t.head.is_none()).map(|t| Hole::Empty(t.id.clone()))
}

/// A team with an empty head that a stage going up from `from` passes over: before the head it goes
/// up to, or anywhere above where it finds nobody ([`Solver::up`]).
fn passed(sv: &Solver, from: &Up) -> Option<Hole> {
    let chain = match from {
        Up::Cannot => return None,
        Up::At(t) => sv.v.up(t),
        Up::Above(t) => sv.v.up(t).into_iter().skip(1).collect(),
    };
    for t in chain {
        match &t.head {
            None => return Some(Hole::Empty(t.id.clone())),
            Some(h) if !sv.left_out(h) => return None,
            Some(_) => {}
        }
    }
    None
}

/// The hole of a stage whose way finds nobody and that goes up when it is empty: the way's own, or
/// one it passes over going up.
fn hole(sv: &Solver, why: &Why, up: &Up) -> Option<Hole> {
    way_hole(sv, why).or_else(|| passed(sv, up))
}

/// The number of approvals a quorum needs of `n` people.
pub fn needed(q: Quorum, n: usize) -> usize {
    match q {
        Quorum::All => n,
        Quorum::AtLeast(k) => k as usize,
        Quorum::Percent(p) => (n * p as usize).div_ceil(100),
    }
}

/// What a finding is said for: its version, its code, the stage, the way, and its cause.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    version: i64,
    code: &'static str,
    stage: String,
    way: String,
    cause: String,
}

/// A finding, gathered over the requesters.
struct Found2 {
    line: usize,
    col: usize,
    lines: BTreeSet<usize>,
    cause: Text,
    /// Each requester, with what came to them (where the stage went, who answers).
    who: Vec<(Text, Option<Text>)>,
    /// The same requesters by their IDs, each with the ID of the person it came to (where the stage
    /// went up to, who answers both), for the JSON's `witness`.
    ids: Vec<(Requester, Option<String>)>,
    seen: BTreeSet<Requester>,
    extra: Vec<Text>,
    /// When it was first found: the findings of a stage are said in this order, the same whatever
    /// the language of the names (the versions, the requesters and the values of a rule's output
    /// in their order).
    seq: usize,
}

struct Gather {
    found: BTreeMap<Key, Found2>,
    /// What the shape of the organization does as the request is written (a W502 or a W507 with no
    /// hole in the data): said under `ok` ([`facts_of`]).
    shaped: BTreeMap<Key, Found2>,
    /// How many requesters each version has (a request a proxy makes for someone is one of its own),
    /// and how many people they are.
    all: BTreeMap<i64, usize>,
    people: BTreeMap<i64, usize>,
}

impl Gather {
    #[allow(clippy::too_many_arguments)]
    /// `r` (named `named`) comes to the finding `key`; `to` is the person it came to, by their ID,
    /// when the finding says one (`v.who` names them).
    fn add(&mut self, v: &Version, key: Key, line: usize, col: usize, cause: Text, r: &Requester, named: Text, to: Option<&str>, extra: Vec<Text>) {
        self.put(true, v, key, line, col, cause, r, named, to, extra);
    }

    /// The same, as a finding (`warn`) or as what the shape of the organization does.
    #[allow(clippy::too_many_arguments)]
    fn put(&mut self, warn: bool, v: &Version, key: Key, line: usize, col: usize, cause: Text, r: &Requester, named: Text, to: Option<&str>, extra: Vec<Text>) {
        let seq = self.found.len() + self.shaped.len();
        let map = if warn { &mut self.found } else { &mut self.shaped };
        let g = map.entry(key).or_insert_with(|| Found2 { line, col, lines: BTreeSet::new(), cause, who: Vec::new(), ids: Vec::new(), seen: BTreeSet::new(), extra, seq });
        g.lines.insert(line);
        if (line, col) < (g.line, g.col) {
            g.line = line;
            g.col = col;
        }
        if g.seen.insert(r.clone()) {
            g.who.push((named, to.map(|t| v.who(t))));
            g.ids.push((r.clone(), to.map(str::to_string)));
        }
    }
}

/// Where a cause that a finding gathers over the versions names the versions: what [`merged`]
/// writes in its place, once it knows every version the finding is found in.
const VERSIONS: &str = "\u{1}";

/// [`why_text`] for a finding gathered over the versions: a person or a team the version does not
/// have is said without the day of the version, which [`merged`] writes in its place, so that the
/// same ID missing from several versions is one finding that names them all (DESIGN 8.6).
fn why_apart(v: &Version, way: &str, why: &Why) -> Text {
    match why {
        Why::PersonNotIn(id) => tr!("`{way}` が指す {id} は、{VERSIONS}にいません", "`{way}` names {id}, who is not in {VERSIONS}"),
        Why::TeamNotIn(id) => tr!("`{way}` が指すチーム `{id}` は、{VERSIONS}にありません", "`{way}` names the team `{id}`, which is not in {VERSIONS}"),
        _ => why_text(v, way, why),
    }
}

/// A text of [`why_apart`] with the versions it is found in written in: `the organization from
/// 2027-04-01`, `the versions of the organization from 2026-04-01 and 2027-04-01`.
fn versions_in(t: &Text, days: &[String]) -> Text {
    if !t.en.contains(VERSIONS) && !t.ja.contains(VERSIONS) {
        return t.clone();
    }
    let (ja, en) = match days {
        [one] => (format!("{one} からの組織"), format!("the organization from {one}")),
        _ => {
            let ja: Vec<String> = days.iter().map(|d| format!("{d} から")).collect();
            let ja = if ja.len() == 2 { format!("{}と {}", ja[0], ja[1]) } else { ja.join("、") };
            (format!("{ja}の組織"), format!("the versions of the organization from {} and {}", days[..days.len() - 1].join(", "), days[days.len() - 1]))
        }
    };
    Text::new(t.ja.replace(VERSIONS, &ja), t.en.replace(VERSIONS, &en))
}

fn why_text(v: &Version, way: &str, why: &Why) -> Text {
    let day = v.day();
    match why {
        Why::PersonNotIn(id) => tr!("`{way}` が指す {id} は、{day} からの組織にいません", "`{way}` names {id}, who is not in the organization from {day}"),
        Why::TeamNotIn(id) => tr!("`{way}` が指すチーム `{id}` は、{day} からの組織にありません", "`{way}` names the team `{id}`, which is not in the organization from {day}"),
        Why::HeadEmpty(t) => {
            let t = v.team_named(t);
            tr!("`{way}` はチーム {}に当たり、その長が空いています", "`{way}` comes to the team {}, whose head is empty", super::ja_word(&t.ja); t.en)
        }
        // the requesters it comes to are listed after it, by where the stage goes
        Why::OnlyRequester(_) => tr!("`{way}` が当たるのは申請者本人だけで、自己承認は許していません", "`{way}` comes only to the requester, and self approval is not allowed"),
        Why::NoTeamOfKind(k) => tr!("`{way}` の種類 `{k}` のチームが、申請者の上にありません", "`{way}` finds no team of the kind `{k}` above the requester"),
        Why::NoneAbove => tr!("申請者の上に、`{way}` に当たる長がいません", "`{way}` finds no head above the requester"),
        Why::NoMembers(t) => {
            let t = v.team_named(t);
            tr!("`{way}` のチーム {}に、人がいません", "the team {} of `{way}` has no member", super::ja_word(&t.ja); t.en)
        }
    }
}

/// What tells one cause from another: the requester a way comes to alone is not, so that the heads
/// of every department are one finding, their requesters listed by where the stage goes.
fn why_key(why: &Why) -> String {
    match why {
        Why::OnlyRequester(_) => "OnlyRequester".to_string(),
        _ => format!("{why:?}"),
    }
}

/// Up to three of the requesters a finding comes to, by name, and how many there are: `e9002 (Legal
/// 2) and e9003 (Legal 3)`, `e1101 (…), e1201 (…), e1203 (…) and 2 more`.
fn names(who: &[&Text]) -> Text {
    let shown: Vec<&Text> = who.iter().take(SHOWN).copied().collect();
    let ja = shown.iter().map(|t| t.ja.clone()).collect::<Vec<_>>().join("、");
    let en_items: Vec<String> = shown.iter().map(|t| t.en.clone()).collect();
    let rest = who.len().saturating_sub(SHOWN);
    let en = if rest > 0 {
        format!("{} and {rest} more", en_items.join(", "))
    } else if en_items.len() > 1 {
        format!("{} and {}", en_items[..en_items.len() - 1].join(", "), en_items[en_items.len() - 1])
    } else {
        en_items.join("")
    };
    let ja = if rest > 0 { format!("{ja}、ほか {rest} 人") } else { ja };
    Text::new(ja, en)
}

/// The requesters a finding comes to: `For 2 requesters: e9002 (Legal 2) and e9003 (Legal 3).`
fn listed(who: &[(Text, Option<Text>)], all: usize) -> Text {
    let n = who.len();
    let ns = names(&who.iter().map(|(r, _)| r).collect::<Vec<_>>());
    let s = if n == 1 { "" } else { "s" };
    if n == all && n > 1 {
        tr!("どの申請者でも（{n} 人）: {}。", "For every requester ({n} requesters): {}.", ns.ja; ns.en)
    } else {
        tr!("申請者 {n} 人: {}。", "For {n} requester{s}: {}.", ns.ja; ns.en)
    }
}

/// The requesters of a finding by the one each comes to (the one person who answers both stages,
/// the head a stage goes up to), the three biggest groups: `e3101 (Head of Sales Department 1)
/// answers both for 3 requesters: …`.
fn by_target(who: &[(Text, Option<Text>)], one: impl Fn(&Text, usize, &Text) -> Text, others: impl Fn(usize) -> Text) -> Vec<Text> {
    let mut groups: Vec<(&Text, Vec<&Text>)> = Vec::new();
    for (r, t) in who {
        let Some(t) = t else { continue };
        match groups.iter_mut().find(|(g, _)| *g == t) {
            Some((_, rs)) => rs.push(r),
            None => groups.push((t, vec![r])),
        }
    }
    // the most requesters first; in the order they came when as many
    groups.sort_by_key(|(_, rs)| std::cmp::Reverse(rs.len()));
    let mut out: Vec<Text> = groups.iter().take(SHOWN).map(|(t, rs)| one(t, rs.len(), &names(rs))).collect();
    let rest: usize = groups.iter().skip(SHOWN).map(|(_, rs)| rs.len()).sum();
    if rest > 0 {
        out.push(others(rest));
    }
    out
}

/// The days of the versions a finding is said for: `In the organization from 2026-04-01`, `In the
/// versions of the organization from 2026-04-01 and 2027-04-01`.
fn in_versions(days: &[String]) -> Text {
    match days {
        [one] => tr!("{one} からの組織では", "In the organization from {one}"),
        _ => {
            let ja: Vec<String> = days.iter().map(|d| format!("{d} から")).collect();
            let ja = if ja.len() == 2 { format!("{}と {}", ja[0], ja[1]) } else { ja.join("、") };
            let en = format!("{} and {}", days[..days.len() - 1].join(", "), days[days.len() - 1]);
            tr!("{ja}の組織では", "In the versions of the organization from {en}")
        }
    }
}

/// The lines of the rows a finding comes from, when it is more than one.
fn rows_note(lines: &BTreeSet<usize>) -> Option<Text> {
    if lines.len() < 2 {
        return None;
    }
    let ls = lines.iter().map(|l| l.to_string()).collect::<Vec<_>>().join(", ");
    Some(tr!("{ls} 行目の行で、どれも同じです。", "The same on the rows of lines {ls}."))
}

/// The versions a request is checked against: the ones in effect on some day it can be used on.
fn versions<'o>(org: &'o Org, usable: Option<&crate::usable::Usable>) -> Vec<&'o Version> {
    let from = usable.and_then(|u| u.from.as_ref()).map(|e| e.day);
    let through = usable.and_then(|u| u.through.as_ref()).map(|e| e.day);
    org.versions
        .iter()
        .filter(|v| {
            let (a, b) = org.period(v);
            from.is_none_or(|f| b.is_none_or(|b| b >= f)) && through.is_none_or(|t| a <= t)
        })
        .collect()
}

/// The findings of the request in `s` against each version of the organization it reads; W500 when
/// it reads one with no data. What it returns is what `ok` says under its line: who may submit,
/// when the request says `submitted by`, and what the shape of the organization does to its stages.
pub fn check(s: &Scope, usable: Option<&crate::usable::Usable>, diags: &mut Vec<Diag>) -> Vec<Fact> {
    let ps = paths(s);
    let approves = ps.iter().flatten().any(|o| o.spec.approves());
    let Some(data) = s.org_rc() else { return Vec::new() };
    let submit = super::submit::of(s.file);
    let mut facts = Vec::new();
    if let (Data::Org(org), Some(l)) = (data.as_ref(), &submit) {
        facts.extend(submitters(s, org, l, usable, diags).into_iter().map(Fact::line));
    }
    if let Data::Org(org) = data.as_ref() {
        owner(s, org, usable, diags);
    }
    if !approves {
        return facts;
    }
    let org = match data.as_ref() {
        Data::Org(o) => o,
        Data::NoOrg(none) => {
            let at = s.uses_of(UseKind::Organization).next().or_else(|| s.uses_of(UseKind::Route).next());
            let (line, col) = at.map(|u| (u.line, 1)).unwrap_or((s.file.name.span.line, 1));
            let what = &none.what;
            diags.push(
                Diag::warning("W500", s.path, line, col, tr!("{}ので、ステップに承認者がいるかを決められません", "{}, so whether each stage has an approver is not decided", what.ja; ritsu_base::text::capitalize(&what.en)))
                    .source(s.src)
                    .note(tr!("{}。", "{}.", none.why.ja; none.why.en))
                    .note(none.fix.clone()),
            );
            return facts;
        }
        Data::Masters(_) => return facts,
    };
    let allowed = self_approval(s.file);
    let once = once(s.file);
    let mut g = Gather { found: BTreeMap::new(), shaped: BTreeMap::new(), all: BTreeMap::new(), people: BTreeMap::new() };
    for v in versions(org, usable) {
        let mut rs = requesters(org, v, s.file);
        if let Some(l) = &submit {
            rs.retain(|r| l.may(org, v, &r.id));
        }
        g.all.insert(v.from, rs.len());
        g.people.insert(v.from, rs.iter().map(|r| &r.id).collect::<BTreeSet<_>>().len());
        for r in &rs {
            let sv = Solver { s, org, v, requester: r, self_approval: allowed };
            let named = r.named(v);
            for path in &ps {
                let cases: Vec<Vec<(String, Option<Quorum>, Vec<Case>)>> = path.iter().map(|o| stage_cases(&sv, o)).collect();
                // the one person who alone answers each part of each stage, for W506
                let mut singles: Vec<Single> = Vec::new();
                for (i, o) in path.iter().enumerate() {
                    for (j, (way, quorum, cs)) in cases[i].iter().enumerate() {
                        for (k, c) in cs.iter().enumerate() {
                            one(&mut g, &sv, v, r, &named, o, way, *quorum, c, (i, j, k), &mut singles);
                            late(&mut g, &sv, v, r, &named, o, way, c);
                        }
                    }
                }
                let seen: Vec<Seen> = path.iter().zip(&cases).map(|(o, cs)| seen_of(&sv, o, cs)).collect();
                nobody(&mut g, v, r, &named, path, &seen);
                if singles.len() < 2 {
                    continue;
                }
                for (n, a) in singles.iter().enumerate() {
                    for b in &singles[n + 1..] {
                        let (oa, ob) = (&path[a.at.0], &path[b.at.0]);
                        if oa.stage == ob.stage || a.way == b.way || a.person != b.person {
                            continue;
                        }
                        let (sa, sb, wa, wb) = (&oa.stage, &ob.stage, &a.way, &b.way);
                        let label = |l: &Option<Text>| l.as_ref().map(|l| tr!("（{}）", " ({})", l.ja; l.en)).unwrap_or_default();
                        let (xa, xb) = (label(&a.label), label(&b.label));
                        let cause = tr!(
                            "ステップ `{sa}` の決め方は `{wa}`{}、ステップ `{sb}` の決め方は `{wb}`{} で、同じ人に当たります",
                            "the stage `{sa}` is `{wa}`{} and the stage `{sb}` is `{wb}`{}, and both come to one person",
                            xa.ja, xb.ja; xa.en, xb.en
                        );
                        // what the ticket's line does to the two stages. `same approver approves
                        // once`: a later stage it does is not said, and one it does not is said with
                        // why. `, at the last stage`: the earlier stage waits for the later, and the
                        // two are not said, unless an agent answers one of them first. Not written, or
                        // written in the way that does not do them: how to write the one that does
                        let agent = |at: (usize, usize, usize)| cases[at.0][at.1].2[at.2].agent;
                        let waits = !agent(a.at) && !agent(b.at);
                        // the stage the one answer is given at: the later of the two, or a later
                        // stage still that the same person alone answers
                        let at_last = if singles.iter().any(|c| c.person == a.person && c.at.0 > b.at.0 && !agent(c.at)) {
                            tr!("は、その人が答える最後のステップの承認を待ちます", "waits for the approval of the last stage they answer")
                        } else {
                            tr!("は、あいだのステップのあとの `{sb}` の承認を待ちます", "waits for the approval of `{sb}`, after the stages between them")
                        };
                        let (why, extra) = match once {
                            Once::Last if waits => continue,
                            Once::Last => {
                                let s = if agent(b.at) { sb } else { sa };
                                (
                                    format!("last agent {s}"),
                                    vec![tr!(
                                        "`same approver approves once, at the last stage` でも、`{sa}` は `{sb}` を待ちません。`{s}` は、エージェントが先に答えるステップです。",
                                        "`same approver approves once, at the last stage` does not make `{sa}` wait for `{sb}`: `{s}` is a stage an agent answers first."
                                    )],
                                )
                            }
                            Once::Before => match carried(path, &seen, &cases, a.at, b.at, &a.person) {
                                Ok(_) => continue,
                                Err(e) => {
                                    let (k, t) = match e {
                                        Break::Between(k) => {
                                            let sk = &path[k].stage;
                                            (format!("between {sk}"), tr!("あいだに `{sk}` があります", "`{sk}` comes between them"))
                                        }
                                        Break::Parts(w) => (format!("parts {w}"), tr!("`{sa}` は `{w}` の承認も待ちます", "`{sa}` waits for `{w}` too")),
                                        Break::Agent => ("agent".to_string(), tr!("`{sb}` はエージェントが答えるステップです", "`{sb}` is a stage an agent answers")),
                                    };
                                    let mut extra = vec![tr!(
                                        "`same approver approves once` でも、`{sb}` は `{sa}` の承認では済みません。{}。",
                                        "`same approver approves once` does not do `{sb}` with the approval of `{sa}`: {}.",
                                        t.ja; t.en
                                    )];
                                    if waits {
                                        extra.push(tr!(
                                            "`same approver approves once, at the last stage` と書けば、一度の承認で済みます。`{sa}` {}。",
                                            "With `same approver approves once, at the last stage`, they answer once: `{sa}` {}.",
                                            at_last.ja; at_last.en
                                        ));
                                    }
                                    (k, extra)
                                }
                            },
                            Once::No => match carried(path, &seen, &cases, a.at, b.at, &a.person) {
                                Ok(k) => {
                                    let sk = &path[k].stage;
                                    (
                                        format!("would {sk}"),
                                        vec![tr!(
                                            "一度の承認でよいなら、`ticket` に `same approver approves once` と書いてください。`{sb}` は `{sk}` の承認で済みます。",
                                            "If one approval is enough, write `same approver approves once` under `ticket`: `{sb}` is then done with the approval of `{sk}`."
                                        )],
                                    )
                                }
                                Err(_) if waits => (
                                    "would wait".to_string(),
                                    vec![tr!(
                                        "一度の承認でよいなら、`ticket` に `same approver approves once, at the last stage` と書いてください。`{sa}` {}。",
                                        "If one approval is enough, write `same approver approves once, at the last stage` under `ticket`: `{sa}` then {}.",
                                        at_last.ja; at_last.en
                                    )],
                                ),
                                Err(_) => (String::new(), Vec::new()),
                            },
                        };
                        let key = Key { version: v.from, code: "W506", stage: format!("{sa}/{sb}"), way: format!("{wa}/{wb}{}{}", xa.en, xb.en), cause: why };
                        g.add(v, key, ob.line, ob.col, cause, r, named.clone(), Some(&a.person), extra);
                    }
                }
            }
        }
    }
    let Gather { found, shaped, all, people } = g;
    say(s, org, found, &all, diags);
    facts.extend(facts_of(s, org, shaped, &all, &people));
    facts
}

/// W510: no stage of a path of the route has anyone to answer it for the requester, and a stage of
/// it approves as written: every one is skipped for lack of an approver, circulates, or does not
/// come on the row. A ticket of theirs is approved as soon as it is submitted, with nobody answering
/// it. A stage a condition decides (a stage line's `when`) may not come, and the conditions are not
/// read: where the others have nobody, the ticket such a stage does not come to is approved so,
/// and the finding names the stage. Said at the first stage that approves; the stages it passes
/// over tell one finding from another.
fn nobody(g: &mut Gather, v: &Version, r: &Requester, named: &Text, path: &[Occ], seen: &[Seen]) {
    let Some(first) = path.iter().position(|o| o.spec.approves()) else { return };
    let passes = |s: &Seen| match s {
        Seen::Pass => true,
        Seen::Other => false,
        Seen::Parts(parts) | Seen::Answered(parts) | Seen::Once(parts) => parts.iter().all(|(_, os)| os.iter().all(|o| *o == Out::Pass)),
    };
    if path.iter().zip(seen).any(|(o, s)| !passes(s) && !o.cond) {
        return;
    }
    let skipped: Vec<String> = path.iter().zip(seen).filter(|(_, s)| matches!(s, Seen::Parts(_) | Seen::Answered(_) | Seen::Once(_)) && passes(s)).map(|(o, _)| o.stage.clone()).collect();
    // with no stage skipped for the requester, it is the conditions alone that keep every stage
    // away, for anyone: not this finding
    if skipped.is_empty() {
        return;
    }
    let away: Vec<String> = path.iter().zip(seen).filter(|(_, s)| !passes(s)).map(|(o, _)| o.stage.clone()).collect();
    let listed = |names: &[String]| {
        let shown: Vec<String> = names.iter().map(|st| format!("`{st}`")).collect();
        (shown.join("、"), match shown.as_slice() {
            [one] => one.clone(),
            [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
            [] => String::new(),
        })
    };
    let (ja, en) = listed(&skipped);
    let (aja, aen) = listed(&away);
    let cause = match (skipped.len() == 1, away.len()) {
        (true, 0) => tr!("{ja} は承認者がいないので飛ばされ、ほかに答える人のいるステップがありません", "{en} is skipped for lack of an approver, and no other stage has anyone to answer it"),
        (false, 0) => tr!("{ja} はどれも承認者がいないので飛ばされ、ほかに答える人のいるステップがありません", "{en} are skipped for lack of an approver, and no other stage has anyone to answer it"),
        (true, 1) => tr!("{ja} は承認者がいないので飛ばされ、条件で来る {aja} が来ないチケットには、答える人のいるステップがありません", "{en} is skipped for lack of an approver, and a ticket that {aen} does not come to (a condition decides it) has no stage with anyone to answer it"),
        (false, 1) => tr!("{ja} はどれも承認者がいないので飛ばされ、条件で来る {aja} が来ないチケットには、答える人のいるステップがありません", "{en} are skipped for lack of an approver, and a ticket that {aen} does not come to (a condition decides it) has no stage with anyone to answer it"),
        (true, _) => tr!("{ja} は承認者がいないので飛ばされ、条件で来る {aja} のどれも来ないチケットには、答える人のいるステップがありません", "{en} is skipped for lack of an approver, and a ticket that none of {aen} comes to (conditions decide them) has no stage with anyone to answer it"),
        (false, _) => tr!("{ja} はどれも承認者がいないので飛ばされ、条件で来る {aja} のどれも来ないチケットには、答える人のいるステップがありません", "{en} are skipped for lack of an approver, and a ticket that none of {aen} comes to (conditions decide them) has no stage with anyone to answer it"),
    };
    // what happens to the ticket, said of the tickets the stages with a condition do not come to
    let extra = if away.is_empty() { Vec::new() } else { vec![tr!("そういうチケットは、出したとたんに、だれも答えないまま承認で終わります。", "Such a ticket of theirs is approved as soon as it is submitted, with nobody answering it.")] };
    let o = &path[first];
    let key = Key { version: v.from, code: "W510", stage: skipped.join("/"), way: away.join("/"), cause: String::new() };
    g.add(v, key, o.line, o.col, cause, r, named.clone(), None, extra);
}

/// Who may submit the request in each version (DESIGN 5.13): E508 for a team or a person the
/// `submitted by` line names that a version does not have, E509 for a version no one may submit in
/// (unless it has E508, which says the cause), each said once for the versions it holds in; and what
/// `ok` says of how many may submit.
/// E511: the team `owner` names, where a version of the organization the request is used in does
/// not have it. The server passes that team to the company's rules when someone manages the kind
/// (DESIGN 14.3), and a team the version does not have is no department's: only the rules that read
/// no department (those of the administrators of the whole) would let anyone manage it.
fn owner(s: &Scope, org: &Org, usable: Option<&crate::usable::Usable>, diags: &mut Vec<Diag>) {
    let Some(w) = &s.file.owner else { return };
    let id = &w.text;
    let days: Vec<String> = versions(org, usable).into_iter().filter(|v| v.team(id).is_none()).map(|v| v.day()).collect();
    if days.is_empty() {
        return;
    }
    let at = in_versions(&days);
    let d = Diag::error("E511", s.path, w.span.line, w.span.col, tr!("{}、`owner` が指すチーム `{id}` がありません", "{}, `owner` names the team `{id}`, which is not there", at.ja; at.en))
        .source(s.src)
        .note(tr!(
            "`owner` は申請の種類を受け持つ部門です。チームが版に無ければ、その版の間、種類はどの部門の物にもならず、管理の画面では全体の管理者しか扱えません。",
            "`owner` is the department that keeps the kind of request: while a version does not have the team, the kind is no department's, and only the administrators of the whole may manage it."
        ))
        .note(tr!("チームの ID を直すか、版のデータにそのチームを足してください。", "Correct the team's ID, or add the team to the data of the version."));
    diags.push(d.with(crate::diag::Spaced { witness: Some(Json::obj([("versions", Json::arr(days.iter().map(Json::str))), ("team", Json::str(id))])), ..Default::default() }));
}

fn submitters(s: &Scope, org: &Org, l: &super::submit::Line, usable: Option<&crate::usable::Usable>, diags: &mut Vec<Diag>) -> Vec<Text> {
    use super::submit::Missing;
    let shown = l.shown();
    let mut missing: Vec<((usize, usize), Missing, String, Vec<String>)> = Vec::new();
    let mut nobody: Vec<String> = Vec::new();
    let mut counts: Vec<(String, usize, usize)> = Vec::new();
    for v in versions(org, usable) {
        let gone = l.missing(v);
        for (span, m, group) in &gone {
            match missing.iter_mut().find(|(at, x, ..)| *at == (span.line, span.col) && x == m) {
                Some((.., days)) => days.push(v.day()),
                None => missing.push(((span.line, span.col), m.clone(), group.clone(), vec![v.day()])),
            }
        }
        let may = v.people.iter().filter(|p| l.may(org, v, &p.id)).count();
        if may == 0 && gone.is_empty() {
            nobody.push(v.day());
        }
        counts.push((v.day(), may, v.people.len()));
    }
    let witness = |days: &[String], group: &str| crate::diag::Spaced { witness: Some(Json::obj([("versions", Json::arr(days.iter().map(Json::str))), ("group", Json::str(group))])), ..Default::default() };
    for ((line, col), m, group, days) in &missing {
        let at = in_versions(days);
        let (msg, fix) = match m {
            Missing::Team(id) => (
                tr!("{}、`submitted by` が指すチーム `{id}` がありません", "{}, `submitted by` names the team `{id}`, which is not there", at.ja; at.en),
                tr!("ID を直すか、版のデータにそのチームを足してください。", "Correct the ID, or add the team to the data of the version."),
            ),
            Missing::Person(id) => (
                tr!("{}、`submitted by` が指す {id} がいません", "{}, `submitted by` names {id}, who is not there", at.ja; at.en),
                tr!("ID を直すか、版のデータにその人を足してください。", "Correct the ID, or add the person to the data of the version."),
            ),
        };
        let d = Diag::error("E508", s.path, *line, *col, msg).source(s.src).note(fix);
        diags.push(d.with(witness(days, group)));
    }
    if !nobody.is_empty() {
        let at = in_versions(&nobody);
        let within = tr!("{}", "{}", at.ja; lower_first(&at.en));
        let d = Diag::error("E509", s.path, l.line, l.col, tr!("{}、この申請をだれも出せません", "{}, no one may submit the request", at.ja; at.en))
            .source(s.src)
            .note(tr!("{}、`{shown}` に当たる人がいません。", "`{shown}` comes to nobody {}.", within.ja; within.en))
            .note(tr!("`submitted by` を広げるか、版のデータを直してください。", "Widen `submitted by`, or correct the data of the version."));
        diags.push(d.with(witness(&nobody, &shown)));
    }
    if counts.is_empty() {
        return Vec::new();
    }
    vec![super::submit::fact(&shown, &counts)]
}

/// `In the organization …` as the middle of a sentence: `in the organization …`.
fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_lowercase().chain(c).collect(),
        None => String::new(),
    }
}

/// One person who alone answers a part of a stage of a path, for W506: where (the stage, the part
/// and the case, by their places), the way of the case and its label, and the person.
struct Single {
    at: (usize, usize, usize),
    way: String,
    label: Option<Text>,
    person: String,
}

#[allow(clippy::too_many_arguments)]
fn one(g: &mut Gather, sv: &Solver, v: &Version, r: &Requester, named: &Text, o: &Occ, way: &str, quorum: Option<Quorum>, c: &Case, at: (usize, usize, usize), singles: &mut Vec<Single>) {
    let stage = &o.stage;
    let single = |p: &str| Single { at, way: c.way.clone(), label: c.label.clone(), person: p.to_string() };
    match &c.found {
        Found::Open => {}
        Found::People(ps) => {
            if let Some(q) = quorum {
                let n = needed(q, ps.len());
                if ps.len() < n {
                    let k = ps.len();
                    let cw = &c.way;
                    let m = if k == 1 { "member" } else { "members" };
                    let cause = tr!("`{cw}` に、申請者のほかの人が {k} 人しかいません", "`{cw}` has only {k} {m} besides the requester");
                    let key = Key { version: v.from, code: "E504", stage: stage.clone(), way: way.to_string(), cause: format!("{n}/{k}") };
                    g.add(v, key, o.line, o.col, cause, r, named.clone(), None, Vec::new());
                    return;
                }
                if n == 1 && ps.len() == 1 {
                    singles.push(single(&ps[0]));
                }
                return;
            }
            if ps.len() == 1 {
                singles.push(single(&ps[0]));
            }
        }
        Found::Nobody(why, up) => {
            let cw = &c.way;
            let mut cause = why_apart(v, cw, why);
            if let Some(l) = &c.label {
                cause = tr!("{}（{}）", "{} ({})", cause.ja, l.ja; cause.en, l.en);
            }
            let ck = format!("{}{}", why_key(why), c.label.as_ref().map(|l| l.en.clone()).unwrap_or_default());
            match &o.empty {
                WhenEmpty::Stay => {
                    let key = Key { version: v.from, code: "E502", stage: stage.clone(), way: way.to_string(), cause: ck };
                    g.add(v, key, o.line, o.col, cause, r, named.clone(), None, Vec::new());
                }
                WhenEmpty::Skip => {
                    let hole = way_hole(sv, why);
                    let key = Key { version: v.from, code: "W502", stage: stage.clone(), way: way.to_string(), cause: format!("skip {ck}{}", Hole::key(&hole)) };
                    let mut extra = vec![tr!("`when empty skip` なので、飛ばされます。", "It is skipped (`when empty skip`).")];
                    extra.extend(Hole::note(v, &hole));
                    g.put(hole.is_some(), v, key, o.line, o.col, cause, r, named.clone(), None, extra);
                }
                WhenEmpty::Escalate(_) if *up == Up::Cannot && matches!(why, Why::PersonNotIn(_) | Why::TeamNotIn(_)) => {
                    // said at the way to choose, where the person or the team is named
                    let (line, col) = match Solver::who(o.file, cw) {
                        Some(w) if std::ptr::eq(o.file, sv.s.file) => (w.line, w.name.span.col),
                        _ => (o.line, o.col),
                    };
                    let key = Key { version: v.from, code: "E503", stage: stage.clone(), way: cw.clone(), cause: ck };
                    g.add(v, key, line, col, cause, r, named.clone(), None, Vec::new());
                }
                WhenEmpty::Escalate(top) => match (sv.up(up), top) {
                    (Some(h), _) => {
                        let written = o.empty.written();
                        let hole = hole(sv, why, up);
                        let key = Key { version: v.from, code: "W502", stage: stage.clone(), way: way.to_string(), cause: format!("up {ck}{}", Hole::key(&hole)) };
                        let mut extra = vec![tr!("ステップは `{written}` なので、上へ回ります。", "The stage is `{written}`, and it goes up.")];
                        extra.extend(Hole::note(v, &hole));
                        g.put(hole.is_some(), v, key, o.line, o.col, cause, r, named.clone(), Some(&h), extra);
                    }
                    // nobody above: skipped where the stage says `then skip`, to the way `then`
                    // names, else it waits (E502)
                    (None, Some(AtTop::Skip)) => {
                        let hole = hole(sv, why, up);
                        let key = Key { version: v.from, code: "W502", stage: stage.clone(), way: way.to_string(), cause: format!("top {ck}{}", Hole::key(&hole)) };
                        let none = Hole::none_above(&hole);
                        let mut extra = vec![tr!(
                            "ステップは `when empty escalate, then skip` で、{}ので、飛ばされます。",
                            "The stage is `when empty escalate, then skip`, and {}: it is skipped.",
                            none.ja; none.en
                        )];
                        extra.extend(Hole::note(v, &hole));
                        g.put(hole.is_some(), v, key, o.line, o.col, cause, r, named.clone(), None, extra);
                    }
                    (None, Some(AtTop::To(tw))) => at_top_to(g, sv, v, r, named, o, way, &cause, &ck, tw, hole(sv, why, up)),
                    (None, _) => {
                        let written = o.empty.written();
                        let key = Key { version: v.from, code: "E502", stage: stage.clone(), way: way.to_string(), cause: format!("top {ck}") };
                        let extra = vec![
                            tr!(
                                "ステップは `{written}` で、上に回せる人がいない（組織のいちばん上）ので、チケットはこのステップで、管理する人が承認者を足すまで待ちます。",
                                "The stage is `{written}`, and nobody is above (the top of the organization): a ticket that comes to it waits there until an administrator adds an approver."
                            ),
                            tr!(
                                "飛ばしてよいなら `when empty escalate, then skip` と書いてください。だれかが見るべきなら、`then` のあとに、その人の申請に答える承認者の決め方を書いてください（`who` に `board = team board` と宣言して `when empty escalate, then board`）。",
                                "If the stage may be skipped for them, write `when empty escalate, then skip`; if someone must see their requests, write after `then` the way to choose who answers them (`when empty escalate, then board`, with `board = team board` in `who`)."
                            ),
                        ];
                        g.add(v, key, o.line, o.col, cause, r, named.clone(), None, extra);
                    }
                },
            }
        }
    }
}

/// Where nobody is above a stage that goes up when it is empty and says `then <way>` (DESIGN 5.4):
/// it goes to that way, solved for the requester. W502 where the way has someone to answer and
/// `hole` is what the data can fill, what the shape of the organization does where there is none;
/// E502 where it finds nobody either (the stage waits); nothing where the organization does not
/// decide it (a person chosen when the ticket runs).
#[allow(clippy::too_many_arguments)]
fn at_top_to(g: &mut Gather, sv: &Solver, v: &Version, r: &Requester, named: &Text, o: &Occ, way: &str, cause: &Text, ck: &str, tw: &Word, hole: Option<Hole>) {
    let written = o.empty.written();
    let tn = &tw.text;
    for c in sv.way(o.empty_in, tn) {
        let label = c.label.as_ref().map(|l| tr!("（{}）", " ({})", l.ja; l.en)).unwrap_or_default();
        match &c.found {
            Found::People(ps) => {
                let to = (ps.len() == 1).then(|| ps[0].as_str());
                let none = Hole::none_above(&hole);
                let goes = if ps.len() == 1 {
                    tr!(
                        "ステップは `{written}` で、{}ので、`{tn}`{} へ回ります。",
                        "The stage is `{written}`, and {}: it goes to `{tn}`{}.",
                        none.ja, label.ja;
                        none.en, label.en
                    )
                } else {
                    let people: Vec<Text> = ps.iter().map(|p| v.who(p)).collect();
                    let ns = names(&people.iter().collect::<Vec<_>>());
                    tr!(
                        "ステップは `{written}` で、{}ので、`{tn}`{} へ回ります。{}のだれかが答えます。",
                        "The stage is `{written}`, and {}: it goes to `{tn}`{}, where one of {} answers.",
                        none.ja, label.ja, super::ja_word(&ns.ja); none.en, label.en, ns.en
                    )
                };
                let key = Key { version: v.from, code: "W502", stage: o.stage.clone(), way: way.to_string(), cause: format!("then {tn}{} {} {ck}{}", label.en, ps.join(" "), Hole::key(&hole)) };
                let mut extra = vec![goes];
                extra.extend(Hole::note(v, &hole));
                g.put(hole.is_some(), v, key, o.line, o.col, cause.clone(), r, named.clone(), to, extra);
            }
            Found::Nobody(why, _) => {
                let w = why_apart(v, tn, why);
                let key = Key { version: v.from, code: "E502", stage: o.stage.clone(), way: way.to_string(), cause: format!("top then {tn}{} {} {ck}", label.en, why_key(why)) };
                let extra = vec![
                    tr!(
                        "ステップは `{written}` で、上に回せる人がいない（組織のいちばん上）ので `{tn}`{} へ回しますが、{}。チケットはこのステップで、管理する人が承認者を足すまで待ちます。",
                        "The stage is `{written}`, and nobody is above (the top of the organization): it goes to `{tn}`{}, but {}. A ticket that comes to it waits there until an administrator adds an approver.",
                        label.ja, w.ja; label.en, w.en
                    ),
                    tr!("版のデータを直すか、`then` のあとに、どの版でも人のいる決め方を書いてください。", "Correct the data of the version, or write after `then` a way that has someone in every version."),
                ];
                g.add(v, key, o.line, o.col, cause.clone(), r, named.clone(), None, extra);
            }
            Found::Open => {}
        }
    }
}

/// Where a late stage goes up to (DESIGN 5.10).
enum Goes {
    /// To this person (a team's first member stands for the team): with everyone it goes to, and
    /// the team when they are a team's members, which the ticket machine reads.
    To(String, Vec<String>, Option<String>),
    /// Nowhere: why (past the top of the organization, above a team, the way a table sends it to
    /// finds nobody), what tells it apart, and the hole in the data that leaves it so, if one does.
    Nowhere(Text, String, Option<Hole>),
    /// The organization does not decide it (a person chosen when the ticket runs).
    Open,
}

/// The first head above `p` who answers in their place: from their primary team up, each team's
/// head who is not them and not left out.
fn above_person(sv: &Solver, p: &str) -> Option<String> {
    let team = sv.v.primary(p)?.team.clone();
    sv.v.up(&team).into_iter().find_map(|t| t.head.clone().filter(|h| h != p && !sv.left_out(h)))
}

/// The ways the table `t` sends a late stage of the way `from` to: the first output of each row
/// whose column of ways takes it (a column that names a field holds the field's values, and is not
/// read here).
pub(crate) fn table_sends(s: &Scope, f: &File, t: &NamedTable, from: &str) -> Vec<String> {
    let is_field = |n: &str| s.field(n).is_some() || f.inputs.iter().any(|i| i.name.text == n);
    let Some(k) = t.inputs.iter().position(|c| !is_field(&c.text)) else { return Vec::new() };
    let mut out: Vec<String> = Vec::new();
    for r in &t.rows {
        let takes = match r.tests.get(k).map(|(x, _)| x) {
            Some(Test::Any) | None => true,
            Some(Test::Values { values, not }) => values.iter().any(|e| matches!(&e.kind, ExprKind::Path(p) if p.len() == 1 && p[0].text == from)) != *not,
            Some(Test::Compare(_)) => false,
        };
        if let (true, Some((Spec::Who(ws), _))) = (takes, r.stages.first())
            && ws.len() == 1
            && !out.contains(&ws[0].text)
        {
            out.push(ws[0].text.clone());
        }
    }
    out
}

/// Where a late stage goes up to from the people who answer it (`from`; the team `team`, when its
/// way is a team): with a table, each way it sends the stage's way `key` to, solved for the
/// requester; else the first head above. Each with the way it went to, where a table sent it.
#[allow(clippy::too_many_arguments)]
fn goes_up(sv: &Solver, line_file: &File, ways_file: &File, by: Option<&Word>, key: Option<&str>, from: &[String], team: Option<&str>) -> Vec<(Option<String>, Goes)> {
    match by {
        None => {
            let to = match team {
                Some(t) => sv.up(&Up::Above(t.to_string())),
                None => from.first().and_then(|p| above_person(sv, p)),
            };
            if let Some(p) = to {
                return vec![(None, Goes::To(p.clone(), vec![p], None))];
            }
            let (why, key, hole) = match team {
                Some(t) => {
                    let tn = sv.v.team_named(t);
                    (tr!("チーム {}の上に、回す先の長がいません", "nobody is above the team {} to go up to", super::ja_word(&tn.ja); tn.en), format!("above {t}"), passed(sv, &Up::Above(t.to_string())))
                }
                None => match from.first().and_then(|p| empty_above(sv, p)) {
                    // a seat above them is empty: they are not at the top
                    Some(h) => (tr!("期限を過ぎる人の上に、回す先の長がいません", "nobody is above the one who is late to go up to"), "above".to_string(), Some(h)),
                    None => (tr!("期限を過ぎる人が組織のいちばん上で、その上に回す先の長がいません", "the one who is late is at the top of the organization, with nobody above to go up to"), "above".to_string(), None),
                },
            };
            vec![(None, Goes::Nowhere(why, key, hole))]
        }
        Some(tw) => {
            let (Some(key), Some(t)) = (key, line_file.tables.iter().find(|x| x.name.text == tw.text)) else { return Vec::new() };
            let mut out = Vec::new();
            for sent in table_sends(sv.s, line_file, t, key) {
                let f = if Solver::who(ways_file, &sent).is_some() { ways_file } else { line_file };
                for c in sv.way(f, &sent) {
                    let team = match Solver::who(f, &sent).map(|w| &w.how) {
                        Some(How::Team(t)) => Some(t.text.clone()),
                        _ => None,
                    };
                    let goes = match &c.found {
                        Found::People(ps) => ps.first().cloned().map(|p| Goes::To(p, ps.clone(), team.clone())).unwrap_or(Goes::Open),
                        // a way that finds nobody goes up from where it stopped, as an empty stage
                        // does with `when empty escalate`
                        Found::Nobody(why, up) => match sv.up(up) {
                            Some(h) => Goes::To(h.clone(), vec![h], None),
                            None => {
                                let w = why_text(sv.v, &sent, why);
                                let tn = &tw.text;
                                let text = tr!(
                                    "表 `{tn}` は期限を過ぎた `{key}` を `{sent}` へ回しますが、{}。その上にも、回す先の長がいません",
                                    "the table `{tn}` sends a late `{key}` to `{sent}`, and {}; nobody is above that either",
                                    w.ja; w.en
                                );
                                Goes::Nowhere(text, format!("{tn} {key} {sent} {}", why_key(why)), hole(sv, why, up))
                            }
                        },
                        Found::Open => Goes::Open,
                    };
                    out.push((Some(sent.clone()), goes));
                }
            }
            out
        }
    }
}

/// Where a late stage goes up to, as the ticket machine reads it (DESIGN 5.10, 7.3): the people who
/// answer it then, with the way a table sent it to and the team when they are a team's members; or
/// why it has nowhere to go; or that a person chooses.
#[derive(Clone, Debug)]
pub enum LateTo {
    People { people: Vec<String>, way: Option<String>, team: Option<String> },
    Nowhere(Text),
    Open,
}

/// Where the stage goes up to when the people `from` are late (the team `team`, when they are a
/// team's members): the same answer the check lists a late stage by ([`nowhere`]). `line_file` is
/// the file the `when late` line is written in, `ways_file` the one the stage's ways are declared
/// in, `by` the table of `escalate by <table>`, `key` the way the table reads.
pub fn late_to(sv: &Solver, line_file: &File, ways_file: &File, by: Option<&Word>, key: Option<&str>, from: &[String], team: Option<&str>) -> Vec<LateTo> {
    goes_up(sv, line_file, ways_file, by, key, from, team)
        .into_iter()
        .map(|(sent, g)| match g {
            Goes::To(_, people, team) => LateTo::People { people, way: sent, team },
            Goes::Nowhere(why, ..) => LateTo::Nowhere(why),
            Goes::Open => LateTo::Open,
        })
        .collect()
}

/// Who answers a stage, for W507: the people, the team when the way is a team (a team is not one
/// person), the label of the case, how the stage came to them where its own way found nobody, what
/// tells that apart in the finding's key, the way a table reads for them where it is not the
/// stage's (the way `then` names), and the hole in the data that sent the stage to them, if one did.
struct Answers {
    people: Vec<String>,
    team: Option<String>,
    label: Option<Text>,
    came: Option<Text>,
    ck: String,
    reads: Option<String>,
    hole: Option<Hole>,
}

/// A late stage with nowhere to go up to (DESIGN 5.10): what W507 says, or what `ok` says where the
/// shape of the organization leaves it so.
pub struct Nowhere {
    /// Why, as the finding's cause says it (with the label of the case).
    why: Text,
    /// What tells the finding apart.
    key: String,
    /// The one who is late, where one person is.
    with: Option<String>,
    /// How the stage came to them where its own way found nobody, and the teams with an empty head.
    notes: Vec<Text>,
    /// What the stage does then: `it cannot go up, and expires at the deadline`.
    pub what: Text,
    /// Whether a hole in the data leaves it so: the stage came to them for one, or a seat above them
    /// is empty.
    pub hole: bool,
}

/// Where a stage that goes up when it is late (`when late escalate`) has nowhere to go up to, the
/// first time or the second (`escalate, then escalate`), for the case `c` of its part `way`. The one
/// who is late is whoever the stage comes to: the people its way finds, or, where that finds nobody
/// and the stage goes up when it is empty, the head it goes up to, and at the top of the
/// organization the people of the way `then` names (DESIGN 5.4). A table reads the stage's way, and
/// the way `then` names for the people of that way (DESIGN 5.10). `simulate` says it of the way too.
pub fn nowhere(sv: &Solver, o: &Occ, way: &str, c: &Case) -> Vec<Nowhere> {
    let Some(l) = o.late else { return Vec::new() };
    let Late::Escalate(by) = l.first else { return Vec::new() };
    // the way a table reads: a rule's output's value names its way; else the stage's way as written
    let key = if matches!(o.spec, Spec::RuleOutput(_)) { c.way.as_str() } else { way };
    let team_of = |f: &File, w: &str| match Solver::who(f, w).map(|w| &w.how) {
        Some(How::Team(t)) => Some(t.text.clone()),
        _ => None,
    };
    let written = o.empty.written();
    let mut answers = Vec::new();
    match &c.found {
        Found::People(ps) if !ps.is_empty() => answers.push(Answers { people: ps.clone(), team: team_of(o.file, &c.way), label: c.label.clone(), came: None, ck: String::new(), reads: None, hole: None }),
        Found::Nobody(why, up) => {
            if let WhenEmpty::Escalate(top) = &o.empty
                && !(*up == Up::Cannot && matches!(why, Why::PersonNotIn(_) | Why::TeamNotIn(_)))
            {
                // where a hole in the data sent the stage, what it is: the cause of the stage's way
                let hole = hole(sv, why, up);
                let cw = &c.way;
                let because = hole.as_ref().map(|_| why_text(sv.v, cw, why));
                match (sv.up(up), top) {
                    (Some(h), _) => {
                        let (came, ck) = match &because {
                            Some(b) => (tr!("{}。ステップは `{written}` なので、上へ回ります。", "{}; the stage is `{written}`, and it goes up.", b.ja; b.en), format!("up {cw} {} ", why_key(why))),
                            None => (tr!("ステップは `{written}` で、承認者がいないので上へ回ります。", "The stage is `{written}`, and it goes up for lack of an approver."), "up ".to_string()),
                        };
                        answers.push(Answers { people: vec![h], team: None, label: c.label.clone(), came: Some(came), ck, reads: None, hole });
                    }
                    (None, Some(AtTop::To(tw))) => {
                        let tn = &tw.text;
                        let none = Hole::none_above(&hole);
                        let goes = tr!("ステップは `{written}` で、{}ので、`{tn}` へ回ります。", "The stage is `{written}`, and {}: it goes to `{tn}`.", none.ja; none.en);
                        let (came, ck) = match &because {
                            Some(b) => (tr!("{}。{}", "{}; {}", b.ja, goes.ja; b.en, lower_first(&goes.en)), format!("then {tn} {cw} {} ", why_key(why))),
                            None => (goes, format!("then {tn} ")),
                        };
                        for tc in sv.way(o.empty_in, tn) {
                            if let Found::People(ps) = &tc.found
                                && !ps.is_empty()
                            {
                                // where the stage went does not hang on the case of its own way
                                answers.push(Answers { people: ps.clone(), team: team_of(o.empty_in, tn), label: tc.label.clone(), came: Some(came.clone()), ck: ck.clone(), reads: Some(tn.clone()), hole: hole.clone() });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    let mut out = Vec::new();
    for a in answers {
        let Answers { people: ps, team, label, came, ck, reads, hole: came_hole } = a;
        let key = reads.as_deref().unwrap_or(key);
        let mut said = |step: u8, why: Text, wk: String, with: Option<String>, hole: Option<Hole>| {
            let what = match (step, l.then) {
                (1, None | Some(Late::Expire)) => tr!("上へ回せないので、その期限で期限切れになります", "it cannot go up, and expires at the deadline"),
                (1, Some(Late::Stay)) => tr!("上へ回せないので、期限のあとも同じ承認者を待ち続けます", "it cannot go up, and waits on for the one who is late"),
                (1, Some(Late::Escalate(_))) => tr!("二度とも上へ回せないので、その期限で期限切れになります", "it cannot go up either time, and expires at the deadline"),
                _ => tr!("二度目は上へ回せないので、二度目の期限で期限切れになります", "the second time, it cannot go up, and expires at the second deadline"),
            };
            let mut why = if step == 1 { why } else { tr!("二度目に上へ回すとき、{}", "going up the second time, {}", why.ja; why.en) };
            if let Some(l) = &label {
                why = tr!("{}（{}）", "{} ({})", why.ja, l.ja; why.en, l.en);
            }
            let mut notes: Vec<Text> = came.iter().cloned().collect();
            let mut hk = String::new();
            for h in [&came_hole, &hole] {
                if let Some(n) = Hole::note(sv.v, h)
                    && !notes.contains(&n)
                {
                    notes.push(n);
                    hk.push_str(&Hole::key(h));
                }
            }
            let lk = label.as_ref().map(|l| l.en.clone()).unwrap_or_default();
            out.push(Nowhere { why, key: format!("{step} {ck}{wk}{lk}{hk}"), with, notes, what, hole: came_hole.is_some() || hole.is_some() });
        };
        // the one who is late: the person who answers; a team is not one person
        let late_with = if team.is_some() { None } else { Some(ps[0].clone()) };
        for (sent, first) in goes_up(sv, l.file, o.file, by.as_ref(), Some(key), &ps, team.as_deref()) {
            match first {
                Goes::Nowhere(why, wk, hole) => said(1, why, wk, late_with.clone(), hole),
                Goes::To(p, people, team) => {
                    // the second time: from the ones it went to (a team, where a table sent it to
                    // one), by the way a table sent it to, as a ticket goes up from them
                    if let Some(Late::Escalate(by2)) = l.then {
                        let with = if team.is_some() { None } else { Some(p.clone()) };
                        for (_, second) in goes_up(sv, l.file, o.file, by2.as_ref(), sent.as_deref(), &people, team.as_deref()) {
                            if let Goes::Nowhere(why, wk, hole) = second {
                                said(2, why, wk, with.clone(), hole);
                            }
                        }
                    }
                }
                Goes::Open => {}
            }
        }
    }
    out
}

/// W507: a late stage with nowhere to go up to ([`nowhere`]), where a hole in the data leaves it
/// so; what the shape of the organization does, where none does.
#[allow(clippy::too_many_arguments)]
fn late(g: &mut Gather, sv: &Solver, v: &Version, r: &Requester, named: &Text, o: &Occ, way: &str, c: &Case) {
    let Some(l) = o.late else { return };
    let lw = Late::line(l.first, l.then);
    for n in nowhere(sv, o, way, c) {
        let mut extra = n.notes;
        extra.push(tr!("ステップは `{lw}` です。{}。", "The stage is `{lw}`: {}.", n.what.ja; n.what.en));
        let key = Key { version: v.from, code: "W507", stage: o.stage.clone(), way: way.to_string(), cause: n.key };
        g.put(n.hole, v, key, o.line, o.col, n.why, r, named.clone(), n.with.as_deref(), extra);
    }
}

/// How to correct a W507: the data, or a table that says where the stage goes.
fn late_fix() -> Text {
    tr!(
        "版のデータを直してください。ほかの人へ回すなら、`when late escalate by <表>` の表で回す先を決めてください。",
        "Correct the data of the version; for it to go to someone else, say where it goes in the table of `when late escalate by <table>`."
    )
}

/// The notes of a finding: why, for whom, and how to correct it.
fn notes_of(k: &Key, f: &Found2, all: usize) -> Vec<Text> {
    let stage = &k.stage;
    let mut out = vec![tr!("{}。", "{}.", f.cause.ja; f.cause.en)];
    match k.code {
        // nobody above a stage that goes up: what it is written, and the two ways to correct it
        "E502" if k.cause.starts_with("top ") => {
            out.push(listed(&f.who, all));
            out.extend(f.extra.iter().cloned());
        }
        "E502" => {
            out.push(listed(&f.who, all));
            out.push(tr!(
                "`when empty stay` なので、チケットはこのステップで、管理する人が承認者を足すまで待ちます。",
                "The stage is `when empty stay`: a ticket that comes to it waits there until an administrator adds an approver."
            ));
            out.push(tr!("版のデータを直すか、ステップの承認者の決め方か `when empty` を変えてください。", "Correct the data of the version, or change the way to choose the stage's approver or its `when empty`."));
        }
        "W502" => {
            out.extend(f.extra.iter().cloned());
            // up to a head, or to the way `then` names where nobody is above
            let up = if k.cause.starts_with("then ") { "" } else { " up" };
            if f.who.iter().any(|(_, t)| t.is_some()) {
                out.extend(by_target(
                    &f.who,
                    |t, n, ns| {
                        let s = if n == 1 { "" } else { "s" };
                        tr!("{}へ回る申請者 {n} 人: {}。", "It goes{up} to {} for {n} requester{s}: {}.", super::ja_word(&t.ja), ns.ja; t.en, ns.en)
                    },
                    |n| {
                        let s = if n == 1 { "" } else { "s" };
                        tr!("ほかの申請者 {n} 人では、別の人へ回ります。", "For {n} more requester{s}, it goes{up} to someone else.")
                    },
                ));
            } else {
                out.push(listed(&f.who, all));
            }
            out.push(tr!("版のデータを直すか、そのままでよいかを確かめてください。", "Correct the data of the version, or make sure this is meant."));
        }
        "E503" => {
            out.push(tr!(
                "ステップ `{stage}` は `when empty escalate` ですが、組織にいない人やチームからは上へ回せません（`then` のあとに書いたことが効くのは、上に人がいないときだけです）。",
                "The stage `{stage}` is `when empty escalate`, and it cannot go up from someone the organization does not have (what `then` says holds only where nobody is above)."
            ));
            out.push(tr!("ID を直すか、その版のデータに人かチームを足してください。", "Correct the ID, or add the person (the team) to the data of the version."));
        }
        "E504" => {
            out.push(listed(&f.who, all));
            out.push(tr!("チームに人を足すか、議決の数を変えてください。", "Add people to the team, or change the quorum."));
        }
        "W510" => {
            match f.extra.first() {
                Some(such) => out.push(such.clone()),
                None => out.push(tr!("チケットは、出したとたんに、だれも答えないまま承認で終わります。", "A ticket of theirs is approved as soon as it is submitted, with nobody answering it.")),
            }
            out.push(listed(&f.who, all));
            out.push(tr!(
                "だれかが見るべきなら、`then` のあとに、その人の申請に答える承認者の決め方を書くか（`who` に `board = team board` と宣言して `when empty escalate, then board`）、版のデータを直してください。その人が出さない申請なら、`submitted by … except …` で外してください。",
                "If someone must see their requests, write after `then` the way to choose who answers them (`when empty escalate, then board`, with `board = team board` in `who`), or correct the data of the version; if they do not make the request, leave them out with `submitted by … except …`."
            ));
        }
        "W507" if !f.who.iter().any(|(_, t)| t.is_some()) => {
            out.extend(f.extra.iter().cloned());
            out.push(listed(&f.who, all));
            out.push(late_fix());
        }
        "W507" => {
            out.extend(f.extra.iter().cloned());
            out.extend(by_target(
                &f.who,
                |t, n, ns| {
                    let s = if n == 1 { "" } else { "s" };
                    tr!("{}が期限を過ぎる申請者 {n} 人: {}。", "Late with {} for {n} requester{s}: {}.", super::ja_word(&t.ja), ns.ja; t.en, ns.en)
                },
                |n| {
                    let s = if n == 1 { "" } else { "s" };
                    tr!("ほかの申請者 {n} 人では、別の人が期限を過ぎます。", "For {n} more requester{s}, it is late with someone else.")
                },
            ));
            out.push(late_fix());
        }
        _ => {
            out.extend(by_target(
                &f.who,
                |t, n, ns| {
                    let s = if n == 1 { "" } else { "s" };
                    tr!("{}が両方に当たる申請者 {n} 人: {}。", "For {n} requester{s}, {} answers both: {}.", super::ja_word(&t.ja), ns.ja; t.en, ns.en)
                },
                |n| {
                    let s = if n == 1 { "" } else { "s" };
                    tr!("ほかの申請者 {n} 人では、別の人が両方に当たります。", "For {n} more requester{s}, someone else answers both.")
                },
            ));
            // what `same approver approves once` does, or would do
            out.extend(f.extra.iter().cloned());
            out.push(tr!(
                "一人が二度確かめても、二人が確かめたことにはなりません。どちらかのステップの承認者の決め方を変えるか、そのままでよいかを確かめてください。",
                "One person who checks twice is not two pairs of eyes: change the way to choose one of the stages, or make sure it is meant."
            ));
        }
    }
    if let Some(n) = rows_note(&f.lines) {
        out.push(n);
    }
    out
}

/// The findings (or what the shape of the organization does), each once: the same in several
/// versions (the same notes, at the same place) is one, with the days of the versions. At one place
/// (a shared route's stages are said where the request names it), the ones of a stage come
/// together, in the order of the route.
fn merged(s: &Scope, org: &Org, found: BTreeMap<Key, Found2>, all: &BTreeMap<i64, usize>) -> Vec<(Key, Found2, Vec<Text>, Vec<String>)> {
    let mut said: Vec<(Key, Found2, Vec<Text>, Vec<String>)> = Vec::new();
    for (k, f) in found {
        let Some(v) = org.versions.iter().find(|v| v.from == k.version) else { continue };
        let notes = notes_of(&k, &f, all.get(&k.version).copied().unwrap_or(0));
        let same = said.iter_mut().find(|(k2, f2, n2, _)| (k2.code, &k2.stage, &k2.way, &k2.cause) == (k.code, &k.stage, &k.way, &k.cause) && (f2.line, f2.col) == (f.line, f.col) && *n2 == notes);
        match same {
            Some((_, _, _, days)) => days.push(v.day()),
            None => said.push((k, f, notes, vec![v.day()])),
        }
    }
    // a person or a team missing from several versions is one finding: its cause names them all
    for (_, f, notes, days) in &mut said {
        for n in notes.iter_mut() {
            *n = versions_in(n, days);
        }
        f.cause = versions_in(&f.cause, days);
        f.extra = f.extra.iter().map(|x| versions_in(x, days)).collect();
    }
    let mut order: Vec<String> = Vec::new();
    for o in paths(s).iter().flatten() {
        if !order.contains(&o.stage) {
            order.push(o.stage.clone());
        }
    }
    let place = |k: &Key, f: &Found2| {
        let first = k.stage.split('/').next().unwrap_or_default();
        (order.iter().position(|x| x == first).unwrap_or(order.len()), f.seq)
    };
    said.sort_by_key(|(k, f, ..)| place(k, f));
    said
}

/// The findings, each said once: the same finding in several versions is one diagnostic that names
/// the versions.
fn say(s: &Scope, org: &Org, found: BTreeMap<Key, Found2>, all: &BTreeMap<i64, usize>, diags: &mut Vec<Diag>) {
    let names = crate::engine::Names::of(s);
    for (k, f, notes, days) in merged(s, org, found, all) {
        let at = in_versions(&days);
        let (stage, way) = (&k.stage, &k.way);
        let n = k.cause.split('/').next().unwrap_or_default();
        let (error, msg) = match k.code {
            "E502" => (true, tr!("{}、ステップ `{stage}` に承認者がいません", "{}, the stage `{stage}` has no approver", at.ja; at.en)),
            "W502" => (false, tr!("{}、ステップ `{stage}` は承認者がいないので、ほかへ回るか飛ばされます", "{}, the stage `{stage}` goes elsewhere or is skipped for lack of an approver", at.ja; at.en)),
            "E503" => (true, tr!("{}、`{way}` が指す人かチームがありません", "{}, `{way}` names someone who is not there", at.ja; at.en)),
            "E504" => (true, tr!("{}、ステップ `{stage}` が {n} 人の承認に届かない申請者がいます", "{}, the stage `{stage}` cannot get {n} approvals for some requesters", at.ja; at.en)),
            "W507" => (false, tr!("{}、ステップ `{stage}` は、期限を過ぎても上へ回す先がありません", "{}, the stage `{stage}` has nowhere to go up to when it is late", at.ja; at.en)),
            "W510" => (false, tr!("{}、どのステップにも答える人がいない申請者がいます", "{}, no stage of the request has anyone to answer it for some requesters", at.ja; at.en)),
            _ => (false, tr!("{}、一つのチケットの二つのステップに同じ人が当たります", "{}, one person answers two stages of a ticket", at.ja; at.en)),
        };
        let mut d = if error { Diag::error(k.code, s.path, f.line, f.col, msg) } else { Diag::warning(k.code, s.path, f.line, f.col, msg) }.source(s.src);
        for x in notes {
            d = d.note(x);
        }
        diags.push(d.with(crate::diag::Spaced { witness: Some(witness(&names, &k, &f, &days)), ..Default::default() }));
    }
}

/// The JSON's `witness` of a finding: the versions it is found in, its stages (the two of W506), and
/// every requester it comes to, by ID, with the one who made the request for them (a submit proxy)
/// and the person it came to, where the finding says one (where the stage goes up to, who answers
/// both stages).
fn witness(names: &crate::engine::Names, k: &Key, f: &Found2, days: &[String]) -> Json {
    let requesters = f.ids.iter().map(|(r, to)| Json::obj([("id", Json::str(&r.id)), ("made_by", Json::opt_str(r.by.clone())), ("to", Json::opt_str(to.clone()))]));
    Json::obj([
        ("versions", Json::arr(days.iter().map(Json::str))),
        ("stages", Json::arr(k.stage.split('/').map(|x| Json::str(names.stage(x))))),
        ("stages_written", Json::arr(k.stage.split('/').map(Json::str))),
        ("requesters", Json::arr(requesters)),
    ])
}

/// One case of what the shape of the organization does to a stage as the request is written, as the
/// JSON's `facts` lists it (DESIGN 16.2): the versions, the stage and its way as the stage names it,
/// what happens (`up`, `then`, `skipped`, or `late` for a late stage with nowhere to go up to), why
/// and how, the lines of the request it comes from, and every requester, by ID, with the one who
/// made the request for them and the person it came to (where the stage goes, who is late).
#[derive(Clone, Debug)]
pub struct Shaped {
    pub versions: Vec<String>,
    pub stage: String,
    pub way: String,
    pub what: &'static str,
    pub cause: Text,
    pub notes: Vec<Text>,
    pub lines: Vec<usize>,
    pub requesters: Vec<(Requester, Option<String>)>,
}

impl Shaped {
    /// The case as the JSON's `facts` lists it; the cause and the notes spaced and capitalized as a
    /// diagnostic's notes are.
    pub fn to_json(&self, names: &crate::engine::Names, lang: Lang) -> Json {
        let requesters = self.requesters.iter().map(|(r, to)| Json::obj([("id", Json::str(&r.id)), ("made_by", Json::opt_str(r.by.clone())), ("to", Json::opt_str(to.clone()))]));
        Json::obj([
            ("versions", Json::arr(self.versions.iter().map(Json::str))),
            ("stage", Json::str(names.stage(&self.stage))),
            ("stage_written", Json::str(&self.stage)),
            // the way as the stage names it: one of `who`, or what a rule gives (`権限表.decider`)
            ("way", Json::str(names.spec(&self.way))),
            ("way_written", Json::str(&self.way)),
            ("what", Json::str(self.what)),
            ("cause", Json::str(ritsu_base::text::spaced(&self.cause, lang))),
            ("notes", Json::arr(self.notes.iter().map(|n| Json::str(ritsu_base::text::spaced(n, lang))))),
            ("lines", Json::arr(self.lines.iter().map(|l| Json::int(*l as i128)))),
            ("requesters", Json::arr(requesters)),
        ])
    }
}

/// What a case of the shape of the organization does to its stage, by its key.
fn what_of(k: &Key) -> &'static str {
    match k.cause.split(' ').next().unwrap_or_default() {
        _ if k.code == "W507" => "late",
        "up" => "up",
        "then" => "then",
        _ => "skipped",
    }
}

/// What `ok` says under its line of what the shape of the organization does as the request is
/// written (DESIGN 8.6): for how many requesters of each version a stage goes elsewhere or is
/// skipped, for how many a late stage has nowhere to go up to, each line with its cases for the
/// JSON, and the command that shows whose requests they are. The requesters are counted as people,
/// of the people who may submit the request (`people`).
fn facts_of(s: &Scope, org: &Org, shaped: BTreeMap<Key, Found2>, all: &BTreeMap<i64, usize>, people: &BTreeMap<i64, usize>) -> Vec<Fact> {
    if shaped.is_empty() {
        return Vec::new();
    }
    // the people of each version, for the stages that go elsewhere (0) and the late ones (1)
    let mut whom: [BTreeMap<i64, BTreeSet<String>>; 2] = Default::default();
    for (k, f) in &shaped {
        whom[usize::from(k.code == "W507")].entry(k.version).or_default().extend(f.seen.iter().map(|r| r.id.clone()));
    }
    let mut cases: [Vec<Shaped>; 2] = Default::default();
    for (k, f, _, days) in merged(s, org, shaped, all) {
        let what = what_of(&k);
        let cause = tr!("{}。", "{}.", f.cause.ja; f.cause.en);
        cases[usize::from(k.code == "W507")].push(Shaped { versions: days, stage: k.stage, way: k.way, what, cause, notes: f.extra, lines: f.lines.into_iter().collect(), requesters: f.ids });
    }
    let mut out = Vec::new();
    for (i, cs) in cases.into_iter().enumerate() {
        if cs.is_empty() {
            continue;
        }
        let mut stages: Vec<String> = Vec::new();
        for c in &cs {
            if !stages.contains(&c.stage) {
                stages.push(c.stage.clone());
            }
        }
        let shown: Vec<String> = stages.iter().map(|st| format!("`{st}`")).collect();
        let (sja, sen) = (shown.join("、"), shown.join(", "));
        let counts: Vec<(String, usize, usize)> = org.versions.iter().filter_map(|v| whom[i].get(&v.from).map(|rs| (v.day(), rs.len(), people.get(&v.from).copied().unwrap_or(0)))).collect();
        let n = super::of_versions(&counts, None);
        let text = if i == 0 {
            tr!("組織の形で、ステップ（{sja}）がほかへ回るか飛ばされる申請者: {}", "requesters whose stages ({sen}) go elsewhere or are skipped by the shape of the organization: {}", n.ja; n.en)
        } else {
            tr!("組織の形で、期限を過ぎたステップ（{sja}）を上へ回せない申請者: {}", "requesters whose late stages ({sen}) have nowhere to go up to by the shape of the organization: {}", n.ja; n.en)
        };
        out.push(Fact { text, cases: cs });
    }
    // the command named the way the check was run: `ritsu ringi simulate …` under `ritsu ringi`
    // (ritsu's DESIGN 6.2, item 10)
    let line = ritsu_base::cli::as_run(&format!("ringi simulate {} --every-requester", crate::run::shell_word(s.path)));
    out.push(Fact::line(tr!("だれの申請の、どのステップがどこへ回るか: `{line}`", "who they are, and where their stages go: `{line}`")));
    out
}

/// W500 in a file of masters: a field of people whose rows are not checked against an organization,
/// for it reads none, or one with no data.
pub fn masters_people(s: &Scope, diags: &mut Vec<Diag>) {
    let reads = s.uses_of(UseKind::Organization).next().is_some();
    let none = match s.org_rc().as_deref() {
        Some(Data::NoOrg(none)) => Some(none.clone()),
        _ => None,
    };
    if reads && none.is_none() {
        return;
    }
    for m in &s.file.masters {
        if m.data.is_none() {
            continue;
        }
        for f in m.fields.iter().filter(|f| f.ty == TypeExpr::Person) {
            let (mn, n) = (&m.name.text, &f.name.text);
            let d = Diag::warning("W500", s.path, f.line, f.name.span.col, tr!(
                "マスタ `{mn}` の `{n}` の人が組織にいるかを、決められません",
                "Whether the people of `{n}` of the master `{mn}` are in the organization is not decided"
            ))
            .source(s.src);
            let d = match &none {
                Some(none) => d.note(tr!("読む組織について: {}（{}）。", "Of the organization it reads: {} ({}).", none.what.ja, none.why.ja; none.what.en, none.why.en)),
                None => d.note(tr!(
                    "このファイルは組織を読みません。`use organization org from \"…\"` を足すと、行の人を組織のどの版とも比べます（E505）。",
                    "The file reads no organization: with `use organization org from \"…\"`, the people of the rows are checked against every version of it (E505)."
                )),
            };
            diags.push(d);
        }
    }
}
