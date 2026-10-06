//! The checks of every combination (DESIGN 4.2–4.5): each action is walked once ([`crate::walk`]),
//! every combination decided as Cedar decides it ([`crate::eval`]), and from what the walk counts:
//!
//! - E301: an action no permit allows in any combination (`nobody "<reason>"` says it is meant;
//!   then a combination that is allowed breaks it, E304);
//! - E302: a permit a forbid overrides in every combination it would allow;
//! - E303: a permit or a forbid whose lines never hold together;
//! - W301: a permit another single permit covers (the later of two that cover each other);
//! - E304: an expectation that some combination it picks breaks;
//! - W304: an expectation that picks no combination, which holds and checks nothing;
//! - E305: two actions of a separation allowed to the same principal;
//! - E306 and W302: the reach of a role against its `can`;
//! - E307: an action past the budget, which stops the check of that action;
//! - W303: an answer that rests on a value another language could not vouch for, when no concrete
//!   input gives it (the walk counted it in case it can happen).
//!
//! Every answer that says some combination exists (an action someone may take, a permit that
//! allows something, an example that breaks an expectation) is given from a combination the
//! languages said can happen, or from one a concrete input was found for ([`walk::confirm`]);
//! every answer that says no combination does is given from the walk as it is, since counting
//! more combinations than can happen cannot hide one that does.

use crate::borders::Langs;
use crate::diag::{Diag, Example};
use crate::model::*;
use crate::walk::{self, Ask, Combo, Kept, Known, Memo, Space, Stop, Val};
use ritsu_base::text::{Text, count};
use std::collections::{BTreeMap, BTreeSet};

/// What sekisho checks of a file whose names hold (`names.rs` gave its scope), with the languages
/// the suite joins: the borders, the contracts, and every combination of every action, walking
/// at most `budget` combinations an action (E307 past it). `root` is the root the references are
/// written from (DESIGN 2.6).
pub fn run(scope: &crate::names::Scope, suite: &crate::suite::Suite, budget: u64, root: &std::path::Path) -> (Vec<Diag>, Checked) {
    let imports: Vec<&crate::ast::File> = scope.files[1..].iter().collect();
    let ports = match (&suite.rules, &suite.dates, &suite.books) {
        (Some(r), Some(d), Some(b)) => Some(ritsu_ports::Ports { rules: r.clone(), dates: d.clone(), books: b.clone() }),
        _ => None,
    };
    let langs = Langs {
        rules: suite.rules.as_deref().filter(|r| r.joined()),
        dates: suite.dates.as_deref().filter(|d| d.joined()),
        books: suite.books.as_deref().filter(|b| b.joined()),
        flows: match (&suite.flows, ports) {
            (Some(f), Some(p)) => Some((f.as_ref(), p)),
            _ => None,
        },
    };
    all(scope.file(), &imports, &langs, budget as u128, root)
}

/// What sekisho checks of a file whose names, types and units pass (`names.rs` said no error), with
/// the files its `use gate` lines read: the borders with the other languages, the contracts its
/// actions guard, and every combination of every action. The diagnostics, in the order of the
/// file, and what the checks found; each operation the check of the contracts finds is kept in the
/// model as its reference.
pub fn all(f: &crate::ast::File, imports: &[&crate::ast::File], langs: &Langs, budget: u128, root: &std::path::Path) -> (Vec<Diag>, Checked) {
    let mut gate = build(f, imports, root);
    let (mut diags, known) = crate::borders::check(&gate, langs);
    let (said, found) = crate::contracts::check(&gate, langs.books);
    diags.extend(said);
    for (ai, gi, reference) in found {
        gate.actions[ai].guards[gi].reference = Some(reference);
    }
    let (more, report) = check(&gate, &known, langs, budget);
    diags.extend(more);
    crate::diag::sort(&mut diags);
    (diags, Checked { gate, known, report })
}

/// What the checks of a file found: its model, what the other languages said of its computed
/// values, and the walks.
pub struct Checked {
    pub gate: Gate,
    pub known: Known,
    pub report: Report,
}

/// The walk's questions, answered by the ports of rulec and koyomi.
impl Ask for Langs<'_> {
    fn outputs(&self, rule: &std::path::Path, output: &str, ranges: &[(String, Option<i128>, Option<i128>)]) -> Result<ritsu_ports::Found<Vec<(ritsu_ports::Value, ritsu_ports::Values)>>, Vec<ritsu_ports::Said>> {
        match self.rules {
            Some(r) => r.outputs_over(rule, output, ranges),
            None => Err(Vec::new()),
        }
    }

    fn rule(&self, rule: &std::path::Path, inputs: &ritsu_ports::Values) -> Result<ritsu_ports::Values, ritsu_ports::RuleError> {
        match self.rules {
            Some(r) => r.eval(rule, inputs),
            None => Err(ritsu_ports::RuleError::Unread(Vec::new())),
        }
    }

    fn dates(&self, file: &std::path::Path, inputs: &[(String, i64)]) -> Result<Vec<(String, ritsu_ports::DateValue)>, Vec<ritsu_ports::Said>> {
        match self.dates {
            Some(d) => d.eval(file, inputs),
            None => Err(Vec::new()),
        }
    }
}

/// The combinations an action may come to before the check stops (DESIGN 4.1): koyomi's.
pub const BUDGET: u128 = 100_000_000;

/// How many combinations that rest on an inexact value are tried with a concrete input, before an
/// answer is said to be undecided.
const TRIES: usize = 64;

/// What the walks found, for the page, the tests and the port (`Gates`).
#[derive(Clone, Debug, Default)]
pub struct Report {
    /// Each action walked: its space and its counts (None for one that was not walked: over the
    /// budget, or reading a value no language answered).
    pub spaces: Vec<Option<Space>>,
    pub actions: Vec<ActionCount>,
    /// Each policy, by its index in the file: its counts over every action it is on.
    pub policies: Vec<PolicyCount>,
    pub expects: Vec<ExpectCount>,
    /// Each separation: whether it holds (None when it could not be decided).
    pub separations: Vec<Option<bool>>,
    /// Each role: the actions a principal holding it alone is allowed (None when undecided).
    pub reach: Vec<Option<BTreeSet<usize>>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActionCount {
    pub combinations: u128,
    pub allowed: u128,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PolicyCount {
    /// The combinations it applies to.
    pub holds: u128,
    /// The combinations it decides (a permit: it applies and the answer is allow; a forbid: it
    /// applies), and those it decides alone.
    pub decides: u128,
    pub alone: u128,
    /// A forbid: the combinations a permit applies to that it turns into a deny.
    pub overturns: u128,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExpectCount {
    pub picked: u128,
    pub broken: u128,
}

/// A combination kept as a witness: the first the languages vouch for, and some that rest on an
/// inexact value, to try with a concrete input when no other is had. Those are taken across the
/// whole walk — the first few, then the 2ⁿth — so that they are not all of one corner of it.
#[derive(Clone, Debug, Default)]
struct Witness {
    exact: Option<(usize, Kept)>,
    loose: Vec<(usize, Kept)>,
    /// How many inexact combinations were offered.
    seen: u64,
}

impl Witness {
    fn add(&mut self, action: usize, c: &Combo) {
        if self.exact.is_some() {
            return;
        }
        if c.exact {
            self.exact = Some((action, c.keep()));
            return;
        }
        self.seen += 1;
        if (self.seen <= 8 || self.seen.is_power_of_two()) && self.loose.len() < TRIES {
            self.loose.push((action, c.keep()));
        }
    }

    fn any(&self) -> bool {
        self.exact.is_some() || !self.loose.is_empty()
    }
}

/// What the walk counts of one policy, over every action it is on.
#[derive(Clone, Debug, Default)]
struct PolAcc {
    count: PolicyCount,
    holds: Witness,
    decides: Witness,
    /// The first combination it applies to, for the example of E302.
    first: Option<(usize, Kept)>,
    /// A permit: for each forbid (by its index in the file), the combinations both apply to.
    by_forbid: BTreeMap<usize, u128>,
    /// A permit: the permits (by index in the file) that apply in every combination it decides.
    covered: Option<BTreeSet<usize>>,
    /// The combinations its `principal` line holds on, and each of its lines within them.
    scope: u128,
    lines: Vec<u128>,
}

#[derive(Clone, Debug, Default)]
struct ExpAcc {
    count: ExpectCount,
    broken: Witness,
    /// The combinations its `principal` line holds on, and each of its lines within them.
    scope: u128,
    lines: Vec<u128>,
}

/// Who a principal is, as far as an action reads it: its type, the roles walked it holds, the
/// attributes walked, and the workflow.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Who2 {
    pub ty: usize,
    pub direct: u128,
    pub attrs: Vec<(usize, Val)>,
    pub workflow: Option<usize>,
}

/// What an action reads of the principals of a type: the roles walked and the attributes walked.
#[derive(Clone, Debug, Default, PartialEq)]
struct Reads {
    roles: u128,
    attrs: BTreeSet<usize>,
}

/// The principals an action allows, each with a witness.
#[derive(Clone, Debug, Default)]
struct Allowed {
    who: BTreeMap<Who2, Witness>,
    reads: BTreeMap<usize, Reads>,
}

fn who_of(f: &walk::Frame, env: &[Val]) -> Who2 {
    let direct = match &env[0] {
        Val::Roles { direct, .. } => *direct,
        _ => 0,
    };
    let mut attrs = Vec::new();
    let mut workflow = None;
    for (at, s) in f.slots.iter().enumerate() {
        if !f.factors.iter().any(|x| x.places.contains(&at)) {
            continue;
        }
        match s {
            walk::Slot::Attr(Owner::Principal, i) => attrs.push((*i, env[at].clone())),
            walk::Slot::Workflow => {
                if let Val::Workflow(w) = env[at] {
                    workflow = Some(w);
                }
            }
            _ => {}
        }
    }
    attrs.sort();
    Who2 { ty: f.principal, direct, attrs, workflow }
}

fn reads_of(f: &walk::Frame) -> Reads {
    let attrs = f
        .slots
        .iter()
        .enumerate()
        .filter(|(at, _)| f.factors.iter().any(|x| x.places.contains(at)))
        .filter_map(|(_, s)| if let walk::Slot::Attr(Owner::Principal, i) = s { Some(*i) } else { None })
        .collect();
    Reads { roles: f.walked_roles, attrs }
}

/// Whether two principals, each as one action reads it, can be one principal: they agree on what
/// both actions read. The principal they are together, when they can.
fn join(a: &Who2, ra: &Reads, b: &Who2, rb: &Reads) -> Option<Who2> {
    if a.ty != b.ty || (a.direct & rb.roles) != (b.direct & ra.roles) {
        return None;
    }
    if let (Some(x), Some(y)) = (a.workflow, b.workflow)
        && x != y
    {
        return None;
    }
    let mut attrs = a.attrs.clone();
    for (i, v) in &b.attrs {
        match a.attrs.iter().find(|(j, _)| j == i) {
            Some((_, w)) if w != v => return None,
            Some(_) => {}
            None => attrs.push((*i, v.clone())),
        }
    }
    attrs.sort();
    Some(Who2 { ty: a.ty, direct: a.direct | b.direct, attrs, workflow: a.workflow.or(b.workflow) })
}

/// Two actions of a separation, the principal both allow, and a combination of each.
type Broken = (usize, usize, Who2, (usize, Kept), (usize, Kept));

/// Which combinations are an asker's, for [`allowance`].
type Pick<'a> = Box<dyn Fn(&walk::Frame, &[Val]) -> bool + 'a>;

/// The checks of every combination of a file whose names, types and borders have no error.
/// `known` is what the other languages said of its computed values; `ask` answers what the walk
/// asks them.
pub fn check(g: &Gate, known: &Known, ask: &dyn Ask, budget: u128) -> (Vec<Diag>, Report) {
    let memo = Memo::default();
    let mut diags = Vec::new();
    let mut report = Report { spaces: vec![None; g.actions.len()], actions: vec![ActionCount::default(); g.actions.len()], ..Report::default() };
    let mut pols: Vec<PolAcc> = vec![PolAcc::default(); g.policies.len()];
    for (i, p) in g.policies.iter().enumerate() {
        pols[i].lines = vec![0; p.conds.len()];
    }
    let mut exps: Vec<ExpAcc> = vec![ExpAcc::default(); g.expects.len()];
    for (i, e) in g.expects.iter().enumerate() {
        exps[i].lines = vec![0; e.conds.len()];
    }
    let mut allowed: Vec<Allowed> = vec![Allowed::default(); g.actions.len()];
    let mut action_allowed: Vec<Witness> = vec![Witness::default(); g.actions.len()];
    let mut walked = vec![false; g.actions.len()];
    let mut stopped = false;
    for (ai, a) in g.actions.iter().enumerate() {
        let s = match walk::space(g, ai, known, ask, &memo, budget) {
            Ok(s) => s,
            Err(Stop::Budget { count: n, days, why, roles }) => {
                diags.push(over_budget(g, a, n, days, roles, budget, &why));
                stopped = true;
                continue;
            }
            Err(Stop::Unknown) => {
                stopped = true;
                continue;
            }
        };
        for f in &s.frames {
            allowed[ai].reads.insert(f.principal, reads_of(f));
        }
        let mut ac = ActionCount::default();
        s.each(|c| {
            ac.combinations += 1;
            let f = &s.frames[c.frame];
            if c.decision.allow {
                ac.allowed += 1;
                action_allowed[ai].add(ai, c);
                allowed[ai].who.entry(who_of(f, c.env)).or_default().add(ai, c);
            }
            for (k, &pi) in s.policies.iter().enumerate() {
                let acc = &mut pols[pi];
                let comp = &f.policies[k];
                if comp.scope.holds(c.env) {
                    acc.scope += 1;
                    for (j, line) in comp.conds.iter().enumerate() {
                        if line.holds(c.env) {
                            acc.lines[j] += 1;
                        }
                    }
                }
                if !c.applies[k] {
                    continue;
                }
                acc.count.holds += 1;
                acc.holds.add(ai, c);
                if acc.first.is_none() {
                    acc.first = Some((ai, c.keep()));
                }
                if s.permit[k] {
                    for (j, &pj) in s.policies.iter().enumerate() {
                        if c.applies[j] && !s.permit[j] {
                            *acc.by_forbid.entry(pj).or_default() += 1;
                        }
                    }
                } else if (0..s.policies.len()).any(|j| c.applies[j] && s.permit[j]) {
                    acc.count.overturns += 1;
                }
                if c.decision.determining.contains(&k) {
                    acc.count.decides += 1;
                    acc.decides.add(ai, c);
                    if c.decision.determining.len() == 1 {
                        acc.count.alone += 1;
                    }
                    if s.permit[k] {
                        let with: BTreeSet<usize> = c.decision.determining.iter().filter(|&&j| j != k).map(|&j| s.policies[j]).collect();
                        acc.covered = Some(match acc.covered.take() {
                            None => with,
                            Some(prev) => prev.intersection(&with).copied().collect(),
                        });
                    }
                }
            }
            for (k, &ei) in s.expects.iter().enumerate() {
                let comp = &f.expects[k];
                if comp.scope.holds(c.env) {
                    let acc = &mut exps[ei];
                    acc.scope += 1;
                    for (j, line) in comp.conds.iter().enumerate() {
                        if line.holds(c.env) {
                            acc.lines[j] += 1;
                        }
                    }
                }
                if comp.all.holds(c.env) {
                    let acc = &mut exps[ei];
                    acc.count.picked += 1;
                    if c.decision.allow != g.expects[ei].allow {
                        acc.count.broken += 1;
                        acc.broken.add(ai, c);
                    }
                }
            }
        });
        report.actions[ai] = ac;
        walked[ai] = true;
        report.spaces[ai] = Some(s);
    }
    let spaces = &report.spaces;
    let confirmed = |w: &Witness| -> Result<Option<(usize, Kept)>, ()> {
        if let Some(x) = &w.exact {
            return Ok(Some(x.clone()));
        }
        if w.loose.is_empty() {
            return Ok(None);
        }
        for (ai, k) in &w.loose {
            if let Some(s) = &spaces[*ai]
                && walk::confirm(g, s, k, known, ask)
            {
                return Ok(Some((*ai, k.clone())));
            }
        }
        Err(())
    };

    // the actions: someone may take each (E301), or no one, as `nobody` says (E304)
    for (ai, a) in g.actions.iter().enumerate() {
        if !walked[ai] {
            continue;
        }
        let s = spaces[ai].as_ref().unwrap();
        let n = report.actions[ai].combinations;
        let k = report.actions[ai].allowed;
        match (&a.nobody, confirmed(&action_allowed[ai])) {
            (None, Ok(None)) => diags.push(nobody_allowed(g, ai, s, n)),
            (None, Err(())) => diags.push(undecided(g, a.named.line, tr!("`{}` を許される principal がいる", "anyone is allowed `{}`", a.named.name))),
            (Some((_, line)), Ok(Some((x, kept)))) => diags.push(nobody_broken(g, a, *line, n, k, spaces[x].as_ref().unwrap(), &kept)),
            (Some((_, line)), Err(())) => diags.push(undecided(g, *line, tr!("`{}` をだれも許されない", "no one is allowed `{}`", a.named.name))),
            _ => {}
        }
    }

    // the policies: each applies somewhere (E303), a permit allows something (E302), and adds
    // something no other single permit does (W301)
    for (pi, p) in g.policies.iter().enumerate() {
        // a forbid read from another file is that file's to answer for (its own check says it)
        if p.from.is_some() || !p.actions.iter().all(|a| walked[*a]) || p.actions.is_empty() {
            continue;
        }
        let acc = &pols[pi];
        if !acc.holds.any() {
            diags.push(never_applies(g, pi, acc.scope, &acc.lines));
            continue;
        }
        match confirmed(&acc.holds) {
            Err(()) => {
                diags.push(undecided(g, p.named.line, tr!("{} `{}` が当てはまる組み合わせがある", "the {} `{}` applies to any combination", effect(p), p.named.name)));
                continue;
            }
            Ok(None) => {
                diags.push(never_applies(g, pi, acc.scope, &acc.lines));
                continue;
            }
            Ok(Some(_)) => {}
        }
        if !p.permit {
            continue;
        }
        if !acc.decides.any() {
            diags.push(overridden(g, pi, acc, spaces));
            continue;
        }
        match confirmed(&acc.decides) {
            Err(()) => {
                diags.push(undecided(g, p.named.line, tr!("permit `{}` が何かを許す", "the permit `{}` allows anything", p.named.name)));
                continue;
            }
            Ok(None) => {
                diags.push(overridden(g, pi, acc, spaces));
                continue;
            }
            Ok(Some(_)) => {}
        }
        // covered by another permit: name the first that covers it, unless the two cover each
        // other and this one comes first (then the other is said to be the one to remove)
        if let Some(cov) = &acc.covered {
            let by = cov.iter().copied().find(|&q| {
                let mutual = pols[q].covered.as_ref().is_some_and(|c| c.contains(&pi));
                !(mutual && p.named.line < g.policies[q].named.line)
            });
            if let Some(q) = by {
                diags.push(covered(g, pi, q, acc.count.decides));
            }
        }
    }

    // the expectations (E304, W304)
    for (ei, e) in g.expects.iter().enumerate() {
        if !e.actions.iter().all(|a| walked[*a]) || e.actions.is_empty() {
            continue;
        }
        let acc = &exps[ei];
        // an expectation that picks no combination holds, and checks nothing
        if acc.count.picked == 0 {
            diags.push(picks_nothing(g, ei, acc.scope, &acc.lines));
            continue;
        }
        if acc.count.broken == 0 {
            continue;
        }
        match confirmed(&acc.broken) {
            Ok(Some((x, kept))) => diags.push(broken(g, ei, &acc.count, spaces[x].as_ref().unwrap(), &kept)),
            Ok(None) => {}
            Err(()) => diags.push(undecided(g, e.line, tr!("期待 `{}` が成り立つ", "the expectation `{}` holds", e.name))),
        }
    }

    // the separations (E305)
    report.separations = vec![Some(true); g.separates.len()];
    for (si, sep) in g.separates.iter().enumerate() {
        if !sep.actions.iter().all(|a| walked[*a]) {
            report.separations[si] = None;
            continue;
        }
        let mut found: Option<Broken> = None;
        let mut loose = false;
        'pairs: for (x, &a) in sep.actions.iter().enumerate() {
            for &b in &sep.actions[x + 1..] {
                for (wa, va) in &allowed[a].who {
                    let Some(ra) = allowed[a].reads.get(&wa.ty) else { continue };
                    for (wb, vb) in &allowed[b].who {
                        let Some(rb) = allowed[b].reads.get(&wb.ty) else { continue };
                        let Some(joined) = join(wa, ra, wb, rb) else { continue };
                        match (confirmed(va), confirmed(vb)) {
                            (Ok(Some(ka)), Ok(Some(kb))) => {
                                found = Some((a, b, joined, ka, kb));
                                break 'pairs;
                            }
                            (Err(()), _) | (_, Err(())) => loose = true,
                            _ => {}
                        }
                    }
                }
            }
        }
        match found {
            Some((a, b, who, ka, kb)) => {
                report.separations[si] = Some(false);
                diags.push(not_separate(g, si, a, b, &who, (spaces[ka.0].as_ref().unwrap(), &ka.1), (spaces[kb.0].as_ref().unwrap(), &kb.1)));
            }
            None if loose => {
                report.separations[si] = None;
                diags.push(undecided(g, sep.line, tr!("職務の分離 `{}` が成り立つ", "the separation `{}` holds", sep.name)));
            }
            None => {}
        }
    }

    // the reach of each role (E306, W302)
    report.reach = vec![None; g.roles.len()];
    for (ri, r) in g.roles.iter().enumerate() {
        let mut reach = BTreeSet::new();
        let mut example: BTreeMap<usize, (usize, Kept)> = BTreeMap::new();
        let mut loose: BTreeSet<usize> = BTreeSet::new();
        for (ai, a) in g.actions.iter().enumerate() {
            if !walked[ai] {
                continue;
            }
            for &t in &a.principals {
                if !g.types[t].roles.contains(&ri) {
                    continue;
                }
                let Some(reads) = allowed[ai].reads.get(&t) else { continue };
                let alone = walk::bit(ri) & reads.roles;
                for (w, wit) in &allowed[ai].who {
                    if w.ty != t || w.direct != alone {
                        continue;
                    }
                    match confirmed(wit) {
                        Ok(Some(k)) => {
                            reach.insert(ai);
                            example.entry(ai).or_insert(k);
                        }
                        Err(()) => {
                            loose.insert(ai);
                        }
                        Ok(None) => {}
                    }
                }
            }
        }
        let decided = loose.iter().all(|a| reach.contains(a));
        report.reach[ri] = decided.then(|| reach.clone());
        let Some((can, line)) = &r.can else { continue };
        for &ai in &reach {
            if !can.contains(&ai) {
                let (x, k) = &example[&ai];
                diags.push(beyond_can(g, ri, *line, ai, spaces[*x].as_ref().unwrap(), k));
            }
        }
        for &ai in can {
            if reach.contains(&ai) || !walked[ai] {
                continue;
            }
            if loose.contains(&ai) {
                diags.push(undecided(g, *line, tr!("役割 `{}` が `{}` を許される", "the role `{}` is allowed `{}`", r.named.name, g.actions[ai].named.name)));
            } else {
                diags.push(short_of_can(g, ri, *line, ai));
            }
        }
    }
    report.policies = pols.into_iter().map(|p| p.count).collect();
    report.expects = exps.into_iter().map(|e| e.count).collect();
    let _ = stopped;
    (diags, report)
}

// ---------------------------------------------------------------------------------------------
// The diagnostics

fn effect(p: &Policy) -> &'static str {
    if p.permit { "permit" } else { "forbid" }
}

fn at(code: &'static str, g: &Gate, line: usize, message: Text) -> Diag {
    Diag::at(code, &g.file, line, 1, message).source(&g.src)
}

fn n(x: u128) -> String {
    count(x.min(u64::MAX as u128) as u64)
}

/// `is` for one, `are` for more.
fn are(x: u128) -> &'static str {
    if x == 1 { "is" } else { "are" }
}

/// A combination in words, and as the values the JSON lists.
fn example(g: &Gate, s: &Space, k: &Kept) -> (Text, Example) {
    (walk::say(g, s, k), Example { values: walk::values(g, s, k) })
}

/// The policies that decided a combination, and what they did, in a sentence.
fn decided_by(g: &Gate, s: &Space, k: &Kept) -> Text {
    let d = s.decision_of(k);
    let names: Vec<Text> = d.determining.iter().map(|&i| Text::same(format!("`{}`", g.policies[s.policies[i]].named.name))).collect();
    let l = Text::list(&names);
    match (d.allow, names.is_empty()) {
        (true, _) => tr!("{} が許します", "allowed by {}", l.ja; l.en),
        (false, false) => tr!("{} が拒みます", "denied by {}", l.ja; l.en),
        (false, true) => tr!("どの permit も当てはまらないので拒まれます", "denied: no permit applies"),
    }
}

fn for_example(g: &Gate, s: &Space, k: &Kept) -> (Text, Example) {
    let (t, ex) = example(g, s, k);
    let by = decided_by(g, s, k);
    (tr!("例：{}。{}。", "For example: {}; {}.", t.ja, by.ja; t.en, by.en), ex)
}

fn nobody_allowed(g: &Gate, ai: usize, s: &Space, combos: u128) -> Diag {
    let a = &g.actions[ai];
    let permits: Vec<Text> = s.policies.iter().filter(|&&p| g.policies[p].permit).map(|&p| Text::same(format!("`{}`", g.policies[p].named.name))).collect();
    let mut d = at("E301", g, a.named.line, tr!("`{}` を許す permit が、どの組み合わせにもありません。だれもこの action をできません", "No permit allows `{}` in any combination: no one can do it", a.named.name));
    d = d.note(if permits.is_empty() {
        tr!("この action の permit がありません。", "No permit is written for it.")
    } else {
        let l = Text::list(&permits);
        tr!(
            "この action の permit（{}）は、どの組み合わせでも当てはまらないか、許すはずの組み合わせがどれも forbid に拒まれます。",
            "Its permits ({}) never apply, or a forbid denies every combination they would allow.",
            l.ja;
            l.en
        )
    });
    d.note(tr!("{} 通りの組み合わせを数えました。", "It comes to {}.", n(combos); ritsu_base::text::plural(combos as usize, "combination", "combinations")))
        .note(tr!(
            "permit を書いてください。だれにもさせないつもりなら、action の下に `nobody \"<理由>\"` を書いてください。",
            "Write a permit for it; if no one is meant to do it, write `nobody \"<why>\"` under the action."
        ))
}

fn nobody_broken(g: &Gate, a: &Action, line: usize, combos: u128, k: u128, s: &Space, kept: &Kept) -> Diag {
    let (t, ex) = for_example(g, s, kept);
    at("E304", g, line, tr!("`nobody` は、だれも `{}` をできないと書いていますが、{} 通りのうち {} 通りが許されます", "`nobody` says no one may `{}`, and {} of its {} combinations {} allowed", a.named.name, n(combos), n(k); a.named.name, n(k), n(combos), are(k)))
        .note(t)
        .note(tr!("許す permit を直すか、`nobody` の行を消してください。", "Fix the permits that allow it, or remove the `nobody` line."))
        .with(ex)
}

fn never_applies(g: &Gate, pi: usize, scope: u128, lines: &[u128]) -> Diag {
    let p = &g.policies[pi];
    let e = effect(p);
    let d = at("E303", g, p.named.line, tr!("{e} `{}` は、どの組み合わせにも当てはまりません", "The {e} `{}` applies to no combination", p.named.name));
    let d = lines_note(d, g, &p.who, &p.actions, &p.conds, scope, lines);
    d.note(tr!("満たせない行か、{e} そのものを消してください。", "Remove the line that cannot be met, or the {e} itself."))
}

/// An expectation that picks no combination (W304): it holds, and checks nothing.
fn picks_nothing(g: &Gate, ei: usize, scope: u128, lines: &[u128]) -> Diag {
    let e = &g.expects[ei];
    let d = at("W304", g, e.line, tr!("期待 `{}` は、どの組み合わせも選びません。成り立ちますが、何も確かめていません", "The expectation `{}` picks no combination: it holds, and checks nothing", e.name));
    let d = lines_note(d, g, &e.who, &e.actions, &e.conds, scope, lines);
    d.note(tr!("同時に満たせない行を直すか、期待を消してください。", "Correct the lines that cannot be met together, or remove the expectation."))
}

/// What the lines of a policy or an expectation that holds nowhere are met on: the `principal` line
/// on no principal the actions take, or each of the other lines on some, and never all at once.
fn lines_note(mut d: Diag, g: &Gate, who: &Who, actions: &[usize], conds: &[Cond], scope: u128, lines: &[u128]) -> Diag {
    let acts: Vec<Text> = actions.iter().map(|a| Text::same(format!("`{}`", g.actions[*a].named.name))).collect();
    let acts = Text::list(&acts);
    if scope == 0 {
        d = d.note(tr!("`principal` の行が、{} の principal のどれにも当てはまりません。", "Its `principal` line holds for no principal {} takes.", acts.ja; acts.en));
    } else if !conds.is_empty() {
        // the combinations the lines are counted over: those of the `principal` line, or with no
        // such line every combination of the actions
        let over = if *who == Who::Anyone {
            tr!("{} の {} 通りのうち", "Of the {} combinations of {}", acts.ja, n(scope); n(scope), acts.en)
        } else {
            tr!("`principal` の行が当てはまる {} 通りのうち", "Of the {} combinations its `principal` line takes", n(scope))
        };
        let parts: Vec<Text> = conds
            .iter()
            .zip(lines)
            .map(|(c, k)| {
                let w = if c.when { "when" } else { "unless" };
                tr!("`{w} {}` は {} 通り", "`{w} {}` on {}", c.text, n(*k))
            })
            .collect();
        let l = Text::join(&parts, "、", ", ");
        d = d.note(if lines.contains(&0) {
            tr!(
                "{}、{}で満たされます。どの組み合わせでも満たされない行があります。",
                "{}, the lines are met: {}. A line is met on none.",
                over.ja,
                l.ja;
                over.en,
                l.en
            )
        } else {
            tr!(
                "{}、{}で満たされますが、全部が同時に満たされることはありません。",
                "{}, the lines are met: {}; but never all at once.",
                over.ja,
                l.ja;
                over.en,
                l.en
            )
        });
    }
    d
}

fn overridden(g: &Gate, pi: usize, acc: &PolAcc, spaces: &[Option<Space>]) -> Diag {
    let p = &g.policies[pi];
    let mut parts: Vec<(u128, String)> = acc.by_forbid.iter().map(|(f, k)| (*k, g.policies[*f].named.name.clone())).collect();
    parts.sort_by(|a, b| b.0.cmp(&a.0));
    // `f1 denies 192 of them, f2 96`: the first with the verb, the rest after it
    let en: Vec<String> = parts.iter().enumerate().map(|(i, (k, name))| if i == 0 { format!("`{name}` denies {} of them", n(*k)) } else { format!("`{name}` {}", n(*k)) }).collect();
    let ja: Vec<String> = parts.iter().map(|(k, name)| format!("`{name}` が {} 通り", n(*k))).collect();
    let mut d = at("E302", g, p.named.line, tr!("permit `{}` は何も許しません。許すはずの組み合わせを、どれも forbid が拒みます", "The permit `{}` allows nothing: a forbid denies every combination it would allow", p.named.name))
        .note(tr!("許すはずの組み合わせは {} 通りで、{}を拒みます。", "It would allow {}; {}.", n(acc.count.holds), ja.join("、"); ritsu_base::text::plural(acc.count.holds as usize, "combination", "combinations"), en.join(", ")));
    if let Some((ai, k)) = &acc.first
        && let Some(s) = &spaces[*ai]
    {
        let (t, ex) = for_example(g, s, k);
        d = d.note(t).with(ex);
    }
    d.note(tr!("forbid を狭めるか、だれにも許さないつもりなら permit を消してください。", "Narrow the forbid, or remove the permit if no one is meant to be allowed this."))
}

fn covered(g: &Gate, pi: usize, q: usize, decides: u128) -> Diag {
    let (p, other) = (&g.policies[pi].named.name, &g.policies[q].named.name);
    at("W301", g, g.policies[pi].named.line, tr!("permit `{p}` が許す組み合わせは、どれも `{other}` も許します", "Every combination the permit `{p}` allows, `{other}` allows too"))
        .note(tr!("`{p}` は {} 通りを許し、そのどれも `{other}` が許すので、`{p}` を消しても答えは変わりません。", "It allows {}, and `{other}` allows each of them: removing `{p}` changes no answer.", n(decides); ritsu_base::text::plural(decides as usize, "combination", "combinations")))
        .note(tr!("`{p}` を消すか、`{p}` のほうが意図どおりなら `{other}` を狭めてください。", "Remove `{p}`, or narrow `{other}` if `{p}` says what is meant."))
}

fn broken(g: &Gate, ei: usize, c: &ExpectCount, s: &Space, k: &Kept) -> Diag {
    let e = &g.expects[ei];
    let msg = if e.allow {
        tr!("期待 `{}` が成り立ちません。選んだ {} 通りのうち {} 通りが拒まれます", "The expectation `{}` does not hold: {} of the {} combinations it picks {} denied", e.name, n(c.picked), n(c.broken); e.name, n(c.broken), n(c.picked), are(c.broken))
    } else {
        tr!("期待 `{}` が成り立ちません。選んだ {} 通りのうち {} 通りが許されます", "The expectation `{}` does not hold: {} of the {} combinations it picks {} allowed", e.name, n(c.picked), n(c.broken); e.name, n(c.broken), n(c.picked), are(c.broken))
    };
    let (t, ex) = for_example(g, s, k);
    at("E304", g, e.line, msg).note(t).note(tr!("ポリシーを直すか、期待が言いすぎているなら期待を直してください。", "Fix the policies, or the expectation if it says more than is meant.")).with(ex)
}

fn not_separate(g: &Gate, si: usize, a: usize, b: usize, who: &Who2, ka: (&Space, &Kept), kb: (&Space, &Kept)) -> Diag {
    let sep = &g.separates[si];
    let (an, bn) = (&g.actions[a].named.name, &g.actions[b].named.name);
    let principal = say_who(g, who);
    let (ta, ex) = example(g, ka.0, ka.1);
    let (tb, _) = example(g, kb.0, kb.1);
    let (ba, bb) = (decided_by(g, ka.0, ka.1), decided_by(g, kb.0, kb.1));
    at("E305", g, sep.line, tr!("職務の分離 `{}` が成り立ちません。同じ principal が `{an}` と `{bn}` の両方を許されます", "The separation `{}` does not hold: the same principal is allowed both `{an}` and `{bn}`", sep.name))
        .note(tr!("その principal：{}。", "The principal: {}.", principal.ja; principal.en))
        .note(tr!("`{an}` の例：{}。{}。", "Example of `{an}`: {}; {}.", ta.ja, ba.ja; ta.en, ba.en))
        .note(tr!("`{bn}` の例：{}。{}。", "Example of `{bn}`: {}; {}.", tb.ja, bb.ja; tb.en, bb.en))
        .note(tr!("片方を許される principal がもう片方をできないよう forbid を足すか、permit を狭めてください。", "Add a forbid that keeps whoever is allowed one from the other, or narrow a permit."))
        .with(ex)
}

/// A principal as far as two actions read it: its type, its roles, its attributes.
fn say_who(g: &Gate, w: &Who2) -> Text {
    let t = &g.types[w.ty];
    let held: Vec<Text> = t.roles.iter().filter(|r| w.direct & walk::bit(**r) != 0).map(|r| Text::same(g.roles[*r].named.name.clone())).collect();
    let mut out = if let Some(wf) = w.workflow {
        tr!("ワークフロー {}", "workflow {}", g.workflows[wf].named.name)
    } else if t.roles.is_empty() {
        Text::same(t.named.name.clone())
    } else if held.is_empty() {
        tr!("役割を持たない{}", "{} holding no role", t.named.name)
    } else {
        let l = walk::and_list(&held);
        tr!("{}を持つ{}", "{} holding {}", l.ja, t.named.name; t.named.name, l.en)
    };
    let attrs: Vec<Text> = w
        .attrs
        .iter()
        .map(|(i, v)| {
            let f = &t.attrs[*i];
            let shown = match (v, &f.ty) {
                (Val::Bool(b), _) => if *b { tr!("はい", "yes") } else { tr!("いいえ", "no") },
                (Val::Enum(x), FieldType::Enum(e)) => Text::same(g.enums[*e].values[*x].name.clone()),
                (Val::Cell(c), FieldType::Num { unit, .. }) => crate::cells::show_cell(*c, unit),
                (Val::Absent, _) => tr!("無い", "absent"),
                _ => Text::default(),
            };
            tr!("{}：{}", "{}: {}", f.named.name, shown.ja; f.named.name, shown.en)
        })
        .collect();
    if !attrs.is_empty() {
        let l = Text::join(&attrs, "、", ", ");
        out = Text { ja: format!("{}（{}）", out.ja, l.ja), en: format!("{} ({})", out.en, l.en) };
    }
    out
}

fn beyond_can(g: &Gate, ri: usize, line: usize, ai: usize, s: &Space, k: &Kept) -> Diag {
    let (r, a) = (&g.roles[ri].named.name, &g.actions[ai].named.name);
    let (t, ex) = for_example(g, s, k);
    at("E306", g, line, tr!("役割 `{r}` は `{a}` を許されますが、`can` の行に `{a}` がありません", "The role `{r}` is allowed `{a}`, which its `can` line does not list"))
        .note(t)
        .note(tr!("`can` に `{a}` を足すか、許す permit を狭めてください。", "Add `{a}` to `can`, or narrow the permit that allows it."))
        .with(ex)
}

fn short_of_can(g: &Gate, ri: usize, line: usize, ai: usize) -> Diag {
    let (r, a) = (&g.roles[ri].named.name, &g.actions[ai].named.name);
    let takes = g.actions[ai].principals.iter().any(|t| g.types[*t].roles.contains(&ri));
    let mut d = at("W302", g, line, tr!("役割 `{r}` だけを持つ principal は、`can` の行にある `{a}` を、どの組み合わせでも許されません", "The role `{r}` alone is never allowed `{a}`, which its `can` line lists"));
    if !takes {
        d = d.note(tr!("`{r}` を持てる principal の型は、どれも `{a}` の principal にありません。", "No principal type that holds `{r}` is one `{a}` takes."));
    }
    d.note(tr!("`can` から `{a}` を消すか、許す permit を書いてください。", "Remove `{a}` from `can`, or write the permit that allows it."))
}

fn over_budget(g: &Gate, a: &Action, total: u128, days: bool, roles: bool, budget: u128, why: &[(Text, u128)]) -> Diag {
    let parts: Vec<Text> = why.iter().filter(|(_, k)| *k > 1).map(|(t, k)| tr!("{}（{} 通り）", "{} ({})", t.ja, n(*k); t.en, n(*k))).collect();
    let l = Text::join(&parts, "、", ", ");
    let msg = if days {
        tr!(
            "action `{}` の日付を確かめるには、today と日付に渡す値の組を {} 通り数えることになり、予算の {} 通りを超えます。この action は確かめず、何も生成しません",
            "The dates of the action `{}` come to {} pairs of today and the values they are given, more than the budget of {}: the action is not checked, and nothing is generated",
            a.named.name,
            n(total),
            n(budget)
        )
    } else {
        tr!(
            "action `{}` の組み合わせは {} 通りで、予算の {} 通りを超えます。この action は確かめず、何も生成しません",
            "The action `{}` comes to {} combinations, more than the budget of {}: it is not checked, and nothing is generated",
            a.named.name,
            n(total),
            n(budget)
        )
    };
    let mut d = at("E307", g, a.named.line, msg);
    if !parts.is_empty() {
        d = d.note(tr!("数を増やしているもの：{}。", "What makes them: {}.", l.ja; l.en));
    }
    d.note(if days {
        tr!("today の範囲か、日付に渡す値の範囲を狭めるか、`--budget` で予算を上げてください。", "Narrow the range of today or of what the dates are given, or raise the budget with `--budget`.")
    } else if roles {
        tr!(
            "型の役割を分けるか、役割ごとに `can` で確かめるか、`--budget` で予算を上げてください。",
            "Split the roles of a type into types, check each role with `can`, or raise the budget with `--budget`."
        )
    } else {
        tr!(
            "条件が読む値を減らすか（読む属性、比べる定数）、型を分けるか、`--budget` で予算を上げてください。",
            "Have the conditions read fewer values (fewer attributes, fewer constants to compare with), split the type, or raise the budget with `--budget`."
        )
    })
}

fn undecided(g: &Gate, line: usize, what: Text) -> Diag {
    at("W303", g, line, tr!("{}かどうかを決められません", "Cannot decide whether {}", what.ja; what.en)).note(tr!(
        "規則が、問われた範囲で出力がとりうる値を正確に言えず、その値になる具体的な入力も見つかりませんでした。",
        "A rule could not say what its output comes to over the values in question, and no concrete input was found that gives it."
    ))
}

/// The report as lines, as the design's prototype wrote it: what each policy allows or denies,
/// each expectation, each separation, and each role's reach. For the tests.
pub fn lines(g: &Gate, r: &Report) -> Vec<String> {
    let mut out = Vec::new();
    let counts: Vec<String> = g.actions.iter().zip(&r.actions).map(|(a, c)| format!("{} {}", a.named.name, c.combinations)).collect();
    let total: u128 = r.actions.iter().map(|c| c.combinations).sum();
    out.push(format!("combinations: {}; {} in all", counts.join(", "), total));
    for (p, c) in g.policies.iter().zip(&r.policies) {
        if p.permit {
            out.push(format!("permit {}: allows {} combinations, {} of them alone", p.named.name, c.decides, c.alone));
        } else {
            out.push(format!("forbid {}: holds on {} combinations, and turns {} that a permit allows into a deny", p.named.name, c.holds, c.overturns));
        }
    }
    for (e, c) in g.expects.iter().zip(&r.expects) {
        let word = if e.allow { "allow" } else { "deny" };
        if c.broken == 0 {
            out.push(format!("expect {word} {}: holds on all {}", e.name, c.picked));
        } else {
            out.push(format!("expect {word} {}: {} of {} fail", e.name, c.broken, c.picked));
        }
    }
    for (s, h) in g.separates.iter().zip(&r.separations) {
        let acts: Vec<&str> = s.actions.iter().map(|a| g.actions[*a].named.name.as_str()).collect();
        out.push(format!("separate {}: {}", acts.join(", "), match h {
            Some(true) => "holds",
            Some(false) => "fails",
            None => "undecided",
        }));
    }
    for (role, reach) in g.roles.iter().zip(&r.reach) {
        match reach {
            Some(set) => {
                let mut names: Vec<&str> = set.iter().map(|a| g.actions[*a].named.name.as_str()).collect();
                names.sort();
                out.push(format!("role {}: can {:?}", role.named.name, names));
            }
            None => out.push(format!("role {}: undecided", role.named.name)),
        }
    }
    out
}

/// How far someone who asks is allowed an action, over every combination of the rest (what the
/// port `Gates::allowed` answers, DESIGN 8.3): always, sometimes (with one combination of each,
/// in both languages), or never. Undecided when the answer rests on a combination no language
/// vouches for and no concrete input gives. A principal type the action does not take is never
/// allowed it.
pub fn allowance(g: &Gate, s: &Space, known: &Known, ask: &dyn Ask, asker: &ritsu_ports::Asker) -> ritsu_ports::Found<ritsu_ports::Allowance> {
    use ritsu_ports::{Allowance, Asker, Found};
    // which combinations are the asker's
    let pick: Pick = match asker {
        Asker::Workflow(name) => {
            let Some(w) = g.workflows.iter().position(|x| x.named.is(name)) else { return Found::Value(Allowance::Never) };
            Box::new(move |f: &walk::Frame, env: &[Val]| {
                g.types[f.principal].kind == Kind::Workflow
                    && match f.place(&walk::Slot::Workflow) {
                        Some(at) => env[at] == Val::Workflow(w),
                        None => true,
                    }
            })
        }
        Asker::Roles { ty, roles } => {
            let Some(t) = g.types.iter().position(|x| x.named.is(ty)) else { return Found::Value(Allowance::Never) };
            let mask = roles.iter().filter_map(|r| g.roles.iter().position(|x| x.named.is(r))).fold(0u128, |m, r| m | walk::bit(r));
            Box::new(move |f: &walk::Frame, env: &[Val]| f.principal == t && matches!(&env[0], Val::Roles { direct, .. } if *direct == mask & f.walked_roles))
        }
    };
    let (mut allowed, mut denied) = (Witness::default(), Witness::default());
    s.each(|c| {
        if pick(&s.frames[c.frame], c.env) {
            if c.decision.allow {
                allowed.add(s.action, c);
            } else {
                denied.add(s.action, c);
            }
        }
    });
    let settle = |w: &Witness| -> Result<Option<Kept>, ()> {
        if let Some((_, k)) = &w.exact {
            return Ok(Some(k.clone()));
        }
        if w.loose.is_empty() {
            return Ok(None);
        }
        w.loose.iter().find(|(_, k)| walk::confirm(g, s, k, known, ask)).map(|(_, k)| Some(k.clone())).ok_or(())
    };
    let why = || tr!("規則が、問われた範囲で出力がとりうる値を正確に言えず、その値になる具体的な入力も見つかりませんでした", "a rule could not say what its output comes to over the values in question, and no concrete input was found that gives it");
    match (settle(&allowed), settle(&denied)) {
        (Ok(Some(a)), Ok(Some(d))) => Found::Value(Allowance::Sometimes { allowed: walk::say(g, s, &a), denied: walk::say(g, s, &d) }),
        (Ok(Some(_)), Ok(None)) => Found::Value(Allowance::Always),
        (Ok(None), Ok(Some(_))) | (Ok(None), Ok(None)) => Found::Value(Allowance::Never),
        _ => Found::Undecided(why()),
    }
}
