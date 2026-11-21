//! Who answers a stage, for a requester, in a version of the organization (DESIGN 5.2, 5.4, 5.5,
//! 8.5): each way to choose an approver of the request, solved against the version's teams, people
//! and members, and where a stage with nobody goes up to.
//!
//! - `manager of requester`: the head of the requester's primary team; when it has none, or it is
//!   the requester, the head of the team above, and so on up.
//! - `head of requester.<kind>`: from the primary team up, the first team of that kind, and its
//!   head; nobody when its head is empty (or is the requester and self approval is not allowed),
//!   or when no team of that kind is above the requester.
//! - `nearest <position> above requester`: from the primary team up, the first head, the requester
//!   left out, whose position in their team is that one or higher.
//! - `team <id>`: every member of the team (its members of any kind, primary or not).
//! - `person "<id>"`: that person, when the version has them.
//! - `lookup m.<master>.<field> by <field>`: the person of each row of the master in effect in the
//!   version's days; each row is a case of its own.
//! - `agent <flow>, else <way>`: the way after `else`, which answers when the flow doubts.
//! - `chosen by …`: a person fills it in when the ticket runs; the organization does not decide it.
//!
//! With self approval not allowed (the default), the requester, and the one who made the request in
//! their name (a submit proxy), are left out of every way. A way that comes to nobody goes up with
//! `when empty escalate`: to the first head above the team it stopped at who is not left out; from
//! a person, above their primary team; from a team, above the team; when the way found no team at
//! all (no head above the requester, no team of the kind), from the requester's primary team. A
//! person or a team the version does not have cannot go up (E503). Past the top of the organization
//! there is nobody to go up to: the stage waits (E502), unless it says `when empty escalate, then
//! skip`, and is skipped (W502).

use super::{Org, Version};
use crate::ast::{File, How, UseKind, Who};
use crate::names::Scope;
use ritsu_base::text::Text;

/// The one a request is made by: the requester, and the proxy who made it in their name, if one
/// did (DESIGN 5.8).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Requester {
    pub id: String,
    pub by: Option<String>,
}

impl Requester {
    /// The requester as a list names them: `e1203 (Staff 3)`, `e1203 (Staff 3) [made by e1201
    /// (Staff 1)]`, in brackets of its own so that a list of them reads.
    pub fn named(&self, v: &Version) -> Text {
        let who = v.who(&self.id);
        match &self.by {
            None => who,
            Some(b) => {
                let by = v.who(b);
                tr!("{}［{}が代理で申請］", "{} [made by {}]", who.ja, super::ja_word(&by.ja); who.en, by.en)
            }
        }
    }
}

/// What solving a way to choose comes to, besides people.
#[derive(Clone, Debug, PartialEq)]
pub enum Why {
    /// `person "e9001"` (or a master's row) names someone the version does not have.
    PersonNotIn(String),
    /// `team legal` names a team the version does not have.
    TeamNotIn(String),
    /// The team the way stops at has no head.
    HeadEmpty(String),
    /// The way comes to the requester (or the one who made the request in their name) alone, and
    /// self approval is not allowed.
    OnlyRequester(String),
    /// No team of that kind is above the requester.
    NoTeamOfKind(String),
    /// No head above the requester (of that position, for `nearest`).
    NoneAbove,
    /// The team has no member.
    NoMembers(String),
}

/// Where a stage with nobody goes up from.
#[derive(Clone, Debug, PartialEq)]
pub enum Up {
    /// From this team's head, and up.
    At(String),
    /// From the head of the team above this one, and up.
    Above(String),
    /// It cannot go up: the person or the team is not in the version.
    Cannot,
}

/// A way to choose, solved.
#[derive(Clone, Debug, PartialEq)]
pub enum Found {
    /// These people answer.
    People(Vec<String>),
    /// Nobody, why, and where it goes up from.
    Nobody(Why, Up),
    /// The organization does not decide it: a person chooses when the ticket runs, or the master
    /// has no data.
    Open,
}

/// One case of a way to choose: the row of a master it looks up, or the value of a rule's output a
/// stage names; None for a way that has one.
#[derive(Clone, Debug)]
pub struct Case {
    pub label: Option<Text>,
    /// The way to choose that answers in this case (the way after `else` for an agent, the value
    /// of a rule's output).
    pub way: String,
    pub found: Found,
    /// Whether an agent answers the stage first, and `way` only when it doubts (`agent …, else
    /// <way>`).
    pub agent: bool,
    /// The key of the master's row it looks up, for a lookup.
    pub row: Option<String>,
}

/// What the ways read: the version, the requester, whether self approval is allowed, and the
/// request's scope (for its masters and the ways of a shared route).
pub struct Solver<'s, 'a> {
    pub s: &'s Scope<'a>,
    pub org: &'s Org,
    pub v: &'s Version,
    pub requester: &'s Requester,
    pub self_approval: bool,
}

impl<'s, 'a> Solver<'s, 'a> {
    /// Whether the person is left out of every way: the requester or the one who made the request,
    /// with self approval not allowed.
    pub fn left_out(&self, id: &str) -> bool {
        !self.self_approval && (self.requester.id == id || self.requester.by.as_deref() == Some(id))
    }

    /// The requester's primary team.
    fn primary(&self) -> Option<&'s str> {
        self.v.primary(&self.requester.id).map(|m| m.team.as_str())
    }

    /// The way named `name`, in the file that declares it (`f`: the request, or a shared route).
    pub fn who<'f>(f: &'f File, name: &str) -> Option<&'f Who> {
        f.who.iter().find(|w| w.name.text == name)
    }

    /// The cases of the way `name` declared in `f`.
    pub fn way(&self, f: &File, name: &str) -> Vec<Case> {
        self.way_depth(f, name, 0)
    }

    fn way_depth(&self, f: &File, name: &str, depth: usize) -> Vec<Case> {
        let Some(w) = Self::who(f, name) else { return vec![Case { label: None, way: name.to_string(), found: Found::Open, agent: false, row: None }] };
        let one = |found: Found| vec![Case { label: None, way: name.to_string(), found, agent: false, row: None }];
        match &w.how {
            How::Person(id, _) => one(self.person(id)),
            How::Team(t) => one(self.team(&t.text)),
            How::ManagerOf => one(self.manager()),
            How::HeadOf(kind) => one(self.head_of(&kind.text)),
            How::Nearest(pos) => one(self.nearest(&pos.text)),
            How::ChosenBy { .. } => one(Found::Open),
            How::Lookup { path, .. } => self.lookup(name, path),
            How::Agent { otherwise, .. } if depth < 4 => self.way_depth(f, &otherwise.text, depth + 1).into_iter().map(|c| Case { agent: true, ..c }).collect(),
            How::Agent { .. } => vec![Case { label: None, way: name.to_string(), found: Found::Open, agent: true, row: None }],
        }
    }

    pub fn person(&self, id: &str) -> Found {
        if self.v.person(id).is_none() {
            return Found::Nobody(Why::PersonNotIn(id.to_string()), Up::Cannot);
        }
        if self.left_out(id) {
            let from = self.v.primary(id).map(|m| Up::At(m.team.clone())).unwrap_or(Up::Cannot);
            return Found::Nobody(Why::OnlyRequester(id.to_string()), from);
        }
        Found::People(vec![id.to_string()])
    }

    pub fn team(&self, id: &str) -> Found {
        if self.v.team(id).is_none() {
            return Found::Nobody(Why::TeamNotIn(id.to_string()), Up::Cannot);
        }
        let all: Vec<String> = self.v.members(id).map(|m| m.person.clone()).collect();
        if all.is_empty() {
            return Found::Nobody(Why::NoMembers(id.to_string()), Up::Above(id.to_string()));
        }
        let people: Vec<String> = all.iter().filter(|p| !self.left_out(p)).cloned().collect();
        if people.is_empty() {
            return Found::Nobody(Why::OnlyRequester(all[0].clone()), Up::Above(id.to_string()));
        }
        Found::People(people)
    }

    pub fn manager(&self) -> Found {
        let Some(p) = self.primary() else { return Found::Nobody(Why::NoneAbove, Up::Cannot) };
        for t in self.v.up(p) {
            if let Some(h) = &t.head
                && h != &self.requester.id
                && !self.left_out(h)
            {
                return Found::People(vec![h.clone()]);
            }
        }
        Found::Nobody(Why::NoneAbove, Up::Cannot)
    }

    pub fn head_of(&self, kind: &str) -> Found {
        let Some(p) = self.primary() else { return Found::Nobody(Why::NoneAbove, Up::Cannot) };
        let Some(t) = self.v.up(p).into_iter().find(|t| t.kind == kind || self.kind_ascii(&t.kind) == kind) else {
            return Found::Nobody(Why::NoTeamOfKind(kind.to_string()), Up::At(p.to_string()));
        };
        match &t.head {
            None => Found::Nobody(Why::HeadEmpty(t.id.clone()), Up::Above(t.id.clone())),
            Some(h) if self.left_out(h) => Found::Nobody(Why::OnlyRequester(h.clone()), Up::Above(t.id.clone())),
            Some(h) => Found::People(vec![h.clone()]),
        }
    }

    /// A kind of team by its ASCII name, as a way may write it.
    fn kind_ascii(&self, kind: &str) -> String {
        kind.to_string()
    }

    pub fn nearest(&self, position: &str) -> Found {
        let Some(p) = self.primary() else { return Found::Nobody(Why::NoneAbove, Up::Cannot) };
        let Some(want) = self.org.rank(position) else { return Found::Open };
        for t in self.v.up(p) {
            let Some(h) = &t.head else { continue };
            if h == &self.requester.id || self.left_out(h) {
                continue;
            }
            if self.v.position_in(h, &t.id).is_some_and(|r| r >= want) {
                return Found::People(vec![h.clone()]);
            }
        }
        Found::Nobody(Why::NoneAbove, Up::Cannot)
    }

    /// `lookup m.<master>.<field> by …`: a case for each row of the master in effect on some day
    /// of the version.
    fn lookup(&self, name: &str, path: &[crate::ast::Word]) -> Vec<Case> {
        let open = vec![Case { label: None, way: name.to_string(), found: Found::Open, agent: false, row: None }];
        if path.len() != 3 {
            return open;
        }
        let Some(md) = self.s.masters_data(&path[0].text).and_then(|m| m.master(&path[1].text)) else { return open };
        let (a, b) = self.org.period(self.v);
        let mut out = Vec::new();
        for r in &md.rows {
            let meets = r.to.is_none_or(|t| t >= a) && b.is_none_or(|e| r.from.is_none_or(|f| f <= e));
            if !meets {
                continue;
            }
            let Some(id) = md.value(r, &path[2].text).filter(|x| !x.is_empty()) else { continue };
            let (m, k) = (&md.name, &r.key);
            out.push(Case { label: Some(tr!("マスタ `{m}` の行 `{k}`", "the row `{k}` of the master `{m}`")), way: name.to_string(), found: self.person(id), agent: false, row: Some(k.clone()) });
        }
        if out.is_empty() { open } else { out }
    }

    /// Where a stage with nobody goes up to: the first head above who is not left out; None past
    /// the top.
    pub fn up(&self, from: &Up) -> Option<String> {
        let chain = match from {
            Up::Cannot => return None,
            Up::At(t) => self.v.up(t),
            Up::Above(t) => self.v.up(t).into_iter().skip(1).collect(),
        };
        chain.into_iter().find_map(|t| t.head.clone().filter(|h| !self.left_out(h)))
    }
}

/// The ways to choose a stage that names a rule's output (`authority.decider`) can come to: the
/// way of each value of the output's enum, by the name `who` declares it with.
pub fn rule_ways(s: &Scope, p: &[crate::ast::Word]) -> Vec<String> {
    if p.len() != 2 {
        return Vec::new();
    }
    let Some(facts) = s.rule_facts(&p[0].text) else { return Vec::new() };
    let Some(o) = facts.outputs.iter().find(|o| o.name == p[1].text || o.alias == p[1].text) else { return Vec::new() };
    let ritsu_ports::ColumnType::Enum { name: en, .. } = &o.ty else { return Vec::new() };
    let Some(e) = facts.enums.iter().find(|e| &e.name == en) else { return Vec::new() };
    let all = s.who_all();
    e.values
        .iter()
        .filter_map(|v| all.iter().find(|w| w.who.name.text == v.public || w.who.name.ascii() == v.public || w.who.name.text == v.name).map(|w| w.who.name.text.clone()))
        .collect()
}

/// Whether a `use` of the request reads masters (what a lookup reads).
pub fn reads_masters(s: &Scope) -> bool {
    s.uses_of(UseKind::Masters).next().is_some()
}
