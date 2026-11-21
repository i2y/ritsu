//! The names the JSON takes (DESIGN 3.4): a stage, a way to choose, a book, a flow and a master are
//! written to JSON by their ASCII names, the ones in parentheses after a name in Japanese
//! (`上長(manager)` is `manager`). The machine holds the names as the request writes them, which
//! is how a scenario and `run` show them; a ticket, an event, a thing to do and a part of the
//! organization are written to JSON through these, so that a request in Japanese names and its
//! English draft give the same JSON, and generated code reads one set of names.

use crate::ast::{Name, UseKind};
use crate::names::Scope;

/// The ASCII names of a request's stages, ways, `use` aliases and masters, by the names as written.
#[derive(Clone, Debug, Default)]
pub struct Names {
    stages: Vec<(String, String)>,
    ways: Vec<(String, String)>,
    uses: Vec<(String, String)>,
    /// `alias.master` as written, in ASCII names, and its fields.
    masters: Vec<(String, String, Vec<(String, String)>)>,
    /// The fields of the form, a field of rows as `rows.field`.
    fields: Vec<(String, String)>,
    /// The kinds of team of the organization the request reads.
    kinds: Vec<(String, String)>,
}

fn pair(n: &Name) -> (String, String) {
    (n.text.clone(), n.ascii().to_string())
}

fn found<'x>(pairs: &'x [(String, String)], name: &'x str) -> &'x str {
    pairs.iter().find(|(w, _)| w == name).map(|(_, a)| a.as_str()).unwrap_or(name)
}

impl Names {
    pub fn of(s: &Scope) -> Names {
        let stages = s.stages().unwrap_or_default().into_iter().map(pair).collect();
        let ways = s.who_all().iter().map(|w| pair(&w.who.name)).collect();
        let uses: Vec<(String, String)> = s.file.uses.iter().filter_map(|u| u.alias.as_ref()).map(pair).collect();
        let mut masters = Vec::new();
        for u in s.uses_of(UseKind::Masters) {
            let Some(alias) = &u.alias else { continue };
            let Some(f) = s.ringi(&alias.text, UseKind::Masters) else { continue };
            for m in &f.masters {
                masters.push((format!("{}.{}", alias.text, m.name.text), format!("{}.{}", alias.ascii(), m.name.ascii()), m.fields.iter().map(|fd| pair(&fd.name)).collect()));
            }
        }
        let mut fields = Vec::new();
        for f in s.fields() {
            fields.push(pair(&f.name));
            fields.extend(f.children.iter().map(|c| (format!("{}.{}", f.name.text, c.name.text), format!("{}.{}", f.name.ascii(), c.name.ascii()))));
        }
        let mut kinds = Vec::new();
        for u in s.uses_of(UseKind::Organization) {
            if let Some(f) = u.alias.as_ref().and_then(|a| s.ringi(&a.text, UseKind::Organization)) {
                kinds.extend(f.team_kinds.iter().map(|k| pair(&k.name)));
            }
        }
        Names { stages, ways, uses, masters, fields, kinds }
    }

    /// A field of the form (`amount`, `items.price`).
    pub fn field<'x>(&'x self, name: &'x str) -> &'x str {
        found(&self.fields, name)
    }

    /// A kind of team of the organization.
    pub fn team_kind<'x>(&'x self, name: &'x str) -> &'x str {
        found(&self.kinds, name)
    }

    /// What a stage's cell or line writes (`部長 and 経理`, `at least 60% of 取締役会`,
    /// `権限.decider`), its names in ASCII: each word that is a way to choose, and the alias before
    /// a dot.
    pub fn spec(&self, written: &str) -> String {
        let word = |w: &str| match w.split_once('.') {
            Some((alias, rest)) => format!("{}.{rest}", self.used(alias)),
            None => self.way(self.used(w)).to_string(),
        };
        written.split(' ').map(word).collect::<Vec<_>>().join(" ")
    }

    pub fn stage<'x>(&'x self, name: &'x str) -> &'x str {
        found(&self.stages, name)
    }

    pub fn way<'x>(&'x self, name: &'x str) -> &'x str {
        found(&self.ways, name)
    }

    /// The alias of a `use`: a book, a flow.
    pub fn used<'x>(&'x self, alias: &'x str) -> &'x str {
        found(&self.uses, alias)
    }

    /// A master (`alias.master`).
    pub fn master<'x>(&'x self, name: &'x str) -> &'x str {
        self.masters.iter().find(|(w, ..)| w == name).map(|(_, a, _)| a.as_str()).unwrap_or(name)
    }

    /// A field of a master (`alias.master`).
    pub fn master_field<'x>(&'x self, master: &str, field: &'x str) -> &'x str {
        self.masters.iter().find(|(w, ..)| w == master).map(|(.., fs)| found(fs, field)).unwrap_or(field)
    }
}
