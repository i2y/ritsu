//! The walk (DESIGN 4.1): every combination of the finite values the policies and expectations of
//! an action read. An action is walked as frames, one for each principal type and resource type a
//! request can name; a frame is the product of its factors, and a factor is the list of values one
//! thing can take, or of the combinations several things that read the same data can take
//! together:
//!
//! - the roles a principal holds: every set of the roles the policies read and of the roles that
//!   include them, the empty set among them;
//! - a bool or an enum: each of its values (and absent, for one that may be);
//! - a number compared with constants: the cells the constants cut its range into
//!   ([`crate::cells`]);
//! - the entities the policies relate (`resource.customer is principal`): each way the terms of a
//!   type can be the same entity or not, and for each, whether the principal is a member of each
//!   group a policy asks of;
//! - a rule's output, with the numbers it reads that a policy also compares: the cells of those
//!   numbers, each with the values rulec says the output can come to over it
//!   (`Rules::outputs_over`);
//! - the dates of `today`: every combination of truths the dates can come to together, from
//!   koyomi's days over every day of `today` and every value the dates are given.
//!
//! What another language cannot say exactly is counted as every value it could be; such a value
//! is marked inexact, so that an answer that rests on one is tried again with a concrete input
//! ([`confirm`]) before it is given. The walk is never sampled: past the budget, the check stops
//! (E307).

use crate::ast::Op;
use crate::cells::{self, Cell};
use crate::eval::{CAtom, CExpr, Decision, decide};
use crate::model::*;
use ritsu_base::text::Text;
use ritsu_base::tr;
use ritsu_ports::{DateValue, DaySet, Found, RuleError, Said, Value, Values};
use ritsu_units::Unit;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path as FsPath, PathBuf};
use std::rc::Rc;

/// What the walk asks of rulec and koyomi. `ritsu sekisho` answers it from the joined ports; the
/// tests answer it from the languages themselves, or from a stand-in that cannot say everything.
pub trait Ask {
    /// The values a rule's output can come to with each numeric input held to a range (by name,
    /// both ends in), each with an input that reaches it (`Rules::outputs_over`).
    fn outputs(&self, rule: &FsPath, output: &str, ranges: &[(String, Option<i128>, Option<i128>)]) -> Result<Found<Vec<(Value, Values)>>, Vec<Said>>;
    /// The outputs of a rule for one input (rulec's reference evaluator).
    fn rule(&self, rule: &FsPath, inputs: &Values) -> Result<Values, RuleError>;
    /// Every date of a dates file for one input (koyomi's evaluator).
    fn dates(&self, file: &FsPath, inputs: &[(String, i64)]) -> Result<Vec<(String, DateValue)>, Vec<Said>>;
}

/// A value of one place of a combination.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Val {
    /// The roles a principal holds, as masks over the file's roles: those given to it, and those
    /// with the roles they include.
    Roles { direct: u128, effective: u128 },
    Workflow(usize),
    Bool(bool),
    Enum(usize),
    Cell(Cell),
    /// An attribute that may be absent and is, or a value the frame cannot compute.
    Absent,
    Relation(Rel),
}

/// Which of the terms of a type are the same entity, and which groups the principal is a member
/// of.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Rel {
    /// For each term, its block (None: the attribute is absent).
    pub blocks: Vec<Option<u8>>,
    /// For each block, whether the principal is a member of that entity.
    pub members: u64,
    /// The term that is the principal, when the principal is of the type.
    pub principal: Option<usize>,
}

impl Rel {
    /// Whether the principal is in the entity the term points to: the same entity (Cedar's `in`
    /// holds of an entity and itself), or a group it is a member of.
    pub fn member(&self, term: usize) -> bool {
        match self.blocks[term] {
            None => false,
            Some(b) => self.principal.and_then(|p| self.blocks[p]) == Some(b) || self.members & (1 << b) != 0,
        }
    }
}

/// What a place of a frame holds.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Slot {
    /// Always the first place of a frame.
    Roles,
    Workflow,
    /// An attribute of the principal's or the resource's type, by its index.
    Attr(Owner, usize),
    /// An input of the action.
    Input(usize),
    /// A value the action computes.
    Computed(usize),
    /// The terms of an entity type that the policies relate.
    Relation(usize),
}

/// What a factor is, for saying it and for trying an inexact value again.
#[derive(Clone, Debug, PartialEq)]
pub enum FactorKind {
    Roles,
    Workflow,
    /// One attribute or input.
    One,
    Relation,
    /// Rule outputs (by place and computed value), with the places of the attributes and inputs
    /// they read that a policy also reads.
    Rules { outputs: Vec<(usize, usize)>, cut: Vec<usize> },
    Dates,
}

/// The values one thing, or several things that read the same data, can take.
#[derive(Clone, Debug, PartialEq)]
pub struct Factor {
    pub kind: FactorKind,
    /// The places it sets.
    pub places: Vec<usize>,
    pub points: Vec<Point>,
}

/// One value of a factor: a value for each of its places.
#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    pub vals: Vec<Val>,
    /// False when another language could not say whether this can happen, and it is counted in
    /// case it can.
    pub exact: bool,
}

/// One policy, compiled for a frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Compiled {
    /// `principal …` (true when the policy has no such line).
    pub scope: CExpr,
    /// Each `when` (and each `unless`, negated), in the order written.
    pub conds: Vec<CExpr>,
    /// The scope and every condition.
    pub all: CExpr,
}

/// The combinations of one principal type and one resource type of an action.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub principal: usize,
    pub resource: usize,
    /// The roles walked, as a mask over the file's roles.
    pub walked_roles: u128,
    pub slots: Vec<Slot>,
    /// The terms of each relation, by the place of the relation, and those the principal is asked
    /// to be a member of.
    pub terms: BTreeMap<usize, Vec<Term>>,
    pub members: BTreeMap<usize, Vec<usize>>,
    pub factors: Vec<Factor>,
    /// The places the frame fixes: a value computed from an attribute the type does not have is
    /// absent.
    pub fixed: Vec<(usize, Val)>,
    /// For each policy of the space.
    pub policies: Vec<Compiled>,
    /// For each expectation of the space: what it picks, compiled as a policy is (its `principal`
    /// line, each of its lines, and all of them).
    pub expects: Vec<Compiled>,
}

/// Every combination of one action.
#[derive(Clone, Debug, PartialEq)]
pub struct Space {
    pub action: usize,
    /// The policies on the action (by index into the file's), and which are permits.
    pub policies: Vec<usize>,
    pub permit: Vec<bool>,
    pub expects: Vec<usize>,
    pub frames: Vec<Frame>,
    /// The values of each computed enum, by the computed value, as the rule names them.
    pub names: BTreeMap<usize, Vec<Named>>,
    /// How many evaluations the dates took to walk (counted in the budget).
    pub day_walk: u128,
}

/// One combination, as a visitor of [`Space::each`] sees it.
pub struct Combo<'a> {
    pub frame: usize,
    /// The point of each factor.
    pub idx: &'a [usize],
    /// The value of each place.
    pub env: &'a [Val],
    pub exact: bool,
    /// For each policy of the space, whether it applies.
    pub applies: &'a [bool],
    pub decision: &'a Decision,
}

/// A combination kept to show as an example: the frame and the point of each factor.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Kept {
    pub frame: usize,
    pub idx: Vec<usize>,
}

impl Combo<'_> {
    pub fn keep(&self) -> Kept {
        Kept { frame: self.frame, idx: self.idx.to_vec() }
    }
}

impl Factor {
    fn set(&self, env: &mut [Val], i: usize) {
        for (at, v) in self.places.iter().zip(&self.points[i].vals) {
            env[*at] = v.clone();
        }
    }
}

impl Space {
    /// The number of combinations.
    pub fn count(&self) -> u128 {
        self.frames.iter().map(|f| f.count()).sum()
    }

    /// Every combination, in order: frame by frame, the last factor turning fastest.
    pub fn each(&self, mut visit: impl FnMut(&Combo)) {
        for (fi, frame) in self.frames.iter().enumerate() {
            if frame.factors.iter().any(|f| f.points.is_empty()) {
                continue;
            }
            let mut env = frame.env0();
            let n = frame.factors.len();
            let mut idx = vec![0usize; n];
            let mut applies = vec![false; frame.policies.len()];
            loop {
                for (i, p) in frame.policies.iter().enumerate() {
                    applies[i] = p.all.holds(&env);
                }
                let decision = decide(&applies, &self.permit);
                let exact = frame.factors.iter().zip(&idx).all(|(f, &i)| f.points[i].exact);
                visit(&Combo { frame: fi, idx: &idx, env: &env, exact, applies: &applies, decision: &decision });
                let mut k = n;
                let more = loop {
                    if k == 0 {
                        break false;
                    }
                    k -= 1;
                    idx[k] += 1;
                    if idx[k] < frame.factors[k].points.len() {
                        frame.factors[k].set(&mut env, idx[k]);
                        break true;
                    }
                    idx[k] = 0;
                    frame.factors[k].set(&mut env, 0);
                };
                if !more {
                    break;
                }
            }
        }
    }

    /// The values of a kept combination.
    pub fn env_of(&self, k: &Kept) -> Vec<Val> {
        let f = &self.frames[k.frame];
        let mut env = f.env0();
        for (x, &i) in f.factors.iter().zip(&k.idx) {
            x.set(&mut env, i);
        }
        env
    }

    /// The decision of a kept combination.
    pub fn decision_of(&self, k: &Kept) -> Decision {
        let env = self.env_of(k);
        let applies: Vec<bool> = self.frames[k.frame].policies.iter().map(|p| p.all.holds(&env)).collect();
        decide(&applies, &self.permit)
    }

    /// Whether some value of the walk is counted though another language could not say it can
    /// happen.
    pub fn inexact(&self) -> bool {
        self.frames.iter().any(|f| f.factors.iter().any(|x| x.points.iter().any(|p| !p.exact)))
    }

    /// Whether a kept combination rests only on values the languages said can happen.
    pub fn exact(&self, k: &Kept) -> bool {
        self.frames[k.frame].factors.iter().zip(&k.idx).all(|(f, &i)| f.points[i].exact)
    }
}

impl Frame {
    pub fn count(&self) -> u128 {
        self.factors.iter().map(|f| f.points.len() as u128).product()
    }

    /// The place of a slot, when the frame walks it.
    pub fn place(&self, s: &Slot) -> Option<usize> {
        self.slots.iter().position(|x| x == s)
    }

    /// The values before any factor sets its own: the fixed ones, and absent elsewhere.
    fn env0(&self) -> Vec<Val> {
        let mut env = vec![Val::Absent; self.slots.len()];
        for (at, v) in &self.fixed {
            env[*at] = v.clone();
        }
        for f in &self.factors {
            if !f.points.is_empty() {
                f.set(&mut env, 0);
            }
        }
        env
    }
}

// ---------------------------------------------------------------------------------------------
// What the other languages said of the computed values

/// What the other languages say of the values the actions compute, as the walk needs it (the
/// checks of the borders gather it, [`crate::borders`]).
#[derive(Clone, Debug, Default)]
pub struct Known {
    /// By (action, computed value).
    pub computed: BTreeMap<(usize, usize), Kn>,
}

#[derive(Clone, Debug)]
pub enum Kn {
    /// A rule's output: the file, the output, its values, each input of the rule by name with its
    /// unit when it is a number, and for an input that is an enum, each value as the rule's
    /// evaluator takes it (its name) with the words that name it (the name, the alias in the
    /// generated code, the public name).
    Rule { file: PathBuf, output: String, domain: Domain, inputs: Vec<(String, Option<Unit>)>, enums: Vec<(String, EnumInput)> },
    /// A date of a dates file: the file, the date, and the file's inputs (name, whether a date).
    Date { file: PathBuf, function: String, inputs: Vec<(String, bool)> },
    /// A date attribute compared with `today`.
    Attr,
    /// A calendar: the days its data covers, and the days it closes.
    Open { data: (Day, Day), closed: Rc<DaySet> },
}

/// The values of an enum input of a rule: each as the rule's evaluator takes it (its name), with the
/// words that name it (the name, the alias in the generated code, the public name).
pub type EnumInput = Vec<(String, Vec<String>)>;

/// The values a computed value can be: true and false, or the values of a rule's enum, each by its
/// name and, as its alias, the string Cedar is given.
#[derive(Clone, Debug, PartialEq)]
pub enum Domain {
    Bool,
    Enum(Vec<Named>),
}

impl Domain {
    pub fn vals(&self) -> Vec<Val> {
        match self {
            Domain::Bool => vec![Val::Bool(false), Val::Bool(true)],
            Domain::Enum(vs) => (0..vs.len()).map(Val::Enum).collect(),
        }
    }

    pub fn of_value(&self, v: &Value) -> Option<Val> {
        match (self, v) {
            (Domain::Bool, Value::Bool(b)) => Some(Val::Bool(*b)),
            (Domain::Enum(vs), Value::Enum(n) | Value::Str(n)) => vs.iter().position(|x| x.is(n)).map(Val::Enum),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Building the space of an action

/// Why an action cannot be walked.
#[derive(Clone, Debug, PartialEq)]
pub enum Stop {
    /// More combinations than the budget (E307): how many, whether they are the pairs of days the
    /// dates are walked over rather than combinations, what makes them (in both languages), and
    /// whether the sets of roles are what makes the most.
    Budget { count: u128, days: bool, why: Vec<(Text, u128)>, roles: bool },
    /// A value the walk needs is not known: its language could not be read, or the file has no
    /// `today` (another diagnostic says why).
    Unknown,
}

/// The numeric inputs of a rule held to ranges, by name (both ends in; None at an open end).
pub type Ranges = Vec<(String, Option<i128>, Option<i128>)>;

/// What rulec answers of an output over ranges, and what koyomi answers for one input.
type Outputs = Result<Found<Vec<(Value, Values)>>, Vec<Said>>;
type DatesOf = Result<Vec<(String, DateValue)>, Vec<Said>>;
/// An input of a dates file, each by name.
type DateInputs = Vec<(String, i64)>;

/// Remembers what was asked of the other languages in one check, so that two frames or two
/// actions that ask the same thing ask once.
#[derive(Default)]
pub struct Memo {
    outputs: RefCell<HashMap<(PathBuf, String, Ranges), Outputs>>,
    dates: RefCell<HashMap<(PathBuf, DateInputs), DatesOf>>,
}

impl Memo {
    fn outputs(&self, ask: &dyn Ask, rule: &FsPath, output: &str, ranges: Ranges) -> Outputs {
        let key = (rule.to_path_buf(), output.to_string(), ranges);
        if let Some(r) = self.outputs.borrow().get(&key) {
            return r.clone();
        }
        let r = ask.outputs(rule, output, &key.2);
        self.outputs.borrow_mut().insert(key, r.clone());
        r
    }

    fn dates(&self, ask: &dyn Ask, file: &FsPath, inputs: DateInputs) -> DatesOf {
        let key = (file.to_path_buf(), inputs);
        if let Some(r) = self.dates.borrow().get(&key) {
            return r.clone();
        }
        let r = ask.dates(file, &key.1);
        self.dates.borrow_mut().insert(key, r.clone());
        r
    }
}

/// The space of an action: its frames, each with its factors and its policies compiled.
pub fn space(g: &Gate, action: usize, known: &Known, ask: &dyn Ask, memo: &Memo, budget: u128) -> Result<Space, Stop> {
    let a = &g.actions[action];
    let policies = g.policies_on(action);
    let permit: Vec<bool> = policies.iter().map(|&p| g.policies[p].permit).collect();
    let expects = g.expects_on(action);
    let mut frames = Vec::new();
    let mut day_walk = 0u128;
    for &pt in &a.principals {
        for &rt in &a.resources {
            let mut c = Compiler::new(g, action, pt, rt, known);
            let compiled: Vec<Compiled> = policies.iter().map(|&p| c.policy(&g.policies[p])).collect();
            let selects: Vec<Compiled> = expects.iter().map(|&e| c.expect(&g.expects[e])).collect();
            // a condition reads a computed value no language answered for: another diagnostic says why
            if c.unknown {
                return Err(Stop::Unknown);
            }
            // what needs nothing from another language: stop before asking, when it alone is over
            let early = c.estimate();
            let n: u128 = early.iter().fold(1u128, |n, (_, k)| n.saturating_mul(*k));
            if n > budget {
                let roles = early.iter().map(|(_, k)| *k).max() == early.first().map(|(_, k)| *k);
                return Err(Stop::Budget { count: n, days: false, why: early, roles });
            }
            let (factors, fixed, walked) = c.factors(ask, memo, budget, &mut day_walk)?;
            let terms = c.terms_by_place();
            let members = c.members_by_place();
            frames.push(Frame { principal: pt, resource: rt, walked_roles: walked, slots: c.slots, terms, members, factors, fixed, policies: compiled, expects: selects });
        }
    }
    let names = known
        .computed
        .iter()
        .filter_map(|((x, i), kn)| match kn {
            Kn::Rule { domain: Domain::Enum(vs), .. } if *x == action => Some((*i, vs.clone())),
            _ => None,
        })
        .collect();
    let s = Space { action, policies, permit, expects, frames, names, day_walk };
    let total = s.count().saturating_add(day_walk);
    if total > budget {
        let mut why = Vec::new();
        for f in &s.frames {
            for x in &f.factors {
                let names: Vec<Text> = x.places.iter().map(|&p| slot_name(g, action, f.principal, f.resource, &f.slots[p])).collect();
                let t = Text::join(&names, "・", ", ");
                let ty = &g.types[f.principal].named.name;
                why.push((tr!("{ty}：{}", "{ty}: {}", t.ja; t.en), x.points.len() as u128));
            }
        }
        if day_walk > 0 {
            why.push((tr!("today と日付の計算", "the days of today and the dates computed"), day_walk));
        }
        let roles = s.frames.iter().any(|f| f.factors.first().is_some_and(|r| f.factors.iter().all(|x| x.points.len() <= r.points.len())));
        return Err(Stop::Budget { count: total, days: false, why, roles });
    }
    Ok(s)
}

/// How a message names a place: `roles`, `principal.suspended`, `amount`.
pub fn slot_name(g: &Gate, action: usize, principal: usize, resource: usize, s: &Slot) -> Text {
    let a = &g.actions[action];
    match s {
        Slot::Roles => tr!("{} の役割", "the roles of {}", g.types[principal].named.name),
        Slot::Workflow => tr!("ワークフロー", "the workflow"),
        Slot::Attr(o, i) => Text::same(format!("{}.{}", o.word(), g.owner_type(*o, principal, resource).attrs[*i].named.name)),
        Slot::Input(i) => Text::same(a.inputs[*i].named.name.clone()),
        Slot::Computed(i) => Text::same(a.computed[*i].named.name.clone()),
        Slot::Relation(t) => tr!("{} を指すもの", "what points to {}", g.types[*t].named.name),
    }
}

/// What a frame's compiler makes: the factors, the places the frame fixes, and the roles walked.
type Made = (Vec<Factor>, Vec<(usize, Val)>, u128);

/// The type of what a path reads, as a condition compares it.
#[derive(Clone, Debug, PartialEq)]
enum Ty {
    Bool,
    Enum(usize),
    Num(Unit),
    Other,
    /// A computed value: an enum's values, or None for a bool.
    Computed(Option<Vec<Named>>),
}

impl Ty {
    fn of(t: &FieldType) -> Ty {
        match t {
            FieldType::Bool => Ty::Bool,
            FieldType::Enum(e) => Ty::Enum(*e),
            FieldType::Num { unit, .. } => Ty::Num(unit.clone()),
            _ => Ty::Other,
        }
    }
}

/// The compiler of one frame: it gives every place a policy reads its index, gathers the
/// constants each number is compared with and the terms each relation relates, and then makes
/// the factors.
struct Compiler<'g> {
    g: &'g Gate,
    action: usize,
    principal: usize,
    resource: usize,
    known: &'g Known,
    slots: Vec<Slot>,
    /// The constants each numeric place is compared with.
    cmps: BTreeMap<usize, Vec<(Op, i128)>>,
    /// The terms of each entity type, and those the principal is asked to be a member of.
    terms: BTreeMap<usize, Vec<Term>>,
    members: BTreeMap<usize, BTreeSet<usize>>,
    roles_read: u128,
    workflow_read: bool,
    /// A condition reads a computed value no language answered for (another diagnostic says why):
    /// the action is not walked.
    unknown: bool,
}

impl<'g> Compiler<'g> {
    fn new(g: &'g Gate, action: usize, principal: usize, resource: usize, known: &'g Known) -> Compiler<'g> {
        Compiler { g, action, principal, resource, known, slots: vec![Slot::Roles], cmps: BTreeMap::new(), terms: BTreeMap::new(), members: BTreeMap::new(), roles_read: 0, workflow_read: false, unknown: false }
    }

    fn act(&self) -> &'g Action {
        &self.g.actions[self.action]
    }

    fn place(&mut self, s: Slot) -> usize {
        match self.slots.iter().position(|x| *x == s) {
            Some(i) => i,
            None => {
                self.slots.push(s);
                self.slots.len() - 1
            }
        }
    }

    fn who(&mut self, w: &Who) -> CExpr {
        match w {
            Who::Anyone => CExpr::truth(true),
            Who::InRoles(rs) => self.atom(&Atom::InRoles(rs.clone())),
            Who::IsType(t) => self.atom(&Atom::IsType(*t)),
            Who::IsWorkflow(w) => self.atom(&Atom::IsWorkflow(*w)),
        }
    }

    fn policy(&mut self, p: &Policy) -> Compiled {
        let scope = self.who(&p.who).simplify();
        let conds: Vec<CExpr> = p.conds.iter().map(|c| self.cond(c)).collect();
        let mut all = vec![scope.clone()];
        all.extend(conds.iter().cloned());
        Compiled { scope, conds, all: CExpr::And(all).simplify() }
    }

    fn expect(&mut self, e: &Expect) -> Compiled {
        let scope = self.who(&e.who).simplify();
        let conds: Vec<CExpr> = e.conds.iter().map(|c| self.cond(c)).collect();
        let mut all = vec![scope.clone()];
        all.extend(conds.iter().cloned());
        Compiled { scope, conds, all: CExpr::And(all).simplify() }
    }

    fn cond(&mut self, c: &Cond) -> CExpr {
        let e = self.expr(&c.expr);
        if c.when { e.simplify() } else { CExpr::Not(Box::new(e)).simplify() }
    }

    fn expr(&mut self, e: &Expr) -> CExpr {
        match e {
            Expr::Atom(a) => self.atom(a),
            Expr::Not(x) => CExpr::Not(Box::new(self.expr(x))),
            Expr::And(xs) => CExpr::And(xs.iter().map(|x| self.expr(x)).collect()),
            Expr::Or(xs) => CExpr::Or(xs.iter().map(|x| self.expr(x)).collect()),
        }
    }

    /// What a path reads in this frame: its slot and its type; None when the frame has no such
    /// thing (an attribute of another type), or the value is computed and no language answered
    /// for it (then the action is not walked).
    fn resolve(&mut self, p: &Path) -> Option<(Slot, Ty)> {
        match p {
            Path::Attr(o, name) => {
                let t = self.g.owner_type(*o, self.principal, self.resource);
                let (i, f) = t.attr(name)?;
                Some((Slot::Attr(*o, i), Ty::of(&f.ty)))
            }
            Path::Value(name) => {
                let a = self.act();
                if let Some((i, f)) = a.input(name) {
                    return Some((Slot::Input(i), Ty::of(&f.ty)));
                }
                let (i, _) = a.computed_value(name)?;
                let Some(kn) = self.known.computed.get(&(self.action, i)) else {
                    self.unknown = true;
                    return None;
                };
                match kn {
                    Kn::Rule { domain: Domain::Enum(vs), .. } => Some((Slot::Computed(i), Ty::Computed(Some(vs.clone())))),
                    _ => Some((Slot::Computed(i), Ty::Computed(None))),
                }
            }
        }
    }

    /// The entity type a term points to, in this frame.
    fn term_type(&self, t: &Term) -> Option<usize> {
        match t {
            Term::Principal => Some(self.principal),
            Term::Attr(o, name) => match &self.g.owner_type(*o, self.principal, self.resource).attr(name)?.1.ty {
                FieldType::Entity(e) => Some(*e),
                _ => None,
            },
        }
    }

    fn term(&mut self, ty: usize, t: &Term) -> (usize, usize) {
        let terms = self.terms.entry(ty).or_default();
        let i = match terms.iter().position(|x| x == t) {
            Some(i) => i,
            None => {
                terms.push(t.clone());
                terms.len() - 1
            }
        };
        (self.place(Slot::Relation(ty)), i)
    }

    fn compare(&mut self, s: Slot, unit: &Unit, op: Op, lit: &Literal) -> CAtom {
        let Literal::Num(num) = lit else { return CAtom::Const(false) };
        match crate::types::count(num, unit) {
            Ok(k) => {
                let at = self.place(s);
                self.cmps.entry(at).or_default().push((op, k));
                CAtom::Cmp(at, op, k)
            }
            // E102 and E103 say why
            Err(_) => CAtom::Const(false),
        }
    }

    fn atom(&mut self, a: &Atom) -> CExpr {
        CExpr::Atom(match a {
            Atom::InRoles(rs) => {
                let mask = rs.iter().fold(0u128, |m, r| m | bit(*r));
                self.roles_read |= mask;
                if self.g.types[self.principal].roles.is_empty() { CAtom::Const(false) } else { CAtom::Roles(mask) }
            }
            Atom::IsType(t) => CAtom::Const(*t == self.principal),
            Atom::IsWorkflow(w) => {
                if self.g.types[self.principal].kind == Kind::Workflow {
                    self.workflow_read = true;
                    CAtom::Workflow(self.place(Slot::Workflow), *w)
                } else {
                    CAtom::Const(false)
                }
            }
            Atom::True(p) => match self.resolve(p) {
                Some((s, Ty::Bool | Ty::Computed(None))) => CAtom::Bool(self.place(s), true),
                _ => CAtom::Const(false),
            },
            Atom::Is(p, lit) => match (self.resolve(p), lit) {
                (Some((s, Ty::Bool | Ty::Computed(None))), Literal::Bool(b)) => CAtom::Bool(self.place(s), *b),
                (Some((s, Ty::Enum(e))), Literal::Word(w)) => match self.g.enums[e].values.iter().position(|v| v.is(w)) {
                    Some(v) => CAtom::Enum(self.place(s), v),
                    None => CAtom::Const(false),
                },
                (Some((s, Ty::Computed(Some(vs)))), Literal::Word(w)) => match vs.iter().position(|v| v.is(w)) {
                    Some(v) => CAtom::Enum(self.place(s), v),
                    None => CAtom::Const(false),
                },
                (Some((s, Ty::Num(unit))), l) => self.compare(s, &unit, Op::Is, l),
                _ => CAtom::Const(false),
            },
            Atom::Cmp(p, op, lit) => match self.resolve(p) {
                Some((s, Ty::Num(unit))) => self.compare(s, &unit, *op, lit),
                _ => CAtom::Const(false),
            },
            Atom::Same(x, y) => match (self.term_type(x), self.term_type(y)) {
                (Some(tx), Some(ty)) if tx == ty => {
                    if x == y && *x == Term::Principal {
                        CAtom::Const(true)
                    } else {
                        let (at, i) = self.term(tx, x);
                        let (_, j) = self.term(tx, y);
                        CAtom::Same(at, i, j)
                    }
                }
                _ => CAtom::Const(false),
            },
            Atom::Member(t) => match self.term_type(t) {
                Some(ty) => {
                    let (at, i) = self.term(ty, t);
                    if ty == self.principal {
                        self.term(ty, &Term::Principal);
                    }
                    self.members.entry(ty).or_default().insert(i);
                    CAtom::Member(at, i)
                }
                None => CAtom::Const(false),
            },
            Atom::Eq(p, q) => {
                // two attributes that point to entities are a relation; two values of one enum, a comparison
                let as_term = |p: &Path| match p {
                    Path::Attr(o, n) => Some(Term::Attr(*o, n.clone())),
                    Path::Value(_) => None,
                };
                if let (Some(x), Some(y)) = (as_term(p), as_term(q))
                    && let (Some(tx), Some(ty)) = (self.term_type(&x), self.term_type(&y))
                {
                    return CExpr::Atom(if tx == ty {
                        let (at, i) = self.term(tx, &x);
                        let (_, j) = self.term(tx, &y);
                        CAtom::Same(at, i, j)
                    } else {
                        CAtom::Const(false)
                    });
                }
                match (self.resolve(p), self.resolve(q)) {
                    (Some((s, Ty::Enum(e))), Some((t, Ty::Enum(f)))) if e == f => {
                        let (x, y) = (self.place(s), self.place(t));
                        CAtom::EnumEq(x, y)
                    }
                    (Some((s, Ty::Computed(Some(a)))), Some((t, Ty::Computed(Some(b))))) if a == b => {
                        let (x, y) = (self.place(s), self.place(t));
                        CAtom::EnumEq(x, y)
                    }
                    _ => CAtom::Const(false),
                }
            }
        })
    }

    fn terms_by_place(&self) -> BTreeMap<usize, Vec<Term>> {
        self.terms.iter().filter_map(|(ty, ts)| self.slots.iter().position(|s| *s == Slot::Relation(*ty)).map(|at| (at, ts.clone()))).collect()
    }

    fn members_by_place(&self) -> BTreeMap<usize, Vec<usize>> {
        self.members.iter().filter_map(|(ty, ts)| self.slots.iter().position(|s| *s == Slot::Relation(*ty)).map(|at| (at, ts.iter().copied().collect()))).collect()
    }

    /// The roles walked: of the principal type's roles, those that are read or include one that
    /// is.
    fn walked_roles(&self) -> Vec<usize> {
        let t = &self.g.types[self.principal];
        t.roles.iter().copied().filter(|&r| self.g.closure(r).iter().any(|x| self.roles_read & bit(*x) != 0)).collect()
    }

    /// The sizes of what needs nothing from another language, to stop early when they alone are
    /// over the budget.
    fn estimate(&self) -> Vec<(Text, u128)> {
        let n = self.walked_roles().len();
        let mut out = vec![(slot_name(self.g, self.action, self.principal, self.resource, &Slot::Roles), if n >= 127 { u128::MAX } else { 1u128 << n })];
        for (at, s) in self.slots.iter().enumerate() {
            if let Slot::Attr(..) | Slot::Input(_) = s
                && let Some(d) = self.domain(at)
            {
                out.push((slot_name(self.g, self.action, self.principal, self.resource, s), d.len() as u128));
            }
        }
        out
    }

    /// The field of an attribute or input place.
    fn field(&self, s: &Slot) -> Option<&'g Field> {
        match s {
            Slot::Attr(o, i) => Some(&self.g.owner_type(*o, self.principal, self.resource).attrs[*i]),
            Slot::Input(i) => Some(&self.g.actions[self.action].inputs[*i]),
            _ => None,
        }
    }

    /// The values the place of an attribute or an input takes on its own.
    fn domain(&self, at: usize) -> Option<Vec<Val>> {
        let f = self.field(&self.slots[at])?;
        let mut out: Vec<Val> = match &f.ty {
            FieldType::Bool => vec![Val::Bool(false), Val::Bool(true)],
            FieldType::Enum(e) => (0..self.g.enums[*e].values.len()).map(Val::Enum).collect(),
            FieldType::Num { lo, hi, .. } => cells::cut(*lo, *hi, self.cmps.get(&at).map(Vec::as_slice).unwrap_or(&[])).into_iter().map(Val::Cell).collect(),
            FieldType::Date { .. } | FieldType::Entity(_) => return None,
        };
        if f.optional {
            out.push(Val::Absent);
        }
        Some(out)
    }

    /// The order of a place among the factors: the attributes of the principal, those of the
    /// resource, the relations, the inputs, the computed values.
    fn order(&self, at: usize) -> usize {
        match &self.slots[at] {
            Slot::Roles => 0,
            Slot::Workflow => 1,
            Slot::Attr(Owner::Principal, i) => 1_000 + i,
            Slot::Attr(Owner::Resource, i) => 2_000 + i,
            Slot::Relation(t) => 3_000 + t,
            Slot::Input(i) => 4_000 + i,
            Slot::Computed(i) => 5_000 + i,
        }
    }

    /// The slot a source reads in this frame; None for a constant, Err for an attribute the
    /// frame's type does not have.
    fn source_slot(&self, src: &Source) -> Result<Option<Slot>, ()> {
        match src {
            Source::Attr(o, n) => self.g.owner_type(*o, self.principal, self.resource).attr(n).map(|(i, _)| Some(Slot::Attr(*o, i))).ok_or(()),
            Source::Input(n) => self.act().input(n).map(|(i, _)| Some(Slot::Input(i))).ok_or(()),
            Source::Lit(_) | Source::Today => Ok(None),
        }
    }

    /// The attributes and inputs a computed value is given, in this frame; None when one of them is
    /// an attribute the frame's type does not have (the value is not computed: absent).
    fn sources_of(&self, c: &Computed) -> Option<Vec<Slot>> {
        let srcs: Vec<Source> = match &c.how {
            How::Rule { args, .. } | How::Date { of: DateOf::Call { args, .. }, .. } => args.iter().map(|(_, s)| s.clone()).collect(),
            How::Date { of: DateOf::Attr(o, n), .. } => vec![Source::Attr(*o, n.clone())],
            How::Open { .. } => vec![],
        };
        let mut out = Vec::new();
        for s in &srcs {
            if let Some(slot) = self.source_slot(s).ok()?
                && !out.contains(&slot)
            {
                out.push(slot);
            }
        }
        Some(out)
    }

    /// The factors of the frame, in order: the roles, the workflow, then each thing read (or
    /// group of things that read the same data) in the order of [`Compiler::order`].
    fn factors(&mut self, ask: &dyn Ask, memo: &Memo, budget: u128, day_walk: &mut u128) -> Result<Made, Stop> {
        let g = self.g;
        let a = self.act();
        let mut fixed: Vec<(usize, Val)> = Vec::new();
        // the computed values read, and what each reads
        let mut rule_values: Vec<(usize, usize)> = Vec::new();
        let mut date_values: Vec<(usize, usize)> = Vec::new();
        let mut reads: BTreeMap<usize, Vec<Slot>> = BTreeMap::new();
        for at in 0..self.slots.len() {
            let Slot::Computed(i) = self.slots[at] else { continue };
            let c = &a.computed[i];
            if !self.known.computed.contains_key(&(self.action, i)) {
                return Err(Stop::Unknown);
            }
            match self.sources_of(c) {
                None => fixed.push((at, Val::Absent)),
                Some(srcs) => {
                    reads.insert(at, srcs);
                    match &c.how {
                        How::Rule { .. } => rule_values.push((at, i)),
                        How::Date { .. } | How::Open { .. } => date_values.push((at, i)),
                    }
                }
            }
        }
        let date_sources: BTreeSet<Slot> = date_values.iter().flat_map(|(at, _)| reads[at].iter().cloned()).collect();
        // the rule outputs joined by what they read
        let mut groups: Vec<Vec<usize>> = Vec::new();
        for &(at, _) in &rule_values {
            let mine: BTreeSet<&Slot> = reads[&at].iter().collect();
            let joined: Vec<usize> = (0..groups.len()).filter(|&gi| groups[gi].iter().any(|o| reads[o].iter().any(|s| mine.contains(s)))).collect();
            let mut merged = vec![at];
            for gi in joined.iter().rev() {
                merged.extend(groups.remove(*gi));
            }
            merged.sort();
            groups.push(merged);
        }
        // the places a condition reads itself
        let direct: Vec<usize> = (1..self.slots.len()).filter(|&at| matches!(self.slots[at], Slot::Attr(..) | Slot::Input(_))).collect();
        let mut taken: BTreeSet<usize> = BTreeSet::new();
        let mut units: Vec<(usize, Factor)> = Vec::new();
        if !date_values.is_empty() {
            let cut: Vec<usize> = direct.iter().copied().filter(|at| date_sources.contains(&self.slots[*at])).collect();
            let f = self.date_factor(&date_values, &cut, ask, memo, budget, day_walk)?;
            taken.extend(f.places.iter().copied());
            let key = f.places.iter().map(|&p| self.order(p)).min().unwrap_or(usize::MAX);
            units.push((key, f));
        }
        for grp in &groups {
            let read: BTreeSet<Slot> = grp.iter().flat_map(|at| reads[at].iter().cloned()).collect();
            let cut: Vec<usize> = direct.iter().copied().filter(|at| read.contains(&self.slots[*at]) && !taken.contains(at)).collect();
            // what is read by two outputs and not cut, or also by the dates, is not joined exactly
            let cut_slots: BTreeSet<&Slot> = cut.iter().map(|at| &self.slots[*at]).collect();
            let mut times: BTreeMap<&Slot, usize> = BTreeMap::new();
            for at in grp {
                for s in &reads[at] {
                    *times.entry(s).or_default() += 1;
                }
            }
            let loose = read.iter().any(|s| date_sources.contains(s)) || times.iter().any(|(s, n)| *n > 1 && !cut_slots.contains(s));
            let outputs: Vec<(usize, usize)> = grp.iter().map(|at| *rule_values.iter().find(|(p, _)| p == at).unwrap()).collect();
            let f = self.rule_factor(&outputs, &cut, loose, ask, memo)?;
            taken.extend(f.places.iter().copied());
            let key = f.places.iter().map(|&p| self.order(p)).min().unwrap_or(usize::MAX);
            units.push((key, f));
        }
        for &at in &direct {
            if taken.contains(&at) {
                continue;
            }
            if let Some(vals) = self.domain(at) {
                units.push((self.order(at), Factor { kind: FactorKind::One, places: vec![at], points: vals.into_iter().map(|v| Point { vals: vec![v], exact: true }).collect() }));
            }
        }
        let rels: Vec<(usize, usize)> = self.slots.iter().enumerate().filter_map(|(at, s)| if let Slot::Relation(t) = s { Some((at, *t)) } else { None }).collect();
        for (at, ty) in rels {
            units.push((self.order(at), self.relation_factor(at, ty)));
        }
        units.sort_by_key(|(k, _)| *k);
        let walked = self.walked_roles();
        let mut factors = vec![Factor { kind: FactorKind::Roles, places: vec![0], points: role_sets(g, &walked).into_iter().map(|v| Point { vals: vec![v], exact: true }).collect() }];
        if self.workflow_read && let Some(at) = self.slots.iter().position(|s| *s == Slot::Workflow) {
            factors.push(Factor { kind: FactorKind::Workflow, places: vec![at], points: (0..g.workflows.len()).map(|w| Point { vals: vec![Val::Workflow(w)], exact: true }).collect() });
        }
        factors.extend(units.into_iter().map(|(_, f)| f));
        Ok((factors, fixed, walked.iter().fold(0u128, |m, r| m | bit(*r))))
    }

    /// The factor of a group of rule outputs: for every value of the attributes and inputs they
    /// read that a policy also reads, the values rulec says each output can come to. `loose` says
    /// the outputs read something together that rulec is not asked of together.
    fn rule_factor(&self, outputs: &[(usize, usize)], cut: &[usize], loose: bool, ask: &dyn Ask, memo: &Memo) -> Result<Factor, Stop> {
        let a = self.act();
        let cut_domains: Vec<Vec<Val>> = cut.iter().map(|&at| self.domain(at).unwrap_or_else(|| vec![Val::Absent])).collect();
        let mut places = cut.to_vec();
        places.extend(outputs.iter().map(|(at, _)| *at));
        // past this many combinations of the cut values, rulec is asked once over the whole of each
        // input, and what it says is counted in case for each of them (marked inexact)
        let assignments: usize = cut_domains.iter().map(Vec::len).product();
        let many = assignments > ASKS;
        let mut points = Vec::new();
        for assignment in product(&cut_domains) {
            let mut per: Vec<(Vec<Val>, bool)> = Vec::new();
            for &(_, ci) in outputs {
                let c = &a.computed[ci];
                let (Some(Kn::Rule { file, output, domain, inputs, .. }), How::Rule { args, .. }) = (self.known.computed.get(&(self.action, ci)), &c.how) else { return Err(Stop::Unknown) };
                let mut ranges = Vec::new();
                let mut absent = false;
                let mut exact = !loose;
                let mut maybe_absent = false;
                for (input, src) in args {
                    let unit = inputs.iter().find(|(n, _)| n == input).and_then(|(_, u)| u.clone());
                    let slot = self.source_slot(src).map_err(|_| Stop::Unknown)?;
                    match (src, slot) {
                        (Source::Lit(Literal::Num(num)), _) => match unit.as_ref().and_then(|unit| crate::types::count(num, unit).ok()) {
                            Some(k) => ranges.push((input.clone(), Some(k), Some(k))),
                            None => exact = false,
                        },
                        // a day: rulec takes the range of a date as day numbers
                        (Source::Lit(Literal::Date(d)), _) => ranges.push((input.clone(), Some(*d as i128), Some(*d as i128))),
                        // today is read by the dates of the action too, which this does not join
                        (Source::Today, _) => {
                            if let Some(t) = &self.g.today {
                                ranges.push((input.clone(), Some(t.lo as i128), Some(t.hi as i128)));
                            }
                            exact = false;
                        }
                        // a bool or an enum given as a constant: rulec is asked over all of the input
                        (Source::Lit(_), _) => exact = false,
                        (_, Some(slot)) => {
                            let f = self.field(&slot).ok_or(Stop::Unknown)?;
                            match cut.iter().position(|at| self.slots[*at] == slot) {
                                Some(k) => match &assignment[k] {
                                    Val::Cell(cell) if !many => ranges.push((input.clone(), Some(cell.lo), Some(cell.hi))),
                                    Val::Cell(_) => {
                                        if let FieldType::Num { lo, hi, .. } = &f.ty {
                                            ranges.push((input.clone(), Some(*lo), Some(*hi)));
                                        }
                                        exact = false;
                                    }
                                    Val::Absent => absent = true,
                                    _ => exact = false,
                                },
                                None => {
                                    match &f.ty {
                                        FieldType::Num { lo, hi, .. } => ranges.push((input.clone(), Some(*lo), Some(*hi))),
                                        FieldType::Date { lo, hi } => ranges.push((input.clone(), Some(*lo as i128), Some(*hi as i128))),
                                        FieldType::Bool | FieldType::Enum(_) => {}
                                        FieldType::Entity(_) => exact = false,
                                    }
                                    maybe_absent |= f.optional;
                                }
                            }
                        }
                        (_, None) => return Err(Stop::Unknown),
                    }
                }
                if absent {
                    per.push((vec![Val::Absent], true));
                    continue;
                }
                let (mut vals, ok) = match memo.outputs(ask, file, output, ranges) {
                    Ok(Found::Value(vs)) if vs.iter().all(|(v, _)| domain.of_value(v).is_some()) => (domain.vals().into_iter().filter(|d| vs.iter().any(|(v, _)| domain.of_value(v).as_ref() == Some(d))).collect(), exact),
                    _ => (domain.vals(), false),
                };
                if maybe_absent {
                    vals.push(Val::Absent);
                }
                per.push((vals, ok));
            }
            let lists: Vec<Vec<Val>> = per.iter().map(|(v, _)| v.clone()).collect();
            let exact = per.iter().all(|(_, e)| *e);
            for outs in product(&lists) {
                let mut vals = assignment.clone();
                vals.extend(outs);
                points.push(Point { vals, exact });
            }
        }
        Ok(Factor { kind: FactorKind::Rules { outputs: outputs.to_vec(), cut: cut.to_vec() }, places, points })
    }

    /// The factor of the dates of `today`: every combination of truths the dates can come to
    /// together, with the cells of the numbers they read that a policy compares.
    fn date_factor(&self, date_values: &[(usize, usize)], cut: &[usize], ask: &dyn Ask, memo: &Memo, budget: u128, day_walk: &mut u128) -> Result<Factor, Stop> {
        let a = self.act();
        let today = self.g.today.as_ref().ok_or(Stop::Unknown)?;
        // the attributes and inputs the dates are given, each once
        let mut raws: Vec<Slot> = Vec::new();
        for (_, i) in date_values {
            for s in self.sources_of(&a.computed[*i]).unwrap_or_default() {
                if !raws.contains(&s) {
                    raws.push(s);
                }
            }
        }
        let mut raw_domains: Vec<Vec<Option<i64>>> = Vec::new();
        for s in &raws {
            let f = self.field(s).ok_or(Stop::Unknown)?;
            let mut vs: Vec<Option<i64>> = match &f.ty {
                FieldType::Date { lo, hi } => (*lo..=*hi).map(Some).collect(),
                FieldType::Num { lo, hi, .. } if *hi - *lo <= budget as i128 => (*lo as i64..=*hi as i64).map(Some).collect(),
                FieldType::Num { lo, hi, .. } => {
                    return Err(Stop::Budget { count: (*hi - *lo + 1) as u128, days: true, why: vec![(slot_name(self.g, self.action, self.principal, self.resource, s), (*hi - *lo + 1) as u128)], roles: false });
                }
                _ => return Err(Stop::Unknown),
            };
            if f.optional {
                vs.push(None);
            }
            raw_domains.push(vs);
        }
        let days = (today.hi - today.lo + 1).max(0) as u128;
        let assignments: u128 = raw_domains.iter().map(|d| d.len() as u128).product();
        if assignments.saturating_mul(days.max(1)) > budget {
            let mut why = vec![(tr!("today の日", "the days of today"), days)];
            for (s, d) in raws.iter().zip(&raw_domains) {
                why.push((slot_name(self.g, self.action, self.principal, self.resource, s), d.len() as u128));
            }
            return Err(Stop::Budget { count: assignments.saturating_mul(days.max(1)), days: true, why, roles: false });
        }
        // the numbers a policy compares, with their cells and their place among the raw values
        let cut_cells: Vec<(usize, usize, Vec<Cell>)> = cut
            .iter()
            .filter_map(|&at| {
                let k = raws.iter().position(|s| *s == self.slots[at])?;
                let FieldType::Num { lo, hi, .. } = &self.field(&self.slots[at])?.ty else { return None };
                Some((at, k, cells::cut(*lo, *hi, self.cmps.get(&at).map(Vec::as_slice).unwrap_or(&[]))))
            })
            .collect();
        // what the dates come to (and the cells cut) for one assignment of the raw values, today
        // being `today` for a date that is given it
        let compute = |asg: &[Option<i64>], today: Option<i64>| -> Result<Vec<DateVal>, Stop> {
            let mut w: Vec<DateVal> = Vec::new();
            for (_, k, cells) in &cut_cells {
                w.push(match asg[*k] {
                    None => DateVal::Absent,
                    Some(v) => DateVal::Cell(cells.iter().position(|c| c.contains(v as i128)).unwrap_or(0)),
                });
            }
            for (_, i) in date_values {
                let c = &a.computed[*i];
                let value_of = |src: &Source| -> Option<i64> {
                    match src {
                        Source::Lit(Literal::Date(d)) => Some(*d),
                        Source::Lit(Literal::Num(num)) if num.value.is_int() => Some(num.value.num as i64),
                        Source::Lit(_) => None,
                        Source::Today => today,
                        _ => {
                            let slot = self.source_slot(src).ok()??;
                            raws.iter().position(|s| *s == slot).and_then(|k| asg[k])
                        }
                    }
                };
                w.push(match &c.how {
                    How::Open { .. } => DateVal::Open,
                    How::Date { of: DateOf::Attr(o, n), .. } => match value_of(&Source::Attr(*o, n.clone())) {
                        Some(d) => DateVal::Day(d),
                        None => DateVal::Absent,
                    },
                    How::Date { of: DateOf::Call { args, .. }, .. } => {
                        let Some(Kn::Date { file, function, .. }) = self.known.computed.get(&(self.action, *i)) else { return Err(Stop::Unknown) };
                        let inputs: Option<Vec<(String, i64)>> = args.iter().map(|(n, s)| value_of(s).map(|v| (n.clone(), v))).collect();
                        match inputs {
                            None => DateVal::Absent,
                            Some(inputs) => match memo.dates(ask, file, inputs) {
                                Ok(out) => match out.iter().find(|(n, _)| n == function) {
                                    Some((_, DateValue::Day(d))) => DateVal::Day(*d),
                                    // the date stops there: the generated code refuses the request
                                    _ => DateVal::Absent,
                                },
                                Err(_) => return Err(Stop::Unknown),
                            },
                        }
                    }
                    How::Rule { .. } => DateVal::Absent,
                });
            }
            Ok(w)
        };
        // the truths of the dates on the day `t`, for what they come to
        let ncut = cut_cells.len();
        let truths = |t: i64, w: &[DateVal]| -> Result<Vec<Val>, Stop> {
            let mut tuple = Vec::with_capacity(w.len());
            for (k, x) in w.iter().enumerate() {
                if k < ncut {
                    tuple.push(match x {
                        DateVal::Cell(c) => Val::Cell(cut_cells[k].2[*c]),
                        _ => Val::Absent,
                    });
                    continue;
                }
                let (_, i) = date_values[k - ncut];
                tuple.push(match (&a.computed[i].how, x) {
                    (How::Open { .. }, _) => match self.known.computed.get(&(self.action, i)) {
                        Some(Kn::Open { data, closed }) => {
                            if t < data.0 || t > data.1 {
                                Val::Absent
                            } else {
                                Val::Bool(!closed.contains(&t))
                            }
                        }
                        _ => return Err(Stop::Unknown),
                    },
                    (How::Date { op, .. }, DateVal::Day(d)) => Val::Bool(cells::holds(*op, t, *d)),
                    _ => Val::Absent,
                });
            }
            Ok(tuple)
        };
        let given_today = date_values.iter().any(|(_, i)| match &a.computed[*i].how {
            How::Date { of: DateOf::Call { args, .. }, .. } => args.iter().any(|(_, s)| *s == Source::Today),
            _ => false,
        });
        let mut tuples: BTreeSet<Vec<Val>> = BTreeSet::new();
        if given_today {
            // a date that is given today is computed again for every day
            for t in today.lo..=today.hi {
                for asg in product_opt(&raw_domains) {
                    let w = compute(&asg, Some(t))?;
                    tuples.insert(truths(t, &w)?);
                }
            }
            *day_walk += assignments * days;
        } else {
            // every distinct combination of what the dates come to, over the raw values; then every
            // day of today against each
            let mut seen: BTreeSet<Vec<DateVal>> = BTreeSet::new();
            for asg in product_opt(&raw_domains) {
                seen.insert(compute(&asg, None)?);
            }
            *day_walk += assignments + days * seen.len() as u128;
            for t in today.lo..=today.hi {
                for w in &seen {
                    tuples.insert(truths(t, w)?);
                }
            }
        }
        let mut places: Vec<usize> = cut_cells.iter().map(|(at, ..)| *at).collect();
        places.extend(date_values.iter().map(|(at, _)| *at));
        Ok(Factor { kind: FactorKind::Dates, places, points: tuples.into_iter().map(|vals| Point { vals, exact: true }).collect() })
    }

    /// The factor of the terms of one entity type: every way they can be the same entity or not
    /// (and absent, for an attribute that may be), and for each, whether the principal is a
    /// member of each group a policy asks of.
    fn relation_factor(&self, at: usize, ty: usize) -> Factor {
        let terms = self.terms.get(&ty).cloned().unwrap_or_default();
        let principal = terms.iter().position(|t| *t == Term::Principal);
        let optional: Vec<bool> = terms
            .iter()
            .map(|t| match t {
                Term::Principal => false,
                Term::Attr(o, n) => self.g.owner_type(*o, self.principal, self.resource).attr(n).is_some_and(|(_, f)| f.optional),
            })
            .collect();
        let targets = self.members.get(&ty).cloned().unwrap_or_default();
        let mut points = Vec::new();
        for blocks in partitions(&optional) {
            // the blocks a membership is asked of, other than the principal's own
            let pb = principal.and_then(|p| blocks[p]);
            let mut asked: Vec<u8> = targets.iter().filter_map(|t| blocks[*t]).filter(|b| Some(*b) != pb).collect();
            asked.sort();
            asked.dedup();
            for bits in 0..(1u64 << asked.len()) {
                let members = asked.iter().enumerate().fold(0u64, |m, (k, b)| if bits & (1 << k) != 0 { m | (1 << b) } else { m });
                points.push(Point { vals: vec![Val::Relation(Rel { blocks: blocks.clone(), members, principal })], exact: true });
            }
        }
        Factor { kind: FactorKind::Relation, places: vec![at], points }
    }
}

/// The most combinations of the cut values of a group of rule outputs that rulec is asked over one
/// by one.
const ASKS: usize = 4096;

/// What a date comes to for one combination of the values it is given.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DateVal {
    Day(i64),
    /// `today is open in …`: it reads today only.
    Open,
    Cell(usize),
    Absent,
}

pub fn bit(r: usize) -> u128 {
    if r < 128 { 1u128 << r } else { 0 }
}

/// Every set of the roles, fewest first, each set in the roles' order; each as its value with the
/// roles it includes.
pub fn role_sets(g: &Gate, roles: &[usize]) -> Vec<Val> {
    let mut out = Vec::new();
    let n = roles.len().min(126);
    for k in 0..=n {
        combinations(n, k, &mut |pick| {
            let direct = pick.iter().fold(0u128, |m, &i| m | bit(roles[i]));
            let effective = pick.iter().fold(0u128, |m, &i| g.closure(roles[i]).iter().fold(m, |m, r| m | bit(*r)));
            out.push(Val::Roles { direct, effective });
        });
    }
    out
}

fn combinations(n: usize, k: usize, f: &mut dyn FnMut(&[usize])) {
    if k > n {
        return;
    }
    let mut pick: Vec<usize> = (0..k).collect();
    loop {
        f(&pick);
        let mut i = k;
        loop {
            if i == 0 {
                return;
            }
            i -= 1;
            if pick[i] < n - k + i {
                pick[i] += 1;
                for j in i + 1..k {
                    pick[j] = pick[j - 1] + 1;
                }
                break;
            }
        }
    }
}

/// Every way to put terms in blocks of the same entity (restricted growth strings), an optional
/// term also absent.
pub fn partitions(optional: &[bool]) -> Vec<Vec<Option<u8>>> {
    fn go(optional: &[bool], cur: &mut Vec<Option<u8>>, out: &mut Vec<Vec<Option<u8>>>) {
        if cur.len() == optional.len() {
            out.push(cur.clone());
            return;
        }
        let next = cur.iter().flatten().max().map(|m| m + 1).unwrap_or(0);
        for b in 0..=next {
            cur.push(Some(b));
            go(optional, cur, out);
            cur.pop();
        }
        if optional[cur.len()] {
            cur.push(None);
            go(optional, cur, out);
            cur.pop();
        }
    }
    let mut out = Vec::new();
    go(optional, &mut Vec::new(), &mut out);
    out
}

/// The product of lists, the last turning fastest.
fn product<T: Clone>(lists: &[Vec<T>]) -> Vec<Vec<T>> {
    let mut out: Vec<Vec<T>> = vec![Vec::new()];
    for l in lists {
        let mut next = Vec::with_capacity(out.len() * l.len());
        for o in &out {
            for x in l {
                let mut v = o.clone();
                v.push(x.clone());
                next.push(v);
            }
        }
        out = next;
    }
    out
}

/// The product of the values the dates are given, one assignment at a time.
fn product_opt(lists: &[Vec<Option<i64>>]) -> impl Iterator<Item = Vec<Option<i64>>> + '_ {
    let n = lists.len();
    let mut idx = vec![0usize; n];
    let mut done = lists.iter().any(|l| l.is_empty());
    std::iter::from_fn(move || {
        if done {
            return None;
        }
        let cur: Vec<Option<i64>> = idx.iter().zip(lists).map(|(i, l)| l[*i]).collect();
        let mut k = n;
        loop {
            if k == 0 {
                done = true;
                break;
            }
            k -= 1;
            idx[k] += 1;
            if idx[k] < lists[k].len() {
                break;
            }
            idx[k] = 0;
        }
        Some(cur)
    })
}

// ---------------------------------------------------------------------------------------------
// Trying an inexact combination again

/// Whether a combination that rests on values another language could not vouch for can happen:
/// for each group of rule outputs it rests on, a concrete input is looked for — the ends and the
/// middle of each number's cell, every value of the rest — that gives every output the value the
/// combination has, by rulec's evaluator. True when every such group finds one.
pub fn confirm(g: &Gate, s: &Space, k: &Kept, known: &Known, ask: &dyn Ask) -> bool {
    let f = &s.frames[k.frame];
    let a = &g.actions[s.action];
    for (x, &i) in f.factors.iter().zip(&k.idx) {
        if x.points[i].exact {
            continue;
        }
        let FactorKind::Rules { outputs, cut } = &x.kind else { return false };
        let point = &x.points[i];
        let value_at = |at: usize| point.vals[x.places.iter().position(|p| *p == at).unwrap_or(0)].clone();
        // the candidates of each attribute and input the outputs read
        let mut raws: Vec<Slot> = Vec::new();
        for &(_, ci) in outputs {
            if let How::Rule { args, .. } = &a.computed[ci].how {
                for (_, src) in args {
                    let slot = match src {
                        Source::Attr(o, n) => g.owner_type(*o, f.principal, f.resource).attr(n).map(|(i, _)| Slot::Attr(*o, i)),
                        Source::Input(n) => a.input(n).map(|(i, _)| Slot::Input(i)),
                        Source::Lit(_) | Source::Today => None,
                    };
                    if let Some(sl) = slot
                        && !raws.contains(&sl)
                    {
                        raws.push(sl);
                    }
                }
            }
        }
        let field = |sl: &Slot| -> Option<&Field> {
            match sl {
                Slot::Attr(o, i) => Some(&g.owner_type(*o, f.principal, f.resource).attrs[*i]),
                Slot::Input(i) => Some(&a.inputs[*i]),
                _ => None,
            }
        };
        let mut cands: Vec<Vec<Option<Value>>> = Vec::new();
        for sl in &raws {
            let Some(fd) = field(sl) else { return false };
            let at = f.place(sl).filter(|at| cut.contains(at));
            let fixed = at.map(&value_at);
            let list: Vec<Option<Value>> = match (&fd.ty, fixed) {
                (_, Some(Val::Absent)) => vec![None],
                (FieldType::Num { .. }, Some(Val::Cell(c))) => spread(c.lo, c.hi).into_iter().map(|v| Some(Value::Int(v))).collect(),
                (FieldType::Num { lo, hi, .. }, _) => spread(*lo, *hi).into_iter().map(|v| Some(Value::Int(v))).collect(),
                (FieldType::Bool, Some(Val::Bool(b))) => vec![Some(Value::Bool(b))],
                (FieldType::Bool, _) => vec![Some(Value::Bool(false)), Some(Value::Bool(true))],
                (FieldType::Enum(e), Some(Val::Enum(v))) => vec![Some(Value::Enum(g.enums[*e].values[v].name.clone()))],
                (FieldType::Enum(e), _) => g.enums[*e].values.iter().map(|v| Some(Value::Enum(v.name.clone()))).collect(),
                (FieldType::Date { lo, hi }, _) => spread(*lo as i128, *hi as i128).into_iter().map(|d| Some(Value::Date(ritsu_ports::day_text(d as i64)))).collect(),
                _ => return false,
            };
            cands.push(list);
        }
        let total: usize = cands.iter().map(Vec::len).product();
        if total > 4096 {
            return false;
        }
        let found = product(&cands).into_iter().any(|asg| {
            outputs.iter().all(|&(at, ci)| {
                let want = value_at(at);
                let (Some(Kn::Rule { file, output, domain, inputs, enums }), How::Rule { args, .. }) = (known.computed.get(&(s.action, ci)), &a.computed[ci].how) else { return false };
                // a value of an enum input, as the rule's evaluator takes it: the rule's name of the
                // value that answers to the word (or to the alias of the gate's value of that name)
                let to_rule = |input: &str, v: Value| -> Value {
                    let Value::Enum(word) = &v else { return v };
                    let Some((_, values)) = enums.iter().find(|(n, _)| n == input) else { return v };
                    let alias = g.enums.iter().flat_map(|e| e.values.iter()).find(|x| x.name == *word).map(|x| x.alias.clone());
                    values
                        .iter()
                        .find(|(_, words)| words.iter().any(|x| x == word || alias.as_ref() == Some(x)))
                        .map(|(name, _)| Value::Enum(name.clone()))
                        .unwrap_or(v)
                };
                let mut vals: Values = Vec::new();
                for (input, src) in args {
                    let v = match src {
                        Source::Lit(Literal::Num(num)) => {
                            let u = inputs.iter().find(|(n, _)| n == input).and_then(|(_, u)| u.clone());
                            match u.and_then(|u| crate::types::count(num, &u).ok()) {
                                Some(k) => Some(Value::Int(k)),
                                None => return false,
                            }
                        }
                        Source::Lit(Literal::Bool(b)) => Some(Value::Bool(*b)),
                        Source::Lit(Literal::Word(w)) => Some(Value::Enum(w.clone())),
                        Source::Lit(Literal::Date(d)) => Some(Value::Date(ritsu_ports::day_text(*d))),
                        Source::Today => return false,
                        _ => {
                            let sl = match src {
                                Source::Attr(o, n) => g.owner_type(*o, f.principal, f.resource).attr(n).map(|(i, _)| Slot::Attr(*o, i)),
                                Source::Input(n) => a.input(n).map(|(i, _)| Slot::Input(i)),
                                Source::Lit(_) | Source::Today => None,
                            };
                            sl.and_then(|sl| raws.iter().position(|r| *r == sl)).and_then(|p| asg[p].clone())
                        }
                    };
                    match v {
                        Some(v) => vals.push((input.clone(), to_rule(input, v))),
                        None => return want == Val::Absent,
                    }
                }
                match ask.rule(file, &vals) {
                    Ok(outs) => outs.iter().find(|(n, _)| n == output).and_then(|(_, v)| domain.of_value(v)) == Some(want),
                    Err(_) => false,
                }
            })
        });
        if !found {
            return false;
        }
    }
    true
}

/// The values a number is tried at: both ends, the middle, and one in from each end.
fn spread(lo: i128, hi: i128) -> Vec<i128> {
    let mut v = vec![lo, hi, lo + (hi - lo) / 2, lo.saturating_add(1).min(hi), hi.saturating_sub(1).max(lo)];
    v.sort();
    v.dedup();
    v
}

// ---------------------------------------------------------------------------------------------
// Saying a combination

/// A combination in words, for an example in a message: the principal with its roles and
/// attributes, the resource with its attributes, the relations, the inputs and the computed
/// values the policies read.
pub fn say(g: &Gate, s: &Space, k: &Kept) -> Text {
    let f = &s.frames[k.frame];
    let env = s.env_of(k);
    let a = &g.actions[s.action];
    let pt = &g.types[f.principal];
    let yes = |b: bool| if b { tr!("はい", "yes") } else { tr!("いいえ", "no") };
    let unit_of = |sl: &Slot| -> Option<Unit> {
        let ty = match sl {
            Slot::Attr(o, i) => &g.owner_type(*o, f.principal, f.resource).attrs[*i].ty,
            Slot::Input(i) => &a.inputs[*i].ty,
            _ => return None,
        };
        match ty {
            FieldType::Num { unit, .. } => Some(unit.clone()),
            _ => None,
        }
    };
    let enum_of = |sl: &Slot, v: usize| -> String {
        let ty = match sl {
            Slot::Attr(o, i) => Some(&g.owner_type(*o, f.principal, f.resource).attrs[*i].ty),
            Slot::Input(i) => Some(&a.inputs[*i].ty),
            Slot::Computed(i) => return s.names.get(i).and_then(|vs| vs.get(v)).map(|n| n.name.clone()).unwrap_or_default(),
            _ => None,
        };
        match ty {
            Some(FieldType::Enum(e)) => g.enums[*e].values[v].name.clone(),
            _ => v.to_string(),
        }
    };
    let val = |at: usize| -> Text {
        match &env[at] {
            Val::Bool(b) => yes(*b),
            Val::Enum(v) => Text::same(enum_of(&f.slots[at], *v)),
            Val::Cell(c) => cells::show_cell(*c, &unit_of(&f.slots[at]).unwrap_or_else(Unit::number)),
            Val::Absent => tr!("無い", "absent"),
            _ => Text::default(),
        }
    };
    let attrs_of = |o: Owner| -> Vec<Text> {
        f.slots
            .iter()
            .enumerate()
            .filter_map(|(at, sl)| match sl {
                Slot::Attr(oo, i) if *oo == o && !matches!(env[at], Val::Relation(_)) && f.factors.iter().any(|x| x.places.contains(&at)) => {
                    let name = &g.owner_type(o, f.principal, f.resource).attrs[*i].named.name;
                    let v = val(at);
                    Some(tr!("{name}：{}", "{name}: {}", v.ja; v.en))
                }
                _ => None,
            })
            .collect()
    };
    let mut parts: Vec<Text> = Vec::new();
    let mut who = match &env[0] {
        Val::Roles { direct, .. } if !pt.roles.is_empty() => {
            let held: Vec<Text> = pt.roles.iter().filter(|r| direct & bit(**r) != 0).map(|r| Text::same(g.roles[*r].named.name.clone())).collect();
            if held.is_empty() {
                tr!("役割を持たない{}", "{} holding no role", pt.named.name)
            } else {
                let l = and_list(&held);
                let ja = format!("{}を持つ{}", l.ja, pt.named.name);
                Text { ja, en: format!("{} holding {}", pt.named.name, l.en) }
            }
        }
        _ => match f.place(&Slot::Workflow).map(|at| &env[at]) {
            Some(Val::Workflow(w)) => tr!("ワークフロー {}", "workflow {}", g.workflows[*w].named.name),
            _ if pt.kind == Kind::Workflow => Text::same(pt.named.name.clone()),
            _ => Text::same(pt.named.name.clone()),
        },
    };
    let pa = attrs_of(Owner::Principal);
    if !pa.is_empty() {
        let l = Text::join(&pa, "、", ", ");
        who = Text { ja: format!("{}（{}）", who.ja, l.ja), en: format!("{} ({})", who.en, l.en) };
    }
    parts.push(who);
    let rt = &g.types[f.resource];
    let mut what = Text::same(rt.named.name.clone());
    let ra = attrs_of(Owner::Resource);
    if !ra.is_empty() {
        let l = Text::join(&ra, "、", ", ");
        what = Text { ja: format!("{}（{}）", what.ja, l.ja), en: format!("{} ({})", what.en, l.en) };
    }
    parts.push(what);
    for (at, terms) in &f.terms {
        if let Val::Relation(r) = &env[*at] {
            for i in 0..terms.len() {
                for j in i + 1..terms.len() {
                    let same = matches!((r.blocks[i], r.blocks[j]), (Some(x), Some(y)) if x == y);
                    let (x, y) = (term_text(&terms[i]), term_text(&terms[j]));
                    parts.push(if same { tr!("{x}は{y}と同じ", "{x} is {y}") } else { tr!("{x}は{y}と違う", "{x} is not {y}") });
                }
            }
            for &i in f.members.get(at).map(Vec::as_slice).unwrap_or(&[]) {
                let x = term_text(&terms[i]);
                parts.push(if r.member(i) { tr!("principalは{x}のメンバー", "principal is in {x}") } else { tr!("principalは{x}のメンバーでない", "principal is not in {x}") });
            }
        }
    }
    for (at, sl) in locals(f) {
        let name = match sl {
            Slot::Input(i) => &a.inputs[*i].named.name,
            Slot::Computed(i) => &a.computed[*i].named.name,
            _ => continue,
        };
        if env[at] == Val::Absent && matches!(sl, Slot::Computed(_)) {
            continue;
        }
        if !f.factors.iter().any(|x| x.places.contains(&at)) {
            continue;
        }
        let v = val(at);
        parts.push(tr!("{name}：{}", "{name}: {}", v.ja; v.en));
    }
    Text::join(&parts, "、", ", ")
}

/// A combination as the values the JSON of a diagnostic lists: the principal and each role walked
/// (with the roles they include), the attributes, the resource, the relations, the inputs and the
/// computed values, each by what it is the value of.
pub fn values(g: &Gate, s: &Space, k: &Kept) -> Vec<(String, Text)> {
    let f = &s.frames[k.frame];
    let env = s.env_of(k);
    let a = &g.actions[s.action];
    let pt = &g.types[f.principal];
    let yes = |b: bool| if b { tr!("はい", "yes") } else { tr!("いいえ", "no") };
    let walked = |at: usize| f.factors.iter().any(|x| x.places.contains(&at));
    let mut out: Vec<(String, Text)> = Vec::new();
    let who = match f.place(&Slot::Workflow).map(|at| &env[at]) {
        Some(Val::Workflow(w)) => Text::same(format!("{} {}", pt.named.name, g.workflows[*w].named.name)),
        _ => Text::same(pt.named.name.clone()),
    };
    out.push(("principal".into(), who));
    if let Val::Roles { effective, .. } = &env[0] {
        for r in &pt.roles {
            if f.walked_roles & bit(*r) != 0 || g.closure(*r).iter().any(|x| f.walked_roles & bit(*x) != 0) {
                out.push((g.roles[*r].named.name.clone(), yes(effective & bit(*r) != 0)));
            }
        }
    }
    let shown = |at: usize| -> Text {
        match (&env[at], &f.slots[at]) {
            (Val::Bool(b), _) => yes(*b),
            (Val::Absent, _) => tr!("無い", "absent"),
            (Val::Cell(c), sl) => {
                let ty = match sl {
                    Slot::Attr(o, i) => Some(&g.owner_type(*o, f.principal, f.resource).attrs[*i].ty),
                    Slot::Input(i) => Some(&a.inputs[*i].ty),
                    _ => None,
                };
                match ty {
                    Some(FieldType::Num { unit, .. }) => cells::show_cell(*c, unit),
                    _ => cells::show_cell(*c, &Unit::number()),
                }
            }
            (Val::Enum(v), Slot::Attr(o, i)) => match &g.owner_type(*o, f.principal, f.resource).attrs[*i].ty {
                FieldType::Enum(e) => Text::same(g.enums[*e].values[*v].name.clone()),
                _ => Text::default(),
            },
            (Val::Enum(v), Slot::Input(i)) => match &a.inputs[*i].ty {
                FieldType::Enum(e) => Text::same(g.enums[*e].values[*v].name.clone()),
                _ => Text::default(),
            },
            (Val::Enum(v), Slot::Computed(i)) => Text::same(s.names.get(i).and_then(|vs| vs.get(*v)).map(|n| n.name.clone()).unwrap_or_default()),
            _ => Text::default(),
        }
    };
    for (at, sl) in f.slots.iter().enumerate() {
        if let Slot::Attr(Owner::Principal, i) = sl
            && walked(at)
        {
            out.push((format!("principal.{}", pt.attrs[*i].named.name), shown(at)));
        }
    }
    let rt = &g.types[f.resource];
    out.push(("resource".into(), Text::same(rt.named.name.clone())));
    for (at, sl) in f.slots.iter().enumerate() {
        if let Slot::Attr(Owner::Resource, i) = sl
            && walked(at)
        {
            out.push((format!("resource.{}", rt.attrs[*i].named.name), shown(at)));
        }
    }
    for (at, terms) in &f.terms {
        if let Val::Relation(r) = &env[*at] {
            for i in 0..terms.len() {
                for j in i + 1..terms.len() {
                    let same = matches!((r.blocks[i], r.blocks[j]), (Some(x), Some(y)) if x == y);
                    out.push((format!("{} is {}", term_text(&terms[i]), term_text(&terms[j])), yes(same)));
                }
            }
            for &i in f.members.get(at).map(Vec::as_slice).unwrap_or(&[]) {
                out.push((format!("principal in {}", term_text(&terms[i])), yes(r.member(i))));
            }
        }
    }
    for (at, sl) in locals(f) {
        let name = match sl {
            Slot::Input(i) => &a.inputs[*i].named.name,
            Slot::Computed(i) => &a.computed[*i].named.name,
            _ => continue,
        };
        if walked(at) {
            out.push((name.clone(), shown(at)));
        }
    }
    out
}

/// Names as a list in a sentence: `a と b` (`a、b、c` for more) and `a, b and c`.
pub fn and_list(parts: &[Text]) -> Text {
    let mut t = Text::list(parts);
    if parts.len() == 2 {
        t.ja = format!("{}と{}", parts[0].ja, parts[1].ja);
    }
    t
}

/// The places of the inputs and the computed values of a frame, the inputs first, each in the
/// order the action declares them.
fn locals(f: &Frame) -> Vec<(usize, &Slot)> {
    let mut out: Vec<(usize, &Slot)> = f.slots.iter().enumerate().filter(|(_, s)| matches!(s, Slot::Input(_) | Slot::Computed(_))).collect();
    out.sort_by_key(|(_, s)| match s {
        Slot::Input(i) => (0, *i),
        Slot::Computed(i) => (1, *i),
        _ => (2, 0),
    });
    out
}

fn term_text(t: &Term) -> String {
    match t {
        Term::Principal => "principal".into(),
        Term::Attr(o, n) => format!("{}.{n}", o.word()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terms_are_put_in_blocks_every_way() {
        assert_eq!(partitions(&[false, false]).len(), 2);
        assert_eq!(partitions(&[false, false, false, false]).len(), 15);
        // an optional term is also absent
        assert_eq!(partitions(&[false, true]).len(), 3);
    }

    #[test]
    fn sets_of_roles_fewest_first() {
        let mut seen = Vec::new();
        combinations(3, 2, &mut |p| seen.push(p.to_vec()));
        assert_eq!(seen, vec![vec![0, 1], vec![0, 2], vec![1, 2]]);
        let mut n = 0;
        combinations(3, 0, &mut |_| n += 1);
        assert_eq!(n, 1);
    }
}
