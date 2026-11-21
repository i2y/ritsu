//! Who may submit a request (DESIGN 5.13): the `submitted by` line under `ticket`, read against a
//! version of the organization. A person may submit when one of the groups before `except` has them
//! and none after it does; a request made for someone (a submit proxy) is theirs, and the proxy is
//! not looked at. The check lists the requesters who may submit alone, says a team or a person the
//! line names that a version does not have (E508), and a version no one may submit in (E509).

use super::{Org, Version};
use crate::ast::{File, Group, Span, TicketLine};
use ritsu_base::text::Text;

/// A request's `submitted by` line.
pub struct Line<'f> {
    pub groups: &'f [Group],
    pub except: &'f [Group],
    pub line: usize,
    pub col: usize,
}

/// The `submitted by` line of the request `f`, when it has one.
pub fn of(f: &File) -> Option<Line<'_>> {
    f.ticket.as_ref()?.lines.iter().find_map(|l| match l {
        TicketLine::SubmittedBy { groups, except, line, col } => Some(Line { groups, except, line: *line, col: *col }),
        _ => None,
    })
}

/// A team or a person a `submitted by` line names, which a version does not have.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Missing {
    Team(String),
    Person(String),
}

impl Line<'_> {
    /// The line as it is written after `submitted by`: `everyone except team board`.
    pub fn shown(&self) -> String {
        Group::line_shown(self.groups, self.except)
    }

    /// Whether `person` may submit the request in the version `v`.
    pub fn may(&self, org: &Org, v: &Version, person: &str) -> bool {
        v.person(person).is_some() && self.groups.iter().any(|g| has(org, v, g, person)) && !self.except.iter().any(|g| has(org, v, g, person))
    }

    /// Why `person`, who may not, does not submit the request of the file `path`: what `simulate`
    /// and a ticket's `submit` both say. The request is named by the file's name, as the code
    /// generated for it names it (the glue's `submitted_by`), however the path is written: the
    /// vectors are the same from any directory, and hold no path of the machine they are made on.
    pub fn refused(&self, v: &Version, person: &str, path: &str) -> Text {
        let (who, d, shown) = (v.who(person), v.day(), self.shown());
        let path = std::path::Path::new(path).file_name().map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned());
        tr!(
            "{}は、{d} からの組織では {path} を出せません（`submitted by {shown}`）",
            "{} may not submit {path} in the organization from {d} (`submitted by {shown}`)",
            super::ja_word(&who.ja);
            who.en
        )
    }

    /// What the line names that the version does not have, each where it is written, with the group
    /// it is in as written.
    pub fn missing(&self, v: &Version) -> Vec<(Span, Missing, String)> {
        let mut out = Vec::new();
        for g in self.groups.iter().chain(self.except) {
            let team = |w: &crate::ast::Word| (v.team(&w.text).is_none()).then(|| (w.span, Missing::Team(w.text.clone()), g.shown()));
            match g {
                Group::Everyone { within: Some(t), .. } | Group::Position { within: Some(t), .. } | Group::Team(t) => out.extend(team(t)),
                Group::Person(id, span) if v.person(id).is_none() => out.push((*span, Missing::Person(id.clone()), g.shown())),
                _ => {}
            }
        }
        out
    }
}

/// Whether the group `g` has `person` in the version `v`.
fn has(org: &Org, v: &Version, g: &Group, person: &str) -> bool {
    match g {
        Group::Everyone { within, .. } => within.as_ref().is_none_or(|t| under(v, person, &t.text)),
        Group::Team(t) => v.memberships(person).any(|m| m.team == t.text),
        Group::Position { position, higher, within } => {
            let (Some(rank), Some(m)) = (org.rank(&position.text), v.primary(person)) else { return false };
            (m.position == rank || (*higher && m.position > rank)) && within.as_ref().is_none_or(|t| under(v, person, &t.text))
        }
        Group::Person(id, _) => id == person,
    }
}

/// Whether the person's primary team is `team`, or a team below it.
fn under(v: &Version, person: &str, team: &str) -> bool {
    v.primary(person).is_some_and(|m| v.up(&m.team).iter().any(|t| t.id == team))
}

/// What `ok` says of who may submit: `submitted by 19 of 24 people in the organization from
/// 2026-04-01, and 18 of 23 from 2027-04-01 (`everyone except team board`)`. `counts` holds each
/// version's day, how many may submit, and how many people it has.
pub fn fact(shown: &str, counts: &[(String, usize, usize)]) -> Text {
    let n = super::of_versions(counts, Some(("person", "people")));
    tr!("出せる人: {}（`{shown}`）", "submitted by {} (`{shown}`)", n.ja; n.en)
}
