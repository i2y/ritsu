//! What the code that builds the requests needs of a gate, whatever the language it is written in
//! (DESIGN 5.3, 5.4): the modules of the rules and the dates it calls, each action's arguments,
//! and for each value an action computes, the types it is computed for and the call that computes
//! it, its arguments resolved to where they come from (DESIGN 3.6: an attribute of the principal or
//! the resource, an input, a constant, today). The Python and the Go of [`super::python`] and
//! [`super::go`] are written from it.

use crate::cedar::{self, Shape};
use crate::checks::Checked;
use crate::model::*;
use crate::names::Scope;
use ritsu_base::text::Lang;
use ritsu_ports::{CalendarFacts, DateFacts, DateKind, RuleFacts};
use std::collections::BTreeMap;

/// Where the request is asked (`--authorizer`, DESIGN 5.5): Cedar in the process (the language's
/// implementation of Cedar), or Amazon Verified Permissions through the AWS SDK.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Authorizer {
    Cedar,
    Avp,
}

impl Authorizer {
    pub const WORDS: [&'static str; 2] = ["cedar", "avp"];

    pub fn parse(s: &str) -> Option<Authorizer> {
        match s {
            "cedar" => Some(Authorizer::Cedar),
            "avp" => Some(Authorizer::Avp),
            _ => None,
        }
    }
}

/// How a module of the code is written, besides the gate: the path its head names, the language of
/// the comments, the authorizer, and for Go the import path of the package the module is in (a
/// rule's package is `<module>/rules/<package>`, as `ritsu gen` lays a package out, DESIGN 5.6).
#[derive(Clone, Debug)]
pub struct Options {
    pub shown: String,
    pub lang: Lang,
    pub authorizer: Authorizer,
    pub go_module: String,
}

/// A module of another language the code imports: a rule's, a dates file's or a calendar's.
#[derive(Clone, Debug)]
pub enum Import {
    Rule(Box<RuleFacts>),
    Dates(DateFacts),
    Calendar(CalendarFacts),
}

/// What a module of the code is written from.
pub struct Plan<'a> {
    pub g: &'a Gate,
    pub scope: &'a Scope,
    pub shape: Shape,
    /// The modules imported, by the index of the `use` that reads each.
    pub imports: BTreeMap<usize, Import>,
    pub actions: Vec<ActionPlan>,
    /// The Cedar the code holds (DESIGN 5.6): the policies' and the schema's JSON forms, each on one
    /// line, as `gen --target cedar` writes them.
    pub policies_json: String,
    pub schema_json: String,
}

/// One action's function.
pub struct ActionPlan {
    pub index: usize,
    /// The action computes a value from today: the function reads the day of `now`.
    pub reads_today: bool,
    pub values: Vec<ValuePlan>,
}

/// One value the action computes.
pub struct ValuePlan {
    pub index: usize,
    /// The principal types and the resource types it is computed for: those that have every
    /// attribute it reads (DESIGN 3.7). Empty when it reads none of that owner's.
    pub principals: Option<Vec<usize>>,
    pub resources: Option<Vec<usize>>,
    pub call: Call,
}

/// What computes a value.
pub enum Call {
    /// A rule's output: the `use`, the output (rulec's column), its arguments in the order of the
    /// rule's inputs (None for an input that may be absent and is given nothing), and for an enum
    /// output each value's name in the rule with the string Cedar is given.
    Rule { module: usize, output: Box<ritsu_ports::Column>, args: Vec<(ritsu_ports::Column, Option<Source>)>, values: Option<Vec<(String, String)>> },
    /// `today <op> <dates>.<date>(…)`: the `use`, the date's alias, its arguments in the order the
    /// date reads its inputs, each with whether it is a day.
    Date { module: usize, function: String, op: crate::ast::Op, args: Vec<(String, bool, Source)> },
    /// `today <op> <owner>.<attribute>`.
    DateAttr { op: crate::ast::Op, owner: Owner, attr: String },
    /// `today is open in <calendar>`.
    Open { module: usize },
}

impl<'a> Plan<'a> {
    /// The plan of a file that passed its check; `calendars` holds what koyomi says of each calendar
    /// it reads, by the index of the `use`. Err when the Cedar writer does not read what it is given
    /// (a bug of sekisho's).
    pub fn new(scope: &'a Scope, checked: &'a Checked, calendars: &BTreeMap<usize, CalendarFacts>, lang: Lang) -> Result<Plan<'a>, ritsu_base::cedar::Error> {
        let g = &checked.gate;
        let shape = cedar::shape(g, scope, checked);
        let files = cedar::files(scope, checked, "", lang)?;
        let mut imports: BTreeMap<usize, Import> = BTreeMap::new();
        for (ui, u) in g.uses.iter().enumerate() {
            match u.kind {
                UseKind::Rule => {
                    if let Some(r) = scope.rules.get(&u.name) {
                        imports.insert(ui, Import::Rule(Box::new(r.facts.clone())));
                    }
                }
                UseKind::Dates => {
                    if let Some(d) = scope.dates.get(&u.name) {
                        imports.insert(ui, Import::Dates(d.facts.clone()));
                    }
                }
                UseKind::Calendar => {
                    if let Some(c) = calendars.get(&ui) {
                        imports.insert(ui, Import::Calendar(c.clone()));
                    }
                }
                _ => {}
            }
        }
        // a module no value of an action calls is not imported
        let mut called: Vec<usize> = Vec::new();
        let actions = g
            .actions
            .iter()
            .enumerate()
            .map(|(ai, a)| {
                let values: Vec<ValuePlan> = a
                    .computed
                    .iter()
                    .enumerate()
                    .filter_map(|(ci, cv)| {
                        let call = call_of(g, &imports, a, cv)?;
                        if let Call::Rule { module, .. } | Call::Date { module, .. } | Call::Open { module } = &call {
                            called.push(*module);
                        }
                        let (principals, resources) = computed_for(g, a, cv);
                        Some(ValuePlan { index: ci, principals, resources, call })
                    })
                    .collect();
                let reads_today = a.computed.iter().any(reads_today);
                ActionPlan { index: ai, reads_today, values }
            })
            .collect();
        imports.retain(|ui, _| called.contains(ui));
        Ok(Plan { g, scope, shape, imports, actions, policies_json: files.policies_json.trim_end().to_string(), schema_json: files.schema_json.trim_end().to_string() })
    }

    /// The types with what the code reads of them from the store (DESIGN 3.6): an attribute or a
    /// role, or a group it is a member of. A type with none is known by its id alone.
    pub fn reads(&self, t: usize) -> bool {
        let ty = &self.g.types[t];
        ty.kind != Kind::Workflow && (!ty.attrs.is_empty() || !ty.roles.is_empty() || !self.shape.types[t].member_of.is_empty())
    }

    /// The types the code names: every principal type and resource type an action takes.
    pub fn types_used(&self) -> Vec<usize> {
        let mut out: Vec<usize> = Vec::new();
        for a in &self.g.actions {
            for &t in a.principals.iter().chain(&a.resources) {
                if !out.contains(&t) {
                    out.push(t);
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// The Cedar name of a type: `Shop::Order`.
    pub fn cedar_type(&self, t: usize) -> String {
        cedar::type_name(self.g, &self.shape, t).to_string()
    }

    /// `Shop::Role`, `Shop::Action`.
    pub fn cedar_name(&self, id: &str) -> String {
        let ns = self.shape.namespace.to_string();
        format!("{ns}::{id}")
    }

    /// The rule's facts of a `use`.
    pub fn rule(&self, ui: usize) -> Option<&RuleFacts> {
        match self.imports.get(&ui) {
            Some(Import::Rule(r)) => Some(r),
            _ => None,
        }
    }

    /// The `use` a value's call reads, as the gate writes its path: `rulec "rules/refund_limit.rule"`.
    pub fn use_path(&self, ui: usize) -> String {
        self.g.uses.get(ui).map(|u| ritsu_base::naming::quote(&u.path)).unwrap_or_default()
    }

    /// The groups a principal of type `t` is a member of, by type (DESIGN 2.5).
    pub fn groups(&self, t: usize) -> &[usize] {
        &self.shape.types[t].member_of
    }

    /// The attributes Cedar holds of a type, in the order declared.
    pub fn cedar_attrs(&self, t: usize) -> &[usize] {
        &self.shape.types[t].attrs
    }

    /// The `today` of the gate: its range and the minutes east of UTC its day changes at.
    pub fn today(&self) -> Option<&Today> {
        self.g.today.as_ref()
    }
}

/// Whether a value reads today.
fn reads_today(cv: &Computed) -> bool {
    match &cv.how {
        How::Rule { args, .. } => args.iter().any(|(_, s)| *s == Source::Today),
        How::Date { .. } | How::Open { .. } => true,
    }
}

/// The types of the action a value is computed for, by owner: None for an owner whose attributes it
/// does not read.
fn computed_for(g: &Gate, a: &Action, cv: &Computed) -> (Option<Vec<usize>>, Option<Vec<usize>>) {
    let mut reads: Vec<(Owner, String)> = Vec::new();
    match &cv.how {
        How::Rule { args, .. } | How::Date { of: DateOf::Call { args, .. }, .. } => {
            for (_, s) in args {
                if let Source::Attr(o, n) = s {
                    reads.push((*o, n.clone()));
                }
            }
        }
        How::Date { of: DateOf::Attr(o, n), .. } => reads.push((*o, n.clone())),
        How::Open { .. } => {}
    }
    let of = |owner: Owner, types: &[usize]| -> Option<Vec<usize>> {
        let names: Vec<&String> = reads.iter().filter(|(o, _)| *o == owner).map(|(_, n)| n).collect();
        if names.is_empty() {
            return None;
        }
        Some(types.iter().copied().filter(|&t| names.iter().all(|n| g.types[t].attr(n).is_some())).collect())
    };
    (of(Owner::Principal, &a.principals), of(Owner::Resource, &a.resources))
}

/// The call that computes a value, its arguments resolved; None for a file whose ports said nothing
/// of it (which its check has said).
fn call_of(g: &Gate, imports: &BTreeMap<usize, Import>, a: &Action, cv: &Computed) -> Option<Call> {
    let _ = a;
    match &cv.how {
        How::Rule { rule, args, output } => {
            let Some(Import::Rule(facts)) = imports.get(rule) else { return None };
            let out = facts.outputs.iter().find(|o| o.name == *output || o.alias == *output)?.clone();
            let given: Vec<(ritsu_ports::Column, Option<Source>)> = facts
                .inputs
                .iter()
                .map(|col| {
                    let s = args.iter().find(|(n, _)| *n == col.name || *n == col.alias).map(|(_, s)| s.clone());
                    (col.clone(), s)
                })
                .collect();
            let values = match strip(&out.ty) {
                ritsu_ports::ColumnType::Enum(e) => facts.enums.iter().find(|x| x.name == *e).map(|x| x.values.iter().map(|v| (v.name.clone(), v.public.clone())).collect()),
                _ => None,
            };
            Some(Call::Rule { module: *rule, output: Box::new(out), args: given, values })
        }
        How::Date { op, of: DateOf::Call { dates, function, args } } => {
            let Some(Import::Dates(facts)) = imports.get(dates) else { return None };
            let f = facts.functions.iter().find(|f| f.name == *function || f.alias == *function)?;
            let mut out = Vec::new();
            for p in &f.params {
                let input = facts.inputs.iter().find(|i| i.name == *p || i.alias == *p)?;
                let s = args.iter().find(|(n, _)| *n == input.name || *n == input.alias).map(|(_, s)| s.clone())?;
                out.push((input.alias.clone(), input.kind == DateKind::Date, s));
            }
            Some(Call::Date { module: *dates, function: f.alias.clone(), op: *op, args: out })
        }
        How::Date { op, of: DateOf::Attr(o, n) } => Some(Call::DateAttr { op: *op, owner: *o, attr: n.clone() }),
        How::Open { calendar } => match imports.get(calendar) {
            Some(Import::Calendar(_)) => Some(Call::Open { module: *calendar }),
            _ => {
                let _ = g;
                None
            }
        },
    }
}

/// A column's type without its `?`.
pub fn strip(t: &ritsu_ports::ColumnType) -> &ritsu_ports::ColumnType {
    match t {
        ritsu_ports::ColumnType::Opt(x) => strip(x),
        other => other,
    }
}

/// The days since 1970-01-01 of a date, as year, month, day.
pub fn ymd(d: Day) -> (i64, i64, i64) {
    let t = ritsu_ports::day_text(d);
    let mut it = t.split('-').map(|x| x.parse::<i64>().unwrap_or(0));
    (it.next().unwrap_or(1970), it.next().unwrap_or(1), it.next().unwrap_or(1))
}

/// `refund_order` as `RefundOrder`; `RefundRecord` as it is.
pub fn pascal(alias: &str) -> String {
    ritsu_emit::ident::pascal(alias)
}

/// `RefundRecord` as `refund_record`: a type's alias as a method or a variable of Python.
pub fn snake(alias: &str) -> String {
    let mut out = String::new();
    for (i, c) in alias.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 && !out.ends_with('_') {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// A field's type, as the comment beside it says it: `money[GBP, incl_tax], 1 to 10000`,
/// `2026-01-01 to 2028-09-30`, `one of paid, shipped`, `the id of a Customer`.
pub fn field_note(g: &Gate, f: &Field, lang: Lang) -> String {
    let ja = lang == Lang::Ja;
    let text = match &f.ty {
        FieldType::Num { written, lo, hi, .. } => {
            if ja {
                format!("{written}、{lo}〜{hi}")
            } else {
                format!("{written}, {lo} to {hi}")
            }
        }
        FieldType::Date { lo, hi } => {
            let (a, b) = (ritsu_ports::day_text(*lo), ritsu_ports::day_text(*hi));
            if ja { format!("{a}〜{b}") } else { format!("{a} to {b}") }
        }
        FieldType::Enum(e) => {
            let vs: Vec<String> = g.enums[*e].values.iter().map(|v| v.alias.clone()).collect();
            if ja { format!("{} のどれか", vs.join("、")) } else { format!("one of {}", vs.join(", ")) }
        }
        FieldType::Entity(t) => {
            let n = &g.types[*t].named.alias;
            if ja { format!("{n} の ID") } else { format!("the id of a {n}") }
        }
        FieldType::Bool => String::new(),
    };
    let named = if f.named.name != f.named.alias { f.named.name.clone() } else { String::new() };
    let mut parts: Vec<String> = Vec::new();
    if !named.is_empty() {
        parts.push(named);
    }
    if !text.is_empty() {
        parts.push(text);
    }
    if f.optional {
        parts.push(if ja { "無いことがある".to_string() } else { "may be absent".to_string() });
    }
    parts.join(if ja { "。" } else { "; " })
}

/// The line `line` of the gate, as written, without its indent: what a comment of the code says a
/// computed value is (`refund_band = refund_limit(amount: amount, limit: principal.refund_limit).band`).
pub fn line_of(g: &Gate, line: usize) -> String {
    g.src.lines().nth(line.saturating_sub(1)).map(|l| l.split(" #").next().unwrap_or(l).trim().to_string()).unwrap_or_default()
}

/// What the gate says of a type: its `description`.
pub fn type_description(scope: &Scope, alias: &str) -> Option<String> {
    match scope.entity(alias) {
        Some(crate::ast::EntityRef::Declared(e)) => e.description.as_ref().map(|(d, _)| d.clone()),
        _ => None,
    }
}

/// What the gate says of an action: its `description`.
pub fn action_description(scope: &Scope, alias: &str) -> Option<String> {
    scope.file().actions.iter().find(|x| x.name.ascii() == alias).and_then(|x| x.description.as_ref().map(|(d, _)| d.clone()))
}

// ---------------------------------------------------------------------------------------------
// What the code reads, as `crate::raw` says (its order of refusals is the one the code follows)

impl Plan<'_> {
    /// Whether value `ci` of action `ai` is computed for a principal of type `pt` and a resource of
    /// type `rt`: the types have every attribute it reads.
    pub fn computable(&self, ai: usize, ci: usize, pt: usize, rt: usize) -> bool {
        let g = self.g;
        let a = &g.actions[ai];
        let cv = &a.computed[ci];
        let srcs: Vec<Source> = match &cv.how {
            How::Rule { args, .. } | How::Date { of: DateOf::Call { args, .. }, .. } => args.iter().map(|(_, s)| s.clone()).collect(),
            How::Date { of: DateOf::Attr(o, n), .. } => vec![Source::Attr(*o, n.clone())],
            How::Open { .. } => Vec::new(),
        };
        srcs.iter().all(|s| match s {
            Source::Attr(o, n) => g.owner_type(*o, pt, rt).attr(n).is_some(),
            Source::Input(n) => a.input(n).is_some(),
            Source::Lit(_) | Source::Today => true,
        })
    }

    /// The attributes of the owner's type the code reads, in a request of types `pt` and `rt`: what
    /// the store gives of an entity of the type, the same in every action (`crate::raw::store_attrs`:
    /// those Cedar holds of the type, and those a value any action computes reads).
    pub fn read_attrs(&self, _ai: usize, owner: Owner, pt: usize, rt: usize) -> Vec<usize> {
        let t = if owner == Owner::Principal { pt } else { rt };
        crate::raw::store_attrs(self.g, &self.shape, t).into_iter().collect()
    }

    /// Whether the code reads the owner's entity from the store: its type holds roles, can be a
    /// member of a group a policy asks of, or has an attribute the code reads (`crate::raw::reads`).
    pub fn reads_entity(&self, _ai: usize, owner: Owner, pt: usize, rt: usize) -> bool {
        let t = if owner == Owner::Principal { pt } else { rt };
        crate::raw::reads(self.g, &self.shape, t)
    }
}

/// The field of an action's input that holds the resource's id: the argument `from` names, or
/// `resource` for an action that names none (as `crate::raw` lays out the data).
pub fn resource_field(a: &Action) -> String {
    a.from.as_ref().map(|(arg, _)| arg.clone()).unwrap_or_else(|| "resource".to_string())
}

/// A description as a sentence: with a period at its end (a full stop for one in Japanese).
pub fn sentence(d: &str) -> String {
    let d = d.trim();
    if d.ends_with(['.', '。', '!', '?']) {
        d.to_string()
    } else if !d.is_ascii() {
        format!("{d}。")
    } else {
        format!("{d}.")
    }
}

/// The operations an action guards, as a comment of the code says them: each as `@guards` writes
/// it, a reference with its path from the root (`openapi "api/orders.json" operation
/// refundOrder`; `Action::references`); None for an action that guards none.
pub fn guarded(_g: &Gate, a: &Action, lang: Lang) -> Option<String> {
    let said: Vec<String> = a.references().iter().map(|(r, _)| r.text()).collect();
    (!said.is_empty()).then(|| said.join(if lang == Lang::Ja { "、" } else { "; " }))
}

/// Sentences one after another: with a space between them in English, none after a Japanese full
/// stop.
pub fn sentences(parts: &[String]) -> String {
    let mut out = String::new();
    for p in parts.iter().filter(|p| !p.is_empty()) {
        if !out.is_empty() && !out.ends_with('。') {
            out.push(' ');
        }
        out.push_str(p);
    }
    out
}

/// `+00:00`, `+09:00`, from minutes east of UTC.
pub fn offset_text(m: i32) -> String {
    let sign = if m < 0 { '-' } else { '+' };
    format!("{sign}{:02}:{:02}", m.abs() / 60, m.abs() % 60)
}
