//! The walk (DESIGN 4.1): every combination of the finite values the policies and expectations of
//! an action read. An action is walked as frames, one for each principal type and resource type a
//! request can name; a frame is the product of its factors, and a factor is the list of values one
//! thing can take, or of the combinations several things that read the same data can take
//! together:
//!
//! - the roles a principal holds: every set of the roles the policies read, of the roles that
//!   include them and of the roles they include, the empty set among them;
//! - a bool or an enum: each of its values (and absent, for one that may be);
//! - a number compared with constants: the cells the constants cut its range into
//!   ([`crate::cells`]);
//! - the entities the policies relate (`resource.customer is principal`): each way the terms of a
//!   type can be the same entity or not, and for each, whether the principal is a member of each
//!   group a policy asks of;
//! - a rule's output, with the numbers, the enums and the bools it reads that a policy also reads:
//!   the cells of those numbers and each value of those enums and bools, each with the values
//!   rulec says the output can come to there (`Rules::outputs_over`, the rule's input held to the
//!   cell or the value; an enum or a bool given as a constant is held to it, and an enum of the
//!   gate's with fewer values than the rule's to those values);
//! - the dates of `today`: every combination of truths the dates can come to together, from
//!   koyomi's days over every day of `today` and every value the dates are given.
//!
//! What another language cannot say exactly is counted as every value it could be; such a value
//! is marked inexact, so that an answer that rests on one is tried again with a concrete input
//! ([`confirm`]) before it is given. The walk is never sampled: past the budget, the check stops
//! (E307).
//!
//! A condition that reads another action's answer (`allowed approve`, DESIGN 2.11) brings that
//! action's policies into the frame, as a link: the action asked with the frame's principal and
//! resource and what its inputs are given, in the frame's own terms. What they read is walked with
//! the rest (an attribute or a role is the frame's own; a value the action asked computes is a
//! place of the link), so a combination fixes everything the answer rests on, and the answer is
//! not walked but derived from the other places, as the action asked decides it.

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
    /// The combinations of values several outputs of one call of a rule can come to together, the
    /// inputs held as for [`Ask::outputs`], each with an input that reaches it
    /// (`Rules::outputs_together`).
    fn together(&self, rule: &FsPath, outputs: &[String], ranges: &[(String, Option<i128>, Option<i128>)]) -> Result<Found<Vec<(Vec<Value>, Values)>>, Vec<Said>>;
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
    /// A value the action of a link computes (DESIGN 2.11): the link, and the value's index in that
    /// action.
    Linked(usize, usize),
    /// Whether the action of a link is allowed: derived from the other places, never walked.
    Allowed(usize),
}

/// An action whose answer a condition of the frame reads (`allowed approve`, DESIGN 2.11): asked
/// with the frame's principal and resource, and what its inputs are given, in the terms of the
/// frame's own action.
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub action: usize,
    /// For each input of the action asked: what it is given (None: nothing, an input that may be
    /// absent).
    pub inputs: Vec<Option<Source>>,
    /// Whether a condition of the frame's own action reads it, not only one of an action asked.
    pub direct: bool,
}

/// What a factor is, for saying it and for trying an inexact value again.
#[derive(Clone, Debug, PartialEq)]
pub enum FactorKind {
    Roles,
    Workflow,
    /// One attribute or input.
    One,
    Relation,
    /// Rule outputs (by place: a value the action computes, or one an action asked computes), with
    /// the places of the attributes and inputs they read that a policy also reads.
    Rules { outputs: Vec<usize>, cut: Vec<usize> },
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
    /// For values of rule outputs that an input run through the rules' evaluator came to: where.
    /// An example says it beside the values ([`confirmed`]); nothing is counted by it.
    pub reached: Reached,
}

impl Point {
    /// A point no input was run for.
    fn of(vals: Vec<Val>, exact: bool) -> Point {
        Point { vals, exact, reached: Reached::default() }
    }
}

/// Where an input run through the rules' evaluator came to the values of a point of rule outputs:
/// the attributes and inputs the rules are given, each by its slot.
///
/// - One input (`region` false): every one of them, with the value that was run.
/// - A region (`region` true), where rulec says every input of it comes to these values and to
///   nothing else, and one of them that was run did: those the region holds further than the
///   point's own cells do (a part of a cell a policy cuts, a stretch of a thing no policy reads,
///   an optional thing absent), each with what it is held to. A region that holds nothing further
///   leaves this empty: every input of the combination comes to the values.
///
/// Empty where no input was run, where the point is counted in case, and in every other factor.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reached {
    pub at: Vec<(Slot, Val)>,
    pub region: bool,
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
    /// The actions whose answers the conditions read (DESIGN 2.11).
    pub links: Vec<Link>,
    /// The places derived from the others, each with what it comes to, in the order they are
    /// computed (an answer an answer reads comes first): whether a link's action is allowed.
    pub derived: Vec<(usize, CExpr)>,
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
                frame.derive(&mut env);
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
        f.derive(&mut env);
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
        self.derive(&mut env);
        env
    }

    /// The derived places of a combination, from the others: whether each link's action is
    /// allowed, an answer another reads first.
    pub fn derive(&self, env: &mut [Val]) {
        for (at, e) in &self.derived {
            let v = e.holds(env);
            env[*at] = Val::Bool(v);
        }
    }

    /// What computes the value at a place: the action (`own` for [`Slot::Computed`], the link's for
    /// [`Slot::Linked`]), the value's index there, and the link; None for a place of no computed
    /// value.
    pub fn computed_at(&self, own: usize, at: usize) -> Option<(usize, usize, Option<usize>)> {
        match self.slots.get(at)? {
            Slot::Computed(i) => Some((own, *i, None)),
            Slot::Linked(l, i) => Some((self.links[*l].action, *i, Some(*l))),
            _ => None,
        }
    }

    /// What an input or a value of the action that computes a value at a place reads, in the terms
    /// of the frame's own action: as it is for the own action's value, and through what the link
    /// gives its action's inputs for a linked one. None for an input of the link's action that is
    /// given nothing.
    pub fn mapped(&self, link: Option<usize>, g: &Gate, src: &Source) -> Option<Source> {
        let Some(l) = link else { return Some(src.clone()) };
        match src {
            Source::Input(n) => {
                let (k, _) = g.actions[self.links[l].action].input(n)?;
                self.links[l].inputs[k].clone()
            }
            other => Some(other.clone()),
        }
    }

    /// The place of the answer of a link a condition of the frame's own action reads: the action,
    /// given inputs the same as `inputs` (in the terms of `own`).
    pub fn allowed_at(&self, g: &Gate, own: usize, action: usize, inputs: &[Option<Source>]) -> Option<usize> {
        let l = self.links.iter().position(|x| x.direct && x.action == action && (0..inputs.len()).all(|k| g.same_given(own, action, k, &x.inputs[k], &inputs[k])))?;
        self.place(&Slot::Allowed(l))
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
    /// generated code, the public name). `members` is the name of each value of an enum output in
    /// the generated code (`WithinLimit`), in the order of the domain: a condition may name a value
    /// by it too (DESIGN 3.2).
    Rule { file: PathBuf, output: String, domain: Domain, inputs: Vec<(String, Option<Unit>)>, enums: Vec<(String, EnumInput)>, members: Vec<String> },
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

/// What rulec answers of an output over ranges and of several outputs together, and what koyomi
/// answers for one input.
type Outputs = Result<Found<Vec<(Value, Values)>>, Vec<Said>>;
type Together = Result<Found<Vec<(Vec<Value>, Values)>>, Vec<Said>>;
type DatesOf = Result<Vec<(String, DateValue)>, Vec<Said>>;
/// An input of a dates file, each by name.
type DateInputs = Vec<(String, i64)>;

/// Remembers what was asked of the other languages in one check, so that two frames or two
/// actions that ask the same thing ask once.
#[derive(Default)]
pub struct Memo {
    outputs: RefCell<HashMap<(PathBuf, String, Ranges), Outputs>>,
    together: RefCell<HashMap<(PathBuf, Vec<String>, Ranges), Together>>,
    dates: RefCell<HashMap<(PathBuf, DateInputs), DatesOf>>,
}

impl Memo {
    fn together(&self, ask: &dyn Ask, rule: &FsPath, outputs: &[String], ranges: Ranges) -> Together {
        let key = (rule.to_path_buf(), outputs.to_vec(), ranges);
        if let Some(r) = self.together.borrow().get(&key) {
            return r.clone();
        }
        let r = ask.together(rule, &key.1, &key.2);
        self.together.borrow_mut().insert(key, r.clone());
        r
    }

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
    space_also(g, action, known, ask, memo, budget, &AlsoByFrame::new())
}

/// What another version of a gate reads in a frame of an action, by name (ritsu's DESIGN 17.9.4):
/// `sekisho diff` walks each version with what the other reads, so that both are cut alike and one
/// combination of the one is one of the other. A role, an attribute or an input this version does
/// not have is passed over.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Also {
    /// The roles the conditions read, by name.
    pub roles: BTreeSet<String>,
    /// The attributes and inputs the conditions read, each with the constants a number is compared
    /// with (in the counts of its unit).
    pub places: Vec<(Place, Vec<(Op, i128)>)>,
    pub workflow: bool,
    /// The terms of each entity type the conditions relate (by the type's name), and those the
    /// principal is asked to be a member of.
    pub terms: BTreeMap<String, (Vec<Term>, BTreeSet<Term>)>,
}

/// An attribute or an input, by name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Place {
    Attr(Owner, String),
    Input(String),
}

/// [`Also`] by the frame: the principal type's name and the resource type's.
pub type AlsoByFrame = BTreeMap<(String, String), Also>;

/// What the conditions of an action read in each of its frames, by name: what the other version's
/// walk is given ([`space_also`]). Nothing is asked of another language.
pub fn reads(g: &Gate, action: usize, known: &Known) -> AlsoByFrame {
    let a = &g.actions[action];
    let mut out = AlsoByFrame::new();
    for &pt in &a.principals {
        for &rt in &a.resources {
            let mut c = Compiler::new(g, action, pt, rt, known);
            for p in g.policies_on(action) {
                c.policy(&g.policies[p]);
            }
            for e in g.expects_on(action) {
                c.expect(&g.expects[e]);
            }
            let mut also = Also { workflow: c.workflow_read, ..Also::default() };
            for r in 0..g.roles.len() {
                if c.roles_read & bit(r) != 0 {
                    also.roles.insert(g.roles[r].named.name.clone());
                }
            }
            for (at, sl) in c.slots.iter().enumerate() {
                let place = match sl {
                    Slot::Attr(o, i) => Place::Attr(*o, g.owner_type(*o, pt, rt).attrs[*i].named.name.clone()),
                    Slot::Input(i) => Place::Input(a.inputs[*i].named.name.clone()),
                    _ => continue,
                };
                also.places.push((place, c.cmps.get(&at).cloned().unwrap_or_default()));
            }
            for (ty, terms) in &c.terms {
                let members: BTreeSet<Term> = c.members.get(ty).map(|ms| ms.iter().map(|&i| terms[i].clone()).collect()).unwrap_or_default();
                also.terms.insert(g.types[*ty].named.name.clone(), (terms.clone(), members));
            }
            out.insert((g.types[pt].named.name.clone(), g.types[rt].named.name.clone()), also);
        }
    }
    out
}

/// [`space`], each frame also reading what `also` says the other version of the gate reads there.
pub fn space_also(g: &Gate, action: usize, known: &Known, ask: &dyn Ask, memo: &Memo, budget: u128, also: &AlsoByFrame) -> Result<Space, Stop> {
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
            if let Some(x) = also.get(&(g.types[pt].named.name.clone(), g.types[rt].named.name.clone())) {
                c.also(x);
            }
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
            frames.push(Frame { principal: pt, resource: rt, walked_roles: walked, slots: c.slots, terms, members, factors, fixed, policies: compiled, expects: selects, links: c.links, derived: c.derived });
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
                let names: Vec<Text> = x.places.iter().map(|&p| frame_slot_name(g, action, f, &f.slots[p])).collect();
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

/// How a message names a place: `roles`, `principal.suspended`, `amount`; a value an action asked
/// computes, with the action (`band (of approve)`), and whether one is allowed (`allowed approve`).
/// `links` are the actions the frame asks.
pub fn slot_name_in(g: &Gate, action: usize, principal: usize, resource: usize, links: &[Link], s: &Slot) -> Text {
    let a = &g.actions[action];
    match s {
        Slot::Roles => tr!("{} の役割", "the roles of {}", g.types[principal].named.name),
        Slot::Workflow => tr!("ワークフロー", "the workflow"),
        Slot::Attr(o, i) => Text::same(format!("{}.{}", o.word(), g.owner_type(*o, principal, resource).attrs[*i].named.name)),
        Slot::Input(i) => Text::same(a.inputs[*i].named.name.clone()),
        Slot::Computed(i) => Text::same(a.computed[*i].named.name.clone()),
        Slot::Relation(t) => tr!("{} を指すもの", "what points to {}", g.types[*t].named.name),
        Slot::Linked(l, i) => {
            let b = &g.actions[links[*l].action];
            tr!("{}（{} の計算した値）", "{} (of {})", b.computed[*i].named.name, b.named.name)
        }
        Slot::Allowed(l) => Text::same(format!("allowed {}", g.actions[links[*l].action].named.name)),
    }
}

/// [`slot_name_in`] for a place of a frame.
pub fn frame_slot_name(g: &Gate, action: usize, f: &Frame, s: &Slot) -> Text {
    slot_name_in(g, action, f.principal, f.resource, &f.links, s)
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
    /// The link whose action's policies are being compiled (None: the frame's own action's), the
    /// links, the derived places, and the actions being asked, outermost first.
    ctx: Option<usize>,
    links: Vec<Link>,
    derived: Vec<(usize, CExpr)>,
    asking: Vec<usize>,
}

impl<'g> Compiler<'g> {
    fn new(g: &'g Gate, action: usize, principal: usize, resource: usize, known: &'g Known) -> Compiler<'g> {
        Compiler {
            g,
            action,
            principal,
            resource,
            known,
            slots: vec![Slot::Roles],
            cmps: BTreeMap::new(),
            terms: BTreeMap::new(),
            members: BTreeMap::new(),
            roles_read: 0,
            workflow_read: false,
            unknown: false,
            ctx: None,
            links: Vec::new(),
            derived: Vec::new(),
            asking: Vec::new(),
        }
    }

    /// How a message names a place of the frame.
    fn name_of(&self, s: &Slot) -> Text {
        slot_name_in(self.g, self.action, self.principal, self.resource, &self.links, s)
    }

    /// The action whose conditions are being compiled: the frame's own, or the link's.
    fn here(&self) -> usize {
        match self.ctx {
            None => self.action,
            Some(l) => self.links[l].action,
        }
    }

    /// A value given in the terms of the action being compiled, in the frame's own terms: an input
    /// of the link's action is what the link gives it (None: nothing).
    fn compose(&self, s: &Source) -> Option<Source> {
        match (self.ctx, s) {
            (Some(l), Source::Input(n)) => {
                let (k, _) = self.g.actions[self.links[l].action].input(n)?;
                self.links[l].inputs[k].clone()
            }
            _ => Some(s.clone()),
        }
    }

    /// The link of the action `b` asked with `inputs` (in the frame's own terms): one already made
    /// for the same inputs, or a new one, its action's policies compiled into the frame and its
    /// answer a derived place. None for an action already being asked, a circle E109 says.
    fn link(&mut self, b: usize, inputs: Vec<Option<Source>>) -> Option<usize> {
        let g = self.g;
        if let Some(l) = self.links.iter().position(|x| x.action == b && (0..inputs.len()).all(|k| g.same_given(self.action, b, k, &x.inputs[k], &inputs[k]))) {
            return Some(l);
        }
        if self.asking.contains(&b) || self.asking.len() > g.actions.len() {
            self.unknown = true;
            return None;
        }
        self.links.push(Link { action: b, inputs, direct: false });
        let l = self.links.len() - 1;
        let saved = self.ctx;
        self.ctx = Some(l);
        self.asking.push(b);
        let mut permits = Vec::new();
        let mut forbids = Vec::new();
        for pi in g.policies_on(b) {
            let c = self.policy(&g.policies[pi]);
            if g.policies[pi].permit {
                permits.push(c.all);
            } else {
                forbids.push(c.all);
            }
        }
        self.asking.pop();
        self.ctx = saved;
        // as Cedar decides: a permit applies, and no forbid does
        let decision = CExpr::And(vec![CExpr::Or(permits), CExpr::Not(Box::new(CExpr::Or(forbids)))]).simplify();
        let at = self.place(Slot::Allowed(l));
        self.derived.push((at, decision));
        Some(l)
    }

    /// An input of the link's action that the link gives a constant, or nothing: the value, and the
    /// input's field. None for any other path.
    fn given_const(&self, p: &Path) -> Option<(Val, &'g Field)> {
        let l = self.ctx?;
        let Path::Value(name) = p else { return None };
        let b = &self.g.actions[self.links[l].action];
        let (k, f) = b.input(name)?;
        match &self.links[l].inputs[k] {
            None => Some((Val::Absent, f)),
            Some(Source::Lit(lit)) => Some((const_val(self.g, f, lit), f)),
            Some(_) => None,
        }
    }

    /// A condition of a link's action on an input the link gives a constant or nothing, decided
    /// here (DESIGN 2.11); None for another.
    fn folded(&mut self, a: &Atom) -> Option<CExpr> {
        self.ctx?;
        match a {
            Atom::True(p) => {
                let (v, _) = self.given_const(p)?;
                Some(CExpr::truth(v == Val::Bool(true)))
            }
            Atom::Is(p, lit) => {
                let (v, f) = self.given_const(p)?;
                Some(CExpr::truth(v != Val::Absent && v == const_val(self.g, f, lit)))
            }
            Atom::Cmp(p, op, lit) => {
                let (v, f) = self.given_const(p)?;
                match (v, const_val(self.g, f, lit)) {
                    (Val::Cell(c), Val::Cell(k)) => Some(CExpr::truth(cells::holds(*op, c.lo, k.lo))),
                    _ => Some(CExpr::truth(false)),
                }
            }
            Atom::Eq(p, q) => match (self.given_const(p), self.given_const(q)) {
                (None, None) => None,
                (Some((v, _)), Some((w, _))) => Some(CExpr::truth(v != Val::Absent && v == w)),
                (Some((v, _)), None) => Some(self.enum_is(q, v)),
                (None, Some((w, _))) => Some(self.enum_is(p, w)),
            },
            _ => None,
        }
    }

    /// `p` is the enum's value `v` (one of an input a link gives a constant).
    fn enum_is(&mut self, p: &Path, v: Val) -> CExpr {
        match (v, self.resolve(p)) {
            (Val::Enum(k), Some((s, Ty::Enum(_)))) => {
                let at = self.place(s);
                CExpr::Atom(CAtom::Enum(at, k))
            }
            _ => CExpr::truth(false),
        }
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
                let g = self.g;
                let act = self.here();
                let a = &g.actions[act];
                if let Some((k, f)) = a.input(name) {
                    return match self.ctx {
                        None => Some((Slot::Input(k), Ty::of(&f.ty))),
                        // an input of the link's action: what the link gives it, in the frame's terms
                        Some(l) => match self.links[l].inputs[k].clone()? {
                            Source::Input(n) => {
                                let (i, x) = self.act().input(&n)?;
                                Some((Slot::Input(i), Ty::of(&x.ty)))
                            }
                            Source::Attr(o, n) => {
                                let (i, x) = g.owner_type(o, self.principal, self.resource).attr(&n)?;
                                Some((Slot::Attr(o, i), Ty::of(&x.ty)))
                            }
                            Source::Lit(_) | Source::Today => None,
                        },
                    };
                }
                let (i, _) = a.computed_value(name)?;
                let Some(kn) = self.known.computed.get(&(act, i)) else {
                    self.unknown = true;
                    return None;
                };
                let slot = match self.ctx {
                    None => Slot::Computed(i),
                    Some(l) => Slot::Linked(l, i),
                };
                match kn {
                    Kn::Rule { domain: Domain::Enum(vs), .. } => Some((slot, Ty::Computed(Some(vs.clone())))),
                    _ => Some((slot, Ty::Computed(None))),
                }
            }
        }
    }

    /// The value of a rule's enum output a word names by the name of the generated code
    /// (`WithinLimit`), by its place in the domain.
    fn member(&self, s: &Slot, word: &str) -> Option<usize> {
        let key = match s {
            Slot::Computed(i) => (self.action, *i),
            Slot::Linked(l, i) => (self.links[*l].action, *i),
            _ => return None,
        };
        match self.known.computed.get(&key) {
            Some(Kn::Rule { members, .. }) => members.iter().position(|m| m == word),
            _ => None,
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
        // an attribute is one term however it is written (by its name in one condition, by its
        // alias in another): by the name it is declared with
        let t = &match t {
            Term::Attr(o, n) => match self.g.owner_type(*o, self.principal, self.resource).attr(n) {
                Some((_, f)) => Term::Attr(*o, f.named.name.clone()),
                None => t.clone(),
            },
            Term::Principal => Term::Principal,
        };
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
        if let Some(e) = self.folded(a) {
            return e;
        }
        CExpr::Atom(match a {
            Atom::Allowed { action: b, args } => {
                // what the action's inputs are given, in the frame's own terms
                let local = self.g.asked(self.here(), *b, args);
                let inputs: Vec<Option<Source>> = local.iter().map(|x| x.as_ref().and_then(|s| self.compose(s))).collect();
                match self.link(*b, inputs) {
                    Some(l) => {
                        if self.ctx.is_none() {
                            self.links[l].direct = true;
                        }
                        CAtom::Bool(self.place(Slot::Allowed(l)), true)
                    }
                    None => CAtom::Const(false),
                }
            }
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
                (Some((s, Ty::Computed(Some(vs)))), Literal::Word(w)) => match vs.iter().position(|v| v.is(w)).or_else(|| self.member(&s, w)) {
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
                    // two names of one enum of an `enums` file are one type, with its values in one order
                    (Some((s, Ty::Enum(e))), Some((t, Ty::Enum(f)))) if self.g.enums[e].one == self.g.enums[f].one => {
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

    /// Read what the other version of the gate reads too ([`Also`]): its roles, its attributes and
    /// inputs with their constants, the workflow, the terms of its relations.
    fn also(&mut self, x: &Also) {
        let g = self.g;
        for name in &x.roles {
            if let Some(r) = g.roles.iter().position(|r| r.named.name == *name) {
                self.roles_read |= bit(r);
            }
        }
        for (p, cmps) in &x.places {
            let (slot, f) = match p {
                Place::Attr(o, n) => match g.owner_type(*o, self.principal, self.resource).attrs.iter().position(|f| f.named.name == *n) {
                    Some(i) => (Slot::Attr(*o, i), &g.owner_type(*o, self.principal, self.resource).attrs[i]),
                    None => continue,
                },
                Place::Input(n) => match self.act().inputs.iter().position(|f| f.named.name == *n) {
                    Some(i) => (Slot::Input(i), &self.act().inputs[i]),
                    None => continue,
                },
            };
            let at = self.place(slot);
            if let FieldType::Num { .. } = f.ty {
                self.cmps.entry(at).or_default().extend(cmps.iter().copied());
            }
        }
        if x.workflow && g.types[self.principal].kind == Kind::Workflow {
            self.workflow_read = true;
            self.place(Slot::Workflow);
        }
        for (name, (terms, members)) in &x.terms {
            let Some(ty) = g.types.iter().position(|t| t.named.name == *name) else { continue };
            for t in terms {
                if self.term_type(t) == Some(ty) {
                    self.term(ty, t);
                }
            }
            for t in members {
                if self.term_type(t) == Some(ty) {
                    let (_, i) = self.term(ty, t);
                    if ty == self.principal {
                        self.term(ty, &Term::Principal);
                    }
                    self.members.entry(ty).or_default().insert(i);
                }
            }
        }
    }

    fn terms_by_place(&self) -> BTreeMap<usize, Vec<Term>> {
        self.terms.iter().filter_map(|(ty, ts)| self.slots.iter().position(|s| *s == Slot::Relation(*ty)).map(|at| (at, ts.clone()))).collect()
    }

    fn members_by_place(&self) -> BTreeMap<usize, Vec<usize>> {
        self.members.iter().filter_map(|(ty, ts)| self.slots.iter().position(|s| *s == Slot::Relation(*ty)).map(|at| (at, ts.iter().copied().collect()))).collect()
    }

    /// The roles walked: of the principal type's roles, those that are read or include one that
    /// is, and those one that is read includes (`in clerk` matches a manager too, so a principal
    /// holding only the included role answers `in` differently — a combination of its own).
    fn walked_roles(&self) -> Vec<usize> {
        let t = &self.g.types[self.principal];
        let mut inc = 0u128;
        for r in 0..self.g.roles.len() {
            if self.roles_read & bit(r) != 0 {
                for &x in &self.g.closure(r) {
                    inc |= bit(x);
                }
            }
        }
        t.roles
            .iter()
            .copied()
            .filter(|&r| self.g.closure(r).iter().any(|x| self.roles_read & bit(*x) != 0) || inc & bit(r) != 0)
            .collect()
    }

    /// The sizes of what needs nothing from another language, to stop early when they alone are
    /// over the budget.
    fn estimate(&self) -> Vec<(Text, u128)> {
        let n = self.walked_roles().len();
        let mut out = vec![(self.name_of(&Slot::Roles), if n >= 127 { u128::MAX } else { 1u128 << n })];
        for (at, s) in self.slots.iter().enumerate() {
            if let Slot::Attr(..) | Slot::Input(_) = s
                && let Some(d) = self.domain(at)
            {
                out.push((self.name_of(s), d.len() as u128));
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
            Slot::Linked(l, i) => 6_000 + l * 100 + i,
            Slot::Allowed(l) => 100_000 + l,
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

    /// The attributes and inputs a computed value is given, in this frame (through the link, for a
    /// value a link's action computes); None when one of them is an attribute the frame's type does
    /// not have, or an input of the link's action given nothing (the value is not computed:
    /// absent).
    fn sources_of(&self, link: Option<usize>, c: &Computed) -> Option<Vec<Slot>> {
        let srcs: Vec<Source> = match &c.how {
            How::Rule { args, .. } | How::Date { of: DateOf::Call { args, .. }, .. } => args.iter().map(|(_, s)| s.clone()).collect(),
            How::Date { of: DateOf::Attr(o, n), .. } => vec![Source::Attr(*o, n.clone())],
            How::Open { .. } => vec![],
        };
        let mut out = Vec::new();
        for s in &srcs {
            let s = self.mapped(link, s)?;
            if let Some(slot) = self.source_slot(&s).ok()?
                && !out.contains(&slot)
            {
                out.push(slot);
            }
        }
        Some(out)
    }

    /// A value given in the terms of the action that computes a value: as it is for the frame's own
    /// action, through what the link gives its action's inputs for a link's.
    fn mapped(&self, link: Option<usize>, s: &Source) -> Option<Source> {
        match (link, s) {
            (Some(l), Source::Input(n)) => {
                let (k, _) = self.g.actions[self.links[l].action].input(n)?;
                self.links[l].inputs[k].clone()
            }
            _ => Some(s.clone()),
        }
    }

    /// What a rule output at a place computes, as a key two places that compute the same share:
    /// the rule's file and output, and what each input is given in this frame (an input or an
    /// attribute by its place, a constant, today, nothing).
    fn rule_key(&self, at: usize) -> Option<String> {
        let (act, ci, _) = self.computed_at(at)?;
        let Some(Kn::Rule { output, .. }) = self.known.computed.get(&(act, ci)) else { return None };
        Some(format!("{output}\u{1}{}", self.call_key(at)?))
    }

    /// Whether the rule output at a place is given today.
    fn given_today(&self, at: usize) -> bool {
        let Some((act, ci, link)) = self.computed_at(at) else { return false };
        let How::Rule { args, .. } = &self.g.actions[act].computed[ci].how else { return false };
        args.iter().any(|(_, src)| self.mapped(link, src) == Some(Source::Today))
    }

    /// The call a rule output at a place is an output of, as a key two outputs of one call share:
    /// the rule's file, and what each input is given in this frame (an input or an attribute by its
    /// place, a constant, today, nothing). One call comes to all of its outputs at once, so two of
    /// them are counted together.
    fn call_key(&self, at: usize) -> Option<String> {
        let (act, ci, link) = self.computed_at(at)?;
        let How::Rule { args, .. } = &self.g.actions[act].computed[ci].how else { return None };
        let Some(Kn::Rule { file, .. }) = self.known.computed.get(&(act, ci)) else { return None };
        // in the order of the inputs' names: two calls that write the same inputs in another order
        // are one call
        let mut given: Vec<String> = args
            .iter()
            .map(|(input, src)| {
                let given = match self.mapped(link, src) {
                    None => "absent".to_string(),
                    Some(Source::Lit(l)) => format!("{l:?}"),
                    Some(Source::Today) => "today".to_string(),
                    Some(other) => match self.source_slot(&other) {
                        Ok(Some(slot)) => format!("{slot:?}"),
                        _ => format!("{other:?}"),
                    },
                };
                format!("\u{1}{input}={given}")
            })
            .collect();
        given.sort();
        Some(format!("{}{}", file.display(), given.concat()))
    }

    /// What computes the value at a place: the action, the value's index there, and the link.
    fn computed_at(&self, at: usize) -> Option<(usize, usize, Option<usize>)> {
        match self.slots.get(at)? {
            Slot::Computed(i) => Some((self.action, *i, None)),
            Slot::Linked(l, i) => Some((self.links[*l].action, *i, Some(*l))),
            _ => None,
        }
    }

    /// The factors of the frame, in order: the roles, the workflow, then each thing read (or
    /// group of things that read the same data) in the order of [`Compiler::order`].
    fn factors(&mut self, ask: &dyn Ask, memo: &Memo, budget: u128, day_walk: &mut u128) -> Result<Made, Stop> {
        let g = self.g;
        let mut fixed: Vec<(usize, Val)> = Vec::new();
        // the computed values read (the frame's own action's, and those the links' actions
        // compute), each by its place, and what each reads
        let mut rule_values: Vec<usize> = Vec::new();
        let mut date_values: Vec<usize> = Vec::new();
        let mut reads: BTreeMap<usize, Vec<Slot>> = BTreeMap::new();
        for at in 0..self.slots.len() {
            let Some((act, i, link)) = self.computed_at(at) else { continue };
            let c = &g.actions[act].computed[i];
            if !self.known.computed.contains_key(&(act, i)) {
                return Err(Stop::Unknown);
            }
            match self.sources_of(link, c) {
                None => fixed.push((at, Val::Absent)),
                Some(srcs) => {
                    reads.insert(at, srcs);
                    match &c.how {
                        How::Rule { .. } => rule_values.push(at),
                        How::Date { .. } | How::Open { .. } => date_values.push(at),
                    }
                }
            }
        }
        let date_sources: BTreeSet<Slot> = date_values.iter().flat_map(|at| reads[at].iter().cloned()).collect();
        // the rule outputs joined by what they read: an attribute or an input two of them are
        // given, today given to both, or the same call of the same rule (two of its outputs)
        let calls: BTreeMap<usize, Option<String>> = rule_values.iter().map(|&at| (at, self.call_key(at))).collect();
        let given_today: BTreeSet<usize> = rule_values.iter().copied().filter(|&at| self.given_today(at)).collect();
        let mut groups: Vec<Vec<usize>> = Vec::new();
        for &at in &rule_values {
            let mine: BTreeSet<&Slot> = reads[&at].iter().collect();
            let together = |o: &usize| reads[o].iter().any(|s| mine.contains(s)) || (given_today.contains(o) && given_today.contains(&at)) || (calls[o].is_some() && calls[o] == calls[&at]);
            let joined: Vec<usize> = (0..groups.len()).filter(|&gi| groups[gi].iter().any(together)).collect();
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
            // what the dates read too is not joined with them exactly (two outputs that read the
            // same thing are counted together, [`Compiler::joint`])
            let loose = read.iter().any(|s| date_sources.contains(s));
            let f = self.rule_factor(grp, &cut, loose, ask, memo)?;
            taken.extend(f.places.iter().copied());
            let key = f.places.iter().map(|&p| self.order(p)).min().unwrap_or(usize::MAX);
            units.push((key, f));
        }
        for &at in &direct {
            if taken.contains(&at) {
                continue;
            }
            if let Some(vals) = self.domain(at) {
                units.push((self.order(at), Factor { kind: FactorKind::One, places: vec![at], points: vals.into_iter().map(|v| Point::of(vec![v], true)).collect() }));
            }
        }
        let rels: Vec<(usize, usize)> = self.slots.iter().enumerate().filter_map(|(at, s)| if let Slot::Relation(t) = s { Some((at, *t)) } else { None }).collect();
        for (at, ty) in rels {
            units.push((self.order(at), self.relation_factor(at, ty)));
        }
        units.sort_by_key(|(k, _)| *k);
        let walked = self.walked_roles();
        let mut factors = vec![Factor { kind: FactorKind::Roles, places: vec![0], points: role_sets(g, &walked).into_iter().map(|v| Point::of(vec![v], true)).collect() }];
        if self.workflow_read && let Some(at) = self.slots.iter().position(|s| *s == Slot::Workflow) {
            factors.push(Factor { kind: FactorKind::Workflow, places: vec![at], points: (0..g.workflows.len()).map(|w| Point::of(vec![Val::Workflow(w)], true)).collect() });
        }
        factors.extend(units.into_iter().map(|(_, f)| f));
        Ok((factors, fixed, walked.iter().fold(0u128, |m, r| m | bit(*r))))
    }

    /// The factor of a group of rule outputs: for every value of the attributes and inputs they
    /// read that a policy also reads, the values rulec says the outputs can come to there. `loose`
    /// says the outputs read something the dates read too, which this does not join.
    ///
    /// One output (in however many places) is counted as rulec answers of it. Two or more, which
    /// read the same data, are counted together ([`Compiler::joint`]): only the combinations of
    /// their values that can happen, and each marked exact only where that is known. Where they
    /// cannot be counted together (more combinations of the cut values than rulec is asked over one
    /// by one), every pair of what each comes to is counted in case, marked inexact.
    fn rule_factor(&self, outputs: &[usize], cut: &[usize], loose: bool, ask: &dyn Ask, memo: &Memo) -> Result<Factor, Stop> {
        let g = self.g;
        let cut_domains: Vec<Vec<Val>> = cut.iter().map(|&at| self.domain(at).unwrap_or_else(|| vec![Val::Absent])).collect();
        let mut places = cut.to_vec();
        places.extend(outputs.iter().copied());
        // past this many combinations of the cut values, rulec is asked once over the whole of each
        // input, and what it says is counted in case for each of them (marked inexact)
        let assignments: usize = cut_domains.iter().map(Vec::len).product();
        let many = assignments > ASKS;
        // an output that computes what an earlier one does is that one's value (by the earlier one's
        // place among the outputs)
        let keys: Vec<Option<String>> = outputs.iter().map(|&at| self.rule_key(at)).collect();
        let same_as: Vec<Option<usize>> = (0..outputs.len()).map(|j| (0..j).find(|&i| keys[i].is_some() && keys[i] == keys[j])).collect();
        let firsts: Vec<usize> = (0..outputs.len()).filter(|&j| same_as[j].is_none()).collect();
        // two outputs, or one whose call is given one thing as two of its inputs (which rulec, asked
        // of each input's range, cannot be told are one value): what happens together is counted
        let together = firsts.len() > 1 || outputs.iter().any(|&at| self.given_twice(at));
        if together
            && !many
            && let Some(points) = self.joint(outputs, cut, &cut_domains, &same_as, &firsts, loose, ask, memo)?
        {
            return Ok(Factor { kind: FactorKind::Rules { outputs: outputs.to_vec(), cut: cut.to_vec() }, places, points });
        }
        // not counted together: nothing says which pairs happen
        let loose = loose || together;
        // the call, to run an input for each value it comes to: an example is said with it
        let mut run = if loose || many { None } else { self.group(outputs, cut, &firsts, ask, memo).ok().filter(|(_, reads)| reads.optional.len() <= MAX_OPTIONAL) };
        let mut points = Vec::new();
        for assignment in product(&cut_domains) {
            let mut per: Vec<(Vec<Val>, bool)> = Vec::new();
            for &out_at in firsts.iter().map(|&j| &outputs[j]) {
                let Some((act, ci, link)) = self.computed_at(out_at) else { return Err(Stop::Unknown) };
                let c = &g.actions[act].computed[ci];
                let (Some(Kn::Rule { file, output, domain, inputs, enums, .. }), How::Rule { args, .. }) = (self.known.computed.get(&(act, ci)), &c.how) else { return Err(Stop::Unknown) };
                let word_place = |input: &str, w: &str| enum_word_place(g, enums, input, w);
                let places_of = |input: &str, e: usize, only: Option<usize>| enum_places_of(g, enums, input, e, only);
                let mut ranges = Vec::new();
                let mut absent = false;
                let mut exact = !loose;
                let mut maybe_absent = false;
                for (input, src) in args {
                    let unit = inputs.iter().find(|(n, _)| n == input).and_then(|(_, u)| u.clone());
                    // an input of a link's action given nothing: the value is absent
                    let Some(src) = self.mapped(link, src) else {
                        absent = true;
                        continue;
                    };
                    let src = &src;
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
                        // a bool or an enum given as a constant: rulec holds the input to it (a bool
                        // as 0 or 1, an enum by the place of its value)
                        (Source::Lit(Literal::Bool(b)), _) => ranges.push((input.clone(), Some(i128::from(*b)), Some(i128::from(*b)))),
                        (Source::Lit(Literal::Word(w)), _) => match word_place(input, w) {
                            Some(k) => ranges.push((input.clone(), Some(k), Some(k))),
                            None => exact = false,
                        },
                        (_, Some(slot)) => {
                            let f = self.field(&slot).ok_or(Stop::Unknown)?;
                            match cut.iter().position(|at| self.slots[*at] == slot) {
                                Some(k) => match (&assignment[k], &f.ty) {
                                    (Val::Cell(cell), _) if !many => ranges.push((input.clone(), Some(cell.lo), Some(cell.hi))),
                                    (Val::Cell(_), _) => {
                                        if let FieldType::Num { lo, hi, .. } = &f.ty {
                                            ranges.push((input.clone(), Some(*lo), Some(*hi)));
                                        }
                                        exact = false;
                                    }
                                    (Val::Absent, _) => absent = true,
                                    // the value of the combination, held as a constant is
                                    (Val::Bool(b), _) if !many => ranges.push((input.clone(), Some(i128::from(*b)), Some(i128::from(*b)))),
                                    (Val::Enum(v), FieldType::Enum(e)) if !many => match places_of(input, *e, Some(*v)) {
                                        Some(ks) => ranges.extend(ks.into_iter().map(|k| (input.clone(), Some(k), Some(k)))),
                                        None => exact = false,
                                    },
                                    _ => exact = false,
                                },
                                None => {
                                    match &f.ty {
                                        FieldType::Num { lo, hi, .. } => ranges.push((input.clone(), Some(*lo), Some(*hi))),
                                        FieldType::Date { lo, hi } => ranges.push((input.clone(), Some(*lo as i128), Some(*hi as i128))),
                                        // an enum of the gate's with fewer values than the rule's:
                                        // rulec holds the input to the values the gate's takes
                                        FieldType::Enum(e) => match places_of(input, *e, None) {
                                            Some(ks) if (ks.len()) < enums.iter().find(|(n, _)| n == input).map_or(0, |(_, vs)| vs.len()) => {
                                                ranges.extend(ks.into_iter().map(|k| (input.clone(), Some(k), Some(k))));
                                            }
                                            Some(_) => {}
                                            None => exact = false,
                                        },
                                        FieldType::Bool => {}
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
            let from = points.len();
            for outs in product(&lists) {
                let mut vals = assignment.clone();
                // each output in its order: its own value, or the value of the one it computes as
                for (j, same) in same_as.iter().enumerate().take(outputs.len()) {
                    let first = same.unwrap_or(j);
                    let k = firsts.iter().position(|&x| x == first).unwrap_or(0);
                    vals.push(outs[k].clone());
                }
                points.push(Point::of(vals, exact));
            }
            if exact && let Some((group, reads)) = &mut run {
                let base = reads.region(&assignment, 0);
                let mut seen: Seen = BTreeMap::new();
                let mut regions: BTreeMap<Vec<Val>, Vec<Hold>> = BTreeMap::new();
                for absent in 0..(1usize << reads.optional.len()) {
                    let region = reads.region(&assignment, absent);
                    let answers: Vec<Answer> = (0..group.calls.len()).map(|i| group.answer(i, &region)).collect();
                    if answers.iter().any(|a| a.tuples.is_empty()) {
                        continue;
                    }
                    let tried = group.observe(&region, &answers, &mut seen);
                    group.keep(&region, &answers, &tried, &base, &mut regions);
                }
                for (point, outs) in points[from..].iter_mut().zip(product(&lists)) {
                    point.reached = reads.reached(&base, regions.get(&outs), seen.get(&outs));
                }
            }
        }
        Ok(Factor { kind: FactorKind::Rules { outputs: outputs.to_vec(), cut: cut.to_vec() }, places, points })
    }

    /// Whether the call of the rule output at a place is given one attribute or input (or today)
    /// as two of its inputs.
    fn given_twice(&self, at: usize) -> bool {
        let Some((act, ci, link)) = self.computed_at(at) else { return false };
        let How::Rule { args, .. } = &self.g.actions[act].computed[ci].how else { return false };
        let mut seen: Vec<Raw> = Vec::new();
        for (_, src) in args {
            let raw = match self.mapped(link, src) {
                Some(Source::Today) => Raw::Today,
                Some(Source::Lit(_)) | None => continue,
                Some(other) => match self.source_slot(&other) {
                    Ok(Some(slot)) => Raw::Slot(slot),
                    _ => continue,
                },
            };
            if seen.contains(&raw) {
                return true;
            }
            seen.push(raw);
        }
        false
    }

    /// The calls of a group of rule outputs (one rule given the same values is one call, which
    /// comes to all of its outputs at once), and the things they are given: what each is, what it
    /// takes with nothing held, which cut value holds it, and those that may be absent and are not
    /// cut.
    fn group<'a>(&self, outputs: &[usize], cut: &[usize], firsts: &[usize], ask: &'a dyn Ask, memo: &'a Memo) -> Result<(Group<'a, 'g>, Reads), Stop> {
        let g = self.g;
        // the calls, and the things they are given
        let mut raws: Vec<Raw> = Vec::new();
        let mut calls: Vec<Call> = Vec::new();
        for (k, &j) in firsts.iter().enumerate() {
            let at = outputs[j];
            let Some((act, ci, link)) = self.computed_at(at) else { return Err(Stop::Unknown) };
            let (Some(Kn::Rule { file, output, domain, inputs, enums, .. }), How::Rule { args, .. }) = (self.known.computed.get(&(act, ci)), &g.actions[act].computed[ci].how) else { return Err(Stop::Unknown) };
            let key = self.call_key(at);
            if let Some(call) = calls.iter_mut().find(|x| key.is_some() && x.key == key) {
                call.outs.push((k, output.as_str(), domain));
                continue;
            }
            let mut given = Vec::new();
            let mut reads = Vec::new();
            let mut today = false;
            for (input, src) in args {
                // an input of a link's action given nothing: the value is absent
                let Some(src) = self.mapped(link, src) else {
                    given.push((input.clone(), Given::Nothing));
                    continue;
                };
                let raw = match &src {
                    Source::Lit(lit) => {
                        let constant = match lit {
                            Literal::Num(num) => {
                                let unit = inputs.iter().find(|(n, _)| n == input).and_then(|(_, u)| u.clone());
                                let k = unit.as_ref().and_then(|unit| crate::types::count(num, unit).ok());
                                Given::Const { held: k, value: k.map(Value::Int) }
                            }
                            // a day: rulec takes the range of a date as day numbers
                            Literal::Date(d) => Given::Const { held: Some(*d as i128), value: Some(Value::Date(ritsu_ports::day_text(*d))) },
                            // rulec holds a bool as 0 or 1, an enum by the place of its value
                            Literal::Bool(b) => Given::Const { held: Some(i128::from(*b)), value: Some(Value::Bool(*b)) },
                            Literal::Word(w) => Given::Const { held: enum_word_place(g, enums, input, w), value: Some(enum_to_rule(g, enums, input, Value::Enum(w.clone()))) },
                        };
                        given.push((input.clone(), constant));
                        continue;
                    }
                    Source::Today => {
                        today = true;
                        Raw::Today
                    }
                    other => match self.source_slot(other) {
                        Ok(Some(slot)) => Raw::Slot(slot),
                        _ => return Err(Stop::Unknown),
                    },
                };
                let r = match raws.iter().position(|x| *x == raw) {
                    Some(r) => r,
                    None => {
                        raws.push(raw);
                        raws.len() - 1
                    }
                };
                if !reads.contains(&r) {
                    reads.push(r);
                }
                given.push((input.clone(), Given::Raw(r)));
            }
            calls.push(Call { key, file, given, outs: vec![(k, output.as_str(), domain)], enums, reads, today });
        }
        // what each thing given is, what it takes with nothing held, which cut value holds it, and
        // those that may be absent and are not cut
        let mut kinds: Vec<RawKind> = Vec::new();
        let mut whole: Vec<Hold> = Vec::new();
        let mut cut_of: Vec<Option<usize>> = Vec::new();
        let mut optional: Vec<usize> = Vec::new();
        for (r, raw) in raws.iter().enumerate() {
            let (kind, hold, k) = match raw {
                Raw::Today => match &g.today {
                    Some(t) => (RawKind::Date, Hold::Span(t.lo as i128, t.hi as i128), None),
                    None => (RawKind::Other, Hold::Other, None),
                },
                Raw::Slot(s) => {
                    let f = self.field(s).ok_or(Stop::Unknown)?;
                    let (kind, hold) = match &f.ty {
                        FieldType::Num { lo, hi, .. } => (RawKind::Num, Hold::Span(*lo, *hi)),
                        FieldType::Date { lo, hi } => (RawKind::Date, Hold::Span(*lo as i128, *hi as i128)),
                        FieldType::Bool => (RawKind::Bool, Hold::Bool(None)),
                        FieldType::Enum(e) => (RawKind::Enum(*e), Hold::Enum(*e, None)),
                        FieldType::Entity(_) => (RawKind::Other, Hold::Other),
                    };
                    let k = cut.iter().position(|at| self.slots[*at] == *s);
                    if k.is_none() && f.optional {
                        optional.push(r);
                    }
                    (kind, hold, k)
                }
            };
            kinds.push(kind);
            whole.push(hold);
            cut_of.push(k);
        }
        Ok((Group { g, calls, kinds, n: firsts.len(), ask, memo, evals: HashMap::new() }, Reads { raws, whole, cut_of, optional }))
    }

    /// The points of a group of rule outputs counted together (DESIGN 4.1): for each combination of
    /// the cut values, the combinations of the outputs' values that can happen there, each with the
    /// cut values in front, in the order of every pair of what each output comes to. None when the
    /// group is past what is counted this way (more attributes that may be absent than
    /// [`MAX_OPTIONAL`]).
    ///
    /// The outputs are put by the call that comes to them (one rule given the same values: one call
    /// comes to all of its outputs at once). The values the calls are given are held to a region:
    /// first the cut values, every other attribute and input whole (one that may be absent, there
    /// and not there in turn; with it absent, every output of a call that is given it is absent).
    /// Over a region, rulec says what each call comes to: the combinations of its outputs together
    /// (`Rules::outputs_together`) where it can, else the values of each output
    /// (`Rules::outputs_over`). Then:
    ///
    /// - where no call is left with every pair of two outputs' values, none that comes to more than
    ///   one combination is given one thing as two of its inputs over more than one value of it, and
    ///   no two calls that come to more than one combination are given the same thing over more than
    ///   one value of it, every pair of the calls' combinations is counted: the others are the same
    ///   wherever one varies;
    /// - elsewhere the region is split, at a value where a concrete input shows the outputs change
    ///   ([`Group::split`]), and each part is asked again. Where no two inputs tried differ, it is
    ///   split by the inputs rulec found instead ([`Group::apart`]): between the values one of them
    ///   gives a thing the call is given twice, which no request does. The combinations of the parts
    ///   are those of the region, without the pairs no part has.
    ///
    /// A region that cannot be split any further (a rule whose answer does not change along one
    /// input alone, or past [`REGIONS`] parts) is counted as every pair in case.
    ///
    /// A combination is marked exact where rulec's answers alone say it happens (the calls given
    /// nothing in common that the region leaves wide, so that an input one takes is one the other
    /// takes), or where a concrete input run through the rules' evaluator comes to it. The rest are
    /// counted in case, and an answer that rests on one is tried again ([`confirm`]).
    #[allow(clippy::too_many_arguments)]
    fn joint(&self, outputs: &[usize], cut: &[usize], cut_domains: &[Vec<Val>], same_as: &[Option<usize>], firsts: &[usize], loose: bool, ask: &dyn Ask, memo: &Memo) -> Result<Option<Vec<Point>>, Stop> {
        let g = self.g;
        let (mut group, reads) = self.group(outputs, cut, firsts, ask, memo)?;
        if reads.optional.len() > MAX_OPTIONAL {
            return Ok(None);
        }
        let n = firsts.len();
        // today is read by the dates of the action too, which this does not join
        let loose = loose || group.calls.iter().any(|c| c.today);
        let mut asked = 0usize;
        let mut points = Vec::new();
        for assignment in product(cut_domains) {
            // each combination counted, with whether rulec's answers alone say it happens; the
            // combinations a concrete input came to, each with the input; and the regions that
            // come to one combination, by it
            let mut counted: BTreeMap<Vec<Val>, bool> = BTreeMap::new();
            let mut seen: Seen = BTreeMap::new();
            let mut regions: BTreeMap<Vec<Val>, Vec<Hold>> = BTreeMap::new();
            let base = reads.region(&assignment, 0);
            for absent in 0..(1usize << reads.optional.len()) {
                let mut parts = vec![reads.region(&assignment, absent)];
                let mut here = 0usize;
                while let Some(region) = parts.pop() {
                    here += 1;
                    asked += 1;
                    let answers: Vec<Answer> = (0..group.calls.len()).map(|i| group.answer(i, &region)).collect();
                    // a call no input reaches over the region: no combination there
                    if answers.iter().any(|a| a.tuples.is_empty()) {
                        continue;
                    }
                    let shares = |i: usize, j: usize| group.calls[i].reads.iter().any(|r| group.calls[j].reads.contains(r) && region[*r].wide(g));
                    let varying: Vec<usize> = (0..answers.len()).filter(|&i| answers[i].tuples.len() > 1).collect();
                    let unsplit = answers.iter().any(|a| a.pairs || (a.twice && a.tuples.len() > 1)) || varying.iter().any(|&i| varying.iter().any(|&j| i < j && shares(i, j)));
                    // rulec's answers alone say every pair happens: each is exactly what its call
                    // comes to, and no two calls are given the same thing over more than one value
                    let vouched = !unsplit && answers.iter().all(|a| a.tight) && (0..answers.len()).all(|i| (0..i).all(|j| !shares(i, j)));
                    // concrete inputs of the region, run through the evaluator: where the region
                    // is split, and an input to say a combination with
                    let tried = group.observe(&region, &answers, &mut seen);
                    if !vouched
                        && unsplit
                        && here < REGIONS
                        && asked < ASKS
                        && let Some(split) = group.split(&region, &tried, &mut seen).or_else(|| group.apart(&region, &answers, REGIONS - here))
                    {
                        parts.extend(split);
                        continue;
                    }
                    group.keep(&region, &answers, &tried, &base, &mut regions);
                    let mut pairs: Vec<Vec<Val>> = vec![vec![Val::Absent; n]];
                    for (call, a) in group.calls.iter().zip(&answers) {
                        let mut next = Vec::with_capacity(pairs.len() * a.tuples.len());
                        for t in &pairs {
                            for u in &a.tuples {
                                let mut x = t.clone();
                                for ((k, _, _), v) in call.outs.iter().zip(u) {
                                    x[*k] = v.clone();
                                }
                                next.push(x);
                            }
                        }
                        pairs = next;
                    }
                    for t in pairs {
                        *counted.entry(t).or_insert(false) |= vouched;
                    }
                }
            }
            for (t, vouched) in counted {
                let exact = !loose && (vouched || seen.contains_key(&t));
                let reached = if exact { reads.reached(&base, regions.get(&t), seen.get(&t)) } else { Reached::default() };
                let mut vals = assignment.clone();
                // each output in its order: its own value, or the value of the one it computes as
                for (j, same) in same_as.iter().enumerate().take(outputs.len()) {
                    let first = same.unwrap_or(j);
                    let k = firsts.iter().position(|&x| x == first).unwrap_or(0);
                    vals.push(t[k].clone());
                }
                points.push(Point { vals, exact, reached });
            }
        }
        Ok(Some(points))
    }

    /// The factor of the dates of `today`: every combination of truths the dates can come to
    /// together, with the cells of the numbers they read that a policy compares.
    fn date_factor(&self, date_values: &[usize], cut: &[usize], ask: &dyn Ask, memo: &Memo, budget: u128, day_walk: &mut u128) -> Result<Factor, Stop> {
        let g = self.g;
        let today = self.g.today.as_ref().ok_or(Stop::Unknown)?;
        // each date's action, its index there, and its link
        let mut refs: Vec<(usize, usize, Option<usize>)> = Vec::new();
        for &at in date_values {
            refs.push(self.computed_at(at).ok_or(Stop::Unknown)?);
        }
        // the attributes and inputs the dates are given, each once
        let mut raws: Vec<Slot> = Vec::new();
        for &(act, i, link) in &refs {
            for s in self.sources_of(link, &g.actions[act].computed[i]).unwrap_or_default() {
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
                    return Err(Stop::Budget { count: (*hi - *lo + 1) as u128, days: true, why: vec![(self.name_of(s), (*hi - *lo + 1) as u128)], roles: false });
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
                why.push((self.name_of(s), d.len() as u128));
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
            for &(act, i, link) in &refs {
                let c = &g.actions[act].computed[i];
                let value_of = |src: &Source| -> Option<i64> {
                    let src = &self.mapped(link, src)?;
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
                        let Some(Kn::Date { file, function, .. }) = self.known.computed.get(&(act, i)) else { return Err(Stop::Unknown) };
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
                let (act, i, _) = refs[k - ncut];
                tuple.push(match (&g.actions[act].computed[i].how, x) {
                    (How::Open { .. }, _) => match self.known.computed.get(&(act, i)) {
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
        let given_today = refs.iter().any(|&(act, i, link)| match &g.actions[act].computed[i].how {
            How::Date { of: DateOf::Call { args, .. }, .. } => args.iter().any(|(_, s)| self.mapped(link, s) == Some(Source::Today)),
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
        places.extend(date_values.iter().copied());
        Ok(Factor { kind: FactorKind::Dates, places, points: tuples.into_iter().map(|vals| Point::of(vals, true)).collect() })
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
                points.push(Point::of(vec![Val::Relation(Rel { blocks: blocks.clone(), members, principal })], true));
            }
        }
        Factor { kind: FactorKind::Relation, places: vec![at], points }
    }
}

/// The most combinations of the cut values of a group of rule outputs that rulec is asked over one
/// by one; and the most regions a group counted together is asked over in all, before a region
/// is not split any further.
const ASKS: usize = 4096;

/// The most parts the region of one combination of the cut values is split into, before a part
/// is counted as every pair in case.
const REGIONS: usize = 64;

/// The most concrete inputs tried over one region.
const PROBES: usize = 64;

/// The most attributes that may be absent, and that no policy reads itself, a group of rule
/// outputs counted together is given: each is there and not there in turn.
const MAX_OPTIONAL: usize = 6;

/// A thing a group of rule outputs is given: an attribute or an input, by its slot, or today.
#[derive(Clone, Debug, PartialEq)]
enum Raw {
    Slot(Slot),
    Today,
}

/// What a thing given is: how a rule's input is held to it, and how the rule is given one value
/// of it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum RawKind {
    Num,
    Date,
    Bool,
    Enum(usize),
    /// Something a rule's input cannot be held to (an entity).
    Other,
}

/// What a region holds one thing given to.
#[derive(Clone, Debug, PartialEq)]
enum Hold {
    /// A number (in the counts of its unit) or a day (its number): both ends in.
    Span(i128, i128),
    /// A bool: one value, or either.
    Bool(Option<bool>),
    /// An enum of the gate's: one of its values, or any.
    Enum(usize, Option<usize>),
    /// An attribute that may be absent, and is.
    Absent,
    Other,
}

impl Hold {
    /// Whether it is more than one value.
    fn wide(&self, g: &Gate) -> bool {
        match self {
            Hold::Span(lo, hi) => lo < hi,
            Hold::Bool(b) => b.is_none(),
            Hold::Enum(e, v) => v.is_none() && g.enums[*e].values.len() > 1,
            Hold::Absent => false,
            Hold::Other => true,
        }
    }

    /// Its first value and its last.
    fn ends(&self, g: &Gate) -> (Pv, Pv) {
        match self {
            Hold::Span(lo, hi) => (Pv::Int(*lo), Pv::Int(*hi)),
            Hold::Bool(Some(b)) => (Pv::Bool(*b), Pv::Bool(*b)),
            Hold::Bool(None) => (Pv::Bool(false), Pv::Bool(true)),
            Hold::Enum(_, Some(v)) => (Pv::Enum(*v), Pv::Enum(*v)),
            Hold::Enum(e, None) => (Pv::Enum(0), Pv::Enum(g.enums[*e].values.len().saturating_sub(1))),
            Hold::Absent => (Pv::Absent, Pv::Absent),
            Hold::Other => (Pv::Other, Pv::Other),
        }
    }

    fn takes(&self, v: &Pv) -> bool {
        match (self, v) {
            (Hold::Span(lo, hi), Pv::Int(k)) => lo <= k && k <= hi,
            (Hold::Bool(b), Pv::Bool(x)) => b.is_none_or(|b| b == *x),
            (Hold::Enum(_, v), Pv::Enum(x)) => v.is_none_or(|v| v == *x),
            (Hold::Absent, Pv::Absent) => true,
            _ => false,
        }
    }
}

/// One value of a thing given: a number or a day, a bool, an enum's value by its place in the
/// gate's enum.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Pv {
    Int(i128),
    Bool(bool),
    Enum(usize),
    Absent,
    Other,
}

/// The combinations concrete inputs came to, each with the lowest input that did.
type Seen = BTreeMap<Vec<Val>, Vec<Pv>>;

/// What an input of a call is given.
#[derive(Clone, Debug, PartialEq)]
enum Given {
    /// A thing given, by its place among the group's.
    Raw(usize),
    /// A constant: how rulec holds the input to it (None: it cannot), and the value its evaluator
    /// takes (None: there is none).
    Const { held: Option<i128>, value: Option<Value> },
    /// Nothing: an input of a link's action the link gives nothing.
    Nothing,
}

/// One call of a rule: the rule given the same values comes to all of its outputs at once.
struct Call<'g> {
    key: Option<String>,
    file: &'g PathBuf,
    /// Each input, by the rule's name of it, and what it is given.
    given: Vec<(String, Given)>,
    /// The outputs read: each by its place among the group's, its name and its values.
    outs: Vec<(usize, &'g str, &'g Domain)>,
    enums: &'g [(String, EnumInput)],
    /// The things given, each once.
    reads: Vec<usize>,
    today: bool,
}

/// The things the calls of a group of rule outputs are given ([`Compiler::group`]).
struct Reads {
    raws: Vec<Raw>,
    /// What each takes with nothing held.
    whole: Vec<Hold>,
    /// The cut value that holds each, by its place among the cut values.
    cut_of: Vec<Option<usize>>,
    /// Those that may be absent and are not cut.
    optional: Vec<usize>,
}

impl Reads {
    /// The region of one combination of the cut values: each thing a cut value holds held to it,
    /// every other whole, and those of `optional` whose bit of `absent` is set not there.
    fn region(&self, assignment: &[Val], absent: usize) -> Vec<Hold> {
        let mut region = self.whole.clone();
        for (r, k) in self.cut_of.iter().enumerate() {
            let Some(k) = k else { continue };
            region[r] = match (&assignment[*k], &self.whole[r]) {
                (Val::Cell(c), _) => Hold::Span(c.lo, c.hi),
                (Val::Bool(b), _) => Hold::Bool(Some(*b)),
                (Val::Enum(v), Hold::Enum(e, _)) => Hold::Enum(*e, Some(*v)),
                (Val::Absent, _) => Hold::Absent,
                _ => Hold::Other,
            };
        }
        for (b, &r) in self.optional.iter().enumerate() {
            if absent & (1 << b) != 0 {
                region[r] = Hold::Absent;
            }
        }
        region
    }

    /// Where an input came to a combination, as a point keeps it ([`Reached`]): the region kept
    /// for it ([`Group::keep`]) where there is one, by what it holds further than the combination
    /// of the cut values does (`base`: its region, nothing absent); else the input, by every
    /// thing it gives a value.
    fn reached(&self, base: &[Hold], region: Option<&Vec<Hold>>, input: Option<&Vec<Pv>>) -> Reached {
        let held = |h: &Hold| match h {
            Hold::Span(lo, hi) => Some(Val::Cell(Cell { lo: *lo, hi: *hi })),
            Hold::Bool(Some(b)) => Some(Val::Bool(*b)),
            Hold::Enum(_, Some(v)) => Some(Val::Enum(*v)),
            Hold::Absent => Some(Val::Absent),
            _ => None,
        };
        let mut at = Vec::new();
        for (r, raw) in self.raws.iter().enumerate() {
            let Raw::Slot(slot) = raw else { continue };
            let v = match (region, input) {
                (Some(region), _) => held(&region[r]).filter(|v| held(&base[r]).as_ref() != Some(v)),
                (None, Some(input)) => match &input[r] {
                    Pv::Int(k) => Some(Val::Cell(Cell { lo: *k, hi: *k })),
                    Pv::Bool(b) => Some(Val::Bool(*b)),
                    Pv::Enum(v) => Some(Val::Enum(*v)),
                    Pv::Absent => Some(Val::Absent),
                    Pv::Other => None,
                },
                (None, None) => None,
            };
            if let Some(v) = v {
                at.push((slot.clone(), v));
            }
        }
        Reached { region: region.is_some() && !at.is_empty(), at }
    }
}

/// What rulec says a call comes to over a region.
struct Answer {
    /// The combinations of its outputs' values, each in the order of the call's outputs.
    tuples: Vec<Vec<Val>>,
    /// Whether they are exactly what the call comes to: each happens, and nothing else does.
    tight: bool,
    /// Whether they are every pair of what each output comes to, two or more outputs with more
    /// than one value: which pairs happen is not known, and a part of the region may tell.
    pairs: bool,
    /// Whether the call is given one thing, over more than one value of it, as two of its inputs:
    /// rulec was asked as if the two were given apart, so some of what it says may not happen, and
    /// a part of the region may tell.
    twice: bool,
    /// The inputs rulec found, each as the rule's evaluator takes it.
    witnesses: Vec<Values>,
}

impl Answer {
    fn absent(n: usize) -> Answer {
        Answer { tuples: vec![vec![Val::Absent; n]], tight: true, pairs: false, twice: false, witnesses: Vec::new() }
    }
}

/// A group of rule outputs counted together ([`Compiler::joint`]): its calls, what the things
/// they are given are, and what the rules' evaluator gave for each concrete input tried.
struct Group<'a, 'g> {
    g: &'g Gate,
    calls: Vec<Call<'g>>,
    kinds: Vec<RawKind>,
    /// How many outputs the group has.
    n: usize,
    ask: &'a dyn Ask,
    memo: &'a Memo,
    /// For each input tried, what each output comes to (None: the rule gives none there).
    evals: HashMap<Vec<Pv>, Vec<Option<Val>>>,
}

impl Group<'_, '_> {
    /// What rulec says the call `i` comes to with the things it is given held to the region.
    fn answer(&self, i: usize, region: &[Hold]) -> Answer {
        let g = self.g;
        let call = &self.calls[i];
        let n = call.outs.len();
        let mut ranges: Ranges = Vec::new();
        // today is read by the dates of the action too, which this does not join
        let mut exact = !call.today;
        let mut twice = false;
        let mut seen: Vec<usize> = Vec::new();
        for (input, given) in &call.given {
            match given {
                Given::Nothing => return Answer::absent(n),
                Given::Const { held: Some(k), .. } => ranges.push((input.clone(), Some(*k), Some(*k))),
                Given::Const { held: None, .. } => exact = false,
                Given::Raw(r) => {
                    // one thing given as two inputs: rulec, asked of each input's range, is not told
                    // they are one value
                    if seen.contains(r) && region[*r].wide(g) {
                        exact = false;
                        twice = true;
                    }
                    seen.push(*r);
                    match &region[*r] {
                        Hold::Absent => return Answer::absent(n),
                        Hold::Span(lo, hi) => ranges.push((input.clone(), Some(*lo), Some(*hi))),
                        Hold::Bool(Some(b)) => ranges.push((input.clone(), Some(i128::from(*b)), Some(i128::from(*b)))),
                        Hold::Bool(None) => {}
                        Hold::Enum(e, Some(v)) => match enum_places_of(g, call.enums, input, *e, Some(*v)) {
                            Some(ks) => ranges.extend(ks.into_iter().map(|k| (input.clone(), Some(k), Some(k)))),
                            None => exact = false,
                        },
                        // an enum of the gate's with fewer values than the rule's: rulec holds the
                        // input to the values the gate's takes
                        Hold::Enum(e, None) => match enum_places_of(g, call.enums, input, *e, None) {
                            Some(ks) if ks.len() < call.enums.iter().find(|(n, _)| n == input).map_or(0, |(_, vs)| vs.len()) => {
                                ranges.extend(ks.into_iter().map(|k| (input.clone(), Some(k), Some(k))));
                            }
                            Some(_) => {}
                            None => exact = false,
                        },
                        Hold::Other => exact = false,
                    }
                }
            }
        }
        // the outputs together, where rulec says them so
        if n > 1 {
            let names: Vec<String> = call.outs.iter().map(|(_, o, _)| o.to_string()).collect();
            if let Ok(Found::Value(found)) = self.memo.together(self.ask, call.file, &names, ranges.clone()) {
                let tuples: Option<Vec<Vec<Val>>> = found.iter().map(|(t, _)| if t.len() == n { t.iter().zip(&call.outs).map(|(v, (_, _, domain))| domain.of_value(v)).collect() } else { None }).collect();
                if let Some(mut tuples) = tuples {
                    tuples.sort();
                    tuples.dedup();
                    return Answer { tuples, tight: exact, pairs: false, twice, witnesses: found.into_iter().map(|(_, w)| w).collect() };
                }
            }
        }
        // each output's values, and every pair of them
        let mut lists: Vec<Vec<Val>> = Vec::new();
        let mut witnesses: Vec<Values> = Vec::new();
        for (_, output, domain) in &call.outs {
            match self.memo.outputs(self.ask, call.file, output, ranges.clone()) {
                Ok(Found::Value(vs)) if vs.iter().all(|(v, _)| domain.of_value(v).is_some()) => {
                    lists.push(domain.vals().into_iter().filter(|d| vs.iter().any(|(v, _)| domain.of_value(v).as_ref() == Some(d))).collect());
                    witnesses.extend(vs.into_iter().map(|(_, w)| w));
                }
                _ => {
                    lists.push(domain.vals());
                    exact = false;
                }
            }
        }
        let varying = lists.iter().filter(|l| l.len() > 1).count();
        Answer { tuples: product(&lists), tight: exact && varying <= 1, pairs: varying > 1, twice, witnesses }
    }

    /// What each output comes to for one concrete input, by the rules' evaluator: its value, absent
    /// for a call given an absent value, None where the rule gives no answer (an input it refuses,
    /// a value it cannot be given). A combination every output has a value in happens: it is put
    /// in `seen`, with the input (the lowest, of those that come to it).
    fn tuple_at(&mut self, point: &[Pv], seen: &mut Seen) -> Vec<Option<Val>> {
        let out = match self.evals.get(point) {
            Some(t) => t.clone(),
            None => {
                let mut out: Vec<Option<Val>> = vec![None; self.n];
                for call in &self.calls {
                    let mut vals: Values = Vec::new();
                    // Some(true): the call is given an absent value; Some(false): a value the rule
                    // cannot be given
                    let mut stopped: Option<bool> = None;
                    for (input, given) in &call.given {
                        let v = match given {
                            Given::Nothing => Err(true),
                            Given::Const { value: Some(v), .. } => Ok(v.clone()),
                            Given::Const { value: None, .. } => Err(false),
                            Given::Raw(r) => match (&point[*r], self.kinds[*r]) {
                                (Pv::Absent, _) => Err(true),
                                (Pv::Int(k), RawKind::Date) => Ok(Value::Date(ritsu_ports::day_text(*k as i64))),
                                (Pv::Int(k), RawKind::Num) => Ok(Value::Int(*k)),
                                (Pv::Bool(b), RawKind::Bool) => Ok(Value::Bool(*b)),
                                (Pv::Enum(v), RawKind::Enum(e)) => Ok(enum_to_rule(self.g, call.enums, input, Value::Enum(self.g.enums[e].values[*v].name.clone()))),
                                _ => Err(false),
                            },
                        };
                        match v {
                            Ok(v) => vals.push((input.clone(), v)),
                            Err(absent) => {
                                stopped = Some(absent);
                                break;
                            }
                        }
                    }
                    match stopped {
                        Some(true) => call.outs.iter().for_each(|(k, _, _)| out[*k] = Some(Val::Absent)),
                        Some(false) => {}
                        None => {
                            if let Ok(outs) = self.ask.rule(call.file, &vals) {
                                for (k, output, domain) in &call.outs {
                                    out[*k] = outs.iter().find(|(n, _)| n == output).and_then(|(_, v)| domain.of_value(v));
                                }
                            }
                        }
                    }
                }
                self.evals.insert(point.to_vec(), out.clone());
                out
            }
        };
        if let Some(t) = out.iter().cloned().collect::<Option<Vec<Val>>>() {
            match seen.get_mut(&t) {
                Some(lowest) => {
                    if point < lowest.as_slice() {
                        *lowest = point.to_vec();
                    }
                }
                None => {
                    seen.insert(t, point.to_vec());
                }
            }
        }
        out
    }

    /// The one combination the calls' answers say a region comes to, when each says one.
    fn sole(&self, answers: &[Answer]) -> Option<Vec<Val>> {
        let mut t = vec![Val::Absent; self.n];
        for (call, a) in self.calls.iter().zip(answers) {
            let [u] = a.tuples.as_slice() else { return None };
            for ((k, _, _), v) in call.outs.iter().zip(u) {
                t[*k] = v.clone();
            }
        }
        Some(t)
    }

    /// Keeps a region for the combination it comes to, where the calls' answers say it comes to
    /// one and an input of it that was tried came to that one: every input of the region does. Of
    /// two regions of one combination, the one that holds fewer things further than `base` (the
    /// region of the cut values, nothing absent) is kept, then the lower one.
    fn keep(&self, region: &[Hold], answers: &[Answer], tried: &[(Vec<Pv>, Vec<Option<Val>>)], base: &[Hold], regions: &mut BTreeMap<Vec<Val>, Vec<Hold>>) {
        let Some(t) = self.sole(answers) else { return };
        if !tried.iter().any(|(_, x)| x.len() == t.len() && x.iter().zip(&t).all(|(x, v)| x.as_ref() == Some(v))) {
            return;
        }
        let weight = |holds: &[Hold]| -> (usize, Vec<i128>) {
            let further = holds.iter().zip(base).filter(|(h, b)| h != b).count();
            let low = holds
                .iter()
                .map(|h| match h {
                    Hold::Span(lo, _) => *lo,
                    Hold::Bool(Some(b)) => i128::from(*b),
                    Hold::Enum(_, Some(v)) => *v as i128,
                    Hold::Absent => i128::MAX,
                    _ => -1,
                })
                .collect();
            (further, low)
        };
        if regions.get(&t).is_none_or(|kept| weight(region) < weight(kept)) {
            regions.insert(t, region.to_vec());
        }
    }

    /// Concrete inputs of a region, each run through the rules' evaluator: both ends of the region,
    /// and the inputs rulec found for the values of each call, each laid over the first end and over
    /// each input found for another call (the things two calls are both given, from the one laid
    /// last). Each with what the outputs come to there.
    fn observe(&mut self, region: &[Hold], answers: &[Answer], seen: &mut Seen) -> Vec<(Vec<Pv>, Vec<Option<Val>>)> {
        let g = self.g;
        let (lo, hi): (Vec<Pv>, Vec<Pv>) = region.iter().map(|h| h.ends(g)).unzip();
        let mut points: Vec<Vec<Pv>> = vec![lo.clone()];
        if hi != lo {
            points.push(hi);
        }
        {
            // an input rulec found for the call `i`, laid over a point: the things the call is given
            // take the input's values (one given twice, the first's); None for a value outside the
            // region, or one the gate has no value for
            let over = |base: &[Pv], i: usize, w: &Values| -> Option<Vec<Pv>> {
                let call = &self.calls[i];
                let mut p = base.to_vec();
                let mut set: Vec<usize> = Vec::new();
                for (input, given) in &call.given {
                    let Given::Raw(r) = given else { continue };
                    if set.contains(r) {
                        continue;
                    }
                    let v = w.iter().find(|(n, _)| n == input).map(|(_, v)| v)?;
                    let pv = pv_of(g, call.enums, input, self.kinds[*r], v)?;
                    if !region[*r].takes(&pv) {
                        return None;
                    }
                    p[*r] = pv;
                    set.push(*r);
                }
                Some(p)
            };
            let mut add = |p: Option<Vec<Pv>>| {
                if let Some(p) = p
                    && points.len() < PROBES
                    && !points.contains(&p)
                {
                    points.push(p);
                }
            };
            for (i, a) in answers.iter().enumerate() {
                for w in &a.witnesses {
                    add(over(&lo, i, w));
                    for (j, b) in answers.iter().enumerate() {
                        if i == j {
                            continue;
                        }
                        for x in &b.witnesses {
                            add(over(&lo, j, x).and_then(|base| over(&base, i, w)));
                        }
                    }
                }
            }
        }
        points
            .into_iter()
            .map(|p| {
                let t = self.tuple_at(&p, seen);
                (p, t)
            })
            .collect()
    }

    /// The parts a region is split into, where the concrete inputs tried show the outputs change:
    /// two of the inputs that come to different combinations (two every output has a value for,
    /// when there are such) are joined by a path that changes one thing given at a time; the first
    /// step that changes what the outputs come to names the thing. A number or a day is split
    /// between the two neighbouring values where the outputs change (found by halving); a bool or
    /// an enum into each of its values. None when no two inputs tried differ.
    fn split(&mut self, region: &[Hold], tried: &[(Vec<Pv>, Vec<Option<Val>>)], seen: &mut Seen) -> Option<Vec<Vec<Hold>>> {
        let whole = |t: &[Option<Val>]| t.iter().all(Option::is_some);
        let pair = |only_whole: bool| -> Option<(usize, usize)> {
            for i in 0..tried.len() {
                for j in i + 1..tried.len() {
                    if tried[i].1 != tried[j].1 && (!only_whole || (whole(&tried[i].1) && whole(&tried[j].1))) {
                        return Some((i, j));
                    }
                }
            }
            None
        };
        let (a, b) = pair(true).or_else(|| pair(false))?;
        let mut cur = tried[a].0.clone();
        let cur_t = tried[a].1.clone();
        for r in 0..region.len() {
            if cur[r] == tried[b].0[r] {
                continue;
            }
            let mut next = cur.clone();
            next[r] = tried[b].0[r].clone();
            let next_t = self.tuple_at(&next, seen);
            if next_t == cur_t {
                cur = next;
                continue;
            }
            let with = |hold: Hold| -> Vec<Hold> {
                let mut x = region.to_vec();
                x[r] = hold;
                x
            };
            return match (&region[r], &cur[r], &next[r]) {
                (Hold::Span(lo, hi), Pv::Int(u), Pv::Int(v)) => {
                    let (mut l, mut h) = (*u.min(v), *u.max(v));
                    // what the outputs come to at the lower of the two
                    let at_l = if l == *u { cur_t } else { next_t };
                    while h - l > 1 {
                        let m = l + (h - l) / 2;
                        let mut p = cur.clone();
                        p[r] = Pv::Int(m);
                        if self.tuple_at(&p, seen) != at_l {
                            h = m;
                        } else {
                            l = m;
                        }
                    }
                    Some(vec![with(Hold::Span(*lo, l)), with(Hold::Span(l + 1, *hi))])
                }
                (Hold::Bool(None), ..) => Some(vec![with(Hold::Bool(Some(false))), with(Hold::Bool(Some(true)))]),
                (Hold::Enum(e, None), ..) => Some((0..self.g.enums[*e].values.len()).map(|v| with(Hold::Enum(*e, Some(v)))).collect()),
                _ => None,
            };
        }
        None
    }

    /// The parts a region is split into where no two inputs tried differ, by the inputs rulec
    /// found. Rulec is asked of a call given one thing as two of its inputs as if the two were
    /// given apart, so an input it finds may give the thing two values, which no request does:
    /// with the thing split between the two, no part takes that input, and rulec is asked again
    /// without it. A thing two calls that come to more than one combination are both given is
    /// split the same way, between the values the inputs found for them give it. A number or a
    /// day: each value found but the lowest starts a part (`room` parts at most); a bool or an
    /// enum: each of its values. None when the inputs found give no such thing two values.
    fn apart(&self, region: &[Hold], answers: &[Answer], room: usize) -> Option<Vec<Vec<Hold>>> {
        let g = self.g;
        for r in 0..region.len() {
            if !region[r].wide(g) {
                continue;
            }
            let varies = |i: usize| answers[i].tuples.len() > 1;
            let twice = |i: usize| answers[i].twice && varies(i) && self.calls[i].given.iter().filter(|(_, x)| *x == Given::Raw(r)).count() > 1;
            let shared = (0..self.calls.len()).filter(|&i| varies(i) && self.calls[i].reads.contains(&r)).count() > 1;
            let mut found: Vec<Pv> = Vec::new();
            for (i, call) in self.calls.iter().enumerate() {
                if !(twice(i) || (shared && varies(i))) {
                    continue;
                }
                for w in &answers[i].witnesses {
                    for (input, given) in &call.given {
                        if *given != Given::Raw(r) {
                            continue;
                        }
                        let Some(v) = w.iter().find(|(n, _)| n == input).map(|(_, v)| v) else { continue };
                        if let Some(pv) = pv_of(g, call.enums, input, self.kinds[r], v)
                            && region[r].takes(&pv)
                            && !found.contains(&pv)
                        {
                            found.push(pv);
                        }
                    }
                }
            }
            if found.len() < 2 {
                continue;
            }
            let with = |hold: Hold| -> Vec<Hold> {
                let mut x = region.to_vec();
                x[r] = hold;
                x
            };
            match &region[r] {
                Hold::Span(lo, hi) => {
                    let mut at: Vec<i128> = found.iter().filter_map(|p| if let Pv::Int(k) = p { Some(*k) } else { None }).collect();
                    at.sort();
                    if at.len() < 2 {
                        continue;
                    }
                    let mut parts = Vec::new();
                    let mut from = *lo;
                    for &start in at[1..].iter().take(room.saturating_sub(1).max(1)) {
                        parts.push(with(Hold::Span(from, start - 1)));
                        from = start;
                    }
                    parts.push(with(Hold::Span(from, *hi)));
                    return Some(parts);
                }
                Hold::Bool(None) => return Some(vec![with(Hold::Bool(Some(false))), with(Hold::Bool(Some(true)))]),
                Hold::Enum(e, None) => return Some((0..g.enums[*e].values.len()).map(|v| with(Hold::Enum(*e, Some(v)))).collect()),
                _ => continue,
            }
        }
        None
    }
}

/// The place of a value in the order the rule declares the values of the enum its input `input`
/// takes, by a word that names it: how rulec is told to hold an enum input to that value.
fn enum_place(enums: &[(String, EnumInput)], input: &str, words: &[&str]) -> Option<i128> {
    let (_, values) = enums.iter().find(|(n, _)| n == input)?;
    values.iter().position(|(_, ws)| ws.iter().any(|w| words.contains(&w.as_str()))).map(|k| k as i128)
}

/// The place of a word given to an enum input: the rule's value, or the gate's value of that name
/// (one a link gives an input of its action).
fn enum_word_place(g: &Gate, enums: &[(String, EnumInput)], input: &str, w: &str) -> Option<i128> {
    enum_place(enums, input, &[w]).or_else(|| {
        let v = g.enums.iter().flat_map(|e| e.values.iter()).find(|v| v.is(w))?;
        enum_place(enums, input, &[v.name.as_str(), v.alias.as_str()])
    })
}

/// The places of the values of the gate's enum `e` (of one of them, with `only`) among the values
/// of the enum the rule's input `input` takes; None when one of them is no value of the rule's.
fn enum_places_of(g: &Gate, enums: &[(String, EnumInput)], input: &str, e: usize, only: Option<usize>) -> Option<Vec<i128>> {
    let values = &g.enums[e].values;
    let picked: Vec<&Named> = match only {
        Some(v) => vec![&values[v]],
        None => values.iter().collect(),
    };
    picked.iter().map(|v| enum_place(enums, input, &[v.name.as_str(), v.alias.as_str()])).collect()
}

/// A value of an enum input, as the rule's evaluator takes it: the rule's name of the value that
/// answers to the word (or to the alias of the gate's value of that name). Any other value as it
/// is.
fn enum_to_rule(g: &Gate, enums: &[(String, EnumInput)], input: &str, v: Value) -> Value {
    let Value::Enum(word) = &v else { return v };
    let Some((_, values)) = enums.iter().find(|(n, _)| n == input) else { return v };
    let alias = g.enums.iter().flat_map(|e| e.values.iter()).find(|x| x.name == *word || x.alias == *word).map(|x| (x.name.clone(), x.alias.clone()));
    values
        .iter()
        .find(|(_, words)| words.iter().any(|x| x == word || alias.as_ref().is_some_and(|(n, al)| x == n || x == al)))
        .map(|(name, _)| Value::Enum(name.clone()))
        .unwrap_or(v)
}

/// A value of a rule's input, as the rule's evaluator takes it, as one value of the thing the
/// gate gives the input: a number as it is, a day by its number, an enum's value by its place in
/// the gate's enum. None for a value the gate has none for.
fn pv_of(g: &Gate, enums: &[(String, EnumInput)], input: &str, kind: RawKind, v: &Value) -> Option<Pv> {
    match (kind, v) {
        (RawKind::Num, Value::Int(k)) => Some(Pv::Int(*k)),
        (RawKind::Date, Value::Date(s)) => ritsu_ports::day_of(s).map(|d| Pv::Int(d as i128)),
        (RawKind::Bool, Value::Bool(b)) => Some(Pv::Bool(*b)),
        (RawKind::Enum(e), Value::Enum(name) | Value::Str(name)) => {
            let words: Vec<&str> = match enums.iter().find(|(n, _)| n == input).and_then(|(_, vs)| vs.iter().find(|(rule_name, _)| rule_name == name)) {
                Some((_, ws)) => ws.iter().map(String::as_str).collect(),
                None => vec![name.as_str()],
            };
            g.enums[e].values.iter().position(|x| words.contains(&x.name.as_str()) || words.contains(&x.alias.as_str())).map(Pv::Enum)
        }
        _ => None,
    }
}

/// What a date comes to for one combination of the values it is given.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DateVal {
    Day(i64),
    /// `today is open in …`: it reads today only.
    Open,
    Cell(usize),
    Absent,
}

/// A constant given to an input, as a combination holds the input's value: true or false, an enum's
/// value by its place, a number as the cell of itself alone. A date is never read by a condition.
pub fn const_val(g: &Gate, f: &Field, lit: &Literal) -> Val {
    match (lit, &f.ty) {
        (Literal::Bool(b), _) => Val::Bool(*b),
        (Literal::Word(w), FieldType::Enum(e)) => g.enums[*e].values.iter().position(|v| v.is(w)).map(Val::Enum).unwrap_or(Val::Absent),
        (Literal::Num(n), FieldType::Num { unit, .. }) => crate::types::count(n, unit).map(|k| Val::Cell(Cell { lo: k, hi: k })).unwrap_or(Val::Absent),
        _ => Val::Absent,
    }
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
    // the slot a value given (in the frame's own terms) reads, in this frame
    let slot_of = |src: &Source| -> Option<Slot> {
        match src {
            Source::Attr(o, n) => g.owner_type(*o, f.principal, f.resource).attr(n).map(|(i, _)| Slot::Attr(*o, i)),
            Source::Input(n) => a.input(n).map(|(i, _)| Slot::Input(i)),
            Source::Lit(_) | Source::Today => None,
        }
    };
    for (x, &i) in f.factors.iter().zip(&k.idx) {
        if x.points[i].exact {
            continue;
        }
        let FactorKind::Rules { outputs, cut } = &x.kind else { return false };
        let point = &x.points[i];
        let value_at = |at: usize| point.vals[x.places.iter().position(|p| *p == at).unwrap_or(0)].clone();
        // the candidates of each attribute and input the outputs read
        let mut raws: Vec<Slot> = Vec::new();
        for &at in outputs {
            let Some((act, ci, link)) = f.computed_at(s.action, at) else { return false };
            if let How::Rule { args, .. } = &g.actions[act].computed[ci].how {
                for (_, src) in args {
                    if let Some(sl) = f.mapped(link, g, src).as_ref().and_then(slot_of)
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
            outputs.iter().all(|&at| {
                let want = value_at(at);
                let Some((act, ci, link)) = f.computed_at(s.action, at) else { return false };
                let cv = &g.actions[act].computed[ci];
                let (Some(Kn::Rule { file, output, domain, inputs, enums, .. }), How::Rule { args, .. }) = (known.computed.get(&(act, ci)), &cv.how) else { return false };
                let mut vals: Values = Vec::new();
                for (input, src) in args {
                    let Some(src) = f.mapped(link, g, src) else { return want == Val::Absent };
                    let v = match &src {
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
                        other => slot_of(other).and_then(|sl| raws.iter().position(|r| *r == sl)).and_then(|p| asg[p].clone()),
                    };
                    match v {
                        Some(v) => vals.push((input.clone(), enum_to_rule(g, enums, input, v))),
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

/// A cell of days as a message shows it: one day, or both ends.
fn show_days(c: Cell) -> Text {
    let (a, b) = (crate::types::day_text(c.lo as i64), crate::types::day_text(c.hi as i64));
    if c.lo == c.hi { Text::same(a) } else { Text { ja: format!("{a}〜{b}"), en: format!("{a} to {b}") } }
}

/// What an example says beside the computed values of one group of rule outputs: the input that
/// was run through the rules and came to them ([`Reached`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Confirmed {
    /// The place of the last of the group's values the example says: the words go after it.
    pub after: usize,
    /// The computed values of the group the example says, by name, in the order said.
    pub computed: Vec<String>,
    /// Whether it is a region, every input of which comes to the values; else it is one input.
    pub region: bool,
    /// The attributes and inputs the rules are given: the principal's, the resource's, then the
    /// action's inputs, each in the order declared.
    pub given: Vec<GivenValue>,
}

/// One attribute or input of a [`Confirmed`], with its value there.
#[derive(Clone, Debug, PartialEq)]
pub struct GivenValue {
    /// As a gate writes it: `principal.refund_limit`, `resource.amount`, an input by its name.
    pub name: String,
    /// The type whose attribute it is; None for an input.
    pub ty: Option<String>,
    /// As the words name it: an attribute by its own name, unless something else the action
    /// reads has the same name.
    pub short: String,
    pub val: Val,
    pub value: Text,
}

/// Two pieces of Japanese joined as the suite writes it: a space where a letter or a digit meets
/// Japanese.
fn glue(a: &str, b: &str) -> String {
    let (p, c) = (a.chars().last(), b.chars().next());
    let plain = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric());
    let wide = |c: Option<char>| c.is_some_and(|c| !c.is_ascii() && !matches!(c, '、' | '。' | '（' | '）' | '「' | '」' | '〜' | '：'));
    if (plain(p) && wide(c)) || (wide(p) && plain(c)) { format!("{a} {b}") } else { format!("{a}{b}") }
}

impl Confirmed {
    /// In words, as an example says it in brackets after the values: `as when refund_limit is
    /// 1GBP and amount is 1GBP` for one input, `whenever amount is 101JPY to 300JPY` for a region.
    pub fn said(&self) -> Text {
        let items: Vec<Text> = self
            .given
            .iter()
            .map(|x| {
                let ja = match x.val {
                    Val::Bool(_) => format!("{}「{}」", glue(&x.short, "が"), x.value.ja),
                    Val::Absent => glue(&x.short, "が無い"),
                    _ => glue(&glue(&x.short, "が"), &x.value.ja),
                };
                Text { ja, en: format!("{} is {}", x.short, x.value.en) }
            })
            .collect();
        let l = Text::list(&items);
        // 無いとき, 1GBP のとき
        let when = if matches!(self.given.last().map(|x| &x.val), Some(Val::Absent)) { "とき" } else { "のとき" };
        if self.region { Text { ja: glue(&l.ja, &format!("{when}はいつも")), en: format!("whenever {}", l.en) } } else { Text { ja: glue(&glue("たとえば", &l.ja), when), en: format!("as when {}", l.en) } }
    }
}

/// For each group of rule outputs of a kept combination that an input was run for, what an
/// example says beside its values. A group none of whose values the example says is left out.
pub fn confirmed(g: &Gate, s: &Space, k: &Kept) -> Vec<Confirmed> {
    let f = &s.frames[k.frame];
    let env = s.env_of(k);
    let a = &g.actions[s.action];
    let (pt, rt) = (&g.types[f.principal], &g.types[f.resource]);
    // a name two things of the action have is said with whose it is
    let shared = |n: &str| pt.attrs.iter().map(|x| &x.named.name).chain(rt.attrs.iter().map(|x| &x.named.name)).chain(a.inputs.iter().map(|x| &x.named.name)).chain(a.computed.iter().map(|x| &x.named.name)).filter(|x| x.as_str() == n).count() > 1;
    let mut out = Vec::new();
    for (x, &i) in f.factors.iter().zip(&k.idx) {
        let FactorKind::Rules { outputs, .. } = &x.kind else { continue };
        let reached = &x.points[i].reached;
        if reached.at.is_empty() {
            continue;
        }
        let said: Vec<(usize, &String)> = locals(f)
            .into_iter()
            .filter_map(|(at, sl)| match sl {
                Slot::Computed(c) if outputs.contains(&at) && env[at] != Val::Absent && !a.computed[*c].of_flow() => Some((at, &a.computed[*c].named.name)),
                _ => None,
            })
            .collect();
        let Some(&(after, _)) = said.last() else { continue };
        let mut at: Vec<&(Slot, Val)> = reached.at.iter().collect();
        at.sort_by_key(|(sl, _)| match sl {
            Slot::Attr(Owner::Principal, i) => (0, *i),
            Slot::Attr(_, i) => (1, *i),
            Slot::Input(i) => (2, *i),
            _ => (3, 0),
        });
        let given = at
            .into_iter()
            .filter_map(|(sl, v)| {
                let (name, ty, short, field) = match sl {
                    Slot::Attr(o, i) => {
                        let t = g.owner_type(*o, f.principal, f.resource);
                        let n = &t.attrs[*i].named.name;
                        let full = format!("{}.{n}", o.word());
                        (full.clone(), Some(t.named.name.clone()), if shared(n) { full } else { n.clone() }, &t.attrs[*i].ty)
                    }
                    Slot::Input(i) => (a.inputs[*i].named.name.clone(), None, a.inputs[*i].named.name.clone(), &a.inputs[*i].ty),
                    _ => return None,
                };
                let value = match (v, field) {
                    (Val::Bool(b), _) => {
                        if *b {
                            tr!("はい", "yes")
                        } else {
                            tr!("いいえ", "no")
                        }
                    }
                    (Val::Absent, _) => tr!("無い", "absent"),
                    (Val::Enum(v), FieldType::Enum(e)) => Text::same(g.enums[*e].values[*v].name.clone()),
                    (Val::Cell(c), FieldType::Date { .. }) => show_days(*c),
                    (Val::Cell(c), FieldType::Num { unit, .. }) => cells::show_cell(*c, unit),
                    _ => return None,
                };
                Some(GivenValue { name, ty, short, val: v.clone(), value })
            })
            .collect();
        out.push(Confirmed { after, computed: said.iter().map(|(_, n)| (*n).clone()).collect(), region: reached.region, given });
    }
    out
}

/// A combination in words, for an example in a message: the principal with its roles and
/// attributes, the resource with its attributes, the relations, the inputs and the computed
/// values the policies read, each attribute and input by its cell, as the walk counts it. Where an
/// input was run that comes to the computed values of a group of rule outputs, it is said in
/// brackets after the last of them ([`Confirmed::said`]).
pub fn say(g: &Gate, s: &Space, k: &Kept) -> Text {
    say_with(g, s, k, &confirmed(g, s, k))
}

/// A combination in words as the walk counts it, and nothing of an input that was run. Two
/// versions of a gate are compared by it (`sekisho diff`): an input run for one version says
/// nothing of the other.
pub fn say_counted(g: &Gate, s: &Space, k: &Kept) -> Text {
    say_with(g, s, k, &[])
}

fn say_with(g: &Gate, s: &Space, k: &Kept, beside: &[Confirmed]) -> Text {
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
            Slot::Computed(i) if !a.computed[*i].of_flow() => &a.computed[*i].named.name,
            _ => continue,
        };
        if env[at] == Val::Absent && matches!(sl, Slot::Computed(_)) {
            continue;
        }
        if !f.factors.iter().any(|x| x.places.contains(&at)) {
            continue;
        }
        let v = val(at);
        parts.push(match beside.iter().find(|c| c.after == at) {
            Some(c) => {
                let c = c.said();
                tr!("{name}：{}（{}）", "{name}: {} ({})", v.ja, c.ja; v.en, c.en)
            }
            None => tr!("{name}：{}", "{name}: {}", v.ja; v.en),
        });
    }
    // the answers of the actions the conditions ask (DESIGN 2.11)
    for (l, link) in f.links.iter().enumerate().filter(|(_, x)| x.direct) {
        let Some(at) = f.place(&Slot::Allowed(l)) else { continue };
        let name = &g.actions[link.action].named.name;
        let v = val(at);
        parts.push(tr!("allowed {name}：{}", "allowed {name}: {}", v.ja; v.en));
    }
    Text::join(&parts, "、", ", ")
}

/// A combination as the values the JSON of a diagnostic lists: the principal and each role walked
/// (with the roles they include), the attributes, the resource, the relations, the inputs and the
/// computed values, each by what it is the value of, as the walk counts it.
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
            Slot::Computed(i) if !a.computed[*i].of_flow() => &a.computed[*i].named.name,
            _ => continue,
        };
        if walked(at) {
            out.push((name.clone(), shown(at)));
        }
    }
    for (l, link) in f.links.iter().enumerate().filter(|(_, x)| x.direct) {
        if let Some(at) = f.place(&Slot::Allowed(l)) {
            out.push((format!("allowed {}", g.actions[link.action].named.name), shown(at)));
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
