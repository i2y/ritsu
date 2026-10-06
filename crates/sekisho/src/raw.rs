//! Raw values (DESIGN 6.3): the data the code that builds the requests is given — what the `Store`
//! holds, the `input` of the operation, the instant of the request — made from each combination
//! the check walks, with the answer sekisho's reference evaluation gives for that data. The tests
//! of the TypeScript, the Python and the Go hand the same data to the code generated in each
//! language, and hold its answers to these.
//!
//! # The data of one combination
//!
//! A combination fixes the finite values (DESIGN 4.1): the roles, a bool or an enum, a number's
//! cell, which entities are the same, a rule's output, the truths of the dates. The data is made
//! from it as follows.
//!
//! - A number a policy compares is tried at both ends of its cell, as the vectors are (DESIGN 6.1):
//!   a combination whose cells have two ends gives two cases, one at the low ends and one at the
//!   high ends, named as the vectors name their tests.
//! - What only a rule reads (the clerk's limit) is taken from the inputs rulec gives for each value
//!   of the output over the cells (`Rules::outputs_over`, from rulec's vectors: inputs made from
//!   the boundaries), then from the ends, the middle and one in from each end of its range; the
//!   first that gives every output its value in the combination, by rulec's evaluator, is used.
//! - What only a date reads (the day the order was paid) is taken from the ends, the middle and one
//!   in from each end of its range, and `today` from every day of its range: of the pairs that give
//!   every date its truth in the combination, by koyomi's evaluator, the one whose `today` is
//!   nearest a day where a truth turns (the last day of a period, and the day after) is used.
//! - The instant of the request is the first second of that day where the day changes (`today`'s
//!   offset), or the last second, by turns, so that both edges of a day are tried.
//! - Entities get ids by their type (`user1`, `order1`, `customer2`, as the vectors do); the
//!   entities the combination says are the same get the same id.
//! - What the combination does not walk (an attribute only another action reads) is the first
//!   value it can be, as in the vectors.
//!
//! A combination no data gives (a value the walk counted though another language could not say
//! it is reached, W303) is left out, and counted.
//!
//! Besides the combinations, each action gets the data the generated code is to refuse before it
//! asks Cedar (DESIGN 3.8), one fault at a time: a day before and after `today`'s range (for an
//! action that computes from `today`; one that does not is given a day after it, and answers), an input
//! below and above its range or not of its enum, a principal or a resource the store does not
//! hold, an attribute of either outside its range or its enum, a role the type does not hold, a
//! principal of a type the action does not take, a workflow the gate does not declare.
//!
//! # The answer, and what the generated code is held to
//!
//! [`Model::evaluate`] answers for any data, as the generated code and Cedar together are to: it
//! is the specification the three generators follow. In this order, the first that fails refuses
//! the request with its kind, and nothing is asked of Cedar:
//!
//! 1. `principal`: the principal's type is one the action takes (and a workflow is one the gate
//!    declares);
//! 2. `today`: for an action that computes a value from `today` ([`reads_today`]), the day of the
//!    instant, at `today`'s offset, is within `today`'s range (an action that does not is answered
//!    on any day: no value of it is computed from a day the check did not walk);
//! 3. `input`: every input is given (unless it may be absent), of its type, within its range or
//!    its enum;
//! 4. `resource`: the resource's type is one the action takes, and when the code reads the
//!    resource, the store holds it, with every attribute the code reads of its type, each within
//!    its range or enum, its roles of the type's and its groups of the types it can be in;
//! 5. `principal`: likewise for the principal;
//! 6. `rule` and `date`: each value the action computes for these types is computed (a rule's
//!    error, a date that stops, a day the calendar does not know refuse).
//!
//! Otherwise the policies are evaluated as Cedar evaluates them ([`crate::eval`]), on the values
//! the data comes to, and the answer holds the decision, the determining policies by `@id`, and
//! the context given to Cedar: the inputs a policy reads (Cedar's `context` in the schema) and
//! every value the action computes for the types, each as Cedar is given it (a number counted in
//! its unit, an enum by its alias, a rule's value by its public name; an absent one left out). The
//! code turns an error of Cedar's (or a question it cannot ask) into a deny of a seventh kind,
//! `cedar`, which no data here comes to.
//!
//! What the store gives of an entity of a type is the same in every action ([`store_attrs`]): its
//! roles, when the type holds roles; the groups it is a member of, when a policy asks whether a
//! principal of the type is in one; and the attributes the schema gives the type (Cedar holds an
//! entity to the schema), with those a value an action computes reads. The code reads an entity
//! whenever its type has any of these ([`reads`]), and holds it to all of them. A type that has
//! none (a customer, a record with no attribute) is built from its id alone, and a workflow from
//! its name.
//!
//! # The data, as the harnesses read it
//!
//! [`Raw::json`] writes one JSON document: `gate`, `types`, `actions` and `cases`. Each type gives
//! the roles it can hold, the group types it can be a member of, and each attribute's kind (`bool`,
//! `enum`, `number`, `date`, `id`); each action, the field of its input that holds the resource's
//! id (the `from` argument, or `resource`), whether a field `resource_type` says the type (an
//! action that takes more than one), and each declared input's kind. A case gives:
//!
//! ```json
//! {"name": "refund_order 12 (amount 1)", "action": "refund_order",
//!  "principal": {"type": "User", "id": "user1"},
//!  "store": [{"type": "User", "id": "user1", "roles": ["clerk"], "member_of": [], "attrs": {"refund_limit": 0, "suspended": false}}, …],
//!  "input": {"orderId": "order1", "amount": 1},
//!  "now": "2026-10-01T00:00:00+00:00",
//!  "expect": {"allowed": false, "policies": [], "error": null, "context": {"amount": 1, …}}}
//! ```
//!
//! A number is counted in its declared unit (a JSON integer; a `bigint` in TypeScript), a date is
//! `YYYY-MM-DD`, an enum's value and a role are their aliases, an attribute that points to an
//! entity holds the entity's id, and an absent value is left out. A harness answers each case with
//! one line of JSON, in the order of the cases: `{"name", "allowed", "policies", "error",
//! "context"}` (the policies as the code returns them; `error` the kind, or null; `context` what
//! the code gave Cedar, or null when it refused), and [`Raw::compare`] holds the lines to the
//! answers: the decision, the determining policies as a set, the kind of a refusal, and the
//! context of a request that was asked.

use crate::ast::Op;
use crate::borders::Langs;
use crate::cedar::{self, Shape};
use crate::cells::{self, Cell};
use crate::checks::Checked;
use crate::eval::decide;
use crate::model::*;
use crate::names::Scope;
use crate::suite::Suite;
use crate::walk::{Ask, Kn, Rel, Slot, Space, Val, bit};
use ritsu_base::json::Json;
use ritsu_ports::{DateValue, Found, Value, Values};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

/// A value of the data: what the store holds of an attribute, or what an input is.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RawValue {
    Bool(bool),
    /// An enum's value, by its alias (what Cedar is given); any other string is not one of it.
    Enum(String),
    /// A number counted in its declared unit.
    Num(i128),
    /// A day, as the number of days since 1970-01-01.
    Date(i64),
    /// The id of the entity an attribute points to.
    Id(String),
}

impl RawValue {
    pub fn json(&self) -> Json {
        match self {
            RawValue::Bool(b) => Json::Bool(*b),
            RawValue::Enum(s) | RawValue::Id(s) => Json::str(s.clone()),
            RawValue::Num(n) => Json::Int(*n),
            RawValue::Date(d) => Json::str(ritsu_ports::day_text(*d)),
        }
    }
}

/// An entity the store holds: its type and id (by aliases), the roles it holds directly, the
/// groups it is a member of (type and id), and its attributes by alias (an absent one left out).
#[derive(Clone, Debug, PartialEq)]
pub struct Entity {
    pub ty: String,
    pub id: String,
    pub roles: Vec<String>,
    pub member_of: Vec<(String, String)>,
    pub attrs: Vec<(String, RawValue)>,
}

impl Entity {
    pub fn attr(&self, alias: &str) -> Option<&RawValue> {
        self.attrs.iter().find(|(k, _)| k == alias).map(|(_, v)| v)
    }
}

/// What the code answers, or is to answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    pub allowed: bool,
    /// The determining policies' `@id`, sorted.
    pub policies: Vec<String>,
    /// The kind of a refusal (`principal`, `resource`, `input`, `today`, `rule`, `date`).
    pub error: Option<String>,
    /// The context given to Cedar; None when the request was refused before it was asked.
    pub context: Option<Json>,
}

impl Answer {
    fn refused(kind: &str) -> Answer {
        Answer { allowed: false, policies: Vec::new(), error: Some(kind.to_string()), context: None }
    }

    pub fn json(&self) -> Json {
        Json::obj([
            ("allowed", Json::Bool(self.allowed)),
            ("policies", Json::arr(self.policies.iter().map(|p| Json::str(p.clone())))),
            ("error", Json::opt_str(self.error.clone())),
            ("context", self.context.clone().unwrap_or(Json::Null)),
        ])
    }
}

/// One request: the action, who asks and about what, the store, the input, the instant, and the
/// answer of the reference evaluation.
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    pub name: String,
    pub action: usize,
    /// The principal's and the resource's type (aliases) and id.
    pub principal: (String, String),
    pub resource: (String, String),
    pub store: Vec<Entity>,
    /// The declared inputs, by alias; an absent one left out.
    pub inputs: Vec<(String, RawValue)>,
    /// The instant, as seconds since 1970-01-01T00:00:00Z, and as RFC 3339 at `today`'s offset.
    pub at: i64,
    pub now: String,
    pub expect: Answer,
}

/// The data of a gate, with what it was made from.
pub struct Raw {
    pub gate: String,
    pub cases: Vec<Case>,
    /// Combinations walked, and those no data was found to give (each by its name).
    pub combinations: usize,
    pub unrealized: Vec<String>,
    /// Cases whose answer is not the walk's for the combination they were made from (a fault of
    /// the walk or of the evaluation; the test holds this empty).
    pub disagree: Vec<String>,
    kinds: Json,
}

/// The data of every action of a file that passes its check, with the languages `suite` joins.
pub fn raw(scope: &Scope, checked: &Checked, suite: &Suite) -> Raw {
    let ask = langs(suite);
    let m = Model::new(scope, checked, &ask);
    m.all()
}

/// The languages of `suite` the data is made and evaluated with: rulec's and koyomi's evaluators.
pub fn langs(suite: &Suite) -> Langs<'_> {
    Langs { rules: suite.rules.as_deref().filter(|r| r.joined()), dates: suite.dates.as_deref().filter(|d| d.joined()), books: None, flows: None }
}

/// Which end of its cell each number is taken at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    Low,
    High,
}

/// Where a computed value's input comes from, in a frame: an attribute of the principal or the
/// resource, or an input, by index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Var {
    Attr(Owner, usize),
    Input(usize),
    Today,
}

/// What a computed value comes to for some data.
#[derive(Clone, Debug, PartialEq)]
enum Got {
    Absent,
    Is(Val),
    Refused(&'static str),
}

/// What a computed value comes to for an assignment of what it reads, before the day is chosen.
#[derive(Clone, Debug)]
enum Pre {
    /// The same on every day.
    Got(Got),
    /// `today <op> day`.
    Turns(Op, i64),
    /// `today is open`: the days the calendar knows, and those it closes.
    Open(i64, i64, std::rc::Rc<ritsu_ports::DaySet>),
    /// Computed again each day (a rule or a date given today).
    Daily,
}

impl Pre {
    fn turn(&self) -> Option<(Op, i64)> {
        match self {
            Pre::Turns(op, d) => Some((*op, *d)),
            _ => None,
        }
    }
}

/// The gate, what Cedar holds of it, and the languages, with what was asked of them remembered.
pub struct Model<'a> {
    g: &'a Gate,
    checked: &'a Checked,
    shape: Shape,
    ask: &'a dyn Ask,
    rules: RefCell<HashMap<String, Result<Values, ()>>>,
    dates: RefCell<HashMap<DateKey, DatesOf>>,
    outputs: RefCell<HashMap<String, Option<Reached>>>,
}

/// A dates file and the inputs it was asked with; what koyomi answered; what rulec says each value
/// of an output is reached with.
type DateKey = (PathBuf, Vec<(String, i64)>);
type DatesOf = Result<Vec<(String, DateValue)>, ()>;
type Reached = Vec<(Value, Values)>;

impl<'a> Model<'a> {
    pub fn new(scope: &Scope, checked: &'a Checked, ask: &'a dyn Ask) -> Model<'a> {
        let g = &checked.gate;
        Model { g, checked, shape: cedar::shape(g, scope, checked), ask, rules: RefCell::default(), dates: RefCell::default(), outputs: RefCell::default() }
    }

    fn all(&self) -> Raw {
        let g = self.g;
        let mut cases = Vec::new();
        let mut unrealized = Vec::new();
        let mut disagree = Vec::new();
        let mut combinations = 0usize;
        for (ai, space) in self.checked.report.spaces.iter().enumerate() {
            let Some(s) = space else { continue };
            let mut n = 0usize;
            let mut made: Vec<Case> = Vec::new();
            s.each(|combo| {
                n += 1;
                combinations += 1;
                let env = combo.env.to_vec();
                let cells = self.cells_in_cedar(s, combo.frame, &env);
                let two = cells.iter().any(|(_, c, _)| c.lo < c.hi);
                let ends: &[End] = if two { &[End::Low, End::High] } else { &[End::Low] };
                let want_allow = combo.decision.allow;
                let mut want_policies: Vec<String> = combo.decision.determining.iter().map(|&i| cedar::policy_id(g, s.policies[i])).collect();
                want_policies.sort();
                for &end in ends {
                    let mut name = format!("{} {n}", g.actions[ai].named.alias);
                    if two {
                        let set: Vec<String> = cells.iter().map(|(shown, c, _)| format!("{shown} {}", if end == End::High { c.hi } else { c.lo })).collect();
                        name.push_str(&format!(" ({})", set.join(", ")));
                    }
                    // the first second of the day, or the last, by turns
                    let late = if two { end == End::High } else { n.is_multiple_of(2) };
                    match self.realize(s, combo.frame, &env, end, late, &name) {
                        Some(c) => {
                            if c.expect.error.is_some() || c.expect.allowed != want_allow || c.expect.policies != want_policies {
                                disagree.push(format!("{name}: the walk says {} {:?}, the data comes to {}", if want_allow { "allow" } else { "deny" }, want_policies, c.expect.json().compact()));
                            }
                            made.push(c);
                        }
                        None => unrealized.push(name),
                    }
                }
            });
            // the combinations first, then what is refused
            let refused = self.refusals(ai, made.first());
            cases.extend(made);
            cases.extend(refused);
        }
        Raw { gate: g.named.alias.clone(), cases, combinations, unrealized, disagree, kinds: self.kinds() }
    }

    // -----------------------------------------------------------------------------------------
    // The types and the actions, as the harnesses read them

    fn kind_of(f: &Field) -> &'static str {
        match f.ty {
            FieldType::Bool => "bool",
            FieldType::Enum(_) => "enum",
            FieldType::Num { .. } => "number",
            FieldType::Date { .. } => "date",
            FieldType::Entity(_) => "id",
        }
    }

    fn kinds(&self) -> Json {
        let g = self.g;
        let types = Json::Obj(
            g.types
                .iter()
                .enumerate()
                .map(|(ti, t)| {
                    let mut o = vec![
                        ("roles".to_string(), Json::arr(t.roles.iter().map(|&r| Json::str(g.roles[r].named.alias.clone())))),
                        ("member_of".to_string(), Json::arr(self.shape.types[ti].member_of.iter().map(|&m| Json::str(g.types[m].named.alias.clone())))),
                        ("attrs".to_string(), Json::Obj(t.attrs.iter().map(|f| (f.named.alias.clone(), Json::str(Self::kind_of(f)))).collect())),
                    ];
                    if t.kind == Kind::Workflow {
                        o.push(("workflows".to_string(), Json::arr(g.workflows.iter().map(|w| Json::str(w.named.alias.clone())))));
                    }
                    (t.named.alias.clone(), Json::Obj(o))
                })
                .collect(),
        );
        let actions = Json::Obj(
            g.actions
                .iter()
                .map(|a| {
                    (
                        a.named.alias.clone(),
                        Json::obj([
                            ("resource", Json::str(resource_field(a))),
                            ("resource_type", Json::Bool(a.resources.len() > 1)),
                            ("inputs", Json::Obj(a.inputs.iter().map(|f| (f.named.alias.clone(), Json::str(Self::kind_of(f)))).collect())),
                        ]),
                    )
                })
                .collect(),
        );
        Json::obj([("types", types), ("actions", actions)])
    }

    // -----------------------------------------------------------------------------------------
    // What the code reads

    /// Whether the value `ci` of action `ai` is computed for a principal of type `pt` and a
    /// resource of type `rt`: every attribute it reads, the types have.
    fn computable(&self, ai: usize, ci: usize, pt: usize, rt: usize) -> bool {
        self.sources(ai, ci).iter().all(|s| match s {
            Source::Attr(o, n) => self.g.owner_type(*o, pt, rt).attr(n).is_some(),
            Source::Input(n) => self.g.actions[ai].input(n).is_some(),
            Source::Lit(_) | Source::Today => true,
        }) && match &self.g.actions[ai].computed[ci].how {
            How::Date { of: DateOf::Attr(o, n), .. } => self.g.owner_type(*o, pt, rt).attr(n).is_some(),
            _ => true,
        }
    }

    /// Whether action `ai` computes a value from `today`: a date, a calendar's business day, or a
    /// rule given today. Only such an action holds the day of the request to `today`'s range.
    pub fn reads_today(&self, ai: usize) -> bool {
        reads_today(&self.g.actions[ai])
    }

    fn sources(&self, ai: usize, ci: usize) -> Vec<Source> {
        match &self.g.actions[ai].computed[ci].how {
            How::Rule { args, .. } | How::Date { of: DateOf::Call { args, .. }, .. } => args.iter().map(|(_, s)| s.clone()).collect(),
            How::Date { of: DateOf::Attr(o, n), .. } => vec![Source::Attr(*o, n.clone())],
            How::Open { .. } => Vec::new(),
        }
    }

    /// The attributes of type `t` the code reads (what the `Store` gives of an entity of it, by
    /// index): those the schema gives the type (Cedar holds an entity to the schema), and those a
    /// value an action computes reads, of any action that takes the type.
    pub fn store_attrs(&self, t: usize) -> BTreeSet<usize> {
        store_attrs(self.g, &self.shape, t)
    }

    /// Whether the code reads an entity of type `t` from the store: it holds roles, it can be a
    /// member of a group a policy asks of, or the code reads an attribute of it. A workflow is
    /// never read.
    pub fn reads(&self, t: usize) -> bool {
        reads(self.g, &self.shape, t)
    }

    // -----------------------------------------------------------------------------------------
    // The reference evaluation

    fn type_of(&self, alias: &str) -> Option<usize> {
        self.g.types.iter().position(|t| t.named.alias == alias)
    }

    /// Whether a value is one of a field: of its type, within its range or its enum.
    fn fits(&self, f: &Field, v: &RawValue) -> bool {
        match (&f.ty, v) {
            (FieldType::Bool, RawValue::Bool(_)) => true,
            (FieldType::Enum(e), RawValue::Enum(s)) => self.g.enums[*e].values.iter().any(|x| x.alias == *s),
            (FieldType::Num { lo, hi, .. }, RawValue::Num(n)) => lo <= n && n <= hi,
            (FieldType::Date { lo, hi }, RawValue::Date(d)) => lo <= d && d <= hi,
            (FieldType::Entity(_), RawValue::Id(_)) => true,
            _ => false,
        }
    }

    /// The answer for some data (the module's documentation says what is refused, and in which
    /// order).
    pub fn evaluate(&self, c: &Case) -> Answer {
        let g = self.g;
        let ai = c.action;
        let a = &g.actions[ai];
        // 1. the principal's type
        let Some(pt) = self.type_of(&c.principal.0).filter(|t| a.principals.contains(t)) else { return Answer::refused("principal") };
        if g.types[pt].kind == Kind::Workflow && !g.workflows.iter().any(|w| w.named.alias == c.principal.1) {
            return Answer::refused("principal");
        }
        // 2. today, for an action that computes a value from it
        let today = match &g.today {
            Some(t) if self.reads_today(ai) => {
                let d = (c.at + i64::from(t.offset) * 60).div_euclid(86_400);
                if d < t.lo || d > t.hi {
                    return Answer::refused("today");
                }
                Some(d)
            }
            _ => None,
        };
        // 3. the inputs
        let input = |n: &str| c.inputs.iter().find(|(k, _)| k == n).map(|(_, v)| v);
        for f in &a.inputs {
            match input(&f.named.alias) {
                None if f.optional => {}
                None => return Answer::refused("input"),
                Some(v) if !self.fits(f, v) => return Answer::refused("input"),
                Some(_) => {}
            }
        }
        // 4. the resource
        let Some(rt) = self.type_of(&c.resource.0).filter(|t| a.resources.contains(t)) else { return Answer::refused("resource") };
        let held = |ty: usize, id: &str| c.store.iter().find(|e| e.ty == g.types[ty].named.alias && e.id == id);
        let check = |ty: usize, id: &str| -> Result<Option<&Entity>, ()> {
            if !self.reads(ty) {
                return Ok(None);
            }
            let e = held(ty, id).ok_or(())?;
            for i in self.store_attrs(ty) {
                let f = &g.types[ty].attrs[i];
                match e.attr(&f.named.alias) {
                    None if f.optional => {}
                    None => return Err(()),
                    Some(v) if !self.fits(f, v) => return Err(()),
                    Some(_) => {}
                }
            }
            let roles: Vec<&str> = g.types[ty].roles.iter().map(|&r| g.roles[r].named.alias.as_str()).collect();
            if e.roles.iter().any(|r| !roles.contains(&r.as_str())) {
                return Err(());
            }
            let groups: Vec<&str> = self.shape.types[ty].member_of.iter().map(|&m| g.types[m].named.alias.as_str()).collect();
            if e.member_of.iter().any(|(t, _)| !groups.contains(&t.as_str())) {
                return Err(());
            }
            Ok(Some(e))
        };
        let Ok(r_entity) = check(rt, &c.resource.1) else { return Answer::refused("resource") };
        // 5. the principal
        let Ok(p_entity) = check(pt, &c.principal.1) else { return Answer::refused("principal") };
        // 6. the computed values
        let value_of = |src: &Source| -> Option<RawValue> {
            match src {
                Source::Attr(o, n) => {
                    let (t, e) = if *o == Owner::Principal { (pt, p_entity) } else { (rt, r_entity) };
                    let (_, f) = g.types[t].attr(n)?;
                    e?.attr(&f.named.alias).cloned()
                }
                Source::Input(n) => {
                    let (_, f) = a.input(n)?;
                    input(&f.named.alias).cloned()
                }
                Source::Lit(_) | Source::Today => None,
            }
        };
        let mut computed: Vec<Got> = Vec::new();
        for ci in 0..a.computed.len() {
            let got = if self.computable(ai, ci, pt, rt) { self.compute(ai, ci, &|s| value_of(s), today) } else { Got::Absent };
            if let Got::Refused(k) = got {
                return Answer::refused(k);
            }
            computed.push(got);
        }
        // the policies, on the values the data comes to
        let Some(Some(space)) = self.checked.report.spaces.get(ai) else { return Answer::refused("principal") };
        let Some(fi) = space.frames.iter().position(|f| f.principal == pt && f.resource == rt) else { return Answer::refused("principal") };
        let env = self.env(space, fi, c, p_entity, r_entity, &computed);
        let f = &space.frames[fi];
        let applies: Vec<bool> = f.policies.iter().map(|p| p.all.holds(&env)).collect();
        let d = decide(&applies, &space.permit);
        let mut policies: Vec<String> = d.determining.iter().map(|&i| cedar::policy_id(g, space.policies[i])).collect();
        policies.sort();
        // the context Cedar is given
        let mut context: Vec<(String, Json)> = Vec::new();
        for &i in &self.shape.actions[ai].inputs {
            let f = &a.inputs[i];
            if let Some(v) = input(&f.named.alias) {
                context.push((f.named.alias.clone(), v.json()));
            }
        }
        for (ci, got) in computed.iter().enumerate() {
            let cv = &a.computed[ci];
            let v = match (got, &self.shape.actions[ai].computed[ci].values) {
                (Got::Is(Val::Bool(b)), _) => Json::Bool(*b),
                (Got::Is(Val::Enum(k)), Some(vs)) => Json::str(vs[*k].alias.clone()),
                _ => continue,
            };
            context.push((cv.named.alias.clone(), v));
        }
        Answer { allowed: d.allow, policies, error: None, context: Some(Json::Obj(context)) }
    }

    /// The values of the frame's places for some data, as the walk would have them.
    fn env(&self, s: &Space, fi: usize, c: &Case, p: Option<&Entity>, r: Option<&Entity>, computed: &[Got]) -> Vec<Val> {
        let g = self.g;
        let f = &s.frames[fi];
        let a = &g.actions[s.action];
        let (pt, rt) = (f.principal, f.resource);
        let attr_val = |o: Owner, i: usize| -> Val {
            let t = if o == Owner::Principal { pt } else { rt };
            let e = if o == Owner::Principal { p } else { r };
            let fd = &g.types[t].attrs[i];
            match e.and_then(|e| e.attr(&fd.named.alias)) {
                Some(v) => self.val_of(fd, v),
                None => Val::Absent,
            }
        };
        let mut env = vec![Val::Absent; f.slots.len()];
        for (at, slot) in f.slots.iter().enumerate() {
            env[at] = match slot {
                Slot::Roles => {
                    let direct = p.map(|e| e.roles.iter().filter_map(|r| g.roles.iter().position(|x| x.named.alias == *r)).fold(0u128, |m, r| m | bit(r))).unwrap_or(0);
                    let effective = (0..g.roles.len()).filter(|r| direct & bit(*r) != 0).fold(0u128, |m, r| g.closure(r).iter().fold(m, |m, x| m | bit(*x)));
                    Val::Roles { direct, effective }
                }
                Slot::Workflow => g.workflows.iter().position(|w| w.named.alias == c.principal.1).map(Val::Workflow).unwrap_or(Val::Absent),
                Slot::Attr(o, i) => attr_val(*o, *i),
                Slot::Input(i) => {
                    let fd = &a.inputs[*i];
                    match c.inputs.iter().find(|(k, _)| *k == fd.named.alias) {
                        Some((_, v)) => self.val_of(fd, v),
                        None => Val::Absent,
                    }
                }
                Slot::Computed(i) => match &computed[*i] {
                    Got::Is(v) => v.clone(),
                    _ => Val::Absent,
                },
                Slot::Relation(ty) => {
                    let terms = f.terms.get(&at).cloned().unwrap_or_default();
                    let id_of = |t: &Term| -> Option<String> {
                        match t {
                            Term::Principal => Some(c.principal.1.clone()),
                            Term::Attr(o, n) => {
                                let owner_t = if *o == Owner::Principal { pt } else { rt };
                                let (i, _) = g.types[owner_t].attr(n)?;
                                match attr_val_raw(g, if *o == Owner::Principal { p } else { r }, owner_t, i) {
                                    Some(RawValue::Id(id)) => Some(id),
                                    _ => None,
                                }
                            }
                        }
                    };
                    let mut ids: Vec<String> = Vec::new();
                    let mut blocks: Vec<Option<u8>> = Vec::new();
                    for t in &terms {
                        blocks.push(id_of(t).map(|id| match ids.iter().position(|x| *x == id) {
                            Some(b) => b as u8,
                            None => {
                                ids.push(id);
                                (ids.len() - 1) as u8
                            }
                        }));
                    }
                    let principal = terms.iter().position(|t| *t == Term::Principal);
                    let ty_alias = &g.types[*ty].named.alias;
                    let members = ids.iter().enumerate().fold(0u64, |m, (b, id)| {
                        let member = p.is_some_and(|e| e.member_of.iter().any(|(t, x)| t == ty_alias && x == id));
                        if member { m | (1u64 << b) } else { m }
                    });
                    Val::Relation(Rel { blocks, members, principal })
                }
            };
        }
        env
    }

    /// A value of the data as the walk has it: a number as the cell of itself alone.
    fn val_of(&self, f: &Field, v: &RawValue) -> Val {
        match (&f.ty, v) {
            (_, RawValue::Bool(b)) => Val::Bool(*b),
            (FieldType::Enum(e), RawValue::Enum(s)) => self.g.enums[*e].values.iter().position(|x| x.alias == *s).map(Val::Enum).unwrap_or(Val::Absent),
            (_, RawValue::Num(n)) => Val::Cell(Cell { lo: *n, hi: *n }),
            _ => Val::Absent,
        }
    }

    /// What computed value `ci` of action `ai` comes to, its inputs read by `get` (None: absent).
    fn compute(&self, ai: usize, ci: usize, get: &dyn Fn(&Source) -> Option<RawValue>, today: Option<i64>) -> Got {
        let a = &self.g.actions[ai];
        let cv = &a.computed[ci];
        let Some(kn) = self.checked.known.computed.get(&(ai, ci)) else { return Got::Refused("rule") };
        match (&cv.how, kn) {
            (How::Rule { args, .. }, Kn::Rule { file, output, domain, inputs, enums, .. }) => {
                let mut vals: Values = Vec::new();
                for (input, src) in args {
                    let unit = inputs.iter().find(|(n, _)| n == input).and_then(|(_, u)| u.clone());
                    let v = match src {
                        Source::Lit(Literal::Num(num)) => match unit.and_then(|u| crate::types::count(num, &u).ok()) {
                            Some(k) => Value::Int(k),
                            None => return Got::Refused("rule"),
                        },
                        Source::Lit(Literal::Bool(b)) => Value::Bool(*b),
                        Source::Lit(Literal::Word(w)) => self.to_rule(enums, input, w),
                        Source::Lit(Literal::Date(d)) => Value::Date(ritsu_ports::day_text(*d)),
                        Source::Today => match today {
                            Some(t) => Value::Date(ritsu_ports::day_text(t)),
                            None => return Got::Refused("rule"),
                        },
                        _ => match get(src) {
                            None => return Got::Absent,
                            Some(RawValue::Num(n)) => Value::Int(n),
                            Some(RawValue::Bool(b)) => Value::Bool(b),
                            Some(RawValue::Date(d)) => Value::Date(ritsu_ports::day_text(d)),
                            Some(RawValue::Enum(s)) => self.to_rule(enums, input, &s),
                            Some(RawValue::Id(_)) => return Got::Refused("rule"),
                        },
                    };
                    vals.push((input.clone(), v));
                }
                match self.rule(file, &vals) {
                    Ok(outs) => match outs.iter().find(|(n, _)| n == output).and_then(|(_, v)| domain.of_value(v)) {
                        Some(v) => Got::Is(v),
                        None => Got::Refused("rule"),
                    },
                    Err(()) => Got::Refused("rule"),
                }
            }
            (How::Date { op, of: DateOf::Call { args, .. } }, Kn::Date { file, function, .. }) => {
                let Some(t) = today else { return Got::Refused("date") };
                let mut ins: Vec<(String, i64)> = Vec::new();
                for (n, src) in args {
                    let v = match src {
                        Source::Lit(Literal::Date(d)) => *d,
                        Source::Lit(Literal::Num(num)) if num.value.is_int() => num.value.num as i64,
                        Source::Lit(_) => return Got::Refused("date"),
                        Source::Today => t,
                        _ => match get(src) {
                            None => return Got::Absent,
                            Some(RawValue::Date(d)) => d,
                            Some(RawValue::Num(n)) => n as i64,
                            Some(_) => return Got::Refused("date"),
                        },
                    };
                    ins.push((n.clone(), v));
                }
                match self.date(file, ins) {
                    Ok(out) => match out.iter().find(|(n, _)| n == function) {
                        Some((_, DateValue::Day(d))) => Got::Is(Val::Bool(cells::holds(*op, t, *d))),
                        _ => Got::Refused("date"),
                    },
                    Err(()) => Got::Refused("date"),
                }
            }
            (How::Date { op, of: DateOf::Attr(o, n) }, Kn::Attr) => {
                let Some(t) = today else { return Got::Refused("date") };
                match get(&Source::Attr(*o, n.clone())) {
                    Some(RawValue::Date(d)) => Got::Is(Val::Bool(cells::holds(*op, t, d))),
                    None => Got::Absent,
                    Some(_) => Got::Refused("date"),
                }
            }
            (How::Open { .. }, Kn::Open { data, closed }) => {
                let Some(t) = today else { return Got::Refused("date") };
                if t < data.0 || t > data.1 { Got::Refused("date") } else { Got::Is(Val::Bool(!closed.contains(&t))) }
            }
            _ => Got::Refused("rule"),
        }
    }

    /// A word for a value of a rule's enum input, as the rule's evaluator takes it: the rule's name
    /// of the value the word (or the gate's value of that alias or name) names.
    fn to_rule(&self, enums: &[(String, crate::walk::EnumInput)], input: &str, word: &str) -> Value {
        let Some((_, values)) = enums.iter().find(|(n, _)| n == input) else { return Value::Enum(word.to_string()) };
        let gate_value = self.g.enums.iter().flat_map(|e| e.values.iter()).find(|x| x.alias == word || x.name == word);
        values
            .iter()
            .find(|(_, words)| words.iter().any(|x| x == word || gate_value.is_some_and(|gv| *x == gv.name || *x == gv.alias)))
            .map(|(name, _)| Value::Enum(name.clone()))
            .unwrap_or_else(|| Value::Enum(word.to_string()))
    }

    fn rule(&self, file: &std::path::Path, vals: &Values) -> Result<Values, ()> {
        let key = format!("{}\u{1}{vals:?}", file.display());
        if let Some(r) = self.rules.borrow().get(&key) {
            return r.clone();
        }
        let r = self.ask.rule(file, vals).map_err(|_| ());
        self.rules.borrow_mut().insert(key, r.clone());
        r
    }

    fn date(&self, file: &std::path::Path, ins: Vec<(String, i64)>) -> Result<Vec<(String, DateValue)>, ()> {
        let key = (file.to_path_buf(), ins);
        if let Some(r) = self.dates.borrow().get(&key) {
            return r.clone();
        }
        let r = self.ask.dates(file, &key.1).map_err(|_| ());
        self.dates.borrow_mut().insert(key, r.clone());
        r
    }

    /// The values `output` of the rule comes to over the ranges, each with an input that reaches
    /// it (rulec's vectors), when rulec says so exactly.
    fn reached(&self, file: &std::path::Path, output: &str, ranges: &[(String, Option<i128>, Option<i128>)]) -> Option<Vec<(Value, Values)>> {
        let key = format!("{}\u{1}{output}\u{1}{ranges:?}", file.display());
        if let Some(r) = self.outputs.borrow().get(&key) {
            return r.clone();
        }
        let r = match self.ask.outputs(file, output, ranges) {
            Ok(Found::Value(v)) => Some(v),
            _ => None,
        };
        self.outputs.borrow_mut().insert(key, r.clone());
        r
    }

    // -----------------------------------------------------------------------------------------
    // Data for a combination

    /// The numbers of a combination that Cedar reads with a cell (vectors' `numbers`): how a name
    /// shows each, its cell, and its place.
    fn cells_in_cedar(&self, s: &Space, fi: usize, env: &[Val]) -> Vec<(String, Cell, usize)> {
        let g = self.g;
        let f = &s.frames[fi];
        let a = &g.actions[s.action];
        let mut out = Vec::new();
        for (at, slot) in f.slots.iter().enumerate() {
            let (in_cedar, shown) = match slot {
                Slot::Input(i) => (self.shape.actions[s.action].inputs.contains(i), a.inputs[*i].named.alias.clone()),
                Slot::Attr(o, i) => {
                    let t = if *o == Owner::Principal { f.principal } else { f.resource };
                    (self.shape.types[t].attrs.contains(i), format!("{}.{}", o.word(), g.types[t].attrs[*i].named.alias))
                }
                _ => (false, String::new()),
            };
            if let (true, Val::Cell(c)) = (in_cedar, &env[at]) {
                out.push((shown, *c, at));
            }
        }
        out
    }

    /// The data of one combination, at one end of its cells; None when none is found.
    fn realize(&self, s: &Space, fi: usize, env: &[Val], end: End, late: bool, name: &str) -> Option<Case> {
        // first with every number at its end; then letting a number a value is computed from move
        // within its cell
        for loose in [false, true] {
            if let Some(c) = self.realize_with(s, fi, env, end, late, name, loose) {
                return Some(c);
            }
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn realize_with(&self, s: &Space, fi: usize, env: &[Val], end: End, late: bool, name: &str, loose: bool) -> Option<Case> {
        let g = self.g;
        let ai = s.action;
        let a = &g.actions[ai];
        let f = &s.frames[fi];
        let (pt, rt) = (f.principal, f.resource);
        let mut ids = Ids::default();
        let p_id = if g.types[pt].kind == Kind::Workflow {
            let w = f.place(&Slot::Workflow).and_then(|at| match &env[at] {
                Val::Workflow(w) => Some(*w),
                _ => None,
            });
            match w.or(if g.workflows.is_empty() { None } else { Some(0) }) {
                Some(w) => g.workflows[w].named.alias.clone(),
                None => ids.fresh(g, pt),
            }
        } else {
            ids.fresh(g, pt)
        };
        let r_id = ids.fresh(g, rt);
        // the entities the relations point to, and the groups the principal is a member of
        let mut pointed: BTreeMap<(Owner, usize), Option<String>> = BTreeMap::new();
        let mut groups: Vec<(String, String)> = Vec::new();
        for (&at, terms) in &f.terms {
            let (Slot::Relation(ty), Val::Relation(rel)) = (&f.slots[at], &env[at]) else { continue };
            let blocks = rel.blocks.iter().flatten().map(|b| *b as usize + 1).max().unwrap_or(0);
            let own = rel.principal.and_then(|p| rel.blocks[p]).map(|b| b as usize);
            let block_ids: Vec<String> = (0..blocks).map(|b| if Some(b) == own { p_id.clone() } else { ids.fresh(g, *ty) }).collect();
            for (i, t) in terms.iter().enumerate() {
                if let Term::Attr(o, n) = t
                    && let Some((k, _)) = g.owner_type(*o, pt, rt).attr(n)
                {
                    pointed.entry((*o, k)).or_insert_with(|| rel.blocks[i].map(|b| block_ids[b as usize].clone()));
                }
            }
            for (b, id) in block_ids.iter().enumerate() {
                if rel.members & (1u64 << b) != 0 {
                    groups.push((g.types[*ty].named.alias.clone(), id.clone()));
                }
            }
        }
        // what the combination fixes
        let mut fixed: BTreeMap<Var, Option<RawValue>> = BTreeMap::new();
        let mut cut: BTreeMap<Var, Cell> = BTreeMap::new();
        for (at, slot) in f.slots.iter().enumerate() {
            let (var, field) = match slot {
                Slot::Attr(o, i) => (Var::Attr(*o, *i), &g.owner_type(*o, pt, rt).attrs[*i]),
                Slot::Input(i) => (Var::Input(*i), &a.inputs[*i]),
                _ => continue,
            };
            let v = match (&env[at], &field.ty) {
                (Val::Bool(b), _) => Some(RawValue::Bool(*b)),
                (Val::Enum(k), FieldType::Enum(e)) => Some(RawValue::Enum(g.enums[*e].values[*k].alias.clone())),
                (Val::Cell(c), _) => {
                    cut.insert(var, *c);
                    Some(RawValue::Num(if end == End::High { c.hi } else { c.lo }))
                }
                _ => None,
            };
            fixed.insert(var, v);
        }
        // the values the action computes for these types, and what the combination says of each
        let computing: Vec<usize> = (0..a.computed.len()).filter(|&ci| self.computable(ai, ci, pt, rt)).collect();
        let target = |ci: usize| -> Option<Val> { f.place(&Slot::Computed(ci)).map(|at| env[at].clone()) };
        // what they read that the combination does not fix (or, loose, a number it cuts)
        let var_of = |src: &Source| -> Option<Var> {
            match src {
                Source::Attr(o, n) => g.owner_type(*o, pt, rt).attr(n).map(|(i, _)| Var::Attr(*o, i)),
                Source::Input(n) => a.input(n).map(|(i, _)| Var::Input(i)),
                Source::Today => Some(Var::Today),
                Source::Lit(_) => None,
            }
        };
        let reads_today = |ci: usize| matches!(a.computed[ci].how, How::Date { .. } | How::Open { .. }) || self.sources(ai, ci).contains(&Source::Today);
        let free = |v: &Var| -> bool {
            match v {
                Var::Today => true,
                _ => !fixed.contains_key(v) || (loose && cut.contains_key(v)),
            }
        };
        let vars_of = |ci: usize| -> Vec<Var> {
            let mut out: Vec<Var> = self.sources(ai, ci).iter().filter_map(var_of).filter(free).collect();
            if reads_today(ci) && g.today.is_some() && !out.contains(&Var::Today) {
                out.push(Var::Today);
            }
            out.sort();
            out.dedup();
            out
        };
        // the computations joined by what they read
        let mut comps: Vec<(Vec<usize>, Vec<Var>)> = Vec::new();
        for &ci in &computing {
            let vs = vars_of(ci);
            let joined: Vec<usize> = (0..comps.len()).filter(|&k| comps[k].1.iter().any(|v| vs.contains(v))).collect();
            let mut cis = vec![ci];
            let mut all = vs;
            for k in joined.iter().rev() {
                let (c2, v2) = comps.remove(*k);
                cis.extend(c2);
                all.extend(v2);
            }
            cis.sort();
            all.sort();
            all.dedup();
            comps.push((cis, all));
        }
        // `today`, when nothing computed reads it: an end of its range
        let mut chosen: BTreeMap<Var, Option<RawValue>> = BTreeMap::new();
        if let Some(t) = &g.today {
            chosen.insert(Var::Today, Some(RawValue::Date(if late { t.hi } else { t.lo })));
        }
        for (cis, vars) in &comps {
            let found = self.search(ai, pt, rt, cis, vars, &fixed, &cut, loose, end, &target)?;
            chosen.extend(found);
        }
        // the data: the fixed values, the chosen ones, then the first value of the rest
        let value = |var: Var, field: &Field, pointed_id: Option<&Option<String>>, ids: &mut Ids| -> Option<RawValue> {
            if let Some(v) = chosen.get(&var) {
                return v.clone();
            }
            if let Some(v) = fixed.get(&var) {
                return v.clone();
            }
            if let Some(p) = pointed_id {
                return p.clone().map(RawValue::Id);
            }
            if field.optional {
                return None;
            }
            Some(match &field.ty {
                FieldType::Bool => RawValue::Bool(false),
                FieldType::Enum(e) => RawValue::Enum(g.enums[*e].values.first().map(|v| v.alias.clone()).unwrap_or_default()),
                FieldType::Num { lo, .. } => RawValue::Num(*lo),
                FieldType::Date { lo, .. } => RawValue::Date(*lo),
                FieldType::Entity(t) => RawValue::Id(ids.fresh(g, *t)),
            })
        };
        let entity = |owner: Owner, t: usize, id: &str, ids: &mut Ids| -> Entity {
            let mut attrs = Vec::new();
            for (i, fd) in g.types[t].attrs.iter().enumerate() {
                if let Some(v) = value(Var::Attr(owner, i), fd, pointed.get(&(owner, i)), ids) {
                    attrs.push((fd.named.alias.clone(), v));
                }
            }
            Entity { ty: g.types[t].named.alias.clone(), id: id.to_string(), roles: Vec::new(), member_of: Vec::new(), attrs }
        };
        let mut p = entity(Owner::Principal, pt, &p_id, &mut ids);
        let r = entity(Owner::Resource, rt, &r_id, &mut ids);
        if let Val::Roles { direct, .. } = &env[0] {
            p.roles = (0..g.roles.len()).filter(|r| direct & bit(*r) != 0).map(|r| g.roles[r].named.alias.clone()).collect();
        }
        p.member_of = groups;
        let mut inputs = Vec::new();
        for (i, fd) in a.inputs.iter().enumerate() {
            if let Some(v) = value(Var::Input(i), fd, None, &mut ids) {
                inputs.push((fd.named.alias.clone(), v));
            }
        }
        let (at, now) = self.instant(match chosen.get(&Var::Today) {
            Some(Some(RawValue::Date(d))) => *d,
            _ => 20_367, // 2025-10-06: a gate with no `today` reads no day
        }, late);
        let store = if g.types[pt].kind == Kind::Workflow { vec![r] } else { vec![p, r] };
        let mut c = Case { name: name.to_string(), action: ai, principal: (g.types[pt].named.alias.clone(), p_id), resource: (g.types[rt].named.alias.clone(), r_id), store, inputs, at, now, expect: Answer::refused("") };
        c.expect = self.evaluate(&c);
        Some(c)
    }

    /// The first or the last second of `day` at `today`'s offset, as seconds and as RFC 3339.
    fn instant(&self, day: i64, late: bool) -> (i64, String) {
        let offset = self.g.today.as_ref().map(|t| t.offset).unwrap_or(0);
        let secs_of_day = if late { 86_399 } else { 0 };
        let at = day * 86_400 + secs_of_day - i64::from(offset) * 60;
        let (h, m, sec) = (secs_of_day / 3600, (secs_of_day / 60) % 60, secs_of_day % 60);
        let sign = if offset < 0 { '-' } else { '+' };
        let now = format!("{}T{h:02}:{m:02}:{sec:02}{sign}{:02}:{:02}", ritsu_ports::day_text(day), offset.abs() / 60, offset.abs() % 60);
        (at, now)
    }

    /// The values of `vars` that give each computed value of `cis` what the combination says of it
    /// (any value, when the combination does not read one), by the languages' evaluators. Of those
    /// that do, the one whose `today` is nearest a day where a date's truth turns (the first found,
    /// when nothing turns on today). None when no candidate does.
    #[allow(clippy::too_many_arguments)]
    fn search(
        &self,
        ai: usize,
        pt: usize,
        rt: usize,
        cis: &[usize],
        vars: &[Var],
        fixed: &BTreeMap<Var, Option<RawValue>>,
        cut: &BTreeMap<Var, Cell>,
        loose: bool,
        end: End,
        target: &dyn Fn(usize) -> Option<Val>,
    ) -> Option<BTreeMap<Var, Option<RawValue>>> {
        let g = self.g;
        let a = &g.actions[ai];
        let field = |v: &Var| -> Option<&Field> {
            match v {
                Var::Attr(o, i) => Some(&g.owner_type(*o, pt, rt).attrs[*i]),
                Var::Input(i) => Some(&a.inputs[*i]),
                Var::Today => None,
            }
        };
        // the candidates of each variable but today, the inputs rulec gives first
        let others: Vec<Var> = vars.iter().copied().filter(|v| *v != Var::Today).collect();
        let examples = self.examples(ai, pt, rt, cis, &others, fixed);
        let mut cands: Vec<Vec<Option<RawValue>>> = Vec::new();
        for v in &others {
            let fd = field(v)?;
            let mut list: Vec<Option<RawValue>> = Vec::new();
            let fits = |x: &Option<RawValue>| x.as_ref().is_none_or(|x| self.fits(fd, x));
            let mut push = |x: Option<RawValue>| {
                if fits(&x) && !list.contains(&x) {
                    list.push(x);
                }
            };
            if loose && let Some(c) = cut.get(v) {
                // a number the combination cuts, moving within its cell: its end first
                push(Some(RawValue::Num(if end == End::High { c.hi } else { c.lo })));
                for e in examples.get(v).into_iter().flatten() {
                    if matches!(e, Some(RawValue::Num(n)) if c.contains(*n)) {
                        push(e.clone());
                    }
                }
                for n in spread(c.lo, c.hi) {
                    push(Some(RawValue::Num(n)));
                }
            } else {
                for e in examples.get(v).into_iter().flatten() {
                    push(e.clone());
                }
                match &fd.ty {
                    FieldType::Bool => {
                        push(Some(RawValue::Bool(false)));
                        push(Some(RawValue::Bool(true)));
                    }
                    FieldType::Enum(e) => {
                        for x in &g.enums[*e].values {
                            push(Some(RawValue::Enum(x.alias.clone())));
                        }
                    }
                    FieldType::Num { lo, hi, .. } => {
                        for n in spread(*lo, *hi) {
                            push(Some(RawValue::Num(n)));
                        }
                    }
                    FieldType::Date { lo, hi } => {
                        for d in spread(i128::from(*lo), i128::from(*hi)) {
                            push(Some(RawValue::Date(d as i64)));
                        }
                    }
                    FieldType::Entity(_) => return None,
                }
                if fd.optional {
                    push(None);
                }
            }
            cands.push(list);
        }
        let days: Vec<i64> = match (&g.today, vars.contains(&Var::Today)) {
            (Some(t), true) => (t.lo..=t.hi).collect(),
            _ => Vec::new(),
        };
        let per = days.len().max(1);
        if cands.iter().map(Vec::len).product::<usize>().saturating_mul(per) > 4_000_000 {
            return None;
        }
        let mut best: Option<(i64, BTreeMap<Var, Option<RawValue>>)> = None;
        for asg in product(&cands) {
            let mut base: BTreeMap<Var, Option<RawValue>> = fixed.clone();
            for (v, x) in others.iter().zip(&asg) {
                base.insert(*v, x.clone());
            }
            let get = |src: &Source| -> Option<RawValue> {
                let var = match src {
                    Source::Attr(o, n) => g.owner_type(*o, pt, rt).attr(n).map(|(i, _)| Var::Attr(*o, i)),
                    Source::Input(n) => a.input(n).map(|(i, _)| Var::Input(i)),
                    Source::Lit(_) | Source::Today => None,
                }?;
                base.get(&var).cloned().flatten()
            };
            // what each value comes to for this assignment, as far as it does not read today
            let pre: Vec<Pre> = cis.iter().map(|&ci| self.pre(ai, ci, &get)).collect();
            let wants: Vec<Option<Val>> = cis.iter().map(|&ci| target(ci)).collect();
            let matches = |got: &Got, want: &Option<Val>| match (got, want) {
                (Got::Refused(_), _) => false,
                (_, None) => true,
                (Got::Absent, Some(Val::Absent)) => true,
                (Got::Is(v), Some(w)) => v == w,
                _ => false,
            };
            let keep = |today: Option<i64>| -> BTreeMap<Var, Option<RawValue>> {
                let mut m: BTreeMap<Var, Option<RawValue>> = BTreeMap::new();
                for (v, x) in others.iter().zip(&asg) {
                    m.insert(*v, x.clone());
                }
                if let Some(t) = today {
                    m.insert(Var::Today, Some(RawValue::Date(t)));
                }
                m
            };
            if days.is_empty() {
                let all = cis.iter().zip(&pre).zip(&wants).all(|((&ci, p), w)| matches(&self.at_day(ai, ci, p, &get, None), w));
                if all {
                    return Some(keep(None));
                }
                continue;
            }
            for &t in &days {
                let all = cis.iter().zip(&pre).zip(&wants).all(|((&ci, p), w)| matches(&self.at_day(ai, ci, p, &get, Some(t)), w));
                if !all {
                    continue;
                }
                let score = pre.iter().filter_map(|p| p.turn()).map(|(op, d)| distance(op, t, d)).min().unwrap_or(0);
                if best.as_ref().is_none_or(|(s, _)| score < *s) {
                    best = Some((score, keep(Some(t))));
                    if score == 0 {
                        return best.map(|(_, m)| m);
                    }
                }
            }
        }
        best.map(|(_, m)| m)
    }

    /// What computed value `ci` comes to for an assignment, as far as it does not read today.
    fn pre(&self, ai: usize, ci: usize, get: &dyn Fn(&Source) -> Option<RawValue>) -> Pre {
        let a = &self.g.actions[ai];
        let reads_today = self.sources(ai, ci).contains(&Source::Today);
        match (&a.computed[ci].how, self.checked.known.computed.get(&(ai, ci))) {
            (How::Rule { .. }, _) if !reads_today => Pre::Got(self.compute(ai, ci, get, None)),
            (How::Date { op, of: DateOf::Call { .. } }, _) if !reads_today => match self.call_day(ai, ci, get, 0) {
                Some(d) => Pre::Turns(*op, d),
                None => match self.compute(ai, ci, get, Some(0)) {
                    Got::Absent => Pre::Got(Got::Absent),
                    _ => Pre::Got(Got::Refused("date")),
                },
            },
            (How::Date { op, of: DateOf::Attr(o, n) }, _) => match get(&Source::Attr(*o, n.clone())) {
                Some(RawValue::Date(d)) => Pre::Turns(*op, d),
                None => Pre::Got(Got::Absent),
                Some(_) => Pre::Got(Got::Refused("date")),
            },
            (How::Open { .. }, Some(Kn::Open { data, closed })) => Pre::Open(data.0, data.1, closed.clone()),
            _ => Pre::Daily,
        }
    }

    /// What computed value `ci` comes to on day `today`, from what [`Model::pre`] found.
    fn at_day(&self, ai: usize, ci: usize, p: &Pre, get: &dyn Fn(&Source) -> Option<RawValue>, today: Option<i64>) -> Got {
        match (p, today) {
            (Pre::Got(g), _) => g.clone(),
            (Pre::Turns(op, d), Some(t)) => Got::Is(Val::Bool(cells::holds(*op, t, *d))),
            (Pre::Open(lo, hi, closed), Some(t)) => {
                if t < *lo || t > *hi {
                    Got::Refused("date")
                } else {
                    Got::Is(Val::Bool(!closed.contains(&t)))
                }
            }
            _ => self.compute(ai, ci, get, today),
        }
    }

    /// The day a date of a dates file comes to, for the inputs `get` reads and `today`.
    fn call_day(&self, ai: usize, ci: usize, get: &dyn Fn(&Source) -> Option<RawValue>, today: i64) -> Option<i64> {
        let (How::Date { of: DateOf::Call { args, .. }, .. }, Some(Kn::Date { file, function, .. })) = (&self.g.actions[ai].computed[ci].how, self.checked.known.computed.get(&(ai, ci))) else { return None };
        let mut ins = Vec::new();
        for (n, src) in args {
            let v = match src {
                Source::Lit(Literal::Date(d)) => *d,
                Source::Lit(Literal::Num(num)) if num.value.is_int() => num.value.num as i64,
                Source::Today => today,
                Source::Lit(_) => return None,
                _ => match get(src)? {
                    RawValue::Date(d) => d,
                    RawValue::Num(n) => n as i64,
                    _ => return None,
                },
            };
            ins.push((n.clone(), v));
        }
        match self.date(file, ins).ok()?.into_iter().find(|(n, _)| n == function)? {
            (_, DateValue::Day(d)) => Some(d),
            _ => None,
        }
    }

    /// For each variable a rule of `cis` reads, the values the inputs rulec gives for the
    /// rule's outputs hold it at: rulec is asked with every fixed number held to its value.
    fn examples(&self, ai: usize, pt: usize, rt: usize, cis: &[usize], vars: &[Var], fixed: &BTreeMap<Var, Option<RawValue>>) -> BTreeMap<Var, Vec<Option<RawValue>>> {
        let g = self.g;
        let a = &g.actions[ai];
        let mut out: BTreeMap<Var, Vec<Option<RawValue>>> = BTreeMap::new();
        for &ci in cis {
            let (How::Rule { args, .. }, Some(Kn::Rule { file, output, inputs, enums, .. })) = (&a.computed[ci].how, self.checked.known.computed.get(&(ai, ci))) else { continue };
            let mut ranges: Vec<(String, Option<i128>, Option<i128>)> = Vec::new();
            let mut by_input: Vec<(String, Var)> = Vec::new();
            for (input, src) in args {
                let unit = inputs.iter().find(|(n, _)| n == input).and_then(|(_, u)| u.clone());
                let var = match src {
                    Source::Attr(o, n) => g.owner_type(*o, pt, rt).attr(n).map(|(i, _)| Var::Attr(*o, i)),
                    Source::Input(n) => a.input(n).map(|(i, _)| Var::Input(i)),
                    Source::Lit(Literal::Num(num)) => {
                        if let Some(k) = unit.and_then(|u| crate::types::count(num, &u).ok()) {
                            ranges.push((input.clone(), Some(k), Some(k)));
                        }
                        None
                    }
                    Source::Lit(Literal::Date(d)) => {
                        ranges.push((input.clone(), Some(i128::from(*d)), Some(i128::from(*d))));
                        None
                    }
                    _ => None,
                };
                let Some(var) = var else { continue };
                let declared = match var {
                    Var::Attr(o, i) => Some(&g.owner_type(o, pt, rt).attrs[i]),
                    Var::Input(i) => Some(&a.inputs[i]),
                    Var::Today => None,
                };
                match (vars.contains(&var), fixed.get(&var), declared.map(|f| &f.ty)) {
                    (false, Some(Some(RawValue::Num(n))), _) => ranges.push((input.clone(), Some(*n), Some(*n))),
                    (false, Some(Some(RawValue::Date(d))), _) => ranges.push((input.clone(), Some(i128::from(*d)), Some(i128::from(*d)))),
                    // what is free is held to its own range, which may be narrower than the rule's
                    (true, _, Some(FieldType::Num { lo, hi, .. })) => ranges.push((input.clone(), Some(*lo), Some(*hi))),
                    (true, _, Some(FieldType::Date { lo, hi })) => ranges.push((input.clone(), Some(i128::from(*lo)), Some(i128::from(*hi)))),
                    _ => {}
                }
                by_input.push((input.clone(), var));
            }
            let Some(reached) = self.reached(file, output, &ranges) else { continue };
            for (_, ins) in reached {
                for (input, var) in &by_input {
                    if !vars.contains(var) {
                        continue;
                    }
                    let Some((_, v)) = ins.iter().find(|(n, _)| n == input) else { continue };
                    let raw = match v {
                        Value::Int(k) => Some(RawValue::Num(*k)),
                        Value::Bool(b) => Some(RawValue::Bool(*b)),
                        Value::Date(s) => parse_day(s).map(RawValue::Date),
                        Value::Enum(name) => {
                            // the gate's value the rule's value is named by
                            let words: Vec<String> = enums.iter().find(|(n, _)| n == input).and_then(|(_, vs)| vs.iter().find(|(n, _)| n == name)).map(|(_, w)| w.clone()).unwrap_or_default();
                            g.enums.iter().flat_map(|e| e.values.iter()).find(|x| words.contains(&x.name) || words.contains(&x.alias)).map(|x| RawValue::Enum(x.alias.clone()))
                        }
                        _ => None,
                    };
                    if let Some(raw) = raw {
                        let list = out.entry(*var).or_default();
                        if !list.contains(&Some(raw.clone())) {
                            list.push(Some(raw));
                        }
                    }
                }
            }
        }
        out
    }

    // -----------------------------------------------------------------------------------------
    // What the code is to refuse

    /// The data of each fault the code refuses, made from the first case of the action (or, for an
    /// action no combination of which was made, nothing).
    fn refusals(&self, ai: usize, first: Option<&Case>) -> Vec<Case> {
        let g = self.g;
        let a = &g.actions[ai];
        let Some(base) = first else { return Vec::new() };
        let mut out: Vec<Case> = Vec::new();
        let action = &a.named.alias;
        let mut add = |what: &str, change: &dyn Fn(&mut Case)| {
            let mut c = base.clone();
            c.name = format!("{action}: {what}");
            change(&mut c);
            c.expect = self.evaluate(&c);
            out.push(c);
        };
        // a day before and after today's range: refused by an action that computes a value from
        // today, and answered as on any other day by one that does not
        if let Some(t) = &g.today {
            let (lo, hi) = (t.lo, t.hi);
            if self.reads_today(ai) {
                add("the day before today's range", &|c| (c.at, c.now) = self.instant(lo - 1, true));
                add("the day after today's range", &|c| (c.at, c.now) = self.instant(hi + 1, false));
            } else {
                add("a day after today's range, which it does not read", &|c| (c.at, c.now) = self.instant(hi + 1, false));
            }
        }
        // an input outside its range, or not of its enum
        for f in &a.inputs {
            let n = f.named.alias.clone();
            let set = |c: &mut Case, v: RawValue| match c.inputs.iter_mut().find(|(k, _)| *k == n) {
                Some(x) => x.1 = v,
                None => c.inputs.push((n.clone(), v)),
            };
            match &f.ty {
                FieldType::Num { lo, hi, .. } => {
                    let (lo, hi) = (*lo, *hi);
                    add(&format!("the input {n} below its range"), &|c| set(c, RawValue::Num(lo - 1)));
                    add(&format!("the input {n} above its range"), &|c| set(c, RawValue::Num(hi + 1)));
                }
                FieldType::Enum(_) => add(&format!("the input {n} not of its enum"), &|c| set(c, RawValue::Enum(format!("{n}_unknown")))),
                FieldType::Date { lo, hi } => {
                    let (lo, hi) = (*lo, *hi);
                    add(&format!("the input {n} before its range"), &|c| set(c, RawValue::Date(lo - 1)));
                    add(&format!("the input {n} after its range"), &|c| set(c, RawValue::Date(hi + 1)));
                }
                FieldType::Bool | FieldType::Entity(_) => {}
            }
        }
        // the store: an entity it does not hold, an attribute outside its range or enum, a role of
        // another type's
        let pt = self.type_of(&base.principal.0).unwrap_or(0);
        let rt = self.type_of(&base.resource.0).unwrap_or(0);
        for owner in [Owner::Resource, Owner::Principal] {
            let (t, who) = if owner == Owner::Principal { (pt, "principal") } else { (rt, "resource") };
            if !self.reads(t) {
                continue;
            }
            let id = if owner == Owner::Principal { base.principal.1.clone() } else { base.resource.1.clone() };
            let ty = g.types[t].named.alias.clone();
            let entity = |c: &mut Case| -> Option<usize> { c.store.iter().position(|e| e.ty == ty && e.id == id) };
            add(&format!("a {who} the store does not hold"), &|c| {
                if let Some(k) = entity(c) {
                    c.store.remove(k);
                }
            });
            for i in self.store_attrs(t) {
                let f = &g.types[t].attrs[i];
                let n = f.named.alias.clone();
                let set = |c: &mut Case, v: RawValue| {
                    if let Some(k) = entity(c) {
                        match c.store[k].attrs.iter_mut().find(|(x, _)| *x == n) {
                            Some(x) => x.1 = v,
                            None => c.store[k].attrs.push((n.clone(), v)),
                        }
                    }
                };
                match &f.ty {
                    FieldType::Num { lo, hi, .. } => {
                        let (lo, hi) = (*lo, *hi);
                        add(&format!("{who}.{n} below its range"), &|c| set(c, RawValue::Num(lo - 1)));
                        add(&format!("{who}.{n} above its range"), &|c| set(c, RawValue::Num(hi + 1)));
                    }
                    FieldType::Enum(_) => add(&format!("{who}.{n} not of its enum"), &|c| set(c, RawValue::Enum(format!("{n}_unknown")))),
                    FieldType::Date { lo, hi } => {
                        let (lo, hi) = (*lo, *hi);
                        add(&format!("{who}.{n} before its range"), &|c| set(c, RawValue::Date(lo - 1)));
                        add(&format!("{who}.{n} after its range"), &|c| set(c, RawValue::Date(hi + 1)));
                    }
                    FieldType::Bool | FieldType::Entity(_) => {}
                }
            }
            if !g.types[t].roles.is_empty() {
                add(&format!("a role the {who}'s type does not hold"), &|c| {
                    if let Some(k) = entity(c) {
                        c.store[k].roles.push("role_unknown".to_string());
                    }
                });
            }
        }
        // a resource of a type the action does not take, when the input says the type
        if a.resources.len() > 1
            && let Some(other) = (0..g.types.len()).find(|t| !a.resources.contains(t))
        {
            let ty = g.types[other].named.alias.clone();
            add("a resource of a type the action does not take", &|c| c.resource.0 = ty.clone());
        }
        // a principal of a type the action does not take, and a workflow the gate does not declare
        if let Some(other) = (0..g.types.len()).find(|&t| g.types[t].kind != Kind::Resource && !a.principals.contains(&t)) {
            let ty = g.types[other].named.alias.clone();
            let id = if g.types[other].kind == Kind::Workflow { g.workflows.first().map(|w| w.named.alias.clone()).unwrap_or_default() } else { format!("{}9", ty.to_lowercase()) };
            add("a principal of a type the action does not take", &|c| c.principal = (ty.clone(), id.clone()));
        }
        if let Some(wt) = g.workflow_type().filter(|t| a.principals.contains(t)) {
            let ty = g.types[wt].named.alias.clone();
            add("a workflow the gate does not declare", &|c| c.principal = (ty.clone(), "workflow_unknown".to_string()));
        }
        out
    }
}

/// Whether an action computes a value from `today` (see [`Model::reads_today`]).
pub fn reads_today(a: &Action) -> bool {
    a.computed.iter().any(|cv| match &cv.how {
        How::Rule { args, .. } => args.iter().any(|(_, s)| *s == Source::Today),
        How::Date { .. } | How::Open { .. } => true,
    })
}

/// The attributes of type `t` the generated code reads, by index (see [`Model::store_attrs`]).
pub fn store_attrs(g: &Gate, shape: &Shape, t: usize) -> BTreeSet<usize> {
    let mut out: BTreeSet<usize> = shape.types[t].attrs.iter().copied().collect();
    for a in &g.actions {
        for cv in &a.computed {
            let read: Vec<(Owner, &String)> = match &cv.how {
                How::Rule { args, .. } | How::Date { of: DateOf::Call { args, .. }, .. } => args
                    .iter()
                    .filter_map(|(_, s)| match s {
                        Source::Attr(o, n) => Some((*o, n)),
                        _ => None,
                    })
                    .collect(),
                How::Date { of: DateOf::Attr(o, n), .. } => vec![(*o, n)],
                How::Open { .. } => Vec::new(),
            };
            for (o, n) in read {
                let types = if o == Owner::Principal { &a.principals } else { &a.resources };
                if types.contains(&t)
                    && let Some((i, _)) = g.types[t].attr(n)
                {
                    out.insert(i);
                }
            }
        }
    }
    out
}

/// Whether the generated code reads an entity of type `t` from the store (see [`Model::reads`]).
pub fn reads(g: &Gate, shape: &Shape, t: usize) -> bool {
    g.types[t].kind != Kind::Workflow && (!g.types[t].roles.is_empty() || !shape.types[t].member_of.is_empty() || !store_attrs(g, shape, t).is_empty())
}

/// The field of an action's input that holds the resource's id: the `from` argument, or
/// `resource`.
pub fn resource_field(a: &Action) -> String {
    a.from.as_ref().map(|(arg, _)| arg.clone()).unwrap_or_else(|| "resource".to_string())
}

fn attr_val_raw(g: &Gate, e: Option<&Entity>, t: usize, i: usize) -> Option<RawValue> {
    e?.attr(&g.types[t].attrs[i].named.alias).cloned()
}

/// `YYYY-MM-DD` as a day.
fn parse_day(s: &str) -> Option<i64> {
    let mut it = s.splitn(3, '-');
    let (y, m, d) = (it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next()?.parse().ok()?);
    crate::types::day(y, m, d)
}

/// How far `today` is from where the truth of `today <op> day` turns.
fn distance(op: Op, today: i64, day: i64) -> i64 {
    match op {
        // true up to day, false from the day after
        Op::Le | Op::Gt => (today - day).abs().min((today - day - 1).abs()),
        // true up to the day before, false from day
        Op::Lt | Op::Ge => (today - day + 1).abs().min((today - day).abs()),
        Op::Is => (today - day).abs(),
    }
}

/// The values a number is tried at: both ends, the middle, and one in from each end.
fn spread(lo: i128, hi: i128) -> Vec<i128> {
    let mut v = vec![lo, hi, lo + (hi - lo) / 2, lo.saturating_add(1).min(hi), hi.saturating_sub(1).max(lo)];
    let mut seen = Vec::new();
    v.retain(|x| {
        let new = !seen.contains(x);
        seen.push(*x);
        new
    });
    v
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

/// The entities' ids of one case: `user1`, `order1`, `customer2`, by type.
#[derive(Default)]
struct Ids {
    next: BTreeMap<usize, usize>,
}

impl Ids {
    fn fresh(&mut self, g: &Gate, t: usize) -> String {
        let n = self.next.entry(t).or_insert(1);
        let id = format!("{}{n}", g.types[t].named.alias.to_lowercase());
        *n += 1;
        id
    }
}

// ---------------------------------------------------------------------------------------------
// The JSON, and the answers of a harness

impl Case {
    /// The case as a harness reads it.
    pub fn json(&self, g: &Gate) -> Json {
        let a = &g.actions[self.action];
        let mut input: Vec<(String, Json)> = vec![(resource_field(a), Json::str(self.resource.1.clone()))];
        if a.resources.len() > 1 {
            input.push(("resource_type".to_string(), Json::str(self.resource.0.clone())));
        }
        input.extend(self.inputs.iter().map(|(k, v)| (k.clone(), v.json())));
        let store = self.store.iter().map(|e| {
            Json::obj([
                ("type", Json::str(e.ty.clone())),
                ("id", Json::str(e.id.clone())),
                ("roles", Json::arr(e.roles.iter().map(|r| Json::str(r.clone())))),
                ("member_of", Json::arr(e.member_of.iter().map(|(t, id)| Json::obj([("type", Json::str(t.clone())), ("id", Json::str(id.clone()))])))),
                ("attrs", Json::Obj(e.attrs.iter().map(|(k, v)| (k.clone(), v.json())).collect())),
            ])
        });
        Json::obj([
            ("name", Json::str(self.name.clone())),
            ("action", Json::str(a.named.alias.clone())),
            ("principal", Json::obj([("type", Json::str(self.principal.0.clone())), ("id", Json::str(self.principal.1.clone()))])),
            ("store", Json::arr(store)),
            ("input", Json::Obj(input)),
            ("now", Json::str(self.now.clone())),
            ("expect", self.expect.json()),
        ])
    }
}

impl Raw {
    /// The data, as the harnesses read it: the types, the actions and the cases, a case a line.
    pub fn json(&self, g: &Gate) -> String {
        let head = |k: &str| self.kinds.get(k).cloned().unwrap_or(Json::Null).compact();
        let cases: Vec<String> = self.cases.iter().map(|c| c.json(g).compact()).collect();
        format!("{{\"gate\": {},\n\"types\": {},\n\"actions\": {},\n\"cases\": [\n{}\n]}}\n", Json::str(self.gate.clone()).compact(), head("types"), head("actions"), cases.join(",\n"))
    }

    /// How many of the cases are refused, and how many allowed.
    pub fn counts(&self) -> (usize, usize) {
        (self.cases.iter().filter(|c| c.expect.error.is_some()).count(), self.cases.iter().filter(|c| c.expect.allowed).count())
    }

    /// The answers a harness printed, a JSON line a case in the order of the cases, held to the
    /// reference: Ok with the number of cases, or each case that differs (the first twenty).
    pub fn compare(&self, printed: &str) -> Result<usize, String> {
        let lines: Vec<&str> = printed.lines().filter(|l| l.trim_start().starts_with('{')).collect();
        let mut wrong: Vec<String> = Vec::new();
        if lines.len() != self.cases.len() {
            wrong.push(format!("{} answers for {} cases", lines.len(), self.cases.len()));
        }
        for (c, line) in self.cases.iter().zip(&lines) {
            let got = match ritsu_base::json::parse(line) {
                Ok(j) => j,
                Err(e) => {
                    wrong.push(format!("{}: not JSON ({}): {line}", c.name, e.message.en));
                    continue;
                }
            };
            if got.get("name").and_then(Json::as_str) != Some(c.name.as_str()) {
                wrong.push(format!("{}: the answer is for {:?}", c.name, got.get("name")));
                continue;
            }
            let allowed = got.get("allowed").and_then(Json::as_bool);
            let mut policies: Vec<String> = got.get("policies").and_then(Json::as_arr).unwrap_or(&[]).iter().filter_map(|p| p.as_str().map(str::to_string)).collect();
            policies.sort();
            let error = got.get("error").and_then(Json::as_str).map(str::to_string);
            let mut why: Vec<String> = Vec::new();
            if allowed != Some(c.expect.allowed) {
                why.push(format!("allowed {allowed:?}, not {}", c.expect.allowed));
            }
            if policies != c.expect.policies {
                why.push(format!("policies {policies:?}, not {:?}", c.expect.policies));
            }
            if error != c.expect.error {
                why.push(format!("error {error:?}, not {:?}", c.expect.error));
            }
            if let Some(want) = &c.expect.context {
                let have = got.get("context").cloned().unwrap_or(Json::Null);
                if !same_json(&have, want) {
                    why.push(format!("context {}, not {}", have.compact(), want.compact()));
                }
            }
            if !why.is_empty() {
                wrong.push(format!("{}: {}", c.name, why.join("; ")));
            }
        }
        if wrong.is_empty() {
            Ok(self.cases.len())
        } else {
            let n = wrong.len();
            wrong.truncate(20);
            Err(format!("{n} of {} answers differ from sekisho's reference evaluation:\n{}", self.cases.len(), wrong.join("\n")))
        }
    }
}

/// Whether two JSON values are the same, the keys of an object in any order.
pub fn same_json(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::Obj(x), Json::Obj(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.iter().any(|(k2, v2)| k == k2 && same_json(v, v2))),
        (Json::Arr(x), Json::Arr(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same_json(p, q)),
        _ => a == b,
    }
}
