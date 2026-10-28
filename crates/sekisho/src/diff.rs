//! `sekisho diff` (ritsu's DESIGN 17.9.4): what changes between two versions of a gate, over every
//! combination both versions decide.
//!
//! Each action both versions have is walked once over one space for both: each version's walk is
//! given what the other's conditions read ([`walk::reads`], [`walk::space_also`]), so that both cut
//! every number at the constants of both, walk the roles both read and relate the terms both
//! relate. The factors of the two walks are then matched by what their places read (by name: an
//! attribute, an input, a computed value, a relation); a factor only one version has is read by that
//! version alone, and does not change the other's answer. The values both can take are walked: a
//! cell of a number is where the two versions' cells meet, an enum's or a role's value one version
//! does not have is not a combination of both (the interface lists it). Rule outputs and dates that
//! both versions compute the same way (the same file, given the same values) are one value; computed
//! another way, every pair of their values for the same values read is walked, and a pair is
//! certain only where one version comes to one value there.
//!
//! Each combination is decided by both versions (`eval::decide`) and is one of four: allowed now
//! and denied before (gained), denied now and allowed before (lost), the same answer decided by
//! other policies (reasons), or the same. A combination that rests on a value another language
//! could not vouch for is tried with a concrete input on each side ([`walk::confirm`], as the check
//! does) before it is said to change, and is undecided when it cannot be: counted apart, never as
//! the same. Changes are grouped by how they change and the deciding policies on each side, each
//! group with its count and up to three combinations, said as the check says them.
//!
//! Each side is read through its own files (a git revision, a version of a dependency: ritsu's
//! DESIGN 17.9.1) and checked first; one that does not pass check stops the diff (exit 2).

use crate::borders::Langs;
use crate::check::{self, Options, Outcome};
use crate::checks::Checked;
use crate::eval::{Decision, decide};
use crate::model::*;
use crate::suite::Suite;
use crate::walk::{self, Ask, Domain, Factor, FactorKind, Frame, Kept, Kn, Known, Memo, Slot, Space, Stop, Val};
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text, count};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

/// How many combinations each group shows.
pub const EXAMPLES: usize = 3;

/// A side of a diff: how it is shown, the files it is read through, and its check.
pub struct Side {
    pub shown: String,
    pub files: Option<Rc<dyn ritsu_base::fs::Files>>,
    pub outcome: Outcome,
}

impl Side {
    /// Run `f` reading this side's files: what the walk asks of rulec and koyomi is read from them.
    fn within<R>(&self, f: impl FnOnce() -> R) -> R {
        match &self.files {
            Some(x) => ritsu_base::fs::with(x.clone(), f),
            None => f(),
        }
    }

    fn checked(&self) -> &Checked {
        self.outcome.walked.as_ref().expect("a side that passes check has its walk")
    }
}

/// A side as the command line writes it (ritsu's DESIGN 17.9.1), read and checked with the
/// languages `suite` joins. Err, with what to say (exit 2), for a file that cannot be read and for
/// a tree of modules that stops.
pub fn side(spelled: &str, suite: &Suite, opts: &Options) -> Result<Side, Text> {
    let a = ritsu_base::modules::address::parse(spelled)?;
    let o = ritsu_base::modules::address::open(&a)?;
    side_through(&o.path, spelled, o.files.clone(), suite, opts)
}

/// A side: the file `path`, read through `files` (a git revision, a version of a dependency, the
/// module at hand with another lock; None for the files as they are), and the walk after it too,
/// shown as `shown`.
pub fn side_through(path: &str, shown: &str, files: Option<Rc<dyn ritsu_base::fs::Files>>, suite: &Suite, opts: &Options) -> Result<Side, Text> {
    let run = || -> Result<Outcome, Text> {
        if let Some(t) = ritsu_base::modules::address::stopped(path) {
            return Err(t);
        }
        check::check_file(path, suite, opts).map_err(|e| tr!("`{shown}` を読めません: {e}", "cannot read `{shown}`: {e}"))
    };
    let outcome = match &files {
        Some(f) => ritsu_base::fs::with(f.clone(), run),
        None => run(),
    }?;
    Ok(Side { shown: shown.to_string(), files, outcome })
}

/// The head of a side: its file as shown, and the name and the version the gate gives itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Head {
    pub file: String,
    pub name: String,
    pub version: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Change {
    /// Denied before, allowed now.
    Gained,
    /// Allowed before, denied now.
    Lost,
    /// The same answer, decided by other policies.
    Reasons,
}

/// A combination shown: in words, and by its values.
#[derive(Clone, Debug)]
pub struct Shown {
    pub said: Text,
    pub values: Vec<(String, Text)>,
}

/// The combinations of an action that change one way, with the same deciding policies on each side.
#[derive(Clone, Debug)]
pub struct Group {
    pub change: Change,
    pub old_policies: Vec<String>,
    pub new_policies: Vec<String>,
    pub count: u128,
    pub examples: Vec<Shown>,
}

#[derive(Clone, Debug, Default)]
pub struct ActionDiff {
    pub name: String,
    /// The combinations walked over both versions.
    pub combinations: u128,
    pub groups: Vec<Group>,
    /// The combinations whose change rests on a value no language could vouch for, and no concrete
    /// input was found for.
    pub unknown: u128,
    pub unknown_examples: Vec<Shown>,
    /// The principal and resource types (`User → Order`) only one version lets the action name.
    pub frames_only_old: Vec<String>,
    pub frames_only_new: Vec<String>,
    /// Each combination that changes or is undecided, when the diff is asked to keep them
    /// ([`diff_kept`]; the tests hold them to the brute force).
    pub detail: Vec<Detail>,
}

/// A combination that changes (how, and the deciding policies of each side) or is undecided (None),
/// by its values: each place by its name (a computed value twice, with `old ` and `new ` in front),
/// a number as the cell both versions walk.
#[derive(Clone, Debug, PartialEq)]
pub struct Detail {
    pub change: Option<(Change, Vec<String>, Vec<String>)>,
    pub values: Vec<(String, String)>,
}

/// What only one version has.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Only {
    pub actions: Vec<String>,
    pub roles: Vec<String>,
    /// `User.refund_limit`.
    pub attributes: Vec<String>,
    /// `refund_order.amount`.
    pub inputs: Vec<String>,
    pub types: Vec<String>,
}

impl Only {
    fn is_empty(&self) -> bool {
        self.actions.is_empty() && self.roles.is_empty() && self.attributes.is_empty() && self.inputs.is_empty() && self.types.is_empty()
    }
}

pub struct Diff {
    pub old: Head,
    pub new: Head,
    pub actions: Vec<ActionDiff>,
    pub only_old: Only,
    pub only_new: Only,
}

impl Diff {
    /// Whether the answers or the interface differ (exit 1): a permission gained or lost, or what
    /// only one version has. A change of the deciding policies alone, or an undecided combination
    /// alone, is not.
    pub fn differs(&self) -> bool {
        !self.only_old.is_empty()
            || !self.only_new.is_empty()
            || self.actions.iter().any(|a| !a.frames_only_old.is_empty() || !a.frames_only_new.is_empty() || a.groups.iter().any(|g| g.change != Change::Reasons))
    }

    fn total(&self, c: Change) -> u128 {
        self.actions.iter().flat_map(|a| &a.groups).filter(|g| g.change == c).map(|g| g.count).sum()
    }

    /// The combinations no version could be decided on, over every action.
    pub fn unknown(&self) -> u128 {
        self.actions.iter().map(|a| a.unknown).sum()
    }
}

/// A side that does not pass check stops the diff: what to say.
fn not_passing(s: &Side) -> Option<Text> {
    if s.outcome.unjoined() {
        let shown = &s.shown;
        let cmd = crate::run::with_ritsu();
        return Some(tr!(
            "`{shown}` は、この sekisho が読めない言語のファイルを読みます。`{cmd}` で走らせてください",
            "`{shown}` reads a file of a language this sekisho does not read; run `{cmd}`"
        ));
    }
    if !s.outcome.has_errors() && s.outcome.walked.is_some() {
        return None;
    }
    let errors = s.outcome.diags.iter().filter(|d| d.is_error()).count().max(1);
    let shown = &s.shown;
    let n = count(errors as u64);
    Some(tr!(
        "`{shown}` は検査を通りません（エラー {n} 件）。`sekisho check` で確かめてください",
        "`{shown}` does not pass check ({n} {}); run `sekisho check` on it",
        ; if errors == 1 { "error" } else { "errors" }
    ))
}

/// The diff of two sides (old, new), the walks asking rulec and koyomi through `suite`, at most
/// `budget` combinations an action.
pub fn diff(old: &Side, new: &Side, suite: &Suite, budget: u128) -> Result<Diff, Text> {
    diff_kept(old, new, suite, budget, false)
}

/// [`diff`], keeping each combination that changes or is undecided when `keep` ([`Detail`]).
pub fn diff_kept(old: &Side, new: &Side, suite: &Suite, budget: u128, keep: bool) -> Result<Diff, Text> {
    for s in [old, new] {
        if let Some(t) = not_passing(s) {
            return Err(t);
        }
    }
    let langs = || Langs { rules: suite.rules.as_deref().filter(|r| r.joined()), dates: suite.dates.as_deref().filter(|d| d.joined()), books: None, flows: None };
    let (co, cn) = (old.checked(), new.checked());
    let (go, gn) = (&co.gate, &cn.gate);
    let head = |s: &Side, g: &Gate| Head { file: s.shown.clone(), name: g.named.name.clone(), version: g.version.clone() };
    let mut d = Diff { old: head(old, go), new: head(new, gn), actions: Vec::new(), only_old: only(go, gn), only_new: only(gn, go) };
    let (memo_o, memo_n) = (Memo::default(), Memo::default());
    let (ask_o, ask_n) = (langs(), langs());
    for (an, a) in gn.actions.iter().enumerate() {
        let Some(ao) = go.actions.iter().position(|x| x.named.name == a.named.name) else { continue };
        let (ro, rn) = (walk::reads(go, ao, &co.known), walk::reads(gn, an, &cn.known));
        let so = old.within(|| walk::space_also(go, ao, &co.known, &ask_o, &memo_o, budget, &rn)).map_err(|e| stopped(&a.named.name, e, budget))?;
        let sn = new.within(|| walk::space_also(gn, an, &cn.known, &ask_n, &memo_n, budget, &ro)).map_err(|e| stopped(&a.named.name, e, budget))?;
        let o = Walked { side: old, g: go, known: &co.known, space: &so, ask: &ask_o };
        let n = Walked { side: new, g: gn, known: &cn.known, space: &sn, ask: &ask_n };
        d.actions.push(action(&o, &n, budget, keep)?);
    }
    Ok(d)
}

/// What stops the walk of an action, as the diff says it (exit 2).
fn stopped(action: &str, e: Stop, budget: u128) -> Text {
    match e {
        Stop::Budget { count: n, .. } => over(action, n, budget),
        Stop::Unknown => tr!(
            "`{action}` をたどれません。読む値を、どの言語も答えませんでした（`sekisho check` が理由を言います）",
            "`{action}` cannot be walked: no language answered for a value it reads (`sekisho check` says why)"
        ),
    }
}

fn over(action: &str, n: u128, budget: u128) -> Text {
    let (c, b) = (count(n.min(u64::MAX as u128) as u64), count(budget.min(u64::MAX as u128) as u64));
    tr!(
        "`{action}` を二つの版で合わせてたどる組み合わせは {c} 通りで、上限 {b} を超えます。`--budget` を大きくしてください",
        "the walk of `{action}` over both versions has {c} combinations, over the budget of {b}; give a larger `--budget`"
    )
}

/// What one version has that the other does not, by name.
fn only(a: &Gate, b: &Gate) -> Only {
    let mut o = Only::default();
    for x in &a.actions {
        match b.actions.iter().find(|y| y.named.name == x.named.name) {
            None => o.actions.push(x.named.name.clone()),
            Some(y) => {
                for i in &x.inputs {
                    if !y.inputs.iter().any(|j| j.named.name == i.named.name) {
                        o.inputs.push(format!("{}.{}", x.named.name, i.named.name));
                    }
                }
            }
        }
    }
    for r in &a.roles {
        if !b.roles.iter().any(|x| x.named.name == r.named.name) {
            o.roles.push(r.named.name.clone());
        }
    }
    for t in &a.types {
        match b.types.iter().find(|x| x.named.name == t.named.name) {
            None => o.types.push(t.named.name.clone()),
            Some(u) => {
                for f in &t.attrs {
                    if !u.attrs.iter().any(|x| x.named.name == f.named.name) {
                        o.attributes.push(format!("{}.{}", t.named.name, f.named.name));
                    }
                }
            }
        }
    }
    o
}

/// One side of the walk of an action.
struct Walked<'a> {
    side: &'a Side,
    g: &'a Gate,
    known: &'a Known,
    space: &'a Space,
    ask: &'a Langs<'a>,
}

/// A value as both versions name it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Cv {
    Roles(Vec<String>),
    Workflow(String),
    Bool(bool),
    Enum(String),
    Cell(i128, i128),
    Absent,
    /// For each term (by how it is written, in that order): its entity, numbered by the order the
    /// terms meet them; the entities the principal is a member of; the term that is the principal.
    Rel(Vec<(String, Option<u8>)>, Vec<u8>, Option<String>),
}

impl Cv {
    /// Whether two values can be one: the same, or two cells that meet. What they are together.
    fn meet(&self, other: &Cv) -> Option<Cv> {
        match (self, other) {
            (Cv::Cell(a, b), Cv::Cell(c, d)) => {
                let (lo, hi) = ((*a).max(*c), (*b).min(*d));
                (lo <= hi).then_some(Cv::Cell(lo, hi))
            }
            (x, y) if x == y => Some(x.clone()),
            _ => None,
        }
    }

    /// The value in a word, as [`Detail`] holds it: `3..5` for a cell.
    pub fn text(&self) -> String {
        match self {
            Cv::Roles(rs) => format!("{{{}}}", rs.join(",")),
            Cv::Workflow(w) | Cv::Enum(w) => w.clone(),
            Cv::Bool(b) => b.to_string(),
            Cv::Cell(lo, hi) => format!("{lo}..{hi}"),
            Cv::Absent => "absent".into(),
            Cv::Rel(..) => format!("{self:?}"),
        }
    }
}

impl Walked<'_> {
    /// How both versions name what a place of a frame reads.
    fn key(&self, f: &Frame, at: usize) -> String {
        let g = self.g;
        let a = &g.actions[self.space.action];
        match &f.slots[at] {
            Slot::Roles => "roles".into(),
            Slot::Workflow => "workflow".into(),
            Slot::Attr(o, i) => format!("{}.{}", o.word(), g.owner_type(*o, f.principal, f.resource).attrs[*i].named.name),
            Slot::Input(i) => format!("input {}", a.inputs[*i].named.name),
            Slot::Computed(i) => format!("value {}", a.computed[*i].named.name),
            Slot::Relation(t) => format!("relation {}", g.types[*t].named.name),
            Slot::Linked(l, i) => {
                let x = &f.links[*l];
                format!("value {} of {} given {:?}", g.actions[x.action].computed[*i].named.name, g.actions[x.action].named.name, x.inputs)
            }
            Slot::Allowed(l) => {
                let x = &f.links[*l];
                format!("allowed {} given {:?}", g.actions[x.action].named.name, x.inputs)
            }
        }
    }

    /// A value of a place, as both versions name it.
    fn val(&self, f: &Frame, at: usize, v: &Val) -> Cv {
        let g = self.g;
        match v {
            Val::Roles { direct, .. } => Cv::Roles((0..g.roles.len()).filter(|&r| direct & walk::bit(r) != 0).map(|r| g.roles[r].named.name.clone()).collect()),
            Val::Workflow(w) => Cv::Workflow(g.workflows[*w].named.name.clone()),
            Val::Bool(b) => Cv::Bool(*b),
            Val::Enum(e) => {
                let name = match &f.slots[at] {
                    Slot::Attr(o, i) => match &g.owner_type(*o, f.principal, f.resource).attrs[*i].ty {
                        FieldType::Enum(x) => g.enums[*x].values.get(*e).map(|v| v.name.clone()),
                        _ => None,
                    },
                    Slot::Input(i) => match &g.actions[self.space.action].inputs[*i].ty {
                        FieldType::Enum(x) => g.enums[*x].values.get(*e).map(|v| v.name.clone()),
                        _ => None,
                    },
                    _ => f.computed_at(self.space.action, at).and_then(|(act, ci, _)| match self.known.computed.get(&(act, ci)) {
                        Some(Kn::Rule { domain: Domain::Enum(vs), .. }) => vs.get(*e).map(|v| v.name.clone()),
                        _ => None,
                    }),
                };
                Cv::Enum(name.unwrap_or_else(|| e.to_string()))
            }
            Val::Cell(c) => Cv::Cell(c.lo, c.hi),
            Val::Absent => Cv::Absent,
            Val::Relation(r) => {
                let terms = f.terms.get(&at).cloned().unwrap_or_default();
                let mut order: Vec<(String, usize)> = terms.iter().enumerate().map(|(i, t)| (format!("{t:?}"), i)).collect();
                order.sort();
                let mut renumber: BTreeMap<u8, u8> = BTreeMap::new();
                let mut blocks = Vec::new();
                for (name, i) in &order {
                    let b = r.blocks.get(*i).copied().flatten().map(|b| {
                        let n = renumber.len() as u8;
                        *renumber.entry(b).or_insert(n)
                    });
                    blocks.push((name.clone(), b));
                }
                let members: Vec<u8> = renumber.iter().filter(|(b, _)| r.members & (1u64 << **b) != 0).map(|(_, n)| *n).collect::<BTreeSet<u8>>().into_iter().collect();
                let principal = r.principal.and_then(|p| terms.get(p)).map(|t| format!("{t:?}"));
                Cv::Rel(blocks, members, principal)
            }
        }
    }

    /// Whether a place holds a computed value.
    fn computed(&self, f: &Frame, at: usize) -> bool {
        matches!(f.slots[at], Slot::Computed(_) | Slot::Linked(..))
    }

    /// What computes the value at a place, as two versions compare it: the file's text (read
    /// through this side's files), what it computes and what it is given. None for a place of no
    /// computed value.
    fn computation(&self, f: &Frame, at: usize) -> Option<String> {
        let (act, ci, link) = f.computed_at(self.space.action, at)?;
        let c = &self.g.actions[act].computed[ci];
        let given = link.map(|l| format!("{:?}", f.links[l].inputs)).unwrap_or_default();
        let text = |file: &std::path::Path| self.side.within(|| ritsu_base::fs::read(file).ok());
        Some(match self.known.computed.get(&(act, ci))? {
            Kn::Rule { file, output, .. } => format!("rule {:?} {output} {:?} {given}", text(file)?, c.how),
            Kn::Date { file, function, .. } => format!("date {:?} {function} {:?} {given}", text(file)?, c.how),
            Kn::Attr => format!("attr {:?} {given}", c.how),
            Kn::Open { data, closed } => format!("open {data:?} {closed:?} {:?} {given}", c.how),
        })
    }
}

/// One factor's points, as both versions name them: by the places' keys in their sorted order.
struct Keyed {
    keys: Vec<String>,
    /// For each point: its values in the order of `keys`.
    points: Vec<Vec<Cv>>,
    /// Which of `keys` are computed values.
    outputs: Vec<bool>,
}

fn keyed(w: &Walked, f: &Frame, x: &Factor) -> Keyed {
    let mut order: Vec<(String, usize)> = x.places.iter().enumerate().map(|(k, &at)| (w.key(f, at), k)).collect();
    order.sort();
    let keys = order.iter().map(|(k, _)| k.clone()).collect();
    let outputs = order.iter().map(|(_, k)| w.computed(f, x.places[*k])).collect();
    let points = x.points.iter().map(|p| order.iter().map(|(_, k)| w.val(f, x.places[*k], &p.vals[*k])).collect()).collect();
    Keyed { keys, points, outputs }
}

/// One point of an axis: the point of each factor of each version the axis sets, whether the point
/// is certain, whether it is certain wherever each version's values are, and its values by name (a
/// computed value one version computes otherwise than the other, with `old ` or `new ` in front).
struct Joined {
    old: Vec<usize>,
    new: Vec<usize>,
    exact: bool,
    joint: bool,
    values: Vec<(String, Cv)>,
}

/// One axis of the walk over both versions: the factors of each version that read the same things,
/// joined, and the values both versions can take together.
struct Axis {
    old: Vec<usize>,
    new: Vec<usize>,
    points: Vec<Joined>,
    /// Whether it reads something only the old version reads, or only the new one.
    old_only: bool,
    new_only: bool,
}

/// The factors of both versions of a frame joined into axes: factors that read a thing both read
/// are one axis, their points joined where the values meet. A computed value both versions compute
/// the same way is a thing both read; computed otherwise, each version's is its own.
fn axes(o: &Walked, fo: &Frame, n: &Walked, fnew: &Frame) -> Vec<Axis> {
    let on: Vec<Keyed> = fo.factors.iter().map(|x| keyed(o, fo, x)).collect();
    let nn: Vec<Keyed> = fnew.factors.iter().map(|x| keyed(n, fnew, x)).collect();
    // the computed values both compute the same way
    let place_of = |w: &Walked, f: &Frame, key: &str| -> Option<usize> { f.factors.iter().flat_map(|x| x.places.iter().copied()).find(|&at| w.key(f, at) == key) };
    let mut same: BTreeSet<String> = BTreeSet::new();
    for k in nn.iter().flat_map(|x| x.keys.iter().zip(&x.outputs).filter(|(_, o)| **o).map(|(k, _)| k.clone())) {
        let (Some(a), Some(b)) = (place_of(o, fo, &k), place_of(n, fnew, &k)) else { continue };
        if matches!((o.computation(fo, a), n.computation(fnew, b)), (Some(x), Some(y)) if x == y) {
            same.insert(k);
        }
    }
    // the keys of each factor as the join reads them: a computed value of its own version's alone
    let names = |x: &Keyed, side: &str| -> Vec<String> {
        x.keys.iter().zip(&x.outputs).map(|(k, out)| if *out && !same.contains(k) { format!("{side}{k}") } else { k.clone() }).collect()
    };
    let (ok, nk): (Vec<Vec<String>>, Vec<Vec<String>>) = (on.iter().map(|x| names(x, "old ")).collect(), nn.iter().map(|x| names(x, "new ")).collect());
    // the factors joined by the things they both read
    let mut comps: Vec<(Vec<usize>, Vec<usize>, BTreeSet<String>)> = Vec::new();
    for (side, keys) in [(false, &ok), (true, &nk)] {
        for (i, ks) in keys.iter().enumerate() {
            let mine: BTreeSet<String> = ks.iter().cloned().collect();
            let touching: Vec<usize> = (0..comps.len()).filter(|&c| !comps[c].2.is_disjoint(&mine)).collect();
            let mut c = (Vec::new(), Vec::new(), mine);
            for t in touching.into_iter().rev() {
                let (a, b, k) = comps.remove(t);
                c.0.extend(a);
                c.1.extend(b);
                c.2.extend(k);
            }
            if side { c.1.push(i) } else { c.0.push(i) }
            comps.push(c);
        }
    }
    let mut out = Vec::new();
    for (mut olds, mut news, _) in comps {
        olds.sort();
        news.sort();
        // the points joined, a factor at a time
        let mut acc: Vec<Joined> = vec![Joined { old: Vec::new(), new: Vec::new(), exact: true, joint: true, values: Vec::new() }];
        let mut shrunk: Vec<bool> = vec![false];
        // the keys a computed factor has set: a value another language gave for a cell is certain on
        // that cell, not on a part of it
        let mut computed_keys: BTreeSet<String> = BTreeSet::new();
        for (side, i) in olds.iter().map(|&i| (false, i)).chain(news.iter().map(|&i| (true, i))) {
            let (x, ks, f) = if side { (&nn[i], &nk[i], &fnew.factors[i]) } else { (&on[i], &ok[i], &fo.factors[i]) };
            let computed = matches!(f.kind, FactorKind::Rules { .. } | FactorKind::Dates);
            let mut next = Vec::new();
            let mut next_shrunk = Vec::new();
            for (j, sh) in acc.iter().zip(&shrunk) {
                'points: for (p, vals) in x.points.iter().enumerate() {
                    let mut values = j.values.clone();
                    let mut cut = false;
                    for (k, v) in ks.iter().zip(vals) {
                        match values.iter_mut().find(|(x, _)| x == k) {
                            Some((_, w)) => match w.meet(v) {
                                Some(m) => {
                                    cut |= (computed && m != *v) || (computed_keys.contains(k) && m != *w);
                                    *w = m;
                                }
                                None => continue 'points,
                            },
                            None => values.push((k.clone(), v.clone())),
                        }
                    }
                    let mut np = Joined { old: j.old.clone(), new: j.new.clone(), exact: j.exact && f.points[p].exact, joint: true, values };
                    if side { np.new.push(p) } else { np.old.push(p) }
                    next.push(np);
                    next_shrunk.push(*sh || cut);
                }
            }
            if computed {
                computed_keys.extend(ks.iter().cloned());
            }
            acc = next;
            shrunk = next_shrunk;
        }
        for p in acc.iter_mut() {
            p.values.sort();
        }
        // values computed otherwise by each version: a pair is certain where one version comes to one
        // value for the values both read
        let split = |p: &Joined| -> (Vec<(String, Cv)>, Vec<(String, Cv)>, Vec<(String, Cv)>) {
            let pick = |f: &dyn Fn(&str) -> bool| p.values.iter().filter(|(k, _)| f(k)).cloned().collect::<Vec<_>>();
            (pick(&|k| !k.starts_with("old ") && !k.starts_with("new ")), pick(&|k| k.starts_with("old ")), pick(&|k| k.starts_with("new ")))
        };
        let mut olds_at: HashMap<Vec<(String, Cv)>, BTreeSet<Vec<(String, Cv)>>> = HashMap::new();
        let mut news_at: HashMap<Vec<(String, Cv)>, BTreeSet<Vec<(String, Cv)>>> = HashMap::new();
        for p in &acc {
            let (r, o, n) = split(p);
            olds_at.entry(r.clone()).or_default().insert(o);
            news_at.entry(r).or_default().insert(n);
        }
        for (p, sh) in acc.iter_mut().zip(&shrunk) {
            let (r, o, n) = split(p);
            let paired = !o.is_empty() && !n.is_empty();
            p.joint = !*sh && (!paired || olds_at[&r].len() == 1 || news_at[&r].len() == 1);
            p.exact &= p.joint;
        }
        let only = |f: &dyn Fn(&str) -> bool| acc.first().is_some_and(|p| p.values.iter().any(|(k, _)| f(k)));
        let old_keys: BTreeSet<String> = olds.iter().flat_map(|&i| ok[i].iter().cloned()).collect();
        let new_keys: BTreeSet<String> = news.iter().flat_map(|&i| nk[i].iter().cloned()).collect();
        let old_only = !old_keys.is_subset(&new_keys) || only(&|k| k.starts_with("old "));
        let new_only = !new_keys.is_subset(&old_keys) || only(&|k| k.starts_with("new "));
        out.push(Axis { old: olds, new: news, points: acc, old_only, new_only });
    }
    out
}

/// The values of a combination over both versions, as [`Detail`] holds them.
fn values_of(ax: &[Axis], idx: &[usize]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = ax.iter().zip(idx).flat_map(|(a, &p)| a.points[p].values.iter().map(|(k, v)| (k.clone(), v.text()))).collect();
    out.sort();
    out
}

/// Every combination of one side's walk of an action, each frame also reading what `also` says,
/// named as [`Detail`] names them (a computed value with `prefix` in front), with its decision:
/// whether it allows and the deciding policies. For the tests' brute force: walked with every
/// number cut at each of its values, each combination is one request.
pub fn each(side: &Side, prefix: &str, action: &str, also: &walk::AlsoByFrame, suite: &Suite, budget: u128, mut visit: impl FnMut(Vec<(String, String)>, bool, Vec<String>)) -> Result<(), Text> {
    let langs = Langs { rules: suite.rules.as_deref().filter(|r| r.joined()), dates: suite.dates.as_deref().filter(|d| d.joined()), books: None, flows: None };
    let c = side.checked();
    let g = &c.gate;
    let Some(ai) = g.actions.iter().position(|a| a.named.name == action) else { return Ok(()) };
    let memo = Memo::default();
    let space = side.within(|| walk::space_also(g, ai, &c.known, &langs, &memo, budget, also)).map_err(|e| stopped(action, e, budget))?;
    let w = Walked { side, g, known: &c.known, space: &space, ask: &langs };
    space.each(|combo| {
        let f = &space.frames[combo.frame];
        let mut vals = Vec::new();
        for x in &f.factors {
            for &at in &x.places {
                let key = w.key(f, at);
                let key = if w.computed(f, at) { format!("{prefix}{key}") } else { key };
                vals.push((key, w.val(f, at, &combo.env[at]).text()));
            }
        }
        vals.sort();
        visit(vals, combo.decision.allow, names(&w, combo.decision));
    });
    Ok(())
}

fn frame_name(g: &Gate, f: &Frame) -> String {
    format!("{} → {}", g.types[f.principal].named.name, g.types[f.resource].named.name)
}

/// The values of a frame before any factor sets its own.
fn base(f: &Frame) -> Vec<Val> {
    let mut env = vec![Val::Absent; f.slots.len()];
    for (at, v) in &f.fixed {
        env[*at] = v.clone();
    }
    env
}

fn set(x: &Factor, env: &mut [Val], i: usize) {
    for (at, v) in x.places.iter().zip(&x.points[i].vals) {
        env[*at] = v.clone();
    }
}

fn decision(w: &Walked, f: &Frame, env: &mut [Val]) -> Decision {
    f.derive(env);
    let applies: Vec<bool> = f.policies.iter().map(|p| p.all.holds(env)).collect();
    decide(&applies, &w.space.permit)
}

fn names(w: &Walked, d: &Decision) -> Vec<String> {
    d.determining.iter().map(|&i| w.g.policies[w.space.policies[i]].named.name.clone()).collect()
}

/// Which versions' walks a combination is said from: the one that reads everything the other reads
/// (the new one when both do), or both.
#[derive(Clone, Copy, PartialEq)]
enum Say {
    New,
    Old,
    Both,
}

/// A combination, shown as the check shows one.
fn shown(n: &Walked, kn: &Kept, o: &Walked, ko: &Kept, from: Say) -> Shown {
    if from == Say::Old {
        return Shown { said: walk::say(o.g, o.space, ko), values: walk::values(o.g, o.space, ko) };
    }
    let (said, mut values) = (walk::say(n.g, n.space, kn), walk::values(n.g, n.space, kn));
    if from == Say::New {
        return Shown { said, values };
    }
    // the old version's values the new one does not have, or has otherwise (a value each computes
    // its own way): said after the new version's sentence, and held under `old <name>` when the new
    // version has that name
    let (mut otherwise, mut held): (Vec<(String, Text)>, Vec<(String, Text)>) = (Vec::new(), Vec::new());
    for (k, v) in walk::values(o.g, o.space, ko) {
        match values.iter().find(|(x, _)| *x == k) {
            Some((_, w)) if *w == v => continue,
            Some(_) => held.push((format!("old {k}"), v.clone())),
            None => held.push((k.clone(), v.clone())),
        }
        otherwise.push((k, v));
    }
    values.extend(held);
    if otherwise.is_empty() {
        return Shown { said, values };
    }
    let listed = |ja: bool| -> String {
        let (sep, colon) = if ja { ("、", "：") } else { (", ", ": ") };
        otherwise.iter().map(|(k, v)| format!("{k}{colon}{}", if ja { &v.ja } else { &v.en })).collect::<Vec<_>>().join(sep)
    };
    let (ja, en) = (listed(true), listed(false));
    Shown { said: tr!("{}（古い版では{}）", "{} (in the old version: {})", said.ja, ja; said.en, en), values }
}

/// What trying concrete inputs came to.
enum Tried {
    /// An input both versions come to the values of the combination on.
    Found,
    /// None, among every input there is (`exhaustive`: the combination is not one), or among some.
    None { exhaustive: bool },
}

/// The most inputs a combination is tried with.
const TRIES: usize = 4096;

/// The values a number is tried at: every one in a range this small, else both ends, the middle and
/// one in from each end.
fn tried(lo: i128, hi: i128) -> (Vec<i128>, bool) {
    if hi.saturating_sub(lo) < 64 {
        return ((lo..=hi).collect(), true);
    }
    let mut v = vec![lo, hi, lo + (hi - lo) / 2, lo.saturating_add(1).min(hi), hi.saturating_sub(1).max(lo)];
    v.sort();
    v.dedup();
    (v, false)
}

/// Try concrete inputs on a combination of both versions whose rule outputs either version could not
/// vouch for, or whose pair of outputs is not certain: an input of the attributes and the inputs the
/// outputs read (each held to the combination's value where a version cuts it) on which every rule
/// output of both versions comes to the combination's value. Each version's rules are asked through
/// its own files.
fn try_both(o: &Walked, fo: &Frame, ko: &Kept, n: &Walked, fnew: &Frame, kn: &Kept) -> Tried {
    use ritsu_ports::Value;
    // the rule outputs to come to, by version: the place and the value
    let mut wants: Vec<(bool, usize, Val)> = Vec::new();
    // what they read, by name, with the field (and whether it is the new version's) and the value a
    // version holds it to
    let mut raws: Vec<(walk::Place, Field, bool, Option<Cv>)> = Vec::new();
    for (is_new, w, f, k) in [(false, o, fo, ko), (true, n, fnew, kn)] {
        for (x, &i) in f.factors.iter().zip(&k.idx) {
            let FactorKind::Rules { outputs, cut } = &x.kind else { continue };
            let point = &x.points[i];
            let value_at = |at: usize| point.vals[x.places.iter().position(|p| *p == at).unwrap_or(0)].clone();
            for &at in outputs {
                wants.push((is_new, at, value_at(at)));
                let Some((act, ci, link)) = f.computed_at(w.space.action, at) else { return Tried::None { exhaustive: false } };
                let How::Rule { args, .. } = &w.g.actions[act].computed[ci].how else { return Tried::None { exhaustive: false } };
                for (_, src) in args {
                    let (place, field, slot) = match f.mapped(link, w.g, src) {
                        Some(Source::Attr(ow, name)) => match w.g.owner_type(ow, f.principal, f.resource).attr(&name) {
                            Some((ai, fd)) => (walk::Place::Attr(ow, fd.named.name.clone()), fd.clone(), Slot::Attr(ow, ai)),
                            None => continue,
                        },
                        Some(Source::Input(name)) => match w.g.actions[w.space.action].input(&name) {
                            Some((ii, fd)) => (walk::Place::Input(fd.named.name.clone()), fd.clone(), Slot::Input(ii)),
                            None => continue,
                        },
                        _ => continue,
                    };
                    let held = f.place(&slot).filter(|at| cut.contains(at)).map(|at| w.val(f, at, &value_at(at)));
                    match raws.iter_mut().find(|(p, _, _, _)| *p == place) {
                        Some(r) => {
                            if let Some(h) = held {
                                r.3 = match &r.3 {
                                    Some(c) => match c.meet(&h) {
                                        Some(m) => Some(m),
                                        None => return Tried::None { exhaustive: true },
                                    },
                                    None => Some(h),
                                };
                            }
                        }
                        None => raws.push((place, field, is_new, held)),
                    }
                }
            }
        }
    }
    // the values each is tried at
    let mut exhaustive = true;
    let mut lists: Vec<Vec<Option<Value>>> = Vec::new();
    for (_, fd, of_new, held) in &raws {
        let mut list: Vec<Option<Value>> = match (&fd.ty, held) {
            (_, Some(Cv::Absent)) => vec![None],
            (FieldType::Num { .. }, Some(Cv::Cell(lo, hi))) | (FieldType::Num { lo, hi, .. }, None) => {
                let (vs, all) = tried(*lo, *hi);
                exhaustive &= all;
                vs.into_iter().map(|v| Some(Value::Int(v))).collect()
            }
            (FieldType::Bool, Some(Cv::Bool(b))) => vec![Some(Value::Bool(*b))],
            (FieldType::Bool, None) => vec![Some(Value::Bool(false)), Some(Value::Bool(true))],
            (FieldType::Enum(_), Some(Cv::Enum(v))) => vec![Some(Value::Enum(v.clone()))],
            (FieldType::Enum(e), None) => {
                let g = if *of_new { n.g } else { o.g };
                g.enums[*e].values.iter().map(|v| Some(Value::Enum(v.name.clone()))).collect()
            }
            (FieldType::Date { lo, hi }, _) => {
                let (vs, all) = tried(*lo as i128, *hi as i128);
                exhaustive &= all;
                vs.into_iter().map(|d| Some(Value::Date(ritsu_ports::day_text(d as i64)))).collect()
            }
            _ => return Tried::None { exhaustive: false },
        };
        if fd.optional && held.is_none() {
            list.push(None);
        }
        lists.push(list);
    }
    let total = lists.iter().fold(1usize, |m, l| m.saturating_mul(l.len()));
    if total > TRIES {
        return Tried::None { exhaustive: false };
    }
    // an input as a rule's evaluator takes it, for one output of one version
    let mut asked: HashMap<(bool, String), Option<ritsu_ports::Values>> = HashMap::new();
    // a rule that gives no answer for an input says nothing of whether the combination is one
    let failed = std::cell::Cell::new(false);
    let mut idx = vec![0usize; lists.len()];
    loop {
        let assignment: Vec<Option<Value>> = lists.iter().zip(&idx).map(|(l, &i)| l[i].clone()).collect();
        let all = wants.iter().all(|(is_new, at, want)| {
            let (w, f) = if *is_new { (n, fnew) } else { (o, fo) };
            let Some((act, ci, link)) = f.computed_at(w.space.action, *at) else { return false };
            let (Some(Kn::Rule { file, output, domain, inputs, enums, .. }), How::Rule { args, .. }) = (w.known.computed.get(&(act, ci)), &w.g.actions[act].computed[ci].how) else { return false };
            let mut vals: ritsu_ports::Values = Vec::new();
            for (input, src) in args {
                let Some(src) = f.mapped(link, w.g, src) else { return *want == Val::Absent };
                let v = match &src {
                    Source::Lit(Literal::Num(num)) => {
                        let u = inputs.iter().find(|(x, _)| x == input).and_then(|(_, u)| u.clone());
                        match u.and_then(|u| crate::types::count(num, &u).ok()) {
                            Some(k) => Some(Value::Int(k)),
                            None => return false,
                        }
                    }
                    Source::Lit(Literal::Bool(b)) => Some(Value::Bool(*b)),
                    Source::Lit(Literal::Word(word)) => Some(Value::Enum(word.clone())),
                    Source::Lit(Literal::Date(d)) => Some(Value::Date(ritsu_ports::day_text(*d))),
                    Source::Today => return false,
                    Source::Attr(ow, name) => {
                        let name = w.g.owner_type(*ow, f.principal, f.resource).attr(name).map(|(_, fd)| fd.named.name.clone()).unwrap_or_default();
                        raws.iter().position(|(p, _, _, _)| *p == walk::Place::Attr(*ow, name.clone())).and_then(|p| assignment[p].clone())
                    }
                    Source::Input(name) => {
                        let name = w.g.actions[w.space.action].input(name).map(|(_, fd)| fd.named.name.clone()).unwrap_or_default();
                        raws.iter().position(|(p, _, _, _)| *p == walk::Place::Input(name.clone())).and_then(|p| assignment[p].clone())
                    }
                };
                // an enum's value as the rule's evaluator takes it
                let v = v.map(|v| match &v {
                    Value::Enum(word) => enums
                        .iter()
                        .find(|(x, _)| x == input)
                        .and_then(|(_, values)| values.iter().find(|(_, words)| words.iter().any(|x| x == word)).map(|(name, _)| Value::Enum(name.clone())))
                        .unwrap_or(v),
                    _ => v,
                });
                match v {
                    Some(v) => vals.push((input.clone(), v)),
                    None => return *want == Val::Absent,
                }
            }
            let key = (*is_new, format!("{}\u{1}{output}\u{1}{vals:?}", file.display()));
            let outs = asked.entry(key).or_insert_with(|| w.side.within(|| w.ask.rule(file, &vals).ok())).clone();
            match outs {
                Some(outs) => outs.iter().find(|(x, _)| x == output).and_then(|(_, v)| domain.of_value(v)) == Some(want.clone()),
                None => {
                    failed.set(true);
                    false
                }
            }
        });
        if all {
            return Tried::Found;
        }
        let mut k = lists.len();
        let more = loop {
            if k == 0 {
                break false;
            }
            k -= 1;
            idx[k] += 1;
            if idx[k] < lists[k].len() {
                break true;
            }
            idx[k] = 0;
        };
        if !more {
            return Tried::None { exhaustive: exhaustive && !failed.get() };
        }
    }
}

/// The walk of one action over both versions.
fn action(o: &Walked, n: &Walked, budget: u128, keep: bool) -> Result<ActionDiff, Text> {
    let name = n.g.actions[n.space.action].named.name.clone();
    let mut out = ActionDiff { name: name.clone(), ..ActionDiff::default() };
    let mut pairs = Vec::new();
    for (fj, f) in n.space.frames.iter().enumerate() {
        let key = frame_name(n.g, f);
        match o.space.frames.iter().position(|x| frame_name(o.g, x) == key) {
            Some(fi) => pairs.push((fi, fj)),
            None => out.frames_only_new.push(key),
        }
    }
    for f in &o.space.frames {
        let key = frame_name(o.g, f);
        if !n.space.frames.iter().any(|x| frame_name(n.g, x) == key) {
            out.frames_only_old.push(key);
        }
    }
    let mut walks = Vec::new();
    for (fi, fj) in pairs {
        let (fo, fnew) = (&o.space.frames[fi], &n.space.frames[fj]);
        let ax = axes(o, fo, n, fnew);
        let size: u128 = ax.iter().fold(1u128, |m, a| m.saturating_mul(a.points.len() as u128));
        out.combinations = out.combinations.saturating_add(size);
        walks.push((fi, fj, ax));
    }
    if out.combinations > budget {
        return Err(over(&name, out.combinations, budget));
    }
    let mut groups: BTreeMap<(Change, Vec<String>, Vec<String>), Group> = BTreeMap::new();
    let mut confirmed: HashMap<(Kept, Kept), Option<bool>> = HashMap::new();
    for (fi, fj, ax) in &walks {
        let (fo, fnew) = (&o.space.frames[*fi], &n.space.frames[*fj]);
        if ax.iter().any(|a| a.points.is_empty()) {
            continue;
        }
        let mut idx = vec![0usize; ax.len()];
        // the examples are said from the version that reads everything the other reads, else from both
        let from = match (ax.iter().any(|a| a.old_only), ax.iter().any(|a| a.new_only)) {
            (true, true) => Say::Both,
            (true, false) => Say::Old,
            (false, _) => Say::New,
        };
        let (mut env_o, mut env_n) = (base(fo), base(fnew));
        let mut ko = Kept { frame: *fi, idx: vec![0; fo.factors.len()] };
        let mut kn = Kept { frame: *fj, idx: vec![0; fnew.factors.len()] };
        loop {
            let mut exact = true;
            let mut joint = true;
            for (a, &p) in ax.iter().zip(&idx) {
                let pt = &a.points[p];
                for (&x, &i) in a.old.iter().zip(&pt.old) {
                    set(&fo.factors[x], &mut env_o, i);
                    ko.idx[x] = i;
                }
                for (&x, &j) in a.new.iter().zip(&pt.new) {
                    set(&fnew.factors[x], &mut env_n, j);
                    kn.idx[x] = j;
                }
                exact &= pt.exact;
                joint &= pt.joint;
            }
            let (d_o, d_n) = (decision(o, fo, &mut env_o), decision(n, fnew, &mut env_n));
            let (p_o, p_n) = (names(o, &d_o), names(n, &d_n));
            let change = if d_o.allow != d_n.allow {
                Some(if d_n.allow { Change::Gained } else { Change::Lost })
            } else if p_o != p_n {
                Some(Change::Reasons)
            } else {
                None
            };
            if let Some(change) = change {
                // a change that rests on a value no language vouched for, or on a pair of values not
                // certain to meet: tried with concrete inputs on both versions, as the check tries one;
                // a combination no input there is reaches is not one
                let certain = if exact {
                    true
                } else {
                    let key = (ko.clone(), kn.clone());
                    let t = *confirmed.entry(key).or_insert_with(|| match try_both(o, fo, &ko, n, fnew, &kn) {
                        Tried::Found => Some(true),
                        Tried::None { exhaustive: true } => None,
                        Tried::None { exhaustive: false } => {
                            // as the check tries one: each version on its own, when the pair is certain wherever each value is
                            Some(joint && [(o, &ko), (n, &kn)].into_iter().all(|(w, k)| w.side.within(|| walk::confirm(w.g, w.space, k, w.known, w.ask))))
                        }
                    });
                    match t {
                        Some(c) => c,
                        None => {
                            if !next(&mut idx, ax) {
                                break;
                            }
                            continue;
                        }
                    }
                };
                if keep {
                    out.detail.push(Detail { change: certain.then(|| (change, p_o.clone(), p_n.clone())), values: values_of(ax, &idx) });
                }
                if certain {
                    let gr = groups.entry((change, p_o.clone(), p_n.clone())).or_insert_with(|| Group { change, old_policies: p_o, new_policies: p_n, count: 0, examples: Vec::new() });
                    gr.count += 1;
                    if gr.examples.len() < EXAMPLES {
                        let e = shown(n, &kn, o, &ko, from);
                        if !gr.examples.iter().any(|x| x.said == e.said) {
                            gr.examples.push(e);
                        }
                    }
                } else {
                    out.unknown += 1;
                    if out.unknown_examples.len() < EXAMPLES {
                        let e = shown(n, &kn, o, &ko, from);
                        if !out.unknown_examples.iter().any(|x| x.said == e.said) {
                            out.unknown_examples.push(e);
                        }
                    }
                }
            }
            if !next(&mut idx, ax) {
                break;
            }
        }
    }
    out.groups = groups.into_values().collect();
    Ok(out)
}

/// The next combination of the axes, the last turning fastest; false after the last.
fn next(idx: &mut [usize], ax: &[Axis]) -> bool {
    let mut k = ax.len();
    loop {
        if k == 0 {
            return false;
        }
        k -= 1;
        idx[k] += 1;
        if idx[k] < ax[k].points.len() {
            return true;
        }
        idx[k] = 0;
    }
}

// ── Saying it ───────────────────────────────────────────────────────────

fn colon(lang: Lang) -> &'static str {
    if lang == Lang::Ja { "：" } else { ": " }
}

fn join(xs: &[String], lang: Lang) -> String {
    xs.join(if lang == Lang::Ja { "、" } else { ", " })
}

fn title(d: &Diff, q: &str) -> String {
    if d.old.name == d.new.name {
        format!("{q}{}{q} {} → {}", d.old.name, d.old.version, d.new.version)
    } else {
        format!("{q}{}{q} {} → {q}{}{q} {}", d.old.name, d.old.version, d.new.name, d.new.version)
    }
}

/// The lines of what only one version has: `action approve_refund`, `role manager`, ….
fn only_lines(o: &Only, lang: Lang) -> Vec<String> {
    let mut out = Vec::new();
    let mut put = |what: Text, xs: &[String]| {
        if !xs.is_empty() {
            out.push(format!("{} {}", what.get(lang), join(xs, lang)));
        }
    };
    put(tr!("action", "action"), &o.actions);
    put(tr!("役割", "role"), &o.roles);
    put(tr!("型", "type"), &o.types);
    put(tr!("属性", "attribute"), &o.attributes);
    put(tr!("入力", "input"), &o.inputs);
    out
}

fn change_text(c: Change) -> Text {
    match c {
        Change::Gained => tr!("許可が増えた（拒否 → 許可）", "allow gained (deny → allow)"),
        Change::Lost => tr!("許可が減った（許可 → 拒否）", "allow lost (allow → deny)"),
        Change::Reasons => tr!("答えは同じで、効いたポリシーが変わった", "the same answer, decided by other policies"),
    }
}

fn policies_text(ps: &[String], lang: Lang) -> String {
    if ps.is_empty() { tr!("なし", "none").get(lang).to_string() } else { join(ps, lang) }
}

fn combos(n: u128, lang: Lang) -> String {
    let c = count(n.min(u64::MAX as u128) as u64);
    match lang {
        Lang::Ja => format!("{c} 通り"),
        Lang::En => format!("{c} {}", if n == 1 { "combination" } else { "combinations" }),
    }
}

/// The closing sentence: every other combination is decided the same, or, when some are undecided,
/// why nothing is said of the others.
fn closing(d: &Diff, lang: Lang, capital: bool) -> String {
    let t = if d.unknown() > 0 {
        tr!(
            "決められない組み合わせがあるので、ほかの組み合わせが同じ決定になるとは言いません。\n",
            "some combinations cannot be decided, so nothing is said of the others.\n"
        )
    } else if d.actions.iter().any(|a| !a.groups.is_empty()) {
        tr!("ほかの組み合わせでは、どの action も同じ決定になります。\n", "for every other combination, every action is decided the same.\n")
    } else {
        tr!("どの組み合わせでも、どの action も同じ決定になります。\n", "for every combination, every action is decided the same.\n")
    };
    if capital { ritsu_base::text::capitalize(t.get(lang)) } else { t.get(lang).to_string() }
}

/// The diff as text, `--lang` `lang`.
pub fn text(d: &Diff, lang: Lang) -> String {
    let c = colon(lang);
    let mut o = format!("sekisho diff{c}{} {}\n", tr!("ゲート", "gate").get(lang), title(d, ""));
    o.push_str(&format!("  {}{c}{}\n", tr!("古い版", "old").get(lang), d.old.file));
    o.push_str(&format!("  {}{c}{}\n", tr!("新しい版", "new").get(lang), d.new.file));
    for (only, what) in [(&d.only_old, tr!("古い版だけにあるもの：", "only in the old version:")), (&d.only_new, tr!("新しい版だけにあるもの：", "only in the new version:"))] {
        let lines = only_lines(only, lang);
        if !lines.is_empty() {
            o.push_str(&format!("\n{}\n", what.get(lang)));
            for l in lines {
                o.push_str(&format!("  {l}\n"));
            }
        }
    }
    for a in &d.actions {
        o.push_str(&format!("\n{}{c}{}\n", a.name, tr!("両方の版を合わせて {}", "{} walked over both versions", combos(a.combinations, lang); combos(a.combinations, lang)).get(lang)));
        for (frames, what) in [(&a.frames_only_old, tr!("古い版だけが受け付ける principal と resource", "principal and resource only the old version takes")), (&a.frames_only_new, tr!("新しい版だけが受け付ける principal と resource", "principal and resource only the new version takes"))] {
            if !frames.is_empty() {
                o.push_str(&format!("  {}{c}{}\n", what.get(lang), join(frames, lang)));
            }
        }
        for g in &a.groups {
            o.push_str(&format!(
                "  {}{c}{}\n    {}{c}{} → {}\n",
                change_text(g.change).get(lang),
                combos(g.count, lang),
                tr!("効いたポリシー", "deciding policies").get(lang),
                policies_text(&g.old_policies, lang),
                policies_text(&g.new_policies, lang)
            ));
            for e in &g.examples {
                o.push_str(&format!("    - {}\n", e.said.get(lang)));
            }
        }
        if a.unknown > 0 {
            o.push_str(&format!("  {}{c}{}\n", tr!("決められない", "cannot be decided").get(lang), combos(a.unknown, lang)));
            o.push_str(&format!(
                "    {}\n",
                tr!(
                    "ほかの言語が言えない値に頼り、その値になる具体的な入力も見つかりませんでした",
                    "they rest on a value another language could not vouch for, and no concrete input was found that gives it"
                )
                .get(lang)
            ));
            for e in &a.unknown_examples {
                o.push_str(&format!("    - {}\n", e.said.get(lang)));
            }
        }
    }
    o.push('\n');
    o.push_str(&closing(d, lang, false));
    o
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

/// The diff as Markdown, `--lang` `lang`.
pub fn markdown(d: &Diff, lang: Lang) -> String {
    let c = colon(lang);
    let mut o = format!("### sekisho diff{c}{} {}\n\n", tr!("ゲート", "gate").get(lang), title(d, "`"));
    o.push_str(&format!("- {}{c}`{}`\n", tr!("古い版", "Old").get(lang), d.old.file));
    o.push_str(&format!("- {}{c}`{}`\n", tr!("新しい版", "New").get(lang), d.new.file));
    for (only, what) in [(&d.only_old, tr!("古い版だけにあるもの", "Only in the old version")), (&d.only_new, tr!("新しい版だけにあるもの", "Only in the new version"))] {
        let lines = only_lines(only, lang);
        if !lines.is_empty() {
            o.push_str(&format!("- {}{c}{}\n", what.get(lang), lines.join(if lang == Lang::Ja { "。" } else { "; " })));
        }
    }
    for a in &d.actions {
        o.push_str(&format!("\n#### `{}`\n\n", a.name));
        o.push_str(&format!("{}\n", tr!("両方の版を合わせて {}をたどりました。", "{} walked over both versions.", combos(a.combinations, lang); ritsu_base::text::capitalize(&combos(a.combinations, lang))).get(lang)));
        for (frames, what) in [(&a.frames_only_old, tr!("古い版だけが受け付ける principal と resource", "Principal and resource only the old version takes")), (&a.frames_only_new, tr!("新しい版だけが受け付ける principal と resource", "Principal and resource only the new version takes"))] {
            if !frames.is_empty() {
                o.push_str(&format!("\n{}{c}{}\n", what.get(lang), join(frames, lang)));
            }
        }
        if !a.groups.is_empty() || a.unknown > 0 {
            o.push_str(&format!(
                "\n| {} | {} | {} | {} | {} |\n|---|---|---|---|---|\n",
                tr!("変わり方", "Change").get(lang),
                tr!("通り", "Combinations").get(lang),
                tr!("古い版で効いたポリシー", "Old deciding policies").get(lang),
                tr!("新しい版で効いたポリシー", "New deciding policies").get(lang),
                tr!("例", "Example").get(lang)
            ));
            for g in &a.groups {
                let example = g.examples.first().map(|e| e.said.get(lang).to_string()).unwrap_or_default();
                o.push_str(&format!(
                    "| {} | {} | {} | {} | {} |\n",
                    cell(change_text(g.change).get(lang)),
                    count(g.count.min(u64::MAX as u128) as u64),
                    cell(&policies_text(&g.old_policies, lang)),
                    cell(&policies_text(&g.new_policies, lang)),
                    cell(&example)
                ));
            }
            if a.unknown > 0 {
                let example = a.unknown_examples.first().map(|e| e.said.get(lang).to_string()).unwrap_or_default();
                o.push_str(&format!("| {} | {} |  |  | {} |\n", tr!("決められない", "Cannot be decided").get(lang), count(a.unknown.min(u64::MAX as u128) as u64), cell(&example)));
            }
        }
    }
    o.push('\n');
    o.push_str(&closing(d, lang, true));
    o
}

fn shown_json(e: &Shown, lang: Lang) -> Json {
    Json::obj(e.values.iter().map(|(k, v)| (k.clone(), Json::str(v.get(lang)))))
}

fn names_json(xs: &[String]) -> Json {
    Json::arr(xs.iter().map(|x| Json::str(x.as_str())))
}

fn only_json(o: &Only) -> Json {
    Json::obj([
        ("actions", names_json(&o.actions)),
        ("roles", names_json(&o.roles)),
        ("attributes", names_json(&o.attributes)),
        ("inputs", names_json(&o.inputs)),
        ("types", names_json(&o.types)),
    ])
}

fn big(n: u128) -> Json {
    Json::int(n.min(i128::MAX as u128) as i128)
}

/// The diff as JSON (ritsu's DESIGN 17.21), the examples' values in `lang`.
pub fn to_json(d: &Diff, lang: Lang) -> Json {
    let group = |g: &Group| {
        Json::obj([
            ("count", big(g.count)),
            ("old_policies", names_json(&g.old_policies)),
            ("new_policies", names_json(&g.new_policies)),
            ("examples", Json::arr(g.examples.iter().map(|e| shown_json(e, lang)))),
        ])
    };
    let actions = d.actions.iter().map(|a| {
        let of = |c: Change| Json::arr(a.groups.iter().filter(|g| g.change == c).map(group));
        let unknown = if a.unknown > 0 { vec![Json::obj([("count", big(a.unknown)), ("examples", Json::arr(a.unknown_examples.iter().map(|e| shown_json(e, lang))))])] } else { Vec::new() };
        Json::obj([
            ("action", Json::str(a.name.as_str())),
            ("combinations", big(a.combinations)),
            ("gained", of(Change::Gained)),
            ("lost", of(Change::Lost)),
            ("reasons", of(Change::Reasons)),
            ("unknown", Json::arr(unknown)),
            ("frames_only_old", names_json(&a.frames_only_old)),
            ("frames_only_new", names_json(&a.frames_only_new)),
            ("same_elsewhere", Json::from(a.unknown == 0)),
        ])
    });
    let head = |h: &Head| Json::obj([("file", Json::str(h.file.as_str())), ("name", Json::str(h.name.as_str())), ("version", Json::str(h.version.as_str()))]);
    Json::obj([
        ("sekisho", Json::str(env!("CARGO_PKG_VERSION"))),
        ("old", head(&d.old)),
        ("new", head(&d.new)),
        ("actions", Json::arr(actions)),
        ("only_old", only_json(&d.only_old)),
        ("only_new", only_json(&d.only_new)),
        (
            "total",
            Json::obj([("gained", big(d.total(Change::Gained))), ("lost", big(d.total(Change::Lost))), ("reasons", big(d.total(Change::Reasons))), ("unknown", big(d.unknown()))]),
        ),
    ])
}
