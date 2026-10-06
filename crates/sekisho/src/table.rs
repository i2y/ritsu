//! The table of an action, rows merged (DESIGN 7): one row for each combination the walk counts,
//! with what Cedar answers and the policies that decide it; then rows that agree on the answer and
//! the policies and differ in one column only are merged into one, the column holding the set of
//! their values, until nothing more merges. A set of every value a column takes is shown as `any`,
//! of every value but one as `not …`.
//!
//! The columns are the principal's type, each role walked (held, after `includes`), the attributes
//! of the principal, the resource's type when the action takes more than one, the attributes of
//! the resource, the relations, the inputs and the computed values: what the policies read, in
//! the order the walk reads them. `-` is a column that does not apply to a row (the roles of a
//! workflow, a value the principal's type does not compute). A value of a rule's enum is shown by
//! the name the `.rule` writes (`上限まで` for `上限まで(within_limit)`), as the `.gate` beside the
//! table writes it; the string Cedar is given (the alias) is shown where the page puts the policies.

use crate::cells;
use crate::model::*;
use crate::walk::{Slot, Space, Val, bit};
use ritsu_base::text::{Lang, Text};
use ritsu_units::Unit;
use std::collections::{BTreeMap, BTreeSet};

/// One column.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Col {
    Principal,
    Role(usize),
    Workflow,
    PrincipalAttr(String),
    Resource,
    ResourceAttr(String),
    Relation(String, String),
    /// The principal is in what the term points to.
    Member(String),
    Input(usize),
    Computed(usize),
}

/// A table: its columns, and its rows, those that allow first.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub columns: Vec<Text>,
    pub rows: Vec<Row>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub cells: Vec<Text>,
    pub allow: bool,
    /// The policies that decide it, by their index in the file.
    pub policies: Vec<usize>,
}

/// The value of a column, as a cell shows it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Cell {
    /// The column does not apply.
    Dash,
    Yes,
    No,
    /// A value, by its order among the column's values, and how it is shown.
    Value(usize, String),
    /// A cell of a number, by its low end.
    Num(i128, String),
    Absent,
}

impl Cell {
    fn text(&self) -> Text {
        match self {
            Cell::Dash => Text::same("-"),
            Cell::Yes => ritsu_base::tr!("はい", "yes"),
            Cell::No => ritsu_base::tr!("いいえ", "no"),
            Cell::Value(_, s) | Cell::Num(_, s) => Text::same(s.clone()),
            Cell::Absent => ritsu_base::tr!("無い", "absent"),
        }
    }
}

/// A row while rows are merged: each column's set of values, the answer, the deciding policies.
type Merged = (Vec<BTreeSet<Cell>>, bool, Vec<usize>);

/// The table of the action of a space, rows merged.
pub fn table(g: &Gate, s: &Space) -> Table {
    let a = &g.actions[s.action];
    let several_resources = a.resources.len() > 1;
    // the columns, in the order the walk reads them: each with a key to sort by
    let mut keyed: Vec<((u8, usize), Col)> = Vec::new();
    let mut add = |key: (u8, usize), c: Col| {
        if !keyed.iter().any(|(_, x)| *x == c) {
            keyed.push((key, c));
        }
    };
    for f in &s.frames {
        add((0, 0), Col::Principal);
        let pt = &g.types[f.principal];
        for r in &pt.roles {
            if f.walked_roles & bit(*r) != 0 || g.closure(*r).iter().any(|x| f.walked_roles & bit(*x) != 0) {
                for x in g.closure(*r) {
                    add((1, x), Col::Role(x));
                }
            }
        }
        if several_resources {
            add((4, 0), Col::Resource);
        }
        for (at, sl) in f.slots.iter().enumerate() {
            if !f.factors.iter().any(|x| x.places.contains(&at)) {
                continue;
            }
            match sl {
                Slot::Workflow => add((2, 0), Col::Workflow),
                Slot::Attr(Owner::Principal, i) => add((3, *i), Col::PrincipalAttr(pt.attrs[*i].named.name.clone())),
                Slot::Attr(Owner::Resource, i) => add((5, *i), Col::ResourceAttr(g.types[f.resource].attrs[*i].named.name.clone())),
                Slot::Input(i) => add((7, *i), Col::Input(*i)),
                Slot::Computed(i) => add((8, *i), Col::Computed(*i)),
                Slot::Relation(t) => {
                    if let Some(terms) = f.terms.get(&at) {
                        for i in 0..terms.len() {
                            for j in i + 1..terms.len() {
                                add((6, *t), Col::Relation(term(&terms[i]), term(&terms[j])));
                            }
                        }
                        for &i in f.members.get(&at).map(Vec::as_slice).unwrap_or(&[]) {
                            add((6, *t), Col::Member(term(&terms[i])));
                        }
                    }
                }
                Slot::Roles => {}
            }
        }
        // a computed value the frame fixes as absent is still a column of the action
        for (at, _) in &f.fixed {
            if let Slot::Computed(i) = f.slots[*at] {
                add((8, i), Col::Computed(i));
            }
        }
    }
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    let cols: Vec<Col> = keyed.into_iter().map(|(_, c)| c).collect();
    // one row a combination, kept once
    let mut rows: BTreeSet<(Vec<Cell>, bool, Vec<usize>)> = BTreeSet::new();
    s.each(|c| {
        let f = &s.frames[c.frame];
        let pt = &g.types[f.principal];
        let cells: Vec<Cell> = cols
            .iter()
            .map(|col| match col {
                Col::Principal => Cell::Value(f.principal, pt.named.name.clone()),
                Col::Role(r) => match &c.env[0] {
                    Val::Roles { effective, .. } if !pt.roles.is_empty() => {
                        if effective & bit(*r) != 0 {
                            Cell::Yes
                        } else {
                            Cell::No
                        }
                    }
                    _ => Cell::Dash,
                },
                Col::Workflow => match f.place(&Slot::Workflow).map(|at| &c.env[at]) {
                    Some(Val::Workflow(w)) => Cell::Value(*w, g.workflows[*w].named.name.clone()),
                    _ => Cell::Dash,
                },
                Col::Resource => Cell::Value(f.resource, g.types[f.resource].named.name.clone()),
                Col::PrincipalAttr(name) => attr_cell(g, s, f, c.env, Owner::Principal, name),
                Col::ResourceAttr(name) => attr_cell(g, s, f, c.env, Owner::Resource, name),
                Col::Relation(x, y) => {
                    let found = f.terms.iter().find_map(|(at, terms)| {
                        let i = terms.iter().position(|t| term(t) == *x)?;
                        let j = terms.iter().position(|t| term(t) == *y)?;
                        Some((*at, i, j))
                    });
                    match found.map(|(at, i, j)| (&c.env[at], i, j)) {
                        Some((Val::Relation(r), i, j)) => {
                            if matches!((r.blocks[i], r.blocks[j]), (Some(p), Some(q)) if p == q) {
                                Cell::Yes
                            } else {
                                Cell::No
                            }
                        }
                        _ => Cell::Dash,
                    }
                }
                Col::Member(x) => {
                    let found = f.terms.iter().find_map(|(at, terms)| terms.iter().position(|t| term(t) == *x).map(|i| (*at, i)));
                    match found.map(|(at, i)| (&c.env[at], i)) {
                        Some((Val::Relation(r), i)) => {
                            if r.member(i) {
                                Cell::Yes
                            } else {
                                Cell::No
                            }
                        }
                        _ => Cell::Dash,
                    }
                }
                Col::Input(i) => match f.place(&Slot::Input(*i)) {
                    Some(at) => value_cell(g, s, f, c.env, at),
                    None => Cell::Dash,
                },
                Col::Computed(i) => match f.place(&Slot::Computed(*i)) {
                    Some(at) if c.env[at] != Val::Absent => value_cell(g, s, f, c.env, at),
                    _ => Cell::Dash,
                },
            })
            .collect();
        let policies: Vec<usize> = c.decision.determining.iter().map(|&k| s.policies[k]).collect();
        rows.insert((cells, c.decision.allow, policies));
    });
    // the values each column takes, `-` aside
    let domains: Vec<BTreeSet<Cell>> = (0..cols.len()).map(|k| rows.iter().map(|r| r.0[k].clone()).filter(|c| *c != Cell::Dash).collect()).collect();
    // merge: rows alike but in one column become one, the column a set
    let mut merged: Vec<Merged> = rows.into_iter().map(|(cells, allow, p)| (cells.into_iter().map(|c| BTreeSet::from([c])).collect(), allow, p)).collect();
    loop {
        let mut changed = false;
        for k in 0..cols.len() {
            let mut groups: BTreeMap<Merged, BTreeSet<Cell>> = BTreeMap::new();
            let mut sizes: BTreeMap<Merged, usize> = BTreeMap::new();
            for (cells, allow, p) in &merged {
                let mut rest = cells.clone();
                let mine = rest.remove(k);
                let key = (rest, *allow, p.clone());
                groups.entry(key.clone()).or_default().extend(mine);
                *sizes.entry(key).or_default() += 1;
            }
            if sizes.values().any(|n| *n > 1) {
                changed = true;
            }
            merged = groups
                .into_iter()
                .map(|((mut rest, allow, p), set)| {
                    rest.insert(k, set);
                    (rest, allow, p)
                })
                .collect();
        }
        if !changed {
            break;
        }
    }
    // the rows that allow first, by their policies in the order of the file; then those a forbid
    // denies, by the forbids; then those no permit applies to
    merged.sort_by(|x, y| {
        let rank = |r: &Merged| (!r.1, r.2.is_empty(), r.2.clone());
        rank(x).cmp(&rank(y)).then_with(|| x.0.cmp(&y.0))
    });
    let shared = |n: &String| cols.contains(&Col::PrincipalAttr(n.clone())) && cols.contains(&Col::ResourceAttr(n.clone()));
    let columns = cols
        .iter()
        .map(|c| match c {
            Col::PrincipalAttr(n) if shared(n) => Text::same(format!("principal.{n}")),
            Col::ResourceAttr(n) if shared(n) => Text::same(format!("resource.{n}")),
            c => col_text(g, a, c),
        })
        .collect();
    let rows = merged
        .into_iter()
        .map(|(sets, allow, policies)| Row { cells: sets.iter().zip(&domains).map(|(set, dom)| shown(set, dom)).collect(), allow, policies })
        .collect();
    Table { columns, rows }
}

fn term(t: &Term) -> String {
    match t {
        Term::Principal => "principal".into(),
        Term::Attr(o, n) => format!("{}.{n}", o.word()),
    }
}

fn col_text(g: &Gate, a: &Action, c: &Col) -> Text {
    match c {
        Col::Principal => Text::same("principal"),
        Col::Role(r) => Text::same(g.roles[*r].named.name.clone()),
        Col::Workflow => ritsu_base::tr!("ワークフロー", "workflow"),
        Col::PrincipalAttr(n) => Text::same(n.clone()),
        Col::Resource => Text::same("resource"),
        Col::ResourceAttr(n) => Text::same(n.clone()),
        Col::Relation(x, y) => Text::same(format!("{x} is {y}")),
        Col::Member(x) => Text::same(format!("principal in {x}")),
        Col::Input(i) => Text::same(a.inputs[*i].named.name.clone()),
        Col::Computed(i) => Text::same(a.computed[*i].named.name.clone()),
    }
}

fn attr_cell(g: &Gate, s: &Space, f: &crate::walk::Frame, env: &[Val], o: Owner, name: &str) -> Cell {
    let t = g.owner_type(o, f.principal, f.resource);
    match t.attr(name).and_then(|(i, _)| f.place(&Slot::Attr(o, i))) {
        Some(at) => value_cell(g, s, f, env, at),
        None => Cell::Dash,
    }
}

fn value_cell(g: &Gate, s: &Space, f: &crate::walk::Frame, env: &[Val], at: usize) -> Cell {
    let a = &g.actions[s.action];
    let field = match &f.slots[at] {
        Slot::Attr(o, i) => Some(&g.owner_type(*o, f.principal, f.resource).attrs[*i]),
        Slot::Input(i) => Some(&a.inputs[*i]),
        _ => None,
    };
    match &env[at] {
        Val::Bool(true) => Cell::Yes,
        Val::Bool(false) => Cell::No,
        Val::Absent => Cell::Absent,
        Val::Enum(v) => {
            let name = match (&f.slots[at], field.map(|x| &x.ty)) {
                (_, Some(FieldType::Enum(e))) => g.enums[*e].values[*v].name.clone(),
                // a value of a rule's enum, by the name the `.rule` writes, as the `.gate` reads it
                (Slot::Computed(i), _) => s.names.get(i).and_then(|vs| vs.get(*v)).map(|n| n.name.clone()).unwrap_or_default(),
                _ => v.to_string(),
            };
            Cell::Value(*v, name)
        }
        Val::Cell(c) => {
            let (unit, lo, hi) = match field.map(|x| &x.ty) {
                Some(FieldType::Num { unit, lo, hi, .. }) => (unit.clone(), *lo, *hi),
                _ => (Unit::number(), c.lo, c.hi),
            };
            Cell::Num(c.lo, cell_label(*c, lo, hi, &unit))
        }
        _ => Cell::Dash,
    }
}

/// A cell of a number as a column shows it: `<=50GBP` for the first of several, `>50GBP` for the
/// last, both ends for one in the middle, the value for one of a single value.
fn cell_label(c: cells::Cell, lo: i128, hi: i128, unit: &Unit) -> String {
    if c.lo == c.hi {
        cells::show(c.lo, unit)
    } else if c.lo == lo && c.hi == hi {
        format!("{}..{}", cells::show(c.lo, unit), cells::show(c.hi, unit))
    } else if c.lo == lo {
        format!("<={}", cells::show(c.hi, unit))
    } else if c.hi == hi {
        format!(">{}", cells::show(c.lo - 1, unit))
    } else {
        format!("{}..{}", cells::show(c.lo, unit), cells::show(c.hi, unit))
    }
}

/// A set of a column's values as a cell shows it.
fn shown(set: &BTreeSet<Cell>, domain: &BTreeSet<Cell>) -> Text {
    let set: BTreeSet<Cell> = set.iter().filter(|c| **c != Cell::Dash).cloned().collect();
    if set.is_empty() {
        return Text::same("-");
    }
    if set.len() == 1 {
        return set.iter().next().unwrap().text();
    }
    if set == *domain {
        return ritsu_base::tr!("どれでも", "any");
    }
    let rest: Vec<&Cell> = domain.iter().filter(|c| !set.contains(c)).collect();
    if rest.len() == 1 {
        // `返金済以外`, and `paid 以外`: a space only after an ASCII word, as the suite's Japanese
        // writes it
        let t = rest[0].text();
        return Text { ja: ritsu_base::text::ja_spacing(&format!("{}以外", t.ja)), en: format!("not {}", t.en) };
    }
    let parts: Vec<Text> = set.iter().map(Cell::text).collect();
    Text::join(&parts, "、", ", ")
}

impl Table {
    /// The table in Markdown, with a column for the answer and one for the policies that decide
    /// it.
    pub fn markdown(&self, g: &Gate, lang: Lang) -> String {
        let (arrow, policies) = match lang {
            Lang::En => ("->", "policies"),
            Lang::Ja => ("→", "決めたポリシー"),
        };
        let mut head: Vec<String> = self.columns.iter().map(|c| c.get(lang).to_string()).collect();
        head.push(arrow.into());
        head.push(policies.into());
        let mut out = format!("| {} |\n|{}\n", head.join(" | "), "---|".repeat(head.len()));
        for r in &self.rows {
            let mut cells: Vec<String> = r.cells.iter().map(|c| c.get(lang).to_string()).collect();
            cells.push(match (r.allow, lang) {
                (true, Lang::En) => "allow".into(),
                (false, Lang::En) => "deny".into(),
                (true, Lang::Ja) => "許す".into(),
                (false, Lang::Ja) => "拒む".into(),
            });
            cells.push(if r.policies.is_empty() {
                match lang {
                    Lang::En => "(no permit)".into(),
                    Lang::Ja => "（当てはまる permit なし）".into(),
                }
            } else {
                r.policies.iter().map(|p| g.policies[*p].named.name.clone()).collect::<Vec<_>>().join(", ")
            });
            out.push_str(&format!("| {} |\n", cells.join(" | ")));
        }
        out
    }

    /// How many rows allow.
    pub fn allowing(&self) -> usize {
        self.rows.iter().filter(|r| r.allow).count()
    }
}
