//! The borders with the other languages (DESIGN 3.2, 3.3, 8.1): what a `.gate` reads of rulec,
//! koyomi and dandori, checked through their ports.
//!
//! - E201: a calendar or a book the file reads does not pass koyomi's or chobo's check, or cannot
//!   be read (the rules and the dates files are `names.rs`'s, which asks their facts first); or a
//!   rule, a dates file, a calendar or a gate it reads is outside the root (DESIGN 2.6: what a gate
//!   reads is named from the root; the contracts and the books are `contracts.rs`'s). A file
//!   outside the root is not read.
//! - E101, E102, E105 of what only the other language knows (a rule's output or input, a date of
//!   a dates file and its inputs): `names.rs` says them, and a file whose names hold has none. They
//!   are said here too, for a reading of the facts that would differ from the names', rather than
//!   leave the computed value out of the walk without a word.
//! - E206: a value given to a rule or a date can be outside the input's range — for an input of
//!   the rule that holds its ordered enum to a range, a value the range does not let in, said in
//!   the enum's values — or can break a precondition of the rule (rulec answers, with an example);
//!   W201 when rulec cannot decide.
//! - E207: a calendar `today is open in` reads does not cover every day of `today`.
//! - E208: a workflow's `.flow` does not pass dandori's check, cannot be read, or is outside the
//!   root.
//! - E212: what a `resource` line under a workflow says the flow holds (DESIGN 2.12) is not what
//!   the flow holds: it has no such value, the value is of another type or unit than the
//!   attribute (both said here), or it can be outside what the attribute takes at a call the line
//!   speaks of ([`values_at_calls`], once the contracts are read and what each action guards is
//!   known).
//!
//! What the languages answer is gathered as [`Known`], what the walk reads of each computed value.

use crate::diag::Diag;
use crate::model::*;
use crate::names::{Spot, foreign_notes, of_the_dates_file, of_the_rule};
use crate::walk::{Domain, Kn, Known};
use ritsu_base::text::Text;
use ritsu_ports::{Answer, ColumnType, DateKind, Dates, Flows, Ports, Precondition, RuleFacts, Rules, Said, Value};
use std::collections::BTreeMap;
use std::rc::Rc;

/// The languages the borders are checked through; None for one that is not joined (the binary of
/// sekisho's own crate, which says so itself, E209).
#[derive(Clone, Default)]
pub struct Langs<'a> {
    pub rules: Option<&'a dyn Rules>,
    pub dates: Option<&'a dyn Dates>,
    pub books: Option<&'a dyn ritsu_ports::Books>,
    /// dandori, which says whether a `.flow` passes its check, with the ports it reads a flow with.
    pub flows: Option<(&'a dyn Flows, Ports)>,
}

fn at(code: &'static str, g: &Gate, line: usize, message: Text) -> Diag {
    Diag::at(code, &g.file, line, 1, message).source(&g.src)
}

/// A file the gate reads outside the root (E201 for a `use`, E208 for a workflow's `.flow`): the
/// generated Cedar's `@doc` and the other languages name it by its path from the root, and from
/// outside there is none.
fn outside_the_root(code: &'static str, g: &Gate, line: usize, written: &str) -> Diag {
    at(code, g, line, tr!("`{written}` はルートの外にあります", "`{written}` is outside the root")).note(crate::contracts::outside_note(tr!(
        "gate が読むファイルは、生成する Cedar の `@doc` でも、ほかの言語からも、ルートからのパスで参照します。",
        "A file a gate reads is named by a reference whose path is from the root, in the `@doc` of the generated Cedar and by the other languages."
    ), &g.root))
}

/// `d` with `notes`, in order.
fn with_notes(d: Diag, notes: Vec<Text>) -> Diag {
    notes.into_iter().fold(d, Diag::note)
}

/// What a language said of a file, one note a finding.
fn said_notes(mut d: Diag, said: &[Said]) -> Diag {
    for s in said {
        let place = match s.line {
            Some(l) => format!("{}:{l}", s.file),
            None => s.file.clone(),
        };
        let code = if s.code.is_empty() { String::new() } else { format!("[{}] ", s.code) };
        d = d.note(Text { ja: format!("{code}{place}: {}", s.message.ja), en: format!("{code}{place}: {}", s.message.en) });
    }
    d
}

/// What a value given to a rule or a date is, in this file: its kind, its unit and its range (on the
/// wire; days for a date).
#[derive(Clone, Debug, PartialEq)]
enum Given {
    Num { unit: Option<ritsu_units::Unit>, lo: i128, hi: i128, shown: String },
    Bool,
    Enum(usize),
    Date { lo: i64, hi: i64 },
    /// A word, which only an enum of the rule can make a value of.
    Word(String),
    /// What the file has no type for (an attribute none of the types has; `names.rs` says why).
    Unknown,
}

/// The types an attribute of an owner has, over the types of the action that the owner can be.
fn attr_given(g: &Gate, a: &Action, o: Owner, name: &str) -> Given {
    let types = match o {
        Owner::Principal => &a.principals,
        Owner::Resource => &a.resources,
    };
    let mut found: Option<Given> = None;
    for &t in types {
        if let Some((_, f)) = g.types[t].attr(name) {
            let gv = field_given(f, &format!("{}.{name}", o.word()));
            match &found {
                None => found = Some(gv),
                // two types with the attribute: the range that covers both
                Some(Given::Num { unit, lo, hi, shown }) => {
                    if let Given::Num { lo: l2, hi: h2, .. } = gv {
                        found = Some(Given::Num { unit: unit.clone(), lo: (*lo).min(l2), hi: (*hi).max(h2), shown: shown.clone() });
                    }
                }
                Some(Given::Date { lo, hi }) => {
                    if let Given::Date { lo: l2, hi: h2 } = gv {
                        found = Some(Given::Date { lo: (*lo).min(l2), hi: (*hi).max(h2) });
                    }
                }
                Some(_) => {}
            }
        }
    }
    found.unwrap_or(Given::Unknown)
}

fn field_given(f: &Field, shown: &str) -> Given {
    match &f.ty {
        FieldType::Num { unit, lo, hi, .. } => Given::Num { unit: Some(unit.clone()), lo: *lo, hi: *hi, shown: shown.to_string() },
        FieldType::Bool => Given::Bool,
        FieldType::Enum(e) => Given::Enum(*e),
        FieldType::Date { lo, hi } => Given::Date { lo: *lo, hi: *hi },
        FieldType::Entity(_) => Given::Unknown,
    }
}

fn given(g: &Gate, a: &Action, src: &Source) -> Given {
    match src {
        Source::Attr(o, n) => attr_given(g, a, *o, n),
        Source::Input(n) => a.input(n).map(|(_, f)| field_given(f, n)).unwrap_or(Given::Unknown),
        Source::Lit(Literal::Num(n)) => Given::Num { unit: None, lo: 0, hi: 0, shown: n.raw.clone() },
        Source::Lit(Literal::Bool(_)) => Given::Bool,
        Source::Lit(Literal::Date(d)) => Given::Date { lo: *d, hi: *d },
        Source::Lit(Literal::Word(w)) => Given::Word(w.clone()),
        Source::Today => match &g.today {
            Some(t) => Given::Date { lo: t.lo, hi: t.hi },
            None => Given::Unknown,
        },
    }
}

pub(crate) fn source_text(src: &Source) -> String {
    match src {
        Source::Attr(o, n) => format!("{}.{n}", o.word()),
        Source::Input(n) => n.clone(),
        Source::Lit(Literal::Num(n)) => n.raw.clone(),
        Source::Lit(Literal::Bool(b)) => b.to_string(),
        Source::Lit(Literal::Date(d)) => crate::types::day_text(*d),
        Source::Lit(Literal::Word(w)) => w.clone(),
        Source::Today => "today".into(),
    }
}

fn show_num(v: i128, unit: Option<&ritsu_units::Unit>) -> String {
    match unit {
        Some(u) => crate::cells::show(v, u),
        None => v.to_string(),
    }
}

/// The checks of the borders, and what the languages answered.
pub fn check(g: &Gate, langs: &Langs) -> (Vec<Diag>, Known) {
    let mut diags = Vec::new();
    let mut known = Known::default();
    // each file read once: its facts, or what its language said
    let mut rules: BTreeMap<usize, Option<RuleFacts>> = BTreeMap::new();
    let mut dates: BTreeMap<usize, Option<ritsu_ports::DateFacts>> = BTreeMap::new();
    let mut cals: BTreeMap<usize, Option<ritsu_ports::CalendarFacts>> = BTreeMap::new();
    for (ui, u) in g.uses.iter().enumerate() {
        // a file outside the root is not read: E201 here, or of a contract or a book in `contracts.rs`
        if g.from_root(&u.file).is_none() {
            if matches!(u.kind, UseKind::Rule | UseKind::Dates | UseKind::Calendar | UseKind::Gate) {
                diags.push(outside_the_root("E201", g, u.line, &u.path));
            }
            continue;
        }
        let unreadable = |said: Vec<Said>, lang: &str| {
            let d = at("E201", g, u.line, tr!("`{}` は {lang} の検査を通らないか、読めません", "`{}` does not pass {lang}'s check, or cannot be read", u.path));
            let d = said_notes(d, &said).note(tr!("そのファイルを {lang} の検査が通るように直してください。", "Fix the file until {lang}'s check passes."));
            crate::names::with_outside(d, &g.file, &u.path, &u.file)
        };
        match u.kind {
            UseKind::Rule => {
                let Some(r) = langs.rules.filter(|r| r.joined()) else { continue };
                match r.facts(&u.file) {
                    Ok(f) => {
                        rules.insert(ui, Some(f));
                    }
                    Err(said) => {
                        diags.push(unreadable(said, "rulec"));
                        rules.insert(ui, None);
                    }
                }
            }
            UseKind::Dates => {
                let Some(d) = langs.dates.filter(|d| d.joined()) else { continue };
                match d.facts(&u.file) {
                    Ok(f) => {
                        dates.insert(ui, Some(f));
                    }
                    Err(said) => {
                        diags.push(unreadable(said, "koyomi"));
                        dates.insert(ui, None);
                    }
                }
            }
            UseKind::Calendar => {
                let Some(d) = langs.dates.filter(|d| d.joined()) else { continue };
                match d.calendar(&u.file) {
                    Ok(f) => {
                        cals.insert(ui, Some(f));
                    }
                    Err(said) => {
                        diags.push(unreadable(said, "koyomi"));
                        cals.insert(ui, None);
                    }
                }
            }
            UseKind::Book => {
                let Some(b) = langs.books.filter(|b| b.joined()) else { continue };
                if let Err(said) = b.facts(&u.file) {
                    diags.push(unreadable(said, "chobo"));
                }
            }
            UseKind::OpenApi | UseKind::Proto | UseKind::AsyncApi | UseKind::Gate => {}
        }
    }

    for (ai, a) in g.actions.iter().enumerate() {
        for (ci, c) in a.computed.iter().enumerate() {
            match &c.how {
                How::Rule { rule, args, output } => {
                    let Some(Some(facts)) = rules.get(rule) else { continue };
                    if let Some(kn) = rule_value(g, a, c, *rule, facts, args, output, langs, &mut diags) {
                        known.computed.insert((ai, ci), kn);
                    }
                }
                How::Date { of: DateOf::Call { dates: du, function, args }, .. } => {
                    let Some(Some(facts)) = dates.get(du) else { continue };
                    if let Some(kn) = date_value(g, a, c, *du, facts, function, args, &mut diags) {
                        known.computed.insert((ai, ci), kn);
                    }
                }
                How::Date { of: DateOf::Attr(o, n), .. } => match attr_given(g, a, *o, n) {
                    Given::Date { .. } => {
                        known.computed.insert((ai, ci), Kn::Attr);
                    }
                    Given::Unknown => {}
                    _ => diags.push(at("E102", g, c.named.line, tr!("`{}.{n}` は日付ではないので、today と比べられません", "`{}.{n}` is not a date, and cannot be compared with today", o.word(); o.word()))),
                },
                How::Open { calendar } => {
                    let Some(Some(cal)) = cals.get(calendar) else { continue };
                    if let Some(t) = &g.today
                        && (cal.data.0 > t.lo || cal.data.1 < t.hi)
                    {
                        let (dl, dh) = (crate::types::day_text(cal.data.0), crate::types::day_text(cal.data.1));
                        let (tl, th) = (crate::types::day_text(t.lo), crate::types::day_text(t.hi));
                        let u = &g.uses[*calendar];
                        diags.push(
                            at("E207", g, c.named.line, tr!("カレンダー `{}` のデータは {dl}〜{dh} の日しか知らず、today の {tl}〜{th} を覆いません", "The calendar `{}` knows the days {dl}..{dh} only, which do not cover today's {tl}..{th}", u.path))
                                .note(tr!(
                                    "today の範囲をデータの範囲の中に狭めるか、新しいデータが出てからカレンダーのコピーを取り直してください。",
                                    "Narrow today's range to the data's, or take the calendar's copy again when newer data is out."
                                )),
                        );
                        continue;
                    }
                    known.computed.insert((ai, ci), Kn::Open { data: cal.data, closed: Rc::new(cal.closed.clone()) });
                }
            }
        }
    }

    // the workflows: each `.flow` under the root, and passing dandori's check
    for w in &g.workflows {
        if g.from_root(&w.file).is_none() {
            diags.push(outside_the_root("E208", g, w.named.line, &w.flow));
        }
    }
    if let Some((flows, ports)) = &langs.flows {
        for w in g.workflows.iter().filter(|w| g.from_root(&w.file).is_some()) {
            if let Err(said) = flows.crossings(&w.file, ports) {
                let d = at("E208", g, w.named.line, tr!("`{}` は dandori の検査を通らないか、読めません", "`{}` does not pass dandori's check, or cannot be read", w.flow));
                let d = said_notes(d, &said).note(tr!("ワークフローを dandori の検査が通るように直してください。", "Fix the workflow until dandori's check passes."));
                diags.push(crate::names::with_outside(d, &g.file, &w.flow, &w.file));
                continue;
            }
            held_by_the_flow(g, w, *flows, ports, &mut diags);
        }
    }
    (diags, known)
}

/// E212: each `resource` line under a workflow, and each line under it, against what dandori says
/// the flow holds (DESIGN 2.12): a value the flow has, of the attribute's type and unit. Whether
/// the value can be outside what the attribute takes is a question of the calls the line speaks of
/// ([`values_at_calls`]).
fn held_by_the_flow(g: &Gate, w: &Workflow, flows: &dyn Flows, ports: &Ports, diags: &mut Vec<Diag>) {
    let Some(rel) = g.from_root(&w.file) else { return };
    let flow = &w.flow;
    let names = tr!(
        "`workflow` の下の `resource` の行には、フローの入力か変数、またはそのフィールド（`order.total`）を書きます。",
        "A `resource` line under a workflow names an input or a variable of the flow, or a field of one (`order.total`)."
    );
    let value = |path: &str, line: usize, diags: &mut Vec<Diag>| -> Option<ritsu_ports::FlowValue> {
        match flows.value(&g.root, &rel, ports, path) {
            Ok(Some(v)) => Some(v),
            Ok(None) => {
                diags.push(at("E212", g, line, tr!("フロー `{flow}` に値 `{path}` はありません", "The flow `{flow}` has no value `{path}`")).note(names.clone()).note(flow_names(flows, g, &rel, ports, path)));
                None
            }
            // a flow that does not pass is E208's
            Err(_) => None,
        }
    };
    for h in &w.holds {
        value(&h.id, h.line, diags);
        let ty = &g.types[h.resource];
        for (a, path, line) in &h.attrs {
            let Some(v) = value(path, *line, diags) else { continue };
            let attr = &ty.attrs[*a];
            if !kinds_differ(attr, &v.holds) {
                continue;
            }
            if let Some((message, notes)) = flow_fits(g, attr, path, &v.holds) {
                diags.push(with_notes(at("E212", g, *line, message), notes));
            }
        }
    }
}

/// Whether an attribute and a value of a flow are of different types or units (E212 says it of the
/// line, whatever the value is).
fn kinds_differ(attr: &Field, holds: &ritsu_ports::Carried) -> bool {
    use ritsu_ports::Carried;
    match (&attr.ty, holds.present().0) {
        (FieldType::Bool, Carried::Bool) | (FieldType::Enum(_), Carried::Enum { .. }) | (FieldType::Date { .. }, Carried::Date { .. }) | (FieldType::Entity(_), _) => false,
        (FieldType::Num { unit, .. }, Carried::Num { unit: have, .. }) => !have.as_ref().is_some_and(|u| u.same(unit)),
        _ => true,
    }
}

/// E212, of what a value of a flow can be: each line under a `resource` line under a workflow
/// (DESIGN 2.12), at each call the line speaks of — a call of an operation an action of the gate
/// guards, where the action takes a resource of the line's type from an argument (`resource Order
/// from orderId`) and the call gives that argument the line's own value — with what the runs of the
/// flow that reach the call last put in the value, less what the arms of a `match` on the way leave
/// out (dandori's `OperationCall`). The line is said of every call that gives the id, so a value
/// put in on a way that reaches none of them is not counted. A line no call speaks of says nothing
/// to check, and where dandori cannot say the calls, the value is read as every value put in it
/// anywhere in the flow. Read once the contracts are, for what each action guards.
pub fn values_at_calls(g: &Gate, langs: &Langs) -> Vec<Diag> {
    let mut diags = Vec::new();
    let Some((flows, ports)) = &langs.flows else { return diags };
    for w in &g.workflows {
        let Some(rel) = g.from_root(&w.file) else { continue };
        // a flow that does not pass is E208's
        if flows.crossings(&w.file, ports).is_err() {
            continue;
        }
        let calls = flows.operation_calls(&g.root, &rel, ports).ok().map(|(_, cs)| cs);
        for h in &w.holds {
            let ty = &g.types[h.resource];
            // the calls the line speaks of
            let speaks: Option<Vec<&ritsu_ports::OperationCall>> = calls.as_ref().map(|cs| {
                cs.iter()
                    .filter(|c| {
                        let Some(a) = g.actions.iter().find(|a| a.references().iter().any(|(r, _)| *r == c.operation)) else { return false };
                        let Some((arg, _)) = &a.from else { return false };
                        a.resources.contains(&h.resource) && c.args.iter().any(|(p, v)| p == arg && v.path.as_deref() == Some(h.id.as_str()))
                    })
                    .collect()
            });
            // W202: a line no call speaks of, of a value the flow has (else E212 says it)
            if let (Some(speaks), Some(calls)) = (&speaks, &calls) {
                if speaks.is_empty() && matches!(flows.value(&g.root, &rel, ports, &h.id), Ok(Some(_))) {
                    if let Some(d) = spoken_of_by_no_call(g, w, h, calls) {
                        diags.push(d);
                    }
                }
            }
            for (a, path, line) in &h.attrs {
                let Ok(Some(v)) = flows.value(&g.root, &rel, ports, path) else { continue };
                let attr = &ty.attrs[*a];
                // said with the line already
                if kinds_differ(attr, &v.holds) {
                    continue;
                }
                let Some(speaks) = &speaks else {
                    if let Some((message, notes)) = flow_fits(g, attr, path, &v.holds) {
                        diags.push(with_notes(at("E212", g, *line, message), notes));
                    }
                    continue;
                };
                // over every value put in it, it fits: so it does at each call
                if flow_fits(g, attr, path, &v.holds).is_none() {
                    continue;
                }
                let mut first: Option<(Text, Vec<Text>)> = None;
                let mut outside: Vec<&ritsu_ports::OperationCall> = Vec::new();
                for c in speaks {
                    let here = c.reached.as_ref().and_then(|r| r.values.iter().find(|(p, _)| p == path)).map(|(_, x)| x.holds.clone()).unwrap_or_else(|| v.holds.clone());
                    let left = c.reached.as_ref().and_then(|r| r.held.iter().find(|(k, _)| k == path)).map(|(_, vals)| vals.as_slice());
                    if let Some(said) = flow_fits(g, attr, path, &narrowed(&here, left)) {
                        first.get_or_insert(said);
                        outside.push(c);
                    }
                }
                let Some((message, mut notes)) = first else { continue };
                // each call, as the flow's line and the task it calls
                let each: Vec<Text> = outside.iter().map(|c| tr!("{} 行目の `{}`", "`{}` on line {}", c.line, c.task; c.task, c.line)).collect();
                let l = Text::list(&each);
                let at_calls = if outside.len() == 1 {
                    tr!("フローの {} の呼び出しに、そういう値が届きます。", "Such a value reaches the call of {} in the flow.", l.ja; l.en)
                } else {
                    tr!("フローの {} の呼び出しに、そういう値が届きます。", "Such a value reaches the calls of {} in the flow.", l.ja; l.en)
                };
                // after the example, where there is one (a number or a date), and before what to do
                let place = if notes.len() >= 2 { 1 } else { 0 };
                notes.insert(place, at_calls);
                diags.push(with_notes(at("E212", g, *line, message), notes));
            }
        }
    }
    diags
}

/// W202: a `resource` line under a workflow that no call of the flow speaks of (one that gives the
/// line's `from` value as the id of a resource of its type, to an operation an action of the gate
/// guards, [`values_at_calls`]), with why: no action takes a resource of the line's type from an
/// argument (the type is written wrong, the line is of no use, or the action that takes one does not
/// say which argument gives its id), the flow calls none of the
/// operations the actions that do guard (the line is under the wrong workflow, or of no use), or
/// its calls of them give the argument another value (the value is written wrong). None where an
/// action that takes one guards an operation the contracts did not resolve (E202 says it), which
/// leaves the calls of it unknown.
fn spoken_of_by_no_call(g: &Gate, w: &Workflow, h: &Holds, calls: &[ritsu_ports::OperationCall]) -> Option<Diag> {
    let ty = &g.types[h.resource].named.name;
    let id = &h.id;
    let flow = &w.flow;
    let takers: Vec<&Action> = g.actions.iter().filter(|a| a.from.is_some() && a.resources.contains(&h.resource)).collect();
    if takers.iter().any(|a| a.guards.iter().any(|x| x.reference.is_none())) {
        return None;
    }
    let mut notes = vec![tr!(
        "`ritsu check` がこの行を前提にするのは、`{id}` を ID として渡す呼び出しだけです（E212 と、ゲートが呼び出しを許すかの確かめ）。この行は、フローのどの呼び出しの確かめにも効きません。",
        "`ritsu check` rests on the line only at a call that gives `{id}` as the id (E212, and whether the gate allows the call): it holds at no call of the flow."
    )];
    // actions that take one, but say of no argument that it gives the id
    let untold: Vec<&Action> = g.actions.iter().filter(|a| a.from.is_none() && a.resources.contains(&h.resource)).collect();
    if takers.is_empty() && !untold.is_empty() {
        let named = Text::list(&untold.iter().map(|a| tr!("`{}`", "`{}`", a.named.name)).collect::<Vec<_>>());
        notes.push(if untold.len() == 1 {
            tr!(
                "{} は `{ty}` を受け取りますが、どの引数が resource の ID かを書いていません（`resource {ty} from <引数>`）。",
                "{} takes a `{ty}`, but does not say which argument gives its id (`resource {ty} from <argument>`).",
                named.ja;
                named.en
            )
        } else {
            tr!(
                "{} は `{ty}` を受け取りますが、どの引数が resource の ID かを書いていません（`resource {ty} from <引数>`）。",
                "{} take a `{ty}`, but do not say which argument gives its id (`resource {ty} from <argument>`).",
                named.ja;
                named.en
            )
        });
        notes.push(tr!(
            "action に、resource の ID を渡す引数を書いてください（`resource {ty} from <引数>`）。",
            "Write which argument of the action gives the resource's id: `resource {ty} from <argument>`."
        ));
    } else if takers.is_empty() {
        let others: Vec<Text> = g
            .actions
            .iter()
            .filter_map(|a| {
                let (arg, _) = a.from.as_ref()?;
                let types = Text::list(&a.resources.iter().map(|r| tr!("`{}`", "`{}`", g.types[*r].named.name)).collect::<Vec<_>>());
                Some(tr!("`{}` は {} を `{arg}` から", "`{}` takes {} from `{arg}`", a.named.name, types.ja; a.named.name, types.en))
            })
            .collect();
        notes.push(if others.is_empty() {
            tr!("このゲートの action は、どれも resource を引数から受け取りません（`resource … from …`）。", "No action of the gate takes its resource from an argument (`resource … from …`).")
        } else {
            let l = Text::list(&others);
            tr!("`{ty}` を引数から受け取る action がありません。{}受け取ります。", "No action of the gate takes a `{ty}` from an argument: {}.", l.ja; l.en)
        });
        notes.push(tr!("action が受け取る resource の型を書くか、行を消してください。", "Write the type of resource an action takes, or take the line out."));
    } else {
        let guarded: Vec<(_, &Action)> = takers.iter().flat_map(|a| a.references().into_iter().map(move |(n, _)| (n, *a))).collect();
        let theirs: Vec<(&ritsu_ports::OperationCall, &Action)> = calls.iter().filter_map(|c| guarded.iter().find(|(n, _)| *n == c.operation).map(|(_, a)| (c, *a))).collect();
        let named = Text::list(&takers.iter().map(|a| tr!("`{}`", "`{}`", a.named.name)).collect::<Vec<_>>());
        if theirs.is_empty() {
            notes.push(tr!(
                "フローは、`{ty}` を受け取る action（{}）が守る操作を、どれも呼びません。",
                "The flow calls none of the operations the actions that take a `{ty}` guard ({}).",
                named.ja;
                named.en
            ));
            notes.push(tr!("その操作を呼ぶワークフローの下に書くか、行を消してください。", "Write the line under the workflow that calls them, or take it out."));
        } else {
            let mut given: Vec<String> = Vec::new();
            let each: Vec<Text> = theirs
                .iter()
                .map(|(c, a)| {
                    let arg = a.from.as_ref().map(|(x, _)| x.as_str()).unwrap_or_default();
                    match c.args.iter().find(|(p, _)| p == arg) {
                        Some((_, v)) => {
                            if let Some(p) = &v.path {
                                if !given.contains(p) {
                                    given.push(p.clone());
                                }
                            }
                            tr!("{} 行目の `{}` は `{arg}` に `{}`", "`{}` on line {} gives `{}` as `{arg}`", c.line, c.task, v.shown; c.task, c.line, v.shown)
                        }
                        None => tr!("{} 行目の `{}` は `{arg}` を渡さない", "`{}` on line {} gives no `{arg}`", c.line, c.task; c.task, c.line),
                    }
                })
                .collect();
            let l = Text::list(&each);
            notes.push(tr!(
                "`{ty}` を受け取る action（{}）が守る操作の呼び出しは、ID の引数に別の値を渡します（{}）。",
                "The calls of the operations the actions that take a `{ty}` guard ({}) give the id another value ({}).",
                named.ja,
                l.ja;
                named.en,
                l.en
            ));
            notes.push(match given.as_slice() {
                [one] => tr!("呼び出しが ID として渡す値を書いてください（`resource {ty} from {one}`）。", "Write the value the calls give as the id: `resource {ty} from {one}`."),
                _ => tr!("呼び出しが ID として渡す値を書いてください。", "Write the value the calls give as the id."),
            });
        }
    }
    let name = &w.named.name;
    let message = tr!(
        "この行が当たる呼び出しがありません。ワークフロー `{name}`（`{flow}`）のどの呼び出しも、`{id}` を `{ty}` の ID として渡しません",
        "The line speaks of no call: no call of the workflow `{name}` (`{flow}`) gives `{id}` as the id of a `{ty}`"
    );
    Some(with_notes(at("W202", g, h.line, message), notes))
}

/// What a value of a flow holds at a call where the arms of a `match` on the way leave `left` of it
/// (`none`, `some` and the values of an enum, as the arms write them).
fn narrowed(holds: &ritsu_ports::Carried, left: Option<&[String]>) -> ritsu_ports::Carried {
    use ritsu_ports::Carried;
    let Some(left) = left else { return holds.clone() };
    match holds {
        Carried::Opt(t) if !left.iter().any(|v| v == "none") => narrowed(t, Some(left)),
        Carried::Opt(t) => Carried::Opt(Box::new(narrowed(t, Some(left)))),
        Carried::Enum { name, values } => Carried::Enum { name: name.clone(), values: values.iter().filter(|v| left.contains(&v.written)).cloned().collect() },
        c => c.clone(),
    }
}

/// What can be written where `path` is no value of the flow (E212): the fields of the longest
/// part of it that is a value, else the flow's inputs and variables.
fn flow_names(flows: &dyn Flows, g: &Gate, rel: &str, ports: &Ports, path: &str) -> Text {
    let quoted = |names: Vec<String>| Text::list(&names.iter().map(|n| Text::same(format!("`{n}`"))).collect::<Vec<Text>>());
    let mut under = path;
    while let Some((head, _)) = under.rsplit_once('.') {
        under = head;
        if !matches!(flows.value(&g.root, rel, ports, under), Ok(Some(_))) {
            continue;
        }
        let fields = flows.value_names(&g.root, rel, ports, under).unwrap_or_default();
        if fields.is_empty() {
            return tr!("フローの `{under}` にフィールドはありません。", "The flow's `{under}` has no fields.");
        }
        let l = quoted(fields);
        return tr!("フローの `{under}` のフィールドは {} です。", "The fields of the flow's `{under}` are {}.", l.ja; l.en);
    }
    let names = flows.value_names(&g.root, rel, ports, "").unwrap_or_default();
    if names.is_empty() {
        return tr!("フローには、入力も変数もありません。", "The flow has no inputs and no variables.");
    }
    let l = quoted(names);
    tr!("フローの入力と変数は {} です。", "The flow's inputs and variables are {}.", l.ja; l.en)
}

/// How a message names what a value of a flow holds.
fn carried_text(c: &ritsu_ports::Carried) -> Text {
    use ritsu_ports::Carried;
    match c {
        Carried::Bool => tr!("真偽", "a bool"),
        Carried::Str => tr!("文字列", "a string"),
        Carried::Date { .. } | Carried::Days(_) => tr!("日付", "a date"),
        Carried::Enum { name, .. } => tr!("列挙 `{name}`", "the enum `{name}`"),
        Carried::Num { written, .. } => Text::same(format!("`{written}`")),
        Carried::Opt(t) => {
            let t = carried_text(t);
            tr!("値が無いことのある{}", "{}, which may be none", t.ja; t.en)
        }
        Carried::Other(t) => Text::same(format!("`{t}`")),
    }
}

/// How a message names what an attribute takes.
fn field_text(g: &Gate, f: &Field) -> Text {
    match &f.ty {
        FieldType::Bool => tr!("真偽", "a bool"),
        FieldType::Enum(e) => tr!("列挙 `{}`", "the enum `{}`", g.enums[*e].named.name),
        FieldType::Num { written, .. } => Text::same(format!("`{written}`")),
        FieldType::Date { .. } => tr!("日付", "a date"),
        FieldType::Entity(t) => tr!("`{}` のエンティティ", "an entity of `{}`", g.types[*t].named.name),
    }
}

/// Whether an attribute can be what a value of a flow holds (E212): None when it can, else the
/// message and its notes. The value is of the attribute's type and unit; it is always there
/// unless the attribute may be absent; and every value it can be is one the attribute takes.
fn flow_fits(g: &Gate, attr: &Field, path: &str, holds: &ritsu_ports::Carried) -> Option<(Text, Vec<Text>)> {
    use ritsu_ports::Carried;
    let name = &attr.named.name;
    let (inner, may_be_none) = holds.present();
    if may_be_none && !attr.optional {
        return Some((
            tr!("フローの `{path}` は値が無いことがありますが、属性 `{name}` にはいつも値があります", "The flow's `{path}` may be none, and the attribute `{name}` always has a value"),
            vec![tr!(
                "resource に値が無いことがあるなら、属性を `{name} : <型>?` と書いてください。無ければ、いつも値のあるフローの値を書いてください。",
                "If the resource may have none, write the attribute as `{name} : <type>?`; else name a value of the flow that is always there."
            )],
        ));
    }
    let differ = || {
        let (have, want) = (carried_text(inner), field_text(g, attr));
        Some((tr!("フローの `{path}` は{}で、属性 `{name}` は{}です", "The flow's `{path}` is {}, and the attribute `{name}` is {}", have.ja, want.ja; have.en, want.en), Vec::new()))
    };
    let narrow = tr!(
        "フローの値がとりうるものを属性の範囲の中に狭めるか（フローの入力やレコードのフィールドの `range`）、属性の範囲を広げてください。",
        "Narrow what the flow's value can be to what the attribute takes (the `range` of the flow's input or of the record's field), or widen the attribute's range."
    );
    match (&attr.ty, inner) {
        (FieldType::Bool, Carried::Bool) => None,
        (FieldType::Enum(e), Carried::Enum { values, .. }) => {
            let en = &g.enums[*e];
            let stray = values.iter().find(|v| !en.values.iter().any(|n| n.is(&v.written) || n.is(&v.public)))?;
            let v = &stray.written;
            let known: Vec<Text> = en.values.iter().map(|n| Text::same(format!("`{}`", n.name))).collect();
            let l = Text::list(&known);
            Some((
                tr!("フローの `{path}` は `{v}` になりえますが、それは属性 `{name}` の列挙 `{}` の値ではありません", "The flow's `{path}` can be `{v}`, which is no value of `{}`, the enum of the attribute `{name}`", en.named.name),
                vec![tr!("`{}` の値は {} です。", "The values of `{}` are {}.", en.named.name, l.ja; en.named.name, l.en)],
            ))
        }
        (FieldType::Num { unit, lo, hi, .. }, Carried::Num { unit: have, min, max, .. }) => {
            if !have.as_ref().is_some_and(|u| u.same(unit)) {
                return differ();
            }
            let (x, y) = (show_num(*lo, Some(unit)), show_num(*hi, Some(unit)));
            let out = match (min, max) {
                (Some(m), _) if m < lo => Some(Ok(*m)),
                (_, Some(m)) if m > hi => Some(Ok(*m)),
                (None, _) => Some(Err(tr!("フローは、`{path}` がどこまで小さくなりうるかを言っていません。", "Nothing in the flow says how small `{path}` can be."))),
                (_, None) => Some(Err(tr!("フローは、`{path}` がどこまで大きくなりうるかを言っていません。", "Nothing in the flow says how large `{path}` can be."))),
                _ => None,
            }?;
            let why = match out {
                Ok(v) => {
                    let ex = show_num(v, Some(unit));
                    tr!("例：`{path}` が {ex} のとき。", "For example: `{path}` at {ex}.")
                }
                Err(open) => open,
            };
            Some((tr!("フローの `{path}` は、属性 `{name}` の範囲（{x}〜{y}）の外になりえます", "The flow's `{path}` can be outside the range of the attribute `{name}` ({x}..{y})"), vec![why, narrow]))
        }
        (FieldType::Date { lo, hi }, Carried::Date { min, max }) => {
            let (x, y) = (crate::types::day_text(*lo), crate::types::day_text(*hi));
            let out = match (min, max) {
                (Some(m), _) if m < lo => Some(Ok(*m)),
                (_, Some(m)) if m > hi => Some(Ok(*m)),
                (None, _) => Some(Err(tr!("フローは、`{path}` がどこまで前の日になりうるかを言っていません。", "Nothing in the flow says how early `{path}` can be."))),
                (_, None) => Some(Err(tr!("フローは、`{path}` がどこまで後の日になりうるかを言っていません。", "Nothing in the flow says how late `{path}` can be."))),
                _ => None,
            }?;
            let why = match out {
                Ok(d) => {
                    let ex = crate::types::day_text(d);
                    tr!("例：`{path}` が {ex} のとき。", "For example: `{path}` at {ex}.")
                }
                Err(open) => open,
            };
            Some((tr!("フローの `{path}` は、属性 `{name}` の範囲（{x}〜{y}）の外になりえます", "The flow's `{path}` can be outside the range of the attribute `{name}` ({x}..{y})"), vec![why, narrow]))
        }
        // `names.rs` has said an attribute that points to an entity (E102)
        (FieldType::Entity(_), _) => None,
        _ => differ(),
    }
}

/// The column of a rule's input or output that `word` names: by its name or its alias.
fn column<'a>(cols: &'a [ritsu_ports::Column], word: &str) -> Option<&'a ritsu_ports::Column> {
    cols.iter().find(|c| c.name == word || c.alias == word)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn rule_value(g: &Gate, a: &Action, c: &Computed, rule: usize, facts: &RuleFacts, args: &[(String, Source)], output: &str, langs: &Langs, diags: &mut Vec<Diag>) -> Option<Kn> {
    let u = &g.uses[rule];
    let line = c.named.line;
    let before = diags.len();
    // the output: an enum or a bool of the rule
    let domain = match column(&facts.outputs, output) {
        None => {
            let names: Vec<Vec<&str>> = facts.outputs.iter().map(|o| vec![o.name.as_str(), o.alias.as_str()]).collect();
            let d = at("E101", g, line, tr!("規則 `{}` に出力 `{output}` はありません", "The rule `{}` has no output `{output}`", facts.rule; facts.rule));
            diags.push(with_notes(d, foreign_notes(&tr!("出力", "outputs"), &of_the_rule(), &names, Spot::Name)));
            None
        }
        Some(col) => match &col.ty {
            ColumnType::Bool => Some(Domain::Bool),
            // each value by the rule's name of it, and by its public name: the alias the `.rule` writes
            // in parentheses (`上限まで(within_limit)`), what Cedar is given and what rulec answers
            // `outputs_over` with
            ColumnType::Enum { name: e, .. } => facts.enums.iter().find(|x| x.name == *e).map(|x| Domain::Enum(x.values.iter().map(|v| Named::new(&v.name, &v.public, 0)).collect())),
            ColumnType::Opt(inner) if matches!(**inner, ColumnType::Bool | ColumnType::Enum { .. }) => {
                diags.push(at("E105", g, line, tr!("規則 `{}` の出力 `{output}` は値が無いことがあり、条件に使えません", "The output `{output}` of the rule `{}` may have no value, and a condition cannot read it", facts.rule; facts.rule)));
                None
            }
            _ => {
                diags.push(at("E105", g, line, tr!("規則 `{}` の出力 `{output}` は列挙でも真偽でもないので、計算した値にできません", "The output `{output}` of the rule `{}` is neither an enum nor a bool, so it cannot be a computed value", facts.rule; facts.rule)).note(tr!(
                    "列挙か真偽の出力を使ってください。数を区間に分ける出力は、まだ書けません。",
                    "Use an output that is an enum or a bool; a numeric output cannot be used yet."
                )));
                None
            }
        },
    };
    // the inputs: each given, each of the type and unit, each within the range
    let mut ranges: Vec<(String, Option<i128>, Option<i128>)> = Vec::new();
    let mut inputs: Vec<(String, Option<ritsu_units::Unit>)> = Vec::new();
    for (name, src) in args {
        let Some(col) = column(&facts.inputs, name) else {
            let names: Vec<Vec<&str>> = facts.inputs.iter().map(|i| vec![i.name.as_str(), i.alias.as_str()]).collect();
            let d = at("E101", g, line, tr!("規則 `{}` に入力 `{name}` はありません", "The rule `{}` has no input `{name}`", facts.rule; facts.rule));
            diags.push(with_notes(d, foreign_notes(&tr!("入力", "inputs"), &of_the_rule(), &names, Spot::Name)));
            continue;
        };
        let (ty, optional) = match &col.ty {
            ColumnType::Opt(inner) => (inner.as_ref().clone(), true),
            t => (t.clone(), false),
        };
        let _ = optional;
        let gv = given(g, a, src);
        let shown = source_text(src);
        let mismatch = |what: Text| at("E102", g, line, tr!("規則 `{}` の入力 `{name}` に `{shown}` は渡せません。{}", "`{shown}` cannot be given to the input `{name}` of the rule `{}`: {}", facts.rule, what.ja; facts.rule, what.en));
        match (&ty, &gv) {
            (_, Given::Unknown) => {}
            (ColumnType::Num { unit: Some(want), min, max, .. }, Given::Num { unit: Some(have), lo, hi, .. }) => {
                if !want.same(have) {
                    diags.push(mismatch(tr!("単位が {have} で、入力は {want} です", "it is {have}, and the input is {want}")));
                    continue;
                }
                inputs.push((col.name.clone(), Some(want.clone())));
                ranges.push((col.name.clone(), Some(*lo), Some(*hi)));
                outside(g, line, &facts.rule, name, &shown, (*lo, *hi), (*min, *max), Some(want), diags);
            }
            (ColumnType::Num { unit: Some(want), min, max, .. }, Given::Num { unit: None, .. }) => {
                let Source::Lit(Literal::Num(n)) = src else { continue };
                match crate::types::count(n, want) {
                    Ok(k) => {
                        inputs.push((col.name.clone(), Some(want.clone())));
                        ranges.push((col.name.clone(), Some(k), Some(k)));
                        outside(g, line, &facts.rule, name, &shown, (k, k), (*min, *max), Some(want), diags);
                    }
                    Err(m) => diags.push(at(m.code(), g, line, tr!("`{shown}` は規則 `{}` の入力 `{name}`（{want}）の値になりません", "`{shown}` is not a value of the input `{name}` of the rule `{}` ({want})", facts.rule; facts.rule))),
                }
            }
            (ColumnType::Num { unit: None, min, max, .. }, Given::Num { lo, hi, unit, .. }) => {
                inputs.push((col.name.clone(), unit.clone()));
                ranges.push((col.name.clone(), Some(*lo), Some(*hi)));
                outside(g, line, &facts.rule, name, &shown, (*lo, *hi), (*min, *max), unit.as_ref(), diags);
            }
            (ColumnType::Bool, Given::Bool) => inputs.push((col.name.clone(), None)),
            (ColumnType::Date { min, max }, Given::Date { lo, hi }) => {
                inputs.push((col.name.clone(), None));
                // rulec takes the range of a date as day numbers, and its column carries the range
                ranges.push((col.name.clone(), Some(*lo as i128), Some(*hi as i128)));
                if min.is_some_and(|m| *lo < m) || max.is_some_and(|m| *hi > m) {
                    let (x, y) = (min.map(crate::types::day_text).unwrap_or_default(), max.map(crate::types::day_text).unwrap_or_default());
                    diags.push(at("E206", g, line, tr!("`{shown}` は規則 `{}` の入力 `{name}` の範囲（{x}〜{y}）の外になりえます", "`{shown}` can be outside the range of the input `{name}` of the rule `{}` ({x}..{y})", facts.rule; facts.rule)));
                }
            }
            (ColumnType::Enum { name: e, range }, Given::Enum(ge)) => {
                let rv: Vec<String> = facts.enums.iter().find(|x| x.name == *e).map(|x| x.values.iter().flat_map(|v| [v.name.clone(), v.alias.clone(), v.public.clone()]).collect()).unwrap_or_default();
                let missing: Vec<Text> = g.enums[*ge].values.iter().filter(|v| !rv.contains(&v.name) && !rv.contains(&v.alias)).map(|v| Text::same(v.name.clone())).collect();
                if missing.is_empty() {
                    inputs.push((col.name.clone(), None));
                    // the values of the gate's enum the input's range does not let in, in the
                    // gate's order, each by the gate's name of it
                    if let Some(takes) = range {
                        let out: Vec<String> = g.enums[*ge].values.iter().filter(|v| enum_value_named(facts, e, &[&v.name, &v.alias]).is_some_and(|x| !takes.contains(&x))).map(|v| v.name.clone()).collect();
                        outside_values(g, line, &facts.rule, name, &shown, takes, &out, false, diags);
                    }
                } else {
                    let l = Text::list(&missing);
                    diags.push(mismatch(tr!("{} は規則の列挙 `{e}` の値にありません", "{} are not values of the rule's enum `{e}`", l.ja; l.en)));
                }
            }
            (ColumnType::Enum { name: e, range }, Given::Word(w)) => {
                match enum_value_named(facts, e, &[w]) {
                    Some(x) => {
                        inputs.push((col.name.clone(), None));
                        if let Some(takes) = range
                            && !takes.contains(&x)
                        {
                            outside_values(g, line, &facts.rule, name, &shown, takes, std::slice::from_ref(w), true, diags);
                        }
                    }
                    None => diags.push(at("E101", g, line, tr!("`{w}` は規則 `{}` の列挙 `{e}` の値ではありません", "`{w}` is not a value of the enum `{e}` of the rule `{}`", facts.rule; facts.rule))),
                }
            }
            (want, _) => diags.push(mismatch(tr!("型が違います（入力は {}）", "it is not of the input's type ({})", column_type(want); column_type(want)))),
        }
    }
    // every input given
    let missing: Vec<Text> = facts.inputs.iter().filter(|i| !args.iter().any(|(n, _)| *n == i.name || *n == i.alias)).map(|i| Text::same(i.name.clone())).collect();
    if !missing.is_empty() {
        let l = Text::list(&missing);
        diags.push(at("E105", g, line, tr!("規則 `{}` の入力 {} が渡されていません", "The rule `{}` is not given its input {}", facts.rule, l.ja; facts.rule, l.en)).note(tr!(
            "どの入力にも、principal か resource の属性、input、定数のどれかを渡してください。",
            "Give every input an attribute of the principal or the resource, an input, or a constant."
        )));
    }
    if diags.len() > before {
        return None;
    }
    // the preconditions of the rule, over the ranges given (a precondition that can break leaves
    // the value out of the walk, as a range that can be crossed does)
    let mut broken = false;
    if let Some(r) = langs.rules
        && let Ok(answers) = r.preconditions_hold(&u.file, &ranges, None, None)
    {
        for (p, ans) in answers {
            match ans {
                Answer::Holds => {}
                Answer::Fails(vals) => {
                    broken = true;
                    let ex = values_text(&vals, facts);
                    diags.push(at("E206", g, line, tr!("規則 `{}` に渡す値が、前提 {} を破りえます", "The values given to the rule `{}` can break its precondition {}", facts.rule, precondition_text(&p); facts.rule, precondition_text(&p))).note(tr!("例：{}。", "For example: {}.", ex.ja; ex.en)).note(tr!(
                        "渡す属性か input の範囲を、前提を守る範囲に狭めてください。",
                        "Narrow the ranges of what is given to ones that keep the precondition."
                    )));
                }
                Answer::Undecided(why) => {
                    diags.push(at("W201", g, line, tr!("規則 `{}` の前提 {} を守るかを決められません", "Cannot decide whether the values given to the rule `{}` keep its precondition {}", facts.rule, precondition_text(&p); facts.rule, precondition_text(&p))).note(why).note(tr!(
                        "生成したコードは、走らせたときに前提を確かめ、破れば拒みます。",
                        "The generated code checks it when it runs, and denies the request when it breaks."
                    )));
                }
            }
        }
    }
    if broken {
        return None;
    }
    // the values of each enum input, as rulec's evaluator takes them, with the words that name them
    let enums = facts
        .inputs
        .iter()
        .filter_map(|i| {
            let e = match &i.ty {
                ColumnType::Enum { name: e, .. } => e,
                ColumnType::Opt(x) => match x.as_ref() {
                    ColumnType::Enum { name: e, .. } => e,
                    _ => return None,
                },
                _ => return None,
            };
            let values = facts.enums.iter().find(|x| x.name == *e)?.values.iter().map(|v| (v.name.clone(), vec![v.name.clone(), v.alias.clone(), v.public.clone()])).collect();
            Some((i.name.clone(), values))
        })
        .collect();
    // the name of each value of an enum output in the generated code, which a condition may write too
    let members: Vec<String> = match column(&facts.outputs, output).map(|c| &c.ty) {
        Some(ColumnType::Enum { name: e, .. }) => facts.enums.iter().find(|x| x.name == *e).map(|x| x.values.iter().map(|v| v.alias.clone()).collect()).unwrap_or_default(),
        _ => Vec::new(),
    };
    domain.map(|domain| Kn::Rule { file: u.file.clone(), output: column(&facts.outputs, output).map(|c| c.name.clone()).unwrap_or_default(), domain, inputs, enums, members })
}

fn column_type(t: &ColumnType) -> String {
    match t {
        ColumnType::Bool => "bool".into(),
        ColumnType::Str => "string".into(),
        ColumnType::Date { .. } => "date".into(),
        ColumnType::Enum { name: e, .. } => e.clone(),
        ColumnType::Num { written, .. } => written.clone(),
        ColumnType::Opt(x) => format!("{}?", column_type(x)),
    }
}

fn precondition_text(p: &Precondition) -> String {
    match p {
        Precondition::Relation { left, op, right } => format!("`{left} {op} {right}`"),
        Precondition::SumRelation { left, op, right } => format!("`{} {op} {}`", left.join(" + "), right.join(" + ")),
        Precondition::Sum { name, over, of, max, min: None, .. } => format!("`{name}` ({of} over {over} <= {max})"),
        Precondition::Sum { name, over, of, max, min: Some(min), .. } => format!("`{name}` ({min} <= {of} over {over} <= {max})"),
        Precondition::Count { name, over, min } => format!("`{name}` (counted over {over} >= {min})"),
        Precondition::Length { sequence, max } => format!("`{sequence}` (<= {max})"),
        Precondition::Unique { sequence, field } => format!("`{sequence}` ({field} unique)"),
        Precondition::Days { input, date, .. } => format!("`{input}` ({date})"),
        Precondition::Never { said, .. } => format!("`{said}`"),
        Precondition::Define { name, range, .. } => format!("`{name}` ({range})"),
    }
}

/// An input as rulec gives it, `low = 100, high = 0` (`、` between them in Japanese); a date, which
/// rulec gives as its day number, as the day.
fn values_text(vals: &[(String, Value)], facts: &RuleFacts) -> Text {
    let is_date = |n: &str| column(&facts.inputs, n).is_some_and(|c| c.ty.is_date());
    let parts: Vec<String> = vals
        .iter()
        .map(|(n, v)| {
            let v = match v {
                Value::Bool(b) => b.to_string(),
                Value::Int(i) if is_date(n) => crate::types::day_text(*i as i64),
                Value::Int(i) => i.to_string(),
                Value::Str(s) | Value::Date(s) | Value::Enum(s) => s.clone(),
                Value::List(_) => "[…]".into(),
                Value::None => "none".into(),
            };
            format!("{n} = {v}")
        })
        .collect();
    Text { ja: parts.join("、"), en: parts.join(", ") }
}

/// E206 when a range given can be outside the range of the input.
#[allow(clippy::too_many_arguments)]
fn outside(g: &Gate, line: usize, rule: &str, input: &str, shown: &str, have: (i128, i128), want: (Option<i128>, Option<i128>), unit: Option<&ritsu_units::Unit>, diags: &mut Vec<Diag>) {
    let low = want.0.filter(|m| have.0 < *m).map(|_| have.0);
    let high = want.1.filter(|m| have.1 > *m).map(|_| have.1);
    let Some(v) = low.or(high) else { return };
    let (x, y) = (want.0.map(|m| show_num(m, unit)).unwrap_or_default(), want.1.map(|m| show_num(m, unit)).unwrap_or_default());
    let ex = show_num(v, unit);
    diags.push(
        at("E206", g, line, tr!("`{shown}` は規則 `{rule}` の入力 `{input}` の範囲（{x}〜{y}）の外になりえます", "`{shown}` can be outside the range of the input `{input}` of the rule `{rule}` ({x}..{y})"))
            .note(tr!("例：`{shown}` が {ex} のとき。", "For example: `{shown}` at {ex}."))
            .note(tr!("渡す値の範囲を、入力の範囲の中に狭めてください。", "Narrow the range of what is given to the input's.")),
    );
}

/// The rule's name of the value of its enum `en` that one of `words` names: by the rule's name of
/// it, its alias in the generated code, or its public name.
fn enum_value_named(facts: &RuleFacts, en: &str, words: &[&str]) -> Option<String> {
    let e = facts.enums.iter().find(|x| x.name == en)?;
    e.values.iter().find(|v| words.iter().any(|w| v.name == *w || v.alias == *w || v.public == *w)).map(|v| v.name.clone())
}

/// E206 when what is given to an input of the rule that holds its ordered enum to a range can be a
/// value the range does not let in (`outside`, as the gate writes those values, in its order), or
/// is one, for a constant: the range said by its two ends, as the rule writes it, and an example of
/// those values for what can be several.
#[allow(clippy::too_many_arguments)]
fn outside_values(g: &Gate, line: usize, rule: &str, input: &str, shown: &str, takes: &[String], outside: &[String], constant: bool, diags: &mut Vec<Diag>) {
    let Some(ex) = outside.first() else { return };
    let (x, y) = (takes.first().cloned().unwrap_or_default(), takes.last().cloned().unwrap_or_default());
    let fix = tr!(
        "値がどれも範囲の中にある列挙の属性か input を渡すか、範囲の中の値を定数で書いてください。",
        "Give it an attribute or an input of an enum whose values are all inside the range, or write a value inside it as a constant."
    );
    let d = if constant {
        at("E206", g, line, tr!("`{shown}` は規則 `{rule}` の入力 `{input}` の範囲（{x}〜{y}）の外です", "`{shown}` is outside the range of the input `{input}` of the rule `{rule}` ({x}..{y})"))
    } else {
        at("E206", g, line, tr!("`{shown}` は規則 `{rule}` の入力 `{input}` の範囲（{x}〜{y}）の外になりえます", "`{shown}` can be outside the range of the input `{input}` of the rule `{rule}` ({x}..{y})"))
            .note(tr!("例：`{shown}` が `{ex}` のとき。", "For example: `{shown}` at `{ex}`."))
    };
    diags.push(d.note(fix));
}

#[allow(clippy::too_many_arguments)]
fn date_value(g: &Gate, a: &Action, c: &Computed, du: usize, facts: &ritsu_ports::DateFacts, function: &str, args: &[(String, Source)], diags: &mut Vec<Diag>) -> Option<Kn> {
    let u = &g.uses[du];
    let line = c.named.line;
    let before = diags.len();
    let Some(func) = facts.functions.iter().find(|f| f.name == function || f.alias == function) else {
        let names: Vec<Vec<&str>> = facts.functions.iter().map(|f| vec![f.name.as_str(), f.alias.as_str()]).collect();
        let d = at("E101", g, line, tr!("日付のファイル `{}` に日付 `{function}` はありません", "The dates file `{}` has no date `{function}`", u.path; u.path));
        diags.push(with_notes(d, foreign_notes(&tr!("日付", "dates"), &of_the_dates_file(), &names, Spot::Name)));
        return None;
    };
    let mut inputs: Vec<(String, bool)> = Vec::new();
    for (name, src) in args {
        let Some(inp) = facts.inputs.iter().find(|i| i.name == *name || i.alias == *name) else {
            // each input the date reads, by its name and its alias
            let names: Vec<Vec<&str>> = func.params.iter().map(|p| std::iter::once(p.as_str()).chain(facts.inputs.iter().filter(|i| i.name == *p).map(|i| i.alias.as_str())).collect()).collect();
            let d = at("E101", g, line, tr!("日付 `{}` に入力 `{name}` はありません", "The date `{}` has no input `{name}`", func.name; func.name));
            diags.push(with_notes(d, foreign_notes(&tr!("入力", "inputs"), &of_the_dates_file(), &names, Spot::Name)));
            continue;
        };
        let shown = source_text(src);
        let gv = given(g, a, src);
        match (inp.kind, &gv) {
            (_, Given::Unknown) => {}
            (DateKind::Date, Given::Date { lo, hi }) => {
                inputs.push((inp.name.clone(), true));
                if *lo < inp.min || *hi > inp.max {
                    let ex = if *lo < inp.min { *lo } else { *hi };
                    let (x, y) = (crate::types::day_text(inp.min), crate::types::day_text(inp.max));
                    diags.push(
                        at("E206", g, line, tr!("`{shown}` は日付 `{}` の入力 `{name}` の範囲（{x}〜{y}）の外になりえます", "`{shown}` can be outside the range of the input `{name}` of the date `{}` ({x}..{y})", func.name; func.name))
                            .note(tr!("例：`{shown}` が {} のとき。", "For example: `{shown}` at {}.", crate::types::day_text(ex)))
                            .note(tr!("渡す値の範囲を、入力の範囲の中に狭めてください。", "Narrow the range of what is given to the input's.")),
                    );
                }
            }
            (DateKind::Int, Given::Num { lo, hi, unit, .. }) => {
                inputs.push((inp.name.clone(), false));
                let (lo, hi) = match src {
                    Source::Lit(Literal::Num(n)) if n.value.is_int() => (n.value.num, n.value.num),
                    _ => (*lo, *hi),
                };
                if lo < inp.min as i128 || hi > inp.max as i128 {
                    let ex = if lo < inp.min as i128 { lo } else { hi };
                    diags.push(
                        at("E206", g, line, tr!("`{shown}` は日付 `{}` の入力 `{name}` の範囲（{}〜{}）の外になりえます", "`{shown}` can be outside the range of the input `{name}` of the date `{}` ({}..{})", func.name, inp.min, inp.max; func.name, inp.min, inp.max))
                            .note(tr!("例：`{shown}` が {} のとき。", "For example: `{shown}` at {}.", show_num(ex, unit.as_ref()); show_num(ex, unit.as_ref())))
                            .note(tr!("渡す値の範囲を、入力の範囲の中に狭めてください。", "Narrow the range of what is given to the input's.")),
                    );
                }
            }
            (kind, _) => {
                let want = if kind == DateKind::Date { "date" } else { "number" };
                diags.push(at("E102", g, line, tr!("日付 `{}` の入力 `{name}` に `{shown}` は渡せません。入力は {want} です", "`{shown}` cannot be given to the input `{name}` of the date `{}`: the input is a {want}", func.name; func.name)));
            }
        }
    }
    let missing: Vec<Text> = func.params.iter().filter(|p| !args.iter().any(|(n, _)| facts.inputs.iter().any(|i| (i.name == *n || i.alias == *n) && i.name == **p))).map(|p| Text::same(p.clone())).collect();
    if !missing.is_empty() {
        let l = Text::list(&missing);
        diags.push(at("E105", g, line, tr!("日付 `{}` の入力 {} が渡されていません", "The date `{}` is not given its input {}", func.name, l.ja; func.name, l.en)));
    }
    if diags.len() > before {
        return None;
    }
    Some(Kn::Date { file: u.file.clone(), function: func.name.clone(), inputs })
}
