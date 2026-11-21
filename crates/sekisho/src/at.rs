//! What a gate allows a workflow at one call of its flow (DESIGN 4.6, 8.3; the port
//! `Gates::allowed_at`, which ritsu's X16 asks): the action walked again over the combinations the
//! runs of the flow that reach the call can come to.
//!
//! dandori says what the call gives the operation and what those runs hold there
//! (`OperationCall`), in the flow's terms. They are read in the gate's terms three ways:
//!
//! - **an input of the action** is what the call gives the operation's argument of its name;
//! - **an attribute of the resource** is the value of the flow a `resource` line under the
//!   workflow says it is (DESIGN 2.12), where the action takes the resource's id from an argument
//!   (`resource Order from orderId`) and the call gives that argument the line's own value;
//! - **a rule's output** is what a `match` on the way to the call leaves of the answer of a call
//!   of the same rule, where every value that call gave is an input or an attribute as above, or
//!   a constant.
//!
//! A number or a date is held to the range dandori knows the value in; an enum or a bool to the
//! values the places it comes from let in, less what the arms of a `match` leave out; a value that
//! is always there is not absent. The action is then walked as the check walks it
//! ([`crate::walk`]), with the ranges narrowed and the flow's calls of rules as values the action
//! computes — a call that gives a rule the same values as a value of the gate's own is counted
//! with it, as two outputs of one call are — and the combinations that agree with what is held are
//! the ones asked of ([`crate::checks::allowance_over`]).
//!
//! What the flow holds that cannot be read so holds nothing, which leaves more combinations, never
//! fewer; it is said ([`AllowedAt::unheld`]), with the attributes of the resource the answer reads
//! that no line says a value of the flow for ([`AllowedAt::unrelated`]).

use crate::borders::Langs;
use crate::checks::Checked;
use crate::model::*;
use crate::walk::{self, Domain, Kn, Known, Memo, Slot, Stop, Val};
use ritsu_base::text::Text;
use ritsu_ports::{AllowedAt, Allowance, Asker, Carried, FlowValue, Found, OperationCall};
use std::collections::BTreeSet;
use std::path::Path as FsPath;

/// The most readings of one call of a rule that are each counted as a value of the action: a
/// value the rule was given can be an input of the action and an attribute of the resource both.
const READINGS: usize = 8;

/// One place of a frame held to some of its values.
struct Test {
    /// The resource type of the frames it holds in; None for every frame.
    on: Option<usize>,
    slot: Slot,
    allowed: Vec<Val>,
    /// Whether the place may be absent.
    absent: bool,
}

/// What a value of the flow holds a field of the gate to.
#[derive(Default)]
struct Hold {
    /// For an enum or a bool held to fewer values than it has: the values, and whether it may be
    /// absent.
    test: Option<(Vec<Val>, bool)>,
    /// The field's range or its `?` was narrowed.
    changed: bool,
    /// The value can be nothing the field takes: no combination has it.
    nothing: bool,
    /// What it is held to, in words, when that is less than the field takes.
    said: Option<Text>,
}

/// The values of a list, as a message says them: `a`, `a` or `b`.
fn or_list(items: &[String]) -> Text {
    let items: Vec<Text> = items.iter().map(|v| Text::same(format!("`{v}`"))).collect();
    match items.len() {
        0 => Text::default(),
        1 => items[0].clone(),
        _ => {
            let (last, rest) = items.split_last().expect("two or more");
            let l = Text::join(rest, "、", ", ");
            tr!("{}か{}", "{} or {}", l.ja, last.ja; l.en, last.en)
        }
    }
}

/// What `v`, a value of the flow where it makes the call, holds `field` to (`field` is narrowed
/// in place): Err with why, for a value the field cannot be read as.
fn hold(g: &Gate, field: &mut Field, v: &FlowValue, narrowed: Option<&[String]>) -> Result<Hold, Text> {
    let (inner, may_be_none) = v.holds.present();
    let can_be_none = may_be_none && narrowed.is_none_or(|n| n.iter().any(|x| x == "none"));
    let is_there = narrowed.is_none_or(|n| n.iter().any(|x| x != "none"));
    let mut out = Hold::default();
    if !is_there {
        // none on every run that reaches the call
        if field.optional {
            out.test = Some((Vec::new(), true));
            out.said = Some(tr!("無い", "absent"));
        } else {
            out.nothing = true;
        }
        return Ok(out);
    }
    // a value that is always there, where the gate lets it be absent
    let there = field.optional && !can_be_none;
    if there {
        field.optional = false;
        out.changed = true;
    }
    let absent = field.optional;
    let shown = &v.shown;
    let differ = || tr!("フローの `{shown}` は、ゲートの `{}` と型が違います", "the flow's `{shown}` is not of the type of the gate's `{}`", field.named.name);
    match (&field.ty, inner) {
        (FieldType::Num { unit, written, lo, hi }, Carried::Num { unit: Some(u), min, max, .. }) if u.same(unit) => {
            let (l, h) = (min.map_or(*lo, |m| m.max(*lo)), max.map_or(*hi, |m| m.min(*hi)));
            if l > h {
                out.nothing = true;
            } else if (l, h) != (*lo, *hi) {
                out.said = Some(crate::cells::show_cell(crate::cells::Cell { lo: l, hi: h }, unit));
                field.ty = FieldType::Num { unit: unit.clone(), written: written.clone(), lo: l, hi: h };
                out.changed = true;
            }
        }
        (FieldType::Date { lo, hi }, Carried::Date { min, max }) => {
            let (l, h) = (min.map_or(*lo, |m| m.max(*lo)), max.map_or(*hi, |m| m.min(*hi)));
            if l > h {
                out.nothing = true;
            } else if (l, h) != (*lo, *hi) {
                let (a, b) = (crate::types::day_text(l), crate::types::day_text(h));
                out.said = Some(if l == h { Text::same(a) } else { Text { ja: format!("{a}〜{b}"), en: format!("{a} to {b}") } });
                field.ty = FieldType::Date { lo: l, hi: h };
                out.changed = true;
            }
        }
        (FieldType::Enum(e), Carried::Enum { values, .. }) => {
            let en = &g.enums[*e];
            let mut vals: Vec<usize> = Vec::new();
            for s in values.iter().filter(|s| narrowed.is_none_or(|n| n.contains(&s.written))) {
                let Some(k) = en.values.iter().position(|n| n.is(&s.written) || n.is(&s.public)) else {
                    return Err(tr!("フローの `{shown}` は `{}` になりえますが、それはゲートの列挙 `{}` の値ではありません", "the flow's `{shown}` can be `{}`, which is no value of the gate's enum `{}`", s.written, en.named.name));
                };
                if !vals.contains(&k) {
                    vals.push(k);
                }
            }
            vals.sort();
            if vals.is_empty() && !absent {
                out.nothing = true;
            } else if vals.len() < en.values.len() {
                let names: Vec<String> = vals.iter().map(|k| en.values[*k].name.clone()).collect();
                out.said = Some(or_list(&names));
                out.test = Some((vals.into_iter().map(Val::Enum).collect(), absent));
            }
        }
        (FieldType::Bool, Carried::Bool) => {
            let vals: Vec<bool> = match (v.constant, narrowed) {
                (Some(b), _) => vec![b],
                (None, Some(n)) => [false, true].into_iter().filter(|b| n.contains(&b.to_string())).collect(),
                (None, None) => vec![false, true],
            };
            if vals.is_empty() && !absent {
                out.nothing = true;
            } else if vals.len() < 2 {
                out.said = Some(or_list(&vals.iter().map(|b| b.to_string()).collect::<Vec<_>>()));
                out.test = Some((vals.into_iter().map(Val::Bool).collect(), absent));
            }
        }
        _ => return Err(differ()),
    }
    if out.nothing {
        out.said = Some(tr!("ゲートの `{}` が取らない値", "no value the gate's `{}` takes", field.named.name));
    } else if there {
        out.said = Some(match out.said.take() {
            Some(s) => tr!("{}（無いことはない）", "{} (never absent)", s.ja; s.en),
            None => tr!("無いことはない", "never absent"),
        });
    }
    Ok(out)
}

/// What the answer for a workflow reads on an action: the attributes of the resource, each by its
/// type and its index there, and the inputs of the action, by index. Those the conditions of the
/// policies a workflow can meet read, those the computed values they read are given, and those an
/// action they ask (`allowed …`) is given or reads.
#[derive(Default)]
struct Reads {
    attrs: BTreeSet<(usize, usize)>,
    inputs: BTreeSet<usize>,
}

impl Reads {
    fn of(g: &Gate, action: usize) -> Reads {
        let mut out = Reads::default();
        out.action(g, action, true, &mut Vec::new());
        out
    }

    /// The attribute `name` of each resource type the action takes.
    fn attr(&mut self, g: &Gate, a: &Action, name: &str) {
        for &t in &a.resources {
            if let Some((i, _)) = g.types[t].attr(name) {
                self.attrs.insert((t, i));
            }
        }
    }

    /// What a value given to a rule, a date or an asked action is; `top` for the action the
    /// question is of, whose inputs are the ones counted.
    fn source(&mut self, g: &Gate, a: &Action, top: bool, s: &Source) {
        match s {
            Source::Attr(Owner::Resource, n) => self.attr(g, a, n),
            Source::Input(n) if top => self.inputs.extend(a.input(n).map(|(i, _)| i)),
            _ => {}
        }
    }

    fn path(&mut self, g: &Gate, a: &Action, top: bool, p: &Path) {
        match p {
            Path::Attr(Owner::Resource, n) => self.attr(g, a, n),
            Path::Attr(Owner::Principal, _) => {}
            Path::Value(n) => match a.computed_value(n) {
                Some((_, c)) => match &c.how {
                    How::Rule { args, .. } | How::Date { of: DateOf::Call { args, .. }, .. } => args.iter().for_each(|(_, s)| self.source(g, a, top, s)),
                    How::Date { of: DateOf::Attr(Owner::Resource, n), .. } => self.attr(g, a, n),
                    How::Date { .. } | How::Open { .. } => {}
                },
                None if top => self.inputs.extend(a.input(n).map(|(i, _)| i)),
                None => {}
            },
        }
    }

    fn action(&mut self, g: &Gate, action: usize, top: bool, seen: &mut Vec<usize>) {
        if seen.contains(&action) {
            return;
        }
        seen.push(action);
        let a = &g.actions[action];
        fn atoms<'e>(e: &'e Expr, out: &mut Vec<&'e Atom>) {
            match e {
                Expr::Atom(a) => out.push(a),
                Expr::Not(x) => atoms(x, out),
                Expr::And(xs) | Expr::Or(xs) => xs.iter().for_each(|x| atoms(x, out)),
            }
        }
        let mut asked: Vec<usize> = Vec::new();
        for p in g.policies.iter().filter(|p| p.actions.contains(&action) && !matches!(p.who, Who::InRoles(_))) {
            if let Who::IsType(t) = p.who
                && g.types[t].kind != Kind::Workflow
            {
                continue;
            }
            let mut found = Vec::new();
            for c in &p.conds {
                atoms(&c.expr, &mut found);
            }
            for x in found {
                match x {
                    Atom::True(p) | Atom::Is(p, _) | Atom::Cmp(p, _, _) => self.path(g, a, top, p),
                    Atom::Eq(p, q) => {
                        self.path(g, a, top, p);
                        self.path(g, a, top, q);
                    }
                    Atom::Same(m, n) => {
                        for t in [m, n] {
                            if let Term::Attr(Owner::Resource, n) = t {
                                self.attr(g, a, n);
                            }
                        }
                    }
                    Atom::Member(Term::Attr(Owner::Resource, n)) => self.attr(g, a, n),
                    Atom::Allowed { action: b, args } => {
                        // what the asked action is given, in this action's terms
                        for s in g.asked(action, *b, args).into_iter().flatten() {
                            self.source(g, a, top, &s);
                        }
                        asked.push(*b);
                    }
                    _ => {}
                }
            }
        }
        for b in asked {
            self.action(g, b, false, seen);
        }
    }
}

/// Whether two paths are one file.
fn same_file(a: &FsPath, b: &FsPath) -> bool {
    let normal = |p: &FsPath| ritsu_base::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    a == b || normal(a) == normal(b)
}

/// How far the workflow `workflow` is allowed action `ai` at `call` (what the port
/// `Gates::allowed_at` answers): `x` is the check of the gate, `langs` the languages it reads
/// (dandori among them, for the values of the flow the gate's `resource` lines name), `budget`
/// the most combinations the action is walked over.
pub fn allowed_at(x: &Checked, langs: &Langs, budget: u128, ai: usize, workflow: &str, call: &OperationCall) -> AllowedAt {
    let g = &x.gate;
    let Some(wi) = g.workflows.iter().position(|w| w.named.is(workflow)) else { return AllowedAt::of(Found::Value(Allowance::Never)) };
    // over every combination: what no narrowing changes (fewer combinations of all that are
    // allowed are all allowed, and of none, none)
    let over_all = match x.report.spaces.get(ai).and_then(|s| s.as_ref()) {
        Some(space) => crate::checks::allowance(g, space, &x.known, langs, &Asker::Workflow(workflow.to_string())),
        None => Found::Undecided(tr!("action `{}` の組み合わせを数えていません", "the combinations of the action `{}` were not walked", g.actions[ai].named.name)),
    };
    if matches!(over_all, Found::Value(Allowance::Always | Allowance::Never)) {
        return AllowedAt::of(over_all);
    }
    let mut out = at(x, langs, budget, ai, wi, call, &over_all, true);
    // an answer that is the same without what the gate says the flow holds of the resource does
    // not rest on it: what the call gives and what the flow's own values hold say it alone
    let settled = |f: &Found<Allowance>| match f {
        Found::Value(Allowance::Always) => Some(true),
        Found::Value(Allowance::Never) => Some(false),
        _ => None,
    };
    if !out.rests_on.is_empty() && settled(&out.allowance).is_some() {
        let without = at(x, langs, budget, ai, wi, call, &over_all, false);
        if settled(&without.allowance) == settled(&out.allowance) {
            out = without;
        }
    }
    // always or never: nothing more said of the flow would change it (fewer combinations of all
    // that are allowed are all allowed, and of none, none), so nothing is left unsaid
    if settled(&out.allowance).is_some() {
        out.unrelated.clear();
        out.unheld.clear();
    }
    out
}

/// [`allowed_at`] for a workflow of the gate and an action it is sometimes allowed (`over_all`,
/// over every combination), with the gate's `resource` lines under the workflow read (`lines`)
/// or left out.
#[allow(clippy::too_many_arguments)]
fn at(x: &Checked, langs: &Langs, budget: u128, ai: usize, wi: usize, call: &OperationCall, over_all: &Found<Allowance>, lines: bool) -> AllowedAt {
    let g = &x.gate;
    let a = &g.actions[ai];
    let reached = call.reached.as_ref();
    let narrowed = |path: Option<&String>| -> Option<&[String]> {
        let path = path?;
        reached?.held.iter().find(|(k, _)| k == path).map(|(_, v)| v.as_slice())
    };
    let mut g2 = g.clone();
    let mut known: Known = x.known.clone();
    let mut tests: Vec<Test> = Vec::new();
    let mut reads: Vec<Expr> = Vec::new();
    // the condition that reads each attribute and each input held, kept until what reads it is known
    let mut attr_reads: Vec<((usize, usize), Expr)> = Vec::new();
    let mut input_reads: Vec<(usize, Expr)> = Vec::new();
    let (mut changed, mut nothing) = (false, false);
    let mut out = AllowedAt::of(Found::Value(Allowance::Never));

    // what the answer reads: what is held of anything else changes nothing, and is not said
    let Reads { attrs: read, inputs: read_inputs } = Reads::of(g, ai);

    // an input of the action: what the call gives the argument of its name
    let mut given: Vec<(usize, &FlowValue)> = Vec::new();
    for (k, inp) in a.inputs.iter().enumerate() {
        let Some((_, v)) = call.args.iter().find(|(p, _)| inp.named.is(p)) else { continue };
        given.push((k, v));
        let name = &inp.named.name;
        match hold(g, &mut g2.actions[ai].inputs[k], v, narrowed(v.path.as_ref())) {
            Ok(h) => {
                changed |= h.changed;
                nothing |= h.nothing;
                if let Some((allowed, absent)) = h.test {
                    input_reads.push((k, read_of(Path::Value(name.clone()), &allowed, g, &inp.ty)));
                    tests.push(Test { on: None, slot: Slot::Input(k), allowed, absent });
                }
                // said where the answer reads the input, or no value is one the gate takes
                if let (Some(said), true) = (h.said, h.nothing || read_inputs.contains(&k)) {
                    out.held.push(tr!("`{name}`（呼び出しが渡す `{}`）は {}", "`{name}` (what the call gives, `{}`) is {}", v.shown, said.ja; v.shown, said.en));
                }
            }
            Err(why) => out.unheld.push(why),
        }
    }

    // an attribute of the resource: the value of the flow a `resource` line under the workflow
    // says it is, where the call gives the line's own value as the resource's id
    let id = a.from.as_ref().and_then(|(arg, _)| call.args.iter().find(|(p, _)| p == arg)).and_then(|(_, v)| v.path.as_ref());
    // (the type, the attribute, the value of the flow)
    let mut related: Vec<(usize, usize, String)> = Vec::new();
    // the types a `resource` line says of at this call: what it leaves out, the flow holds nothing of
    let mut said_of: Vec<usize> = Vec::new();
    if lines {
        // a `resource` line of a type the action takes that says nothing of this call
        let given = a.from.as_ref().and_then(|(arg, _)| call.args.iter().find(|(p, _)| p == arg)).map(|(_, v)| v.shown.clone());
        for h in g.workflows[wi].holds.iter().filter(|h| a.resources.contains(&h.resource) && id != Some(&h.id)) {
            let (head, line) = (h.head_text(g), h.line);
            out.unheld.push(match (&a.from, &given) {
                (Some(_), Some(shown)) => tr!(
                    "`{head}`（{line} 行目）は、この呼び出しには当たりません。呼び出しが resource の ID として渡すのは `{shown}` です",
                    "`{head}` (line {line}) says nothing of this call, which gives `{shown}` as the resource's id"
                ),
                _ => tr!(
                    "`{head}`（{line} 行目）は、この呼び出しには当たりません。action `{}` が、どの引数が resource の ID かを書いていません（`resource <型> from <引数>`）",
                    "`{head}` (line {line}) says nothing of this call: the action `{}` does not say which argument is the resource's id (`resource <type> from <argument>`)",
                    a.named.name
                ),
            });
        }
    }
    if let (true, Some(id), Some((flows, ports)), Some(reached)) = (lines, id, &langs.flows, reached) {
        let rel = g.from_root(&g.workflows[wi].file);
        for h in g.workflows[wi].holds.iter().filter(|h| a.resources.contains(&h.resource) && h.id == *id) {
            said_of.push(h.resource);
            for (k, (i, path, line)) in h.attrs.iter().enumerate() {
                let written = h.line_text(g, k);
                let root_var = path.split('.').next().unwrap_or(path);
                if !reached.set.iter().any(|v| v == root_var) {
                    out.unheld.push(tr!(
                        "`{written}`（{line} 行目）は数えていません：フローがこの呼び出しまで進んでも、`{root_var}` に値が入っていないことがあります",
                        "`{written}` (line {line}) is not counted: `{root_var}` may have no value yet where the flow makes this call"
                    ));
                    continue;
                }
                // what the runs that reach the call last put in it, else what every value put in it
                // anywhere in the flow can be
                let here = reached.values.iter().find(|(p, _)| p == path).map(|(_, v)| v.clone());
                let Some(v) = here.or_else(|| match rel.as_ref().map(|rel| flows.value(&g.root, rel, ports, path)) {
                    Some(Ok(Some(v))) => Some(v),
                    _ => None,
                }) else {
                    continue;
                };
                let attr = &g.types[h.resource].attrs[*i];
                let name = &attr.named.name;
                match hold(g, &mut g2.types[h.resource].attrs[*i], &v, narrowed(Some(path))) {
                    Ok(hd) => {
                        changed |= hd.changed;
                        // no value the attribute takes: no combination, where the action takes
                        // resources of this type only
                        nothing |= hd.nothing && read.contains(&(h.resource, *i)) && a.resources.len() == 1;
                        if let Some((allowed, absent)) = hd.test {
                            attr_reads.push(((h.resource, *i), read_of(Path::Attr(Owner::Resource, name.clone()), &allowed, g, &attr.ty)));
                            tests.push(Test { on: Some(h.resource), slot: Slot::Attr(Owner::Resource, *i), allowed, absent });
                        }
                        if let (Some(said), true) = (hd.said, read.contains(&(h.resource, *i))) {
                            out.held.push(tr!("`resource.{name}`（フローの `{path}`）は {}", "`resource.{name}` (the flow's `{path}`) is {}", said.ja; said.en));
                        }
                        related.push((h.resource, *i, path.clone()));
                        if read.contains(&(h.resource, *i)) {
                            let head = h.head_text(g);
                            out.rests_on.push((tr!("`{head}` の `{written}`", "`{written}` under `{head}`"), *line));
                        }
                    }
                    Err(why) => out.unheld.push(tr!("`{written}`（{line} 行目）は数えていません：{}", "`{written}` (line {line}) is not counted: {}", why.ja; why.en)),
                }
            }
        }
    }
    for &(t, i) in &read {
        // an attribute that points to an entity is none a line can say a value of the flow for
        // (E102): the answer over it is what the gate says, with nothing left unsaid
        if !said_of.contains(&t) && !matches!(g.types[t].attrs[i].ty, FieldType::Entity(_)) {
            let place = format!("resource.{}", g.types[t].attrs[i].named.name);
            if !out.unrelated.contains(&place) {
                out.unrelated.push(place);
            }
        }
    }

    // a rule's output: what a `match` on the way leaves of the answer of a call of the same rule
    if let (Some(reached), Some(rules)) = (reached, langs.rules) {
        for (var, calls) in &reached.answers {
            for (key, values) in reached.held.iter().filter(|(k, _)| k.strip_prefix(var.as_str()).is_some_and(|rest| rest.starts_with('.') && !rest[1..].contains('.'))) {
                let output = &key[var.len() + 1..];
                let [c] = calls.as_slice() else {
                    out.unheld.push(tr!(
                        "フローの `{key}` を `match` で絞った分は数えていません：この呼び出しまでに、`{var}` に答えを入れる規則の呼び出しが二つ以上あります",
                        "what a `match` leaves of the flow's `{key}` is not counted: more than one call of a rule can have given `{var}` its answer by this call"
                    ));
                    continue;
                };
                // the `use rule` of the same file, or the file as one more
                let ui = match g2.uses.iter().position(|u| u.kind == UseKind::Rule && same_file(&u.file, &c.rule)) {
                    Some(ui) => ui,
                    None => {
                        g2.uses.push(Use { kind: UseKind::Rule, name: String::new(), path: c.rule.to_string_lossy().to_string(), file: c.rule.clone(), line: 0 });
                        g2.uses.len() - 1
                    }
                };
                let file = g2.uses[ui].file.clone();
                let Ok(facts) = rules.facts(&file) else { continue };
                // each value the call gave, in the gate's terms, every way it reads (a value of
                // the flow can be an input of the action and, by a `resource` line, an attribute
                // too); the input by the spelling a value of the gate's own that calls the same
                // rule gives it by, so that the two are one call
                let own: Vec<&Vec<(String, Source)>> = a.computed.iter().filter_map(|c| match &c.how { How::Rule { rule, args, .. } if *rule == ui => Some(args), _ => None }).collect();
                let column = |word: &str| facts.inputs.iter().find(|col| col.name == word || col.alias == word);
                let mut readings: Vec<(String, Vec<(Source, Option<usize>)>)> = Vec::new();
                // a value the rule was given that the gate has no word for: as the flow shows
                // it, and whether the flow still holds it where it makes the call
                let mut lost: Option<(String, bool)> = None;
                // the first such value the walk reads by what the flow says it can be, and what g2
                // held before any was added
                let mut free: Option<(String, bool)> = None;
                let before = (g2.actions[ai].inputs.len(), g2.enums.len());
                // one value, as the gate writes it
                let literal = |v: &FlowValue, col: Option<&ritsu_ports::ColumnType>| -> Option<Literal> {
                    match (v.constant, v.holds.present().0, col) {
                        (Some(b), _, _) => Some(Literal::Bool(b)),
                        (None, Carried::Enum { values, .. }, _) if values.len() == 1 => Some(Literal::Word(values[0].written.clone())),
                        // a whole number of the input's unit, written as the gate writes one (the
                        // value the flow passes is that integer of the input's unit, as dandori's
                        // check holds the call to the rule's type); on a rate in steps, that many
                        // steps (`5` of `rate[step 0.1%]` is `0.5%`), as the code passes it
                        (None, Carried::Num { min: Some(lo), max: Some(hi), .. }, Some(ritsu_ports::ColumnType::Num { unit: Some(unit), .. })) if lo == hi => {
                            let (value, word) = match (&unit.dim, unit.step) {
                                (ritsu_units::Dim::Rate, Some(step)) => (ritsu_units::Rat::int(*lo).mul(step).mul(ritsu_units::Rat::int(100)), "%"),
                                (ritsu_units::Dim::Number | ritsu_units::Dim::Count(_), _) => (ritsu_units::Rat::int(*lo), ""),
                                _ => (ritsu_units::Rat::int(*lo), unit.unit.as_str()),
                            };
                            Some(Literal::Num(crate::ast::Num { value, unit: word.to_string(), raw: format!("{value}{word}") }))
                        }
                        _ => None,
                    }
                };
                for (input, v) in &c.inputs {
                    let spelled = column(input).and_then(|col| own.iter().flat_map(|args| args.iter()).map(|(n, _)| n).find(|n| column(n).is_some_and(|c2| c2.name == col.name))).cloned().unwrap_or_else(|| input.clone());
                    let mut candidates: Vec<(Source, Option<usize>)> = Vec::new();
                    if let Some(p) = &v.path {
                        for (t, i, _) in related.iter().filter(|(_, _, q)| q == p) {
                            candidates.push((Source::Attr(Owner::Resource, g.types[*t].attrs[*i].named.name.clone()), Some(*t)));
                        }
                        for (k, _) in given.iter().filter(|(_, x)| x.path.as_ref() == Some(p)) {
                            candidates.push((Source::Input(a.inputs[*k].named.name.clone()), None));
                        }
                    }
                    // a value the gate has no word for: one value as written, or what the flow says
                    // it can be where it called the rule (dandori's range of it: a number with both
                    // ends, a date with both, an enum's values, a bool), as an input of the action
                    // the gate does not have, free over that
                    if candidates.is_empty() {
                        candidates.extend(literal(v, column(input).map(|col| &col.ty)).map(|l| (Source::Lit(l), None)));
                    }
                    if candidates.is_empty()
                        && let Some(name) = free_input(&mut g2, ai, v)
                    {
                        free.get_or_insert((v.shown.clone(), v.path.is_some()));
                        candidates.push((Source::Input(name), None));
                    }
                    if candidates.is_empty() {
                        lost = Some((v.shown.clone(), v.path.is_some()));
                        break;
                    }
                    readings.push((spelled, candidates));
                }
                let line = c.line;
                // every reading of the call, each a value the action computes; past a few, the
                // first reading of each input only
                let mut calls_read: Vec<(Vec<(String, Source)>, Option<usize>)> = vec![(Vec::new(), None)];
                let few = readings.iter().map(|(_, c)| c.len()).product::<usize>() <= READINGS;
                for (spelled, candidates) in &readings {
                    let taken = if few { &candidates[..] } else { &candidates[..1] };
                    calls_read = calls_read.iter().flat_map(|(args, on)| taken.iter().map(move |(s, t)| (args.iter().cloned().chain([(spelled.clone(), s.clone())]).collect(), on.or(*t)))).collect();
                }
                // a reading none of whose values the answer reads says nothing of the answer
                calls_read.retain(|(args, _)| {
                    args.iter().any(|(_, s)| match s {
                        Source::Attr(Owner::Resource, n) => a.resources.iter().any(|&t| g.types[t].attr(n).is_some_and(|(i, _)| read.contains(&(t, i)))),
                        Source::Input(n) => a.input(n).is_some_and(|(k, _)| read_inputs.contains(&k)),
                        _ => false,
                    })
                });
                // a value read by what the flow says it can be, where nothing else the call was
                // given is read: the answer says nothing of the request, as before that was read
                if lost.is_none() && calls_read.is_empty() {
                    lost = free.take();
                }
                if lost.is_some() || calls_read.is_empty() {
                    g2.actions[ai].inputs.truncate(before.0);
                    g2.enums.truncate(before.1);
                }
                if let Some((shown, held)) = lost {
                    out.unheld.push(if held {
                        tr!(
                            "フローの `{key}`（{line} 行目の規則の呼び出しの答え）を `match` で絞った分は数えていません：規則に渡した `{shown}` は、action のどの input でもなく、`workflow` の下の `resource` の行も、どの属性の値とも書いていません",
                            "what a `match` leaves of the flow's `{key}` (the answer of the call of the rule on line {line}) is not counted: `{shown}`, which the rule was given, is no input of the action, and no `resource` line under the workflow says an attribute is it"
                        )
                    } else {
                        tr!(
                            "フローの `{key}`（{line} 行目の規則の呼び出しの答え）を `match` で絞った分は数えていません：規則に渡した `{shown}` は、この呼び出しまでに別の値が入ることがあるか、入力・変数・定数のどれでもありません",
                            "what a `match` leaves of the flow's `{key}` (the answer of the call of the rule on line {line}) is not counted: `{shown}`, which the rule was given, can be given another value on the way to this call, or is no input, variable or constant"
                        )
                    });
                    continue;
                }
                for (n, (args, on)) in calls_read.into_iter().enumerate() {
                    // the name no word of a gate is: the flow's, and which reading
                    let name = if n == 0 { key.clone() } else { format!("{key}#{}", n + 1) };
                    let ci = g2.actions[ai].computed.len();
                    g2.actions[ai].computed.push(Computed { named: Named::new(&name, &name, 0), how: How::Rule { rule: ui, args: args.clone(), output: output.to_string() } });
                    let mut said = Vec::new();
                    let kn = crate::borders::rule_value(&g2, &g2.actions[ai], &g2.actions[ai].computed[ci], ui, &facts, &args, output, langs, &mut said);
                    let Some(kn) = kn else {
                        g2.actions[ai].computed.pop();
                        let why = said.first().map(|d| d.message.clone()).unwrap_or_default();
                        out.unheld.push(tr!(
                            "フローの `{key}`（{line} 行目の規則の呼び出しの答え）を `match` で絞った分は数えていません：{}",
                            "what a `match` leaves of the flow's `{key}` (the answer of the call of the rule on line {line}) is not counted: {}",
                            why.ja; why.en
                        ));
                        continue;
                    };
                    let Kn::Rule { domain, .. } = &kn else { unreachable!("a rule's value") };
                    let allowed: Vec<Val> = match domain {
                        Domain::Bool => [false, true].into_iter().filter(|b| values.contains(&b.to_string())).map(Val::Bool).collect(),
                        Domain::Enum(vs) => vs.iter().enumerate().filter(|(_, n)| values.iter().any(|v| n.is(v))).map(|(k, _)| Val::Enum(k)).collect(),
                    };
                    // every value of the answer: the `match` leaves nothing out, and nothing is held
                    if allowed.len() == domain.vals().len() {
                        g2.actions[ai].computed.pop();
                        continue;
                    }
                    reads.push(match domain {
                        Domain::Bool => Expr::Atom(Atom::True(Path::Value(name.clone()))),
                        Domain::Enum(vs) => Expr::Atom(Atom::Is(Path::Value(name.clone()), Literal::Word(vs.first().map(|n| n.name.clone()).unwrap_or_default()))),
                    });
                    known.computed.insert((ai, ci), kn);
                    let rule = if g2.uses[ui].name.is_empty() { facts.rule.clone() } else { g2.uses[ui].name.clone() };
                    let shown_args = args.iter().map(|(n, s)| format!("{n}: {}", crate::borders::source_text(s))).collect::<Vec<_>>().join(", ");
                    let l = or_list(values);
                    out.held.push(tr!("`{rule}({shown_args}).{output}`（フローの `{key}`）は {}", "`{rule}({shown_args}).{output}` (the flow's `{key}`) is {}", l.ja; l.en));
                    tests.push(Test { on, slot: Slot::Computed(ci), allowed, absent: false });
                    changed = true;
                }
            }
        }
    }

    if nothing {
        // a value no combination has: no request of the kind the call makes is allowed
        return out;
    }
    // an attribute or an input held is read where a policy reads it or a value is computed from
    // it: elsewhere no combination differs by it
    let given_to_rules: Vec<&Source> = g2.actions[ai].computed.iter().filter(|c| c.of_flow()).filter_map(|c| match &c.how { How::Rule { args, .. } => Some(args), _ => None }).flatten().map(|(_, s)| s).collect();
    for ((t, i), e) in attr_reads {
        let field = &g.types[t].attrs[i].named;
        if read.contains(&(t, i)) || given_to_rules.iter().any(|s| matches!(s, Source::Attr(Owner::Resource, n) if field.is(n))) {
            reads.push(e);
        }
    }
    for (k, e) in input_reads {
        let field = &a.inputs[k].named;
        if read_inputs.contains(&k) || given_to_rules.iter().any(|s| matches!(s, Source::Input(n) if field.is(n))) {
            reads.push(e);
        }
    }
    if tests.is_empty() && !changed {
        out.allowance = over_all.clone();
        out.rests_on.clear();
        return out;
    }
    // what is held is read, so that the walk counts it: an expectation no check sees
    if !reads.is_empty() {
        g2.expects.push(Expect { name: String::new(), line: 0, allow: true, who: Who::Anyone, actions: vec![ai], conds: vec![Cond { when: true, expr: Expr::Or(reads), line: 0, text: String::new() }] });
    }
    let space = match walk::space(&g2, ai, &known, langs, &Memo::default(), budget) {
        Ok(s) => s,
        Err(Stop::Budget { .. }) => {
            out.allowance = Found::Undecided(tr!("呼び出しで起こりうる組み合わせが、数える上限を超えます", "the combinations the call can come to are more than the budget"));
            return out;
        }
        Err(Stop::Unknown) => {
            out.allowance = Found::Undecided(tr!("計算した値のうち、ほかの言語が答えなかったものがあります", "a value the action computes is one no language answered for"));
            return out;
        }
    };
    let (g2, tests) = (&g2, &tests);
    out.allowance = crate::checks::allowance_over(g2, &space, &known, langs, &|frame, env| {
        let f = &space.frames[frame];
        g2.types[f.principal].kind == Kind::Workflow
            && f.place(&Slot::Workflow).is_none_or(|at| env[at] == Val::Workflow(wi))
            && tests.iter().filter(|t| t.on.is_none_or(|rt| rt == f.resource)).all(|t| match f.place(&t.slot).map(|at| &env[at]) {
                None => true,
                Some(Val::Absent) => t.absent,
                Some(v) => t.allowed.contains(v),
            })
    });
    out
}

/// An input of the action `ai` of `g2` that no word of the gate is, for a value `v` the flow gave
/// a rule that no input, attribute or constant of the gate is: free over what dandori says it can
/// be where the rule was called (a number with both ends of its range, a date with both, the
/// values of an enum, a bool). The walk asks the rule's answers over it, as over any input. None
/// where the flow does not say: a number or a date with an end not known, a value of another type.
fn free_input(g2: &mut Gate, ai: usize, v: &FlowValue) -> Option<String> {
    let (inner, may_be_none) = v.holds.present();
    let ty = match inner {
        Carried::Num { unit: Some(u), written, min: Some(lo), max: Some(hi) } => FieldType::Num { unit: u.clone(), written: written.clone(), lo: *lo, hi: *hi },
        Carried::Date { min: Some(lo), max: Some(hi) } => FieldType::Date { lo: *lo, hi: *hi },
        Carried::Bool => FieldType::Bool,
        Carried::Enum { name, values } if !values.is_empty() => {
            // the values the flow says, by the rule's names (the flow's are the rule's)
            let k = g2.enums.len();
            g2.enums.push(Enum { named: Named::new(name, "", 0), values: values.iter().map(|s| Named::new(&s.written, "", 0)).collect(), from: None, one: k });
            FieldType::Enum(k)
        }
        _ => return None,
    };
    // the flow's name of it, unless the action has an input of that name
    let mut name = v.shown.clone();
    while g2.actions[ai].input(&name).is_some() {
        name.push('\'');
    }
    g2.actions[ai].inputs.push(Field { named: Named::new(&name, "", 0), ty, optional: may_be_none });
    Some(name)
}

/// A condition that reads a place held to `allowed`, so that the walk counts the place.
fn read_of(path: Path, allowed: &[Val], g: &Gate, ty: &FieldType) -> Expr {
    match (ty, allowed.first()) {
        (FieldType::Enum(e), Some(Val::Enum(k))) => Expr::Atom(Atom::Is(path, Literal::Word(g.enums[*e].values[*k].name.clone()))),
        (FieldType::Enum(e), _) => Expr::Atom(Atom::Is(path, Literal::Word(g.enums[*e].values.first().map(|n| n.name.clone()).unwrap_or_default()))),
        _ => Expr::Atom(Atom::True(path)),
    }
}
