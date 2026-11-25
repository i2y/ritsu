//! The part of the organization a ticket is given with each event (DESIGN 7.5): of the version in
//! effect at the event's time, the people and the teams the ticket needs, the proxies in effect
//! that day, and the rows of the masters the form chose. The machine reads the organization from
//! here and from nowhere else.

use super::{Machine, Submit, Ticket};
use crate::ast::{Group, How, TypeExpr, UseKind};
use crate::names::Scope;
use crate::org::{Org, ProxyKind, Version};
use ritsu_base::json::Json;
use ritsu_ports::Value;
use std::collections::BTreeSet;

/// A row of a master in effect on the day, by the master (`m.systems`) and its key.
#[derive(Clone, Debug, PartialEq)]
pub struct PartRow {
    pub master: String,
    pub key: String,
    /// The row's fields, by their names as written.
    pub values: Vec<(String, String)>,
}

/// The part of the organization an event is given with.
#[derive(Clone, Debug)]
pub struct OrgPart {
    /// The day the event's time is, in the UTC offset the request reads its days in.
    pub day: i64,
    /// One version, the part of the one in effect on the day; the proxies in effect on the day for
    /// the request; the positions.
    pub org: Org,
    pub rows: Vec<PartRow>,
}

impl OrgPart {
    pub fn v(&self) -> &Version {
        &self.org.versions[0]
    }

    /// The row of the master (`m.systems`) with the key, when one is in effect on the day.
    pub fn row(&self, master: &str, key: &str) -> Option<&PartRow> {
        self.rows.iter().find(|r| r.master == master && r.key == key)
    }

    /// The part as the JSON writes it, a master and its fields by their ASCII names (DESIGN 3.4).
    pub fn to_json(&self, n: &super::Names) -> Json {
        let v = self.v();
        Json::obj([
            ("day", Json::str(crate::lex::date_text(self.day))),
            ("version", Json::str(v.day())),
            ("positions", Json::arr(self.org.positions.iter().map(|(_, a)| Json::str(a)))),
            (
                "teams",
                Json::arr(v.teams.iter().map(|t| {
                    Json::obj([("id", Json::str(&t.id)), ("kind", Json::str(&t.kind)), ("parent", Json::from(t.parent.clone())), ("head", Json::from(t.head.clone()))])
                })),
            ),
            // each with their names, which the machine says in a refusal as the reference does
            ("people", Json::arr(v.people.iter().map(|p| Json::obj([("id", Json::str(&p.id)), ("name", Json::str(&p.name)), ("name_ja", Json::str(&p.name_ja))])))),
            (
                "members",
                Json::arr(v.members.iter().map(|m| {
                    Json::obj([
                        ("person", Json::str(&m.person)),
                        ("team", Json::str(&m.team)),
                        ("position", Json::str(self.org.positions.get(m.position).map(|p| p.1.as_str()).unwrap_or(""))),
                        ("primary", Json::Bool(m.primary)),
                    ])
                })),
            ),
            (
                "proxies",
                Json::arr(self.org.proxies.iter().map(|x| {
                    Json::obj([("person", Json::str(&x.person)), ("proxy", Json::str(&x.proxy)), ("kind", Json::str(if x.kind == ProxyKind::Approve { "approve" } else { "submit" }))])
                })),
            ),
            (
                "rows",
                Json::arr(self.rows.iter().map(|r| {
                    Json::obj([("master", Json::str(n.master(&r.master))), ("key", Json::str(&r.key)), ("values", Json::obj(r.values.iter().map(|(k, x)| (n.master_field(&r.master, k).to_string(), Json::str(x)))))])
                })),
            ),
        ])
    }
}

/// What a part is cut for: the ticket as it is, or the request about to be made, and the people an
/// event names.
pub struct Need<'x> {
    pub ticket: Option<&'x Ticket>,
    pub submit: Option<&'x Submit>,
    /// The one the event is by, and the people it names (an approver added, the two of a
    /// hand-over).
    pub people: Vec<String>,
    /// The values the event gives the form (`resubmit`, `edit`), by their lowered names: the people
    /// and the teams they name, the rows of the masters they choose.
    pub values: Option<&'x ritsu_ports::Values>,
}

impl<'x> Need<'x> {
    /// What the part is cut for when `sub` is about to be made.
    pub fn submit(sub: &'x Submit) -> Need<'x> {
        Need { ticket: None, submit: Some(sub), people: Vec::new(), values: None }
    }

    /// What the part is cut for when the event `ev` comes to the ticket `t`.
    pub fn event(t: &'x Ticket, ev: &'x super::Event) -> Need<'x> {
        let mut people: Vec<String> = ev.by().map(str::to_string).into_iter().collect();
        people.extend(ev.names());
        let values = match ev {
            super::Event::Resubmit { form, .. } => Some(form),
            super::Event::Edit { fields, .. } => Some(fields),
            _ => None,
        };
        Need { ticket: Some(t), submit: None, people, values }
    }
}

/// What every part for a kind has, whatever the ticket and the event: the people and the teams its
/// routes and `submitted by` name, by their IDs (DESIGN 7.5). `types/<name>.json` writes it
/// (`organization.needs`), so that a server cuts a part as this does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Named {
    pub people: BTreeSet<String>,
    /// The teams in the part, with the teams above them.
    pub teams: BTreeSet<String>,
    /// The teams whose every member a ticket may come to.
    pub whole: BTreeSet<String>,
}

impl Named {
    pub fn of(s: &Scope) -> Named {
        let mut n = Named::default();
        // the ways to choose, of the request and of the shared routes it uses
        let mut files = vec![s.file];
        files.extend(s.shared_routes().into_iter().flatten());
        for w in files.iter().flat_map(|f| &f.who) {
            match &w.how {
                How::Person(id, _) => {
                    n.people.insert(id.clone());
                }
                How::Team(t) => {
                    n.whole.insert(t.text.clone());
                }
                _ => {}
            }
        }
        // who may submit: the teams and the people the line names
        if let Some(line) = crate::org::submit::of(s.file) {
            for g in line.groups.iter().chain(line.except) {
                match g {
                    Group::Team(t) | Group::Everyone { within: Some(t), .. } | Group::Position { within: Some(t), .. } => {
                        n.teams.insert(t.text.clone());
                    }
                    Group::Person(id, _) => {
                        n.people.insert(id.clone());
                    }
                    _ => {}
                }
            }
        }
        n
    }
}

impl Machine<'_, '_> {
    /// The part of `org` an event at `now` is given with; None when no version of it is in effect
    /// that day.
    pub fn part(&self, org: &Org, need: &Need, now: i64) -> Option<OrgPart> {
        let day = self.day(now);
        let v = org.in_effect(day)?;
        let s = self.s;
        let request = [s.file.name.text.as_str(), s.file.name.ascii()];
        let Named { mut people, mut teams, mut whole } = Named::of(s);
        people.extend(need.people.iter().cloned());
        let mut forms: Vec<&ritsu_ports::Values> = Vec::new();
        if let Some(t) = need.ticket {
            people.insert(t.requester.clone());
            people.extend(t.made_by.clone());
            forms.push(&t.form);
            for st in &t.stages {
                for p in &st.parts {
                    people.extend(p.approvers.iter().cloned());
                    people.extend(p.answers.iter().flat_map(|a| [a.by.clone(), a.seat.clone()]));
                    whole.extend(p.team.clone());
                }
            }
        }
        if let Some(sub) = need.submit {
            people.insert(sub.requester.clone());
            people.extend(sub.by.clone());
            forms.push(&sub.form);
        }
        forms.extend(need.values);
        // the form: the people and the teams it names, the rows of the masters it chose, the fields
        // in the order the form declares them
        let mut rows = Vec::new();
        let mut add_row = |alias: &str, name: &str, key: &str, people: &mut BTreeSet<String>| {
            let Some(md) = s.masters_data(alias).and_then(|m| m.master(name)) else { return };
            let Some(r) = md.rows.iter().find(|r| r.key == key && r.on(day)) else { return };
            let master = format!("{alias}.{name}");
            if rows.iter().any(|x: &PartRow| x.master == master && x.key == key) {
                return;
            }
            // a person a row names answers a stage that looks them up
            if let Some(mf) = s.ringi(alias, UseKind::Masters).and_then(|f| f.masters.iter().find(|m| m.name.text == name)) {
                for fd in mf.fields.iter().filter(|fd| fd.ty == TypeExpr::Person) {
                    if let Some(id) = md.value(r, &fd.name.text).filter(|x| !x.is_empty()) {
                        people.insert(id.to_string());
                    }
                }
            }
            rows.push(PartRow { master, key: key.to_string(), values: md.fields.iter().cloned().zip(r.values.iter().cloned()).collect() });
        };
        for (form, fd) in forms.iter().flat_map(|form| s.fields().iter().map(move |fd| (form, fd))) {
            let Some((_, val)) = form.iter().find(|(n, _)| n == fd.name.ascii()) else { continue };
            match (&fd.ty, val) {
                (TypeExpr::Person, Value::Str(id)) => {
                    people.insert(id.clone());
                }
                (TypeExpr::Team, Value::Str(id)) => {
                    teams.insert(id.clone());
                }
                (TypeExpr::ChoiceFrom(p), Value::Str(k) | Value::Enum(k)) if p.len() == 2 => add_row(&p[0].text, &p[1].text, k, &mut people),
                _ => {}
            }
        }
        // up from each person and each team, and across the proxies in effect on the day
        let proxies: Vec<_> = org.proxies.iter().filter(|x| request.iter().any(|r| x.on(day, r))).cloned().collect();
        loop {
            let before = (people.len(), teams.len());
            teams.extend(whole.iter().cloned());
            for t in &whole {
                people.extend(v.members(t).map(|m| m.person.clone()));
            }
            for p in people.clone() {
                teams.extend(v.memberships(&p).map(|m| m.team.clone()));
            }
            for t in teams.clone() {
                for u in v.up(&t) {
                    teams.insert(u.id.clone());
                    people.extend(u.head.clone());
                }
            }
            for x in &proxies {
                if people.contains(&x.person) {
                    people.insert(x.proxy.clone());
                }
            }
            if (people.len(), teams.len()) == before {
                break;
            }
        }
        let part = v.part(&people, &teams);
        let proxies = proxies.into_iter().filter(|x| people.contains(&x.person)).collect();
        let org = Org { written: org.written.clone(), dir: org.dir.clone(), versions: vec![part], days: org.days.clone(), proxies, proxies_file: org.proxies_file.clone(), positions: org.positions.clone(), at: None };
        Some(OrgPart { day, org, rows })
    }
}
