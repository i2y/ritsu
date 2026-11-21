//! The checks of every combination (DESIGN 4.2–4.5): each action is walked once ([`crate::walk`]),
//! every combination decided as Cedar decides it ([`crate::eval`]), and from what the walk counts:
//!
//! - E301: an action no permit allows in any combination (`nobody "<reason>"` says it is meant;
//!   then a combination that is allowed breaks it, E304);
//! - E302: a permit a forbid overrides in every combination it would allow;
//! - E303: a permit or a forbid whose lines never hold together;
//! - W301: a permit another single permit covers (the later of two that cover each other);
//! - W305: a policy that names an action it applies to in no combination, while it applies to
//!   another it names;
//! - W306: a permit that names an action a forbid overrides it on in every combination it would
//!   allow there, while it allows something of another it names;
//! - W307: a line of a permit that changes no decision: a forbid denies every combination it leaves
//!   out;
//! - W308: a line of a policy met wherever its other lines are: it leaves out no combination that
//!   can happen;
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
use crate::eval::{CAtom, CExpr};
use crate::walk::{self, Ask, Combo, Compiled, FactorKind, Frame, Kept, Known, Memo, Slot, Space, Stop, Val};
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
    // what each action guards is known: the lines under a workflow, at the calls they speak of
    diags.extend(crate::borders::values_at_calls(&gate, langs));
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

    fn together(&self, rule: &std::path::Path, outputs: &[String], ranges: &[(String, Option<i128>, Option<i128>)]) -> Result<ritsu_ports::Found<Vec<(Vec<ritsu_ports::Value>, ritsu_ports::Values)>>, Vec<ritsu_ports::Said>> {
        match self.rules {
            Some(r) => r.outputs_together(rule, outputs, ranges),
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

/// How many ways of meeting the lines of a policy or an expectation that holds nowhere are tried,
/// with the values rules compute taken apart, to say that the rules are why ([`apart`]).
const APART_TRIES: u128 = 100_000;

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
    /// The same, on each action it is on, with the combinations it applies to there (W305).
    per: BTreeMap<usize, PerAction>,
    /// Each of its lines: what the line alone leaves out (W307, W308).
    left: Vec<LeftOut>,
    /// The lines (as a mask over the first 64) that are not met together, two or more of them, on
    /// a combination its `principal` line and its other lines hold on, where taking them out would
    /// change something: of a permit, where no forbid and no other permit applies; of a forbid,
    /// anywhere.
    together: BTreeSet<u64>,
}

/// What one line of a policy leaves out alone, over every action the policy is on: the
/// combinations its `principal` line and its other lines hold on and it does not.
#[derive(Clone, Debug, Default)]
struct LeftOut {
    count: u128,
    /// Of a permit: those a forbid denies, and how many each forbid does.
    denied: u128,
    denied_by: BTreeMap<usize, u128>,
    /// One of them, to show.
    example: Witness,
}

/// What the walk counts of a policy on one action it is on: the combinations it applies to, those
/// its `principal` line holds on, and each of its lines within them.
#[derive(Clone, Debug, Default)]
struct PerAction {
    holds: u128,
    scope: u128,
    lines: Vec<u128>,
    /// The combinations it applies to here, to show one (W306).
    applies: Witness,
    /// A permit: the combinations it decides here, and for each forbid, those both apply to.
    decides: u128,
    by_forbid: BTreeMap<usize, u128>,
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
        pols[i].left = vec![LeftOut::default(); p.conds.len()];
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
            // whether a forbid applies here, and a permit: what taking a line of a policy out would
            // change, where the policy does not apply
            let forbids = (0..s.policies.len()).any(|j| c.applies[j] && !s.permit[j]);
            let permits = (0..s.policies.len()).any(|j| c.applies[j] && s.permit[j]);
            for (k, &pi) in s.policies.iter().enumerate() {
                let acc = &mut pols[pi];
                let comp = &f.policies[k];
                let per = acc.per.entry(ai).or_insert_with(|| PerAction { lines: vec![0; comp.conds.len()], ..PerAction::default() });
                if comp.scope.holds(c.env) {
                    acc.scope += 1;
                    per.scope += 1;
                    // the lines not met, as a mask, how many, and the last of them
                    let (mut unmet, mut n_unmet, mut last) = (0u64, 0usize, 0usize);
                    for (j, line) in comp.conds.iter().enumerate() {
                        if line.holds(c.env) {
                            acc.lines[j] += 1;
                            per.lines[j] += 1;
                        } else {
                            unmet |= 1u64.checked_shl(j as u32).unwrap_or(0);
                            n_unmet += 1;
                            last = j;
                        }
                    }
                    // taken out, the lines not met would make a forbid apply here, and a permit decide
                    // here where no forbid and no other permit applies (none of it applies here)
                    let change = !s.permit[k] || (!forbids && !permits);
                    if n_unmet == 1 {
                        let x = &mut acc.left[last];
                        x.count += 1;
                        x.example.add(ai, c);
                        if s.permit[k] && forbids {
                            x.denied += 1;
                            for (j, &pj) in s.policies.iter().enumerate() {
                                if c.applies[j] && !s.permit[j] {
                                    *x.denied_by.entry(pj).or_default() += 1;
                                }
                            }
                        }
                    } else if n_unmet >= 2 && change {
                        acc.together.insert(unmet);
                    }
                }
                if !c.applies[k] {
                    continue;
                }
                per.holds += 1;
                per.applies.add(ai, c);
                acc.count.holds += 1;
                acc.holds.add(ai, c);
                if acc.first.is_none() {
                    acc.first = Some((ai, c.keep()));
                }
                if s.permit[k] {
                    for (j, &pj) in s.policies.iter().enumerate() {
                        if c.applies[j] && !s.permit[j] {
                            *acc.by_forbid.entry(pj).or_default() += 1;
                            *per.by_forbid.entry(pj).or_default() += 1;
                        }
                    }
                } else if (0..s.policies.len()).any(|j| c.applies[j] && s.permit[j]) {
                    acc.count.overturns += 1;
                }
                if c.decision.determining.contains(&k) {
                    acc.count.decides += 1;
                    per.decides += 1;
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

    // one of the combinations a witness holds that can happen, where one is found
    let shown = |w: &Witness| confirmed(w).ok().flatten();

    // the policies: each applies somewhere (E303), a permit allows something (E302), and adds
    // something no other single permit does (W301); the actions each names (W305, W306), and each
    // of its lines (W307, W308)
    for (pi, p) in g.policies.iter().enumerate() {
        // a forbid read from another file is that file's to answer for (its own check says it)
        if p.from.is_some() || !p.actions.iter().all(|a| walked[*a]) || p.actions.is_empty() {
            continue;
        }
        let acc = &pols[pi];
        if !acc.holds.any() {
            diags.push(never_applies(g, pi, acc.scope, &acc.lines, spaces));
            continue;
        }
        match confirmed(&acc.holds) {
            Err(()) => {
                diags.push(undecided(g, p.named.line, tr!("{} `{}` が当てはまる組み合わせがある", "the {} `{}` applies to any combination", effect(p), p.named.name)));
                continue;
            }
            Ok(None) => {
                diags.push(never_applies(g, pi, acc.scope, &acc.lines, spaces));
                continue;
            }
            Ok(Some(_)) => {}
        }
        // an action it names that it applies to in no combination, counted generously, where it
        // applies to another (W305); `action any` names none, and is on every action
        if !p.any {
            for &ai in &p.actions {
                if acc.per.get(&ai).is_none_or(|x| x.holds == 0) {
                    diags.push(never_on(g, pi, ai, acc, spaces));
                }
            }
        }
        if !p.permit {
            // a line of a forbid that changes nothing because no permit allows what it leaves out
            // is not said, as a forbid that decides nothing is not (DESIGN 4.3): a forbid that
            // keeps something out a second time is kept on purpose
            diags.extend(lines_of_no_use(g, pi, acc, spaces, &shown));
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
        // an action it names where a forbid denies all it would allow, counted generously, while
        // it allows something of another (W306), said where one of the combinations it applies to
        // there is shown to occur (else it may apply to none of it, which W305 says when it is
        // decided); `action any` names none, and a forbid that keeps one action from it is what it
        // is written with
        if !p.any {
            for &ai in &p.actions {
                if let Some(x) = acc.per.get(&ai)
                    && x.holds > 0
                    && x.decides == 0
                    && let Some(one) = shown(&x.applies)
                {
                    diags.push(denied_on(g, pi, ai, acc, spaces, one));
                }
            }
        }
        // covered by another permit: name the first that covers it, unless the two cover each
        // other and this one comes first (then the other is said to be the one to remove)
        let mut covered_by = None;
        if let Some(cov) = &acc.covered {
            covered_by = cov.iter().copied().find(|&q| {
                let mutual = pols[q].covered.as_ref().is_some_and(|c| c.contains(&pi));
                !(mutual && p.named.line < g.policies[q].named.line)
            });
            if let Some(q) = covered_by {
                diags.push(covered(g, pi, q, acc.count.decides));
            }
        }
        // its lines, unless the permit itself is said to change nothing
        if covered_by.is_none() {
            diags.extend(lines_of_no_use(g, pi, acc, spaces, &shown));
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
            diags.push(picks_nothing(g, ei, acc.scope, &acc.lines, spaces));
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

/// `combination` for one, `combinations` for more.
fn combinations(x: u128) -> &'static str {
    if x == 1 { "combination" } else { "combinations" }
}

/// `k` of `all` combinations, with the verb that follows: `2 of the 3 combinations it picks are`,
/// and for one of one, `the 1 combination it picks is`.
fn some_of(k: u128, all: u128, the: &str, which: &str) -> String {
    if all == 1 { format!("{the} 1 combination{which} is") } else { format!("{} of {the} {} combinations{which} {}", n(k), n(all), are(k)) }
}

/// A combination in words, and as the values the JSON lists, each with the inputs that were run
/// for its computed values.
fn example(g: &Gate, s: &Space, k: &Kept) -> (Text, Example) {
    (walk::say(g, s, k), Example { values: walk::values(g, s, k), inputs: walk::confirmed(g, s, k) })
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
    at("E304", g, line, tr!("`nobody` は、だれも `{}` をできないと書いていますが、{} 通りのうち {} 通りが許されます", "`nobody` says no one may `{}`, and {} allowed", a.named.name, n(combos), n(k); a.named.name, some_of(k, combos, "its", "")))
        .note(t)
        .note(tr!("許す permit を直すか、`nobody` の行を消してください。", "Fix the permits that allow it, or remove the `nobody` line."))
        .with(ex)
}

fn never_applies(g: &Gate, pi: usize, scope: u128, lines: &[u128], spaces: &[Option<Space>]) -> Diag {
    let p = &g.policies[pi];
    let e = effect(p);
    let d = at("E303", g, p.named.line, tr!("{e} `{}` は、どの組み合わせにも当てはまりません", "The {e} `{}` applies to no combination", p.named.name));
    let d = lines_note(d, g, &p.who, &p.actions, &p.conds, scope, lines);
    let why = if scope > 0 && !p.conds.is_empty() { because_of_rules(g, spaces, &p.actions, &|s, f| s.policies.iter().position(|x| *x == pi).map(|k| &f.policies[k])) } else { None };
    match why {
        Some((why, Why::Apart)) => d.note(why).note(tr!("規則が一緒に出す値に行を直すか、{e} そのものを消してください。", "Correct the lines to values the rules give together, or remove the {e} itself.")),
        Some((why, Why::Never)) => d.note(why).note(tr!("規則が出す値に行を直すか、{e} そのものを消してください。", "Correct the lines to a value the rule gives, or remove the {e} itself.")),
        None => d.note(tr!("満たせない行か、{e} そのものを消してください。", "Remove the line that cannot be met, or the {e} itself.")),
    }
}

/// A policy that names an action it applies to in no combination, while it applies to another it
/// names (W305): what its lines come to on that action, why where the rules decide it, the actions
/// it applies to, and what to do.
fn never_on(g: &Gate, pi: usize, ai: usize, acc: &PolAcc, spaces: &[Option<Space>]) -> Diag {
    let p = &g.policies[pi];
    let e = effect(p);
    let a = &g.actions[ai].named.name;
    let d = at("W305", g, p.named.line, tr!("{e} `{}` は action `{a}` を挙げていますが、`{a}` のどの組み合わせにも当てはまりません", "The {e} `{}` names the action `{a}`, and applies to no combination of it", p.named.name));
    let (scope, lines) = acc.per.get(&ai).map(|x| (x.scope, x.lines.clone())).unwrap_or_else(|| (0, vec![0; p.conds.len()]));
    let d = lines_note(d, g, &p.who, &[ai], &p.conds, scope, &lines);
    let why = if scope > 0 && !p.conds.is_empty() { because_of_rules(g, spaces, &[ai], &|s, f| s.policies.iter().position(|x| *x == pi).map(|k| &f.policies[k])) } else { None };
    // the actions it does apply to, each with its count
    let on: Vec<Text> = p.actions.iter().filter_map(|b| acc.per.get(b).filter(|x| x.holds > 0).map(|x| (b, x.holds))).map(|(b, k)| tr!("`{}` の {} 通り", "{} {} of `{}`", g.actions[*b].named.name, n(k); n(k), combinations(k), g.actions[*b].named.name)).collect();
    let l = Text::list(&on);
    let d = match why {
        Some((why, _)) => d.note(why),
        None => d,
    };
    d.note(tr!("ほかに挙げた action では、{}に当てはまります。", "On the other actions it names, it applies to {}.", l.ja; l.en)).note(tr!(
        "`action` の行から `{a}` を外すか、`{a}` にも当てはまるよう行を直してください。",
        "Remove `{a}` from its `action` line, or correct its lines so that it applies to `{a}` too."
    ))
}

/// A permit that names an action where a forbid denies every combination it would allow, while it
/// allows something of another it names (W306): what it would allow there and which forbids deny
/// it, an example, what it allows on the other actions, and what to do.
fn denied_on(g: &Gate, pi: usize, ai: usize, acc: &PolAcc, spaces: &[Option<Space>], shown: (usize, Kept)) -> Diag {
    let p = &g.policies[pi];
    let a = &g.actions[ai].named.name;
    let x = &acc.per[&ai];
    let mut d = at("W306", g, p.named.line, tr!("permit `{}` は action `{a}` を挙げていますが、`{a}` では何も許しません。許すはずの組み合わせを、どれも forbid が拒みます", "The permit `{}` names the action `{a}`, and allows nothing of it: a forbid denies every combination it would allow there", p.named.name; p.named.name));
    let (ja, en) = denials(g, &x.by_forbid, x.holds);
    d = d.note(tr!("`{a}` で許すはずの組み合わせは {} 通りで、{}を拒みます。", "On `{a}` it would allow {}; {}.", n(x.holds), ja; ritsu_base::text::plural(x.holds as usize, "combination", "combinations"), en));
    let (ai, k) = &shown;
    if let Some(s) = &spaces[*ai] {
        let (t, ex) = for_example(g, s, k);
        d = d.note(t).with(ex);
    }
    // the actions it does allow something of, each with its count
    let on: Vec<Text> = p.actions.iter().filter_map(|b| acc.per.get(b).filter(|y| y.decides > 0).map(|y| (b, y.decides))).map(|(b, k)| tr!("`{}` の {} 通り", "{} {} of `{}`", g.actions[*b].named.name, n(k); n(k), combinations(k), g.actions[*b].named.name)).collect();
    let l = Text::list(&on);
    d.note(tr!("ほかに挙げた action では、{}を許します。", "On the other actions it names, it allows {}.", l.ja; l.en)).note(tr!(
        "`action` の行から `{a}` を外すか、`{a}` も許すつもりなら forbid を狭めてください。",
        "Remove `{a}` from its `action` line, or narrow the forbid if the permit is meant to allow `{a}`."
    ))
}

/// The forbids that deny some of `total` combinations, by how many, the most first: `` `f1` が 192
/// 通り、`f2` が 96 通り `` and `` `f1` denies 192 of them, `f2` 96 ``; of one combination, `` `f1`
/// denies it `` (`` `f1` and `f2` deny it ``).
fn denials(g: &Gate, by: &BTreeMap<usize, u128>, total: u128) -> (String, String) {
    let mut parts: Vec<(u128, String)> = by.iter().map(|(f, k)| (*k, g.policies[*f].named.name.clone())).collect();
    parts.sort_by(|a, b| b.0.cmp(&a.0));
    let ja: Vec<String> = parts.iter().map(|(k, name)| format!("`{name}` が {} 通り", n(*k))).collect();
    if total == 1 {
        let names = Text::list(&parts.iter().map(|(_, name)| Text::same(format!("`{name}`"))).collect::<Vec<_>>());
        return (ja.join("、"), format!("{} {} it", names.en, if parts.len() == 1 { "denies" } else { "deny" }));
    }
    let en: Vec<String> = parts.iter().enumerate().map(|(i, (k, name))| if i == 0 { format!("`{name}` denies {} of them", n(*k)) } else { format!("`{name}` {}", n(*k)) }).collect();
    (ja.join("、"), en.join(", "))
}

/// The lines of a policy that change nothing, counted generously as W305 is (a combination
/// counted in case it can happen is one the line leaves out): a line met wherever the policy's
/// other lines are (W308, of a permit or a forbid), and a line of a permit every combination it
/// leaves out of which a forbid denies (W307). A line of a permit whose combinations left out
/// other permits allow is not said: permits written case by case (refunds in the period, and late
/// ones) are each a whole case, and the line says which case the permit is; W301 says it where the
/// whole permit adds nothing. Where two or more such lines of the policy, taken out together,
/// would change something, each says so.
fn lines_of_no_use(g: &Gate, pi: usize, acc: &PolAcc, spaces: &[Option<Space>], shown: &dyn Fn(&Witness) -> Option<(usize, Kept)>) -> Vec<Diag> {
    let p = &g.policies[pi];
    let mut said: Vec<(usize, Diag)> = Vec::new();
    for (j, x) in acc.left.iter().enumerate() {
        if x.count == 0 {
            said.push((j, met_wherever(g, pi, j, acc, spaces)));
        } else if p.permit && x.denied == x.count {
            said.push((j, decided_without(g, pi, j, x, spaces, shown(&x.example))));
        }
    }
    if said.len() < 2 {
        return said.into_iter().map(|(_, d)| d).collect();
    }
    // taken out together: some lines not met together where taking them out changes something
    let mask = said.iter().fold(0u64, |m, (j, _)| m | 1u64.checked_shl(*j as u32).unwrap_or(0));
    let together = p.conds.len() > 64 || acc.together.iter().any(|m| m & !mask == 0);
    if !together {
        return said.into_iter().map(|(_, d)| d).collect();
    }
    let line_of = |j: usize| {
        let c = &p.conds[j];
        Text::same(format!("`{} {}`", if c.when { "when" } else { "unless" }, c.text))
    };
    let all: Vec<usize> = said.iter().map(|(j, _)| *j).collect();
    said.into_iter()
        .map(|(j, d)| {
            let others: Vec<Text> = all.iter().filter(|&&o| o != j).map(|&o| line_of(o)).collect();
            let l = Text::list(&others);
            d.note(if others.len() == 1 {
                tr!(
                    "{} も、一つだけ消すなら何も変えませんが、この行と一緒に消すと、答えが変わることがあります。一つずつ消して、検査し直してください。",
                    "{} changes nothing taken out alone too, but taken out together with this line, the two may change an answer: take out one at a time, and check again.",
                    l.ja;
                    l.en
                )
            } else {
                tr!(
                    "{} も、一つだけ消すなら何も変えませんが、この行と一緒に消すと、答えが変わることがあります。一つずつ消して、検査し直してください。",
                    "{} change nothing taken out alone too, but taken out together with this line, they may change an answer: take out one at a time, and check again.",
                    l.ja;
                    l.en
                )
            })
        })
        .collect()
}

/// A line of a policy met wherever the policy's other lines are (W308): on how many combinations,
/// why where the rules decide it, and what to do.
fn met_wherever(g: &Gate, pi: usize, j: usize, acc: &PolAcc, spaces: &[Option<Space>]) -> Diag {
    let p = &g.policies[pi];
    let e = effect(p);
    let c = &p.conds[j];
    let line = format!("{} {}", if c.when { "when" } else { "unless" }, c.text);
    let d = at("W308", g, c.line, tr!("{e} `{}` の行 `{line}` は、起こりうるどの組み合わせも外しません", "The line `{line}` of the {e} `{}` leaves out no combination that can happen", p.named.name; p.named.name));
    // the combinations it is counted over: those its `principal` line and its other lines hold on,
    // which are those the policy applies to
    let k = acc.count.holds;
    let each = if k == 1 { "the 1 combination" } else { "each of the" };
    let kn = if k == 1 { String::new() } else { format!(" {} combinations", n(k)) };
    // 「1 通りで満たされます」, 「4 通りの、どれでも満たされます」
    let met = if k == 1 { "で満たされます" } else { "の、どれでも満たされます" };
    let over = match (p.who == Who::Anyone, p.conds.len() > 1) {
        (false, true) => tr!("`principal` の行とほかの行が満たされる {} 通り{met}。", "It is met on {each}{kn} its `principal` line and its other lines are met on.", n(k);),
        (false, false) => tr!("`principal` の行が当てはまる {} 通り{met}。", "It is met on {each}{kn} its `principal` line takes.", n(k);),
        (true, true) => tr!("ほかの行が満たされる {} 通り{met}。", "It is met on {each}{kn} its other lines are met on.", n(k);),
        (true, false) => {
            let acts = Text::list(&p.actions.iter().map(|a| Text::same(format!("`{}`", g.actions[*a].named.name))).collect::<Vec<_>>());
            tr!("{} の {} 通り{met}。", "It is met on {each}{kn} of {}.", acts.ja, n(k); acts.en)
        }
    };
    let d = d.note(over);
    match line_because_of_rules(g, spaces, pi, j) {
        // what the rules decide is the rules' to hold, as W304 says of an expectation
        Some(why) => d.note(why).note(tr!(
            "これは規則が決めていることで、この行は、{e} が当てはまる組み合わせを何も変えません。規則の答えを確かめたいなら規則の側に書き（`examples`。rulec が検査します）、この行は消すか、直してください。",
            "The rules decide this, so the line changes nothing of what the {e} applies to. To hold what a rule answers, write it in the rule (its `examples`, which rulec checks); here, remove the line or correct it."
        )),
        None => d.note(tr!("行を消すか、外すつもりの組み合わせを外すよう行を直してください。", "Remove the line, or correct it so that it leaves out what it is meant to.")),
    }
}

/// Why the rules leave a line met wherever the other lines of its policy are (W308): what
/// [`because_of_rules`] says of the policy with the line turned around, which then applies to no
/// combination.
fn line_because_of_rules(g: &Gate, spaces: &[Option<Space>], pi: usize, j: usize) -> Option<Text> {
    let p = &g.policies[pi];
    let mut turned: BTreeMap<(usize, usize), Compiled> = BTreeMap::new();
    for &ai in &p.actions {
        let Some(s) = spaces[ai].as_ref() else { continue };
        let Some(k) = s.policies.iter().position(|x| *x == pi) else { continue };
        for (fi, f) in s.frames.iter().enumerate() {
            let c = &f.policies[k];
            let mut parts = vec![c.scope.clone()];
            parts.extend(c.conds.iter().enumerate().map(|(i, x)| if i == j { CExpr::Not(Box::new(x.clone())) } else { x.clone() }));
            turned.insert((ai, fi), Compiled { scope: c.scope.clone(), conds: c.conds.clone(), all: CExpr::And(parts).simplify() });
        }
    }
    let compiled = |s: &Space, f: &Frame| -> Option<&Compiled> {
        let fi = s.frames.iter().position(|x| std::ptr::eq(x, f))?;
        turned.get(&(s.action, fi))
    };
    because_of_rules(g, spaces, &p.actions, &compiled).map(|(t, _)| t)
}

/// A line of a permit that changes no decision (W307): a forbid denies every combination it alone
/// leaves out. How many, by which forbids, an example, and what to do.
fn decided_without(g: &Gate, pi: usize, j: usize, x: &LeftOut, spaces: &[Option<Space>], shown: Option<(usize, Kept)>) -> Diag {
    let p = &g.policies[pi];
    let c = &p.conds[j];
    let line = format!("{} {}", if c.when { "when" } else { "unless" }, c.text);
    let name = &p.named.name;
    let mut d = at(
        "W307",
        g,
        c.line,
        tr!(
            "permit `{name}` の行 `{line}` は、どの決定も変えません。この行が外す組み合わせを、どれも forbid が拒みます",
            "The line `{line}` of the permit `{name}` changes no decision: a forbid denies every combination it leaves out"
        ),
    );
    d = d.note(if x.count == 1 {
        let names = Text::list(&x.denied_by.keys().map(|f| Text::same(format!("`{}`", g.policies[*f].named.name))).collect::<Vec<_>>());
        let deny = if x.denied_by.len() == 1 { "denies" } else { "deny" };
        tr!(
            "この行が無ければ、permit は、この行だけが満たされない 1 通りにも当てはまります。その組み合わせは、{} が拒みます。",
            "Without it, the permit would also apply to the 1 combination only this line is not met on, which {} {deny}.",
            names.ja;
            names.en
        )
    } else {
        let (ja, en) = denials(g, &x.denied_by, x.count);
        tr!(
            "この行が無ければ、permit は、この行だけが満たされない {} 通りにも当てはまります。そのうち、{}を拒みます。",
            "Without it, the permit would also apply to the {} combinations only this line is not met on: {}.",
            n(x.count), ja;
            n(x.count), en
        )
    });
    if let Some((ai, k)) = &shown
        && let Some(s) = &spaces[*ai]
    {
        let (t, ex) = for_example(g, s, k);
        d = d.note(t).with(ex);
    }
    d.note(tr!(
        "行を消しても、答えは変わりません。行を消すか、ほかの組み合わせを外すつもりだったなら、行を直してください。",
        "Taking it out changes no answer: remove it, or correct it if it is meant to leave out something else."
    ))
}

/// An expectation that picks no combination (W304): it holds, and checks nothing.
fn picks_nothing(g: &Gate, ei: usize, scope: u128, lines: &[u128], spaces: &[Option<Space>]) -> Diag {
    let e = &g.expects[ei];
    let d = at("W304", g, e.line, tr!("期待 `{}` は、どの組み合わせも選びません。成り立ちますが、何も確かめていません", "The expectation `{}` picks no combination: it holds, and checks nothing", e.name));
    let d = lines_note(d, g, &e.who, &e.actions, &e.conds, scope, lines);
    let why = if scope > 0 && !e.conds.is_empty() { because_of_rules(g, spaces, &e.actions, &|s, f| s.expects.iter().position(|x| *x == ei).map(|k| &f.expects[k])) } else { None };
    match why {
        // what the rules decide is the rules' to hold: a rule's own examples say what it answers
        Some((why, _)) => d.note(why).note(tr!(
            "これは規則が決めていることで、ポリシーをどう書いても成り立ちます。この期待は、ポリシーについて何も確かめていません。規則の答えを確かめたいなら規則の側に書き（`examples`。rulec が検査します）、この期待は消すか、行を直してください。",
            "The rules decide this whatever the policies say, so the expectation checks nothing of them. To hold what a rule answers, write it in the rule (its `examples`, which rulec checks); here, remove the expectation or correct its lines."
        )),
        None => d.note(tr!("同時に満たせない行を直すか、期待を消してください。", "Correct the lines that cannot be met together, or remove the expectation.")),
    }
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
            tr!("{} の {} 通りのうち", "Of the {} {} of {}", acts.ja, n(scope); n(scope), combinations(scope), acts.en)
        } else {
            tr!("`principal` の行が当てはまる {} 通りのうち", "Of the {} {} its `principal` line takes", n(scope); n(scope), combinations(scope))
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

/// The places an expression reads.
fn places_read(e: &CExpr, out: &mut BTreeSet<usize>) {
    match e {
        CExpr::Atom(a) => match a {
            CAtom::Const(_) => {}
            CAtom::Roles(_) => {
                out.insert(0);
            }
            CAtom::Workflow(at, _) | CAtom::Bool(at, _) | CAtom::Enum(at, _) | CAtom::Cmp(at, _, _) | CAtom::Same(at, _, _) | CAtom::Member(at, _) => {
                out.insert(*at);
            }
            CAtom::EnumEq(a, b) => {
                out.insert(*a);
                out.insert(*b);
            }
        },
        CExpr::Not(e) => places_read(e, out),
        CExpr::And(es) | CExpr::Or(es) => es.iter().for_each(|e| places_read(e, out)),
    }
}

/// Why the rules leave the lines of a policy or an expectation met on no combination.
#[derive(Clone, Copy, PartialEq)]
enum Why {
    /// Values that each happen, and never together.
    Apart,
    /// A value the rule answers for nothing the gate gives it.
    Never,
}

/// The rules' part in lines met on no combination, in a sentence ([`apart`]): values that never
/// come together where there are such, else a value a rule never comes to.
fn because_of_rules<'a>(g: &Gate, spaces: &'a [Option<Space>], actions: &[usize], compiled: &dyn Fn(&'a Space, &'a Frame) -> Option<&'a Compiled>) -> Option<(Text, Why)> {
    apart(g, spaces, actions, compiled, false).map(|t| (t, Why::Apart)).or_else(|| apart(g, spaces, actions, compiled, true).map(|t| (t, Why::Never)))
}

/// Whether the lines of a policy or an expectation that holds on no combination (`compiled`
/// gives them for a frame) are met nowhere because of what rules compute: values an action
/// computes by rules, with the attributes and inputs they are computed from that a policy reads
/// too, that each happen and never happen together. The walk counts such values together
/// (DESIGN 4.1): here the lines are tried again with each value of one such group taken apart
/// from the others, every value it takes in turn. Where the lines are then met, the group is why,
/// and the values it was met with are said: the two of them that no combination has together, or
/// all of them, in a sentence with what they are computed from. With `declared`, each computed
/// value is tried at every value its rule's output is declared with, taken or not: where the lines
/// are then met, a value they were met with that no combination has is said, as one the rule
/// answers for nothing the gate gives it. None where that is not why, where the lines read a value
/// of another action, and past [`APART_TRIES`] ways of meeting them.
fn apart<'a>(g: &Gate, spaces: &'a [Option<Space>], actions: &[usize], compiled: &dyn Fn(&'a Space, &'a Frame) -> Option<&'a Compiled>, declared: bool) -> Option<Text> {
    for &ai in actions {
        let Some(s) = spaces[ai].as_ref() else { continue };
        for (fi, f) in s.frames.iter().enumerate() {
            if f.factors.iter().any(|x| x.points.is_empty()) {
                continue;
            }
            let Some(c) = compiled(s, f) else { continue };
            // the places the lines read, with those an answer they read is derived from
            let mut read = BTreeSet::new();
            places_read(&c.all, &mut read);
            loop {
                let before = read.len();
                for (at, e) in &f.derived {
                    if read.contains(at) {
                        places_read(e, &mut read);
                    }
                }
                if read.len() == before {
                    break;
                }
            }
            for (xi, x) in f.factors.iter().enumerate() {
                if !matches!(x.kind, FactorKind::Rules { .. }) {
                    continue;
                }
                // the places of the group the lines read (by their place among the group's), each
                // with the values it takes
                let own: Vec<usize> = (0..x.places.len()).filter(|j| read.contains(&x.places[*j])).collect();
                if own.len() < if declared { 1 } else { 2 } || !own.iter().all(|&j| matches!(f.slots[x.places[j]], Slot::Computed(_) | Slot::Attr(..) | Slot::Input(_))) {
                    continue;
                }
                let lists: Vec<Vec<Val>> = own
                    .iter()
                    .map(|&j| {
                        let mut vs: Vec<Val> = x.points.iter().map(|p| p.vals[j].clone()).collect();
                        // with every value the rule's output is declared with, taken or not
                        if declared && let Slot::Computed(i) = &f.slots[x.places[j]] {
                            match s.names.get(i) {
                                Some(names) => vs.extend((0..names.len()).map(Val::Enum)),
                                None if vs.iter().any(|v| matches!(v, Val::Bool(_))) => vs.extend([Val::Bool(false), Val::Bool(true)]),
                                None => {}
                            }
                        }
                        vs.sort();
                        vs.dedup();
                        vs
                    })
                    .collect();
                // the other factors the lines read are tried at every point, the rest at one
                let others: Vec<usize> = (0..f.factors.len()).filter(|&k| k != xi && f.factors[k].places.iter().any(|at| read.contains(at))).collect();
                let ways = others.iter().map(|&k| f.factors[k].points.len() as u128).chain(lists.iter().map(|l| l.len() as u128)).fold(1u128, u128::saturating_mul);
                if ways > APART_TRIES {
                    continue;
                }
                let mut idx = vec![0usize; f.factors.len()];
                let mut at = vec![0usize; own.len()];
                let met: Option<Vec<Val>> = 'ways: loop {
                    let mut env = s.env_of(&Kept { frame: fi, idx: idx.clone() });
                    loop {
                        for (k, &j) in own.iter().enumerate() {
                            env[x.places[j]] = lists[k][at[k]].clone();
                        }
                        f.derive(&mut env);
                        if c.all.holds(&env) {
                            break 'ways Some(own.iter().enumerate().map(|(k, _)| lists[k][at[k]].clone()).collect());
                        }
                        // the next values of the group
                        let mut k = own.len();
                        let more = loop {
                            if k == 0 {
                                break false;
                            }
                            k -= 1;
                            at[k] += 1;
                            if at[k] < lists[k].len() {
                                break true;
                            }
                            at[k] = 0;
                        };
                        if !more {
                            break;
                        }
                    }
                    // the next points of the others
                    let mut k = others.len();
                    loop {
                        if k == 0 {
                            break 'ways None;
                        }
                        k -= 1;
                        idx[others[k]] += 1;
                        if idx[others[k]] < f.factors[others[k]].points.len() {
                            break;
                        }
                        idx[others[k]] = 0;
                    }
                };
                let Some(met) = met else { continue };
                let together = |ks: &[usize]| x.points.iter().any(|p| ks.iter().all(|&k| p.vals[own[k]] == met[k]));
                let ks: Vec<usize> = if declared {
                    // the one of them no combination has
                    match (0..own.len()).find(|&k| !together(&[k])) {
                        Some(k) => vec![k],
                        None => continue,
                    }
                } else {
                    // the two of them no combination has together, or all of them
                    let pair = (0..own.len()).flat_map(|i| (i + 1..own.len()).map(move |j| [i, j])).find(|ks| !together(ks));
                    match pair {
                        Some(ks) => ks.to_vec(),
                        None if !together(&(0..own.len()).collect::<Vec<_>>()) => (0..own.len()).collect(),
                        None => continue,
                    }
                };
                if let Some(why) = apart_said(g, s, f, ks.iter().map(|&k| (x.places[own[k]], &met[k])).collect()) {
                    return Some(why);
                }
            }
        }
    }
    None
}

/// The sentence of [`apart`]: the values, each by what it is the value of, the computed ones
/// first, and what those are computed from. None for a value that is not said this way.
fn apart_said(g: &Gate, s: &Space, f: &Frame, mut parts: Vec<(usize, &Val)>) -> Option<Text> {
    let a = &g.actions[s.action];
    let computed = |at: usize| matches!(f.slots[at], Slot::Computed(_));
    parts.sort_by_key(|(at, _)| !computed(*at));
    let mut said: Vec<(Text, Text)> = Vec::new();
    for (at, v) in &parts {
        let slot = &f.slots[*at];
        let ty = match slot {
            Slot::Attr(o, i) => Some(&g.owner_type(*o, f.principal, f.resource).attrs[*i].ty),
            Slot::Input(i) => Some(&a.inputs[*i].ty),
            _ => None,
        };
        let value = match (v, slot, ty) {
            (Val::Bool(b), ..) => Text::same(format!("`{b}`")),
            (Val::Enum(k), Slot::Computed(i), _) => Text::same(format!("`{}`", s.names.get(i)?.get(*k)?.name)),
            (Val::Enum(k), _, Some(FieldType::Enum(e))) => Text::same(format!("`{}`", g.enums[*e].values.get(*k)?.name)),
            (Val::Cell(c), _, Some(FieldType::Num { unit, .. })) => crate::cells::show_cell(*c, unit),
            _ => return None,
        };
        let name = walk::frame_slot_name(g, s.action, f, slot);
        said.push((tr!("`{}`", "`{}`", name.ja; name.en), value));
    }
    let (first, rest) = said.split_first()?;
    if !computed(parts[0].0) {
        return None;
    }
    if rest.is_empty() {
        // one value: what it is computed from, each thing with the inputs it is given as when it
        // is given as more than one
        let (act, ci, link) = f.computed_at(s.action, parts[0].0)?;
        let How::Rule { args, .. } = &g.actions[act].computed[ci].how else { return None };
        let mut sources: Vec<(String, Vec<String>)> = Vec::new();
        for (input, src) in args {
            let text = match f.mapped(link, g, src) {
                Some(Source::Attr(o, n)) => format!("`{}.{}`", o.word(), g.owner_type(o, f.principal, f.resource).attr(&n).map(|(_, fd)| fd.named.name.clone()).unwrap_or(n)),
                Some(Source::Input(n)) => format!("`{}`", a.input(&n).map(|(_, fd)| fd.named.name.clone()).unwrap_or(n)),
                Some(Source::Today) => "`today`".to_string(),
                _ => continue,
            };
            match sources.iter_mut().find(|(t, _)| *t == text) {
                Some((_, inputs)) => inputs.push(format!("`{input}`")),
                None => sources.push((text, vec![format!("`{input}`")])),
            }
        }
        let from: Vec<Text> = sources
            .iter()
            .map(|(text, inputs)| match inputs.as_slice() {
                [_] => Text::same(text.clone()),
                [x, y] => tr!("{x} と {y} の両方に渡す {text}", "{text} (given as both {x} and {y})"),
                more => {
                    let l = Text::list(&more.iter().map(|x| Text::same(x.clone())).collect::<Vec<_>>());
                    tr!("{} に渡す {text}", "{text} (given as {})", l.ja; l.en)
                }
            })
            .collect();
        let (name, value) = first;
        return Some(if from.is_empty() {
            tr!(
                "{} が {} になることはありません。ゲートが渡す値のどれにも、規則は {} を答えません。",
                "{} is never {}: the rule answers {} for none of the values the gate gives it.",
                name.ja, value.ja, value.ja;
                name.en, value.en, value.en
            )
        } else {
            let l = Text::list(&from);
            tr!(
                "{} が {} になることはありません。{} は {} から計算する値で、ゲートが渡す値のどれにも、規則は {} を答えません。",
                "{} is never {}: it is computed from {}, and the rule answers {} for none of the values the gate gives it.",
                name.ja, value.ja, name.ja, l.ja, value.ja;
                name.en, value.en, l.en, value.en
            )
        });
    }
    let ja: Vec<String> = said.iter().map(|(n, v)| format!("{} が {}", n.ja, v.ja)).collect();
    let en: Vec<String> = rest.iter().map(|(n, v)| format!("{} is {}", n.en, v.en)).collect();
    let never = Text { ja: format!("{} になることはありません", ja.join(" で、")), en: format!("{} is never {} while {}", first.0.en, first.1.en, en.join(" and ")) };
    // what each computed value is computed from, in the terms of the action
    let mut from: Option<Vec<String>> = None;
    let mut names: Vec<Text> = Vec::new();
    for ((at, _), (name, _)) in parts.iter().zip(&said) {
        if !computed(*at) {
            continue;
        }
        let (act, ci, link) = f.computed_at(s.action, *at)?;
        let How::Rule { args, .. } = &g.actions[act].computed[ci].how else { return None };
        let mut sources: Vec<String> = Vec::new();
        for (_, src) in args {
            let text = match f.mapped(link, g, src) {
                Some(Source::Attr(o, n)) => format!("`{}.{}`", o.word(), g.owner_type(o, f.principal, f.resource).attr(&n).map(|(_, fd)| fd.named.name.clone()).unwrap_or(n)),
                Some(Source::Input(n)) => format!("`{}`", a.input(&n).map(|(_, fd)| fd.named.name.clone()).unwrap_or(n)),
                Some(Source::Today) => "`today`".to_string(),
                _ => continue,
            };
            if !sources.contains(&text) {
                sources.push(text);
            }
        }
        from = Some(match from {
            None => sources,
            Some(common) => common.into_iter().filter(|x| sources.contains(x)).collect(),
        });
        names.push(name.clone());
    }
    let from: Vec<Text> = from.unwrap_or_default().into_iter().map(Text::same).collect();
    let said = if from.is_empty() {
        tr!("{}。", "{}.", never.ja; never.en)
    } else {
        let l = Text::list(&from);
        match (names.len() == parts.len(), names.len()) {
            (true, 2) => tr!("{}。どちらも {} から計算する値です。", "{}: both are computed from {}.", never.ja, l.ja; never.en, l.en),
            (true, _) => tr!("{}。どれも {} から計算する値です。", "{}: all are computed from {}.", never.ja, l.ja; never.en, l.en),
            (false, k) => {
                let c = Text::list(&names);
                tr!("{}。{} は {} から計算する値です。", "{}: {} {} computed from {}.", never.ja, c.ja, l.ja; never.en, c.en, if k == 1 { "is" } else { "are" }, l.en)
            }
        }
    };
    Some(said)
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
        tr!("期待 `{}` が成り立ちません。選んだ {} 通りのうち {} 通りが拒まれます", "The expectation `{}` does not hold: {} denied", e.name, n(c.picked), n(c.broken); e.name, some_of(c.broken, c.picked, "the", " it picks"))
    } else {
        tr!("期待 `{}` が成り立ちません。選んだ {} 通りのうち {} 通りが許されます", "The expectation `{}` does not hold: {} allowed", e.name, n(c.picked), n(c.broken); e.name, some_of(c.broken, c.picked, "the", " it picks"))
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
    allowance_over(g, s, known, ask, &|frame, env| pick(&s.frames[frame], env))
}

/// How far an action is allowed over the combinations `pick` takes (each by its frame and its
/// values): what [`allowance`] and [`allowance_with`] both answer with, so that the two read a
/// gate the same way.
pub(crate) fn allowance_over(g: &Gate, s: &Space, known: &Known, ask: &dyn Ask, pick: &dyn Fn(usize, &[Val]) -> bool) -> ritsu_ports::Found<ritsu_ports::Allowance> {
    use ritsu_ports::{Allowance, Found};
    let (mut allowed, mut denied) = (Witness::default(), Witness::default());
    s.each(|c| {
        if pick(c.frame, c.env) {
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

/// What a question of [`allowance_with`] holds a combination of one frame to.
enum HeldTo {
    /// The place has this value.
    Is(usize, Val),
    /// In the relation at the place, the first term is the same entity as the second (the
    /// principal), or is not.
    Same(usize, usize, usize, bool),
    /// In the relation at the place, the principal is in what the term points to, or is not.
    Member(usize, usize, bool),
    /// No combination of the frame has the value.
    Nothing,
}

impl HeldTo {
    fn holds(&self, env: &[Val]) -> bool {
        match self {
            HeldTo::Is(at, v) => env[*at] == *v,
            HeldTo::Same(at, i, j, want) => match &env[*at] {
                Val::Relation(r) => matches!((r.blocks[*i], r.blocks[*j]), (Some(x), Some(y)) if x == y) == *want,
                _ => !*want,
            },
            HeldTo::Member(at, i, want) => match &env[*at] {
                Val::Relation(r) => r.member(*i) == *want,
                _ => !*want,
            },
            HeldTo::Nothing => false,
        }
    }
}

/// What one attribute held to a value holds the combinations of a frame to: None when nothing in
/// the frame rests on it (the type has no such attribute, or no policy of the action reads it);
/// Err with the reason when the attribute cannot have the value (a bool held to an enum's value).
fn held_to(g: &Gate, f: &walk::Frame, x: &ritsu_ports::GateGiven) -> Result<Option<HeldTo>, Text> {
    use ritsu_ports::GateGivenValue as V;
    let owner = if x.of_resource { Owner::Resource } else { Owner::Principal };
    let ty = g.owner_type(owner, f.principal, f.resource);
    let Some(i) = ty.attrs.iter().position(|a| a.named.is(&x.attribute)) else { return Ok(None) };
    let attr = &ty.attrs[i];
    let place = x.place();
    // the term of the attribute in the relation of its entity type, where the policies relate it
    let term = |t: usize| -> Option<(usize, usize)> {
        let at = f.place(&walk::Slot::Relation(t))?;
        let i = f.terms.get(&at)?.iter().position(|term| matches!(term, Term::Attr(o, n) if *o == owner && attr.named.is(n)))?;
        Some((at, i))
    };
    Ok(match (&x.value, &attr.ty) {
        (V::Bool(b), FieldType::Bool) => f.place(&walk::Slot::Attr(owner, i)).map(|at| HeldTo::Is(at, Val::Bool(*b))),
        (V::Enum(v), FieldType::Enum(e)) => {
            let Some(k) = g.enums[*e].values.iter().position(|n| n.is(v)) else {
                let name = &g.enums[*e].named.name;
                return Err(tr!("`{v}` は、`{place}` の列挙 `{name}` の値ではありません", "`{v}` is no value of `{name}`, the enum of `{place}`"));
            };
            f.place(&walk::Slot::Attr(owner, i)).map(|at| HeldTo::Is(at, Val::Enum(k)))
        }
        (V::IsPrincipal(want), FieldType::Entity(t)) => {
            if *t != f.principal {
                // an entity of another type is never the principal
                return Ok(want.then_some(HeldTo::Nothing));
            }
            let Some((at, i)) = term(*t) else { return Ok(None) };
            let Some(j) = f.terms.get(&at).and_then(|ts| ts.iter().position(|term| *term == Term::Principal)) else { return Ok(None) };
            Some(HeldTo::Same(at, i, j, *want))
        }
        (V::HoldsPrincipal(want), FieldType::Entity(t)) => {
            let Some((at, i)) = term(*t) else { return Ok(None) };
            if !f.members.get(&at).is_some_and(|m| m.contains(&i)) {
                return Ok(None);
            }
            Some(HeldTo::Member(at, i, *want))
        }
        (V::Bool(_), _) => return Err(tr!("`{place}` は bool ではありません", "`{place}` is not a bool")),
        (V::Enum(_), _) => return Err(tr!("`{place}` は列挙ではありません", "`{place}` is not an enum")),
        (V::IsPrincipal(_) | V::HoldsPrincipal(_), _) => return Err(tr!("`{place}` は、エンティティを指す属性ではありません", "`{place}` does not point to an entity")),
    })
}

/// [`allowance`] with some attributes held to values (what the port `Gates::allowed_with` answers,
/// DESIGN 8.3): how far a principal of the type `principal` (every type the action takes,
/// for None), whatever roles it holds, is allowed the action over the combinations that agree with
/// `given`, and over every value of the rest. An attribute nothing in a frame rests on holds its
/// combinations to nothing; a value none of them has leaves none, which is never allowed.
/// Undecided, with the reason, for a value the attribute's type cannot have, and where
/// [`allowance`] is.
pub fn allowance_with(g: &Gate, s: &Space, known: &Known, ask: &dyn Ask, principal: Option<&str>, given: &[ritsu_ports::GateGiven]) -> ritsu_ports::Found<ritsu_ports::Allowance> {
    use ritsu_ports::{Allowance, Found};
    let ty = match principal {
        Some(p) => match g.types.iter().position(|x| x.named.is(p)) {
            Some(t) => Some(t),
            None => return Found::Value(Allowance::Never),
        },
        None => None,
    };
    let mut frames: Vec<Option<Vec<HeldTo>>> = Vec::new();
    for f in &s.frames {
        if ty.is_some_and(|t| t != f.principal) {
            frames.push(None);
            continue;
        }
        let mut tests = Vec::new();
        for x in given {
            match held_to(g, f, x) {
                Ok(Some(t)) => tests.push(t),
                Ok(None) => {}
                Err(why) => return Found::Undecided(why),
            }
        }
        frames.push(Some(tests));
    }
    allowance_over(g, s, known, ask, &|frame, env| frames[frame].as_ref().is_some_and(|tests| tests.iter().all(|t| t.holds(env))))
}
