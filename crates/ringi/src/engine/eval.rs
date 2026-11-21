//! The form and the route of a request for the values a requester gives (DESIGN 10.1): what the
//! lowered files answer through the ports. The computations of the form are `form.rule`
//! (`Rules::eval`) and `form.cal` (`Dates::eval`), the stages `route.rule`, with the outputs of the
//! rules the route reads. `simulate --form` and the ticket machine both ask here, so that a value
//! the one takes and the other refuses does not come to be.

use crate::lower::{LFile, Lowered, Part};
use crate::names::Scope;
use crate::suite::Suite;
use ritsu_base::text::Text;
use ritsu_ports::{DateValue, Outside, RuleError, Value, Values};
use std::rc::Rc;

/// What the lowered files answer for the values of a form.
#[derive(Clone, Debug, Default)]
pub struct Routed {
    /// Each stage of the route, and the value its output takes, by the name the request gives it
    /// (`after` where the lowered rule writes `after_approver`; None for a stage that is not there).
    pub stages: Vec<(String, Option<String>)>,
    /// What the form computes, by the lowered names of the fields, in the form's order.
    pub computed: Values,
    /// Every output of the lowered form: what it computes, and whether each field is shown and
    /// asked for (`<field>_shown`, `<field>_required`), by the names the file gives them.
    pub outs: Values,
}

/// Why the lowered files gave no answer.
#[derive(Clone, Debug)]
pub enum Unrouted {
    /// A language the lowered files are of is not joined.
    Unjoined(&'static str),
    /// A lowered file does not read.
    Unread(Text),
    /// A value the form's rule (or, with `route`, the route) reads is not given.
    Lacks { name: String, route: bool },
    /// A rule refused the values: which rule, in words.
    Rule(RuleError, Text),
    /// A date of the form cannot be counted: the field, and what koyomi says.
    Date(String, Text),
    /// A computation that may have no value (DESIGN 4.9) is outside the range it writes, where it
    /// has one: the refusal's words and the place it names.
    Outside(Text, String),
}

/// The lowered files laid over the files the program reads, so that the ports read them where the
/// request's lowered directory would be: the ones the check laid ([`Lowered::laid`]), so that what
/// the check read of them is not read again.
pub fn overlay(lowered: &Lowered) -> Rc<dyn ritsu_base::fs::Files> {
    lowered.laid()
}

/// The name of the request a lowered file's name stands for: the file's own, unless it renamed it.
fn asked<'x>(f: &'x LFile, given: &'x str) -> &'x str {
    f.renamed.iter().find(|(g, _)| g == given).map(|(_, want)| want.as_str()).unwrap_or(given)
}

/// The values a rule takes, of `vals`, by the names of its inputs and lists. With `lf`, the rule is a
/// lowered file, which takes a value of the request by the name the file gives it (`count_value`
/// for `count`), a choice's value as the file writes it (`after_value` for `after`), and the fields
/// of a row by the names it gives them.
fn pick(s: &Scope, lf: Option<&LFile>, rules: &dyn ritsu_ports::Rules, p: &std::path::Path, vals: &Values) -> Result<Values, Unrouted> {
    let facts = rules.facts(p).map_err(|e| Unrouted::Unread(e.first().map(|x| x.message.clone()).unwrap_or_default()))?;
    let written = |n: &str| lf.map(|f| asked(f, n).to_string()).unwrap_or_else(|| n.to_string());
    // a lowered file that works out the form's computations reads a field that may be empty that a
    // computation reads as its value and whether it is filled in: where it is empty, a value of its
    // range, which no answer of the file reads (DESIGN 4.9)
    let computes = lf.is_some_and(|f| matches!(f.part, Part::Form | Part::Values | Part::Hidden(_)));
    let paired = |want: &str| computes && s.fields().iter().any(|f| f.name.ascii() == want && crate::lower::computed_from(s, f));
    let mut out = Values::new();
    for c in &facts.inputs {
        let want = written(&c.name);
        let got = vals.iter().find(|(n, _)| *n == c.name || *n == c.alias).or_else(|| vals.iter().find(|(n, _)| *n == want)).map(|(_, v)| v).filter(|v| **v != Value::None);
        match got {
            Some(v) => out.push((c.name.clone(), lowered_value(s, lf, &want, v))),
            // a value that may not be there is `none` where it is not (DESIGN 4.9)
            None if matches!(c.ty, ritsu_ports::ColumnType::Opt(_)) => out.push((c.name.clone(), Value::None)),
            None if paired(&want) => out.push((c.name.clone(), s.fields().iter().find(|f| f.name.ascii() == want).map(|f| stand_in(f)).unwrap_or(Value::None))),
            None => return Err(Unrouted::Lacks { name: c.name.clone(), route: false }),
        }
    }
    for (name, alias, fields) in &facts.elements {
        let want = written(name);
        if let Some((_, v)) = vals.iter().find(|(n, _)| n == name || n == alias).or_else(|| vals.iter().find(|(n, _)| *n == want)) {
            // each field of a row by the name the file gives it; a cell that may be empty, its value
            // and whether it is filled in
            let v = match v {
                Value::List(rows) if lf.is_some() => Value::List(
                    rows.iter()
                        .map(|row| {
                            let mut one: Values = row.iter().filter(|(_, x)| *x != Value::None).map(|(k, x)| (fields.iter().find(|c| written(&c.name) == *k).map(|c| c.name.clone()).unwrap_or_else(|| k.clone()), x.clone())).collect();
                            for c in fields {
                                if one.iter().any(|(n, _)| *n == c.name) {
                                    continue;
                                }
                                let cell = |g: &str| lf.and_then(|f| f.given.iter().find(|(col, cell)| col == g && cell.contains('.')).map(|(_, cell)| cell.clone()));
                                if let Some(cell) = cell(&c.name) {
                                    let child = cell.rsplit('.').next().unwrap_or_default().to_string();
                                    one.push((c.name.clone(), Value::Bool(row.iter().any(|(k, x)| *k == child && *x != Value::None))));
                                } else if lf.is_some_and(|f| f.given.iter().any(|(_, cell)| *cell == format!("{want}.{}", written(&c.name)))) {
                                    let child = s.fields().iter().find(|f| f.name.ascii() == want).and_then(|r| r.children.iter().find(|x| x.name.ascii() == written(&c.name)));
                                    one.push((c.name.clone(), child.map(stand_in).unwrap_or(Value::None)));
                                }
                            }
                            one
                        })
                        .collect(),
                ),
                v => v.clone(),
            };
            out.push((name.clone(), v));
        }
    }
    Ok(out)
}

/// The value of the range of the field `f` a lowered file takes in place of its value where it is
/// empty, which no answer of the file reads (DESIGN 4.9), on the wire: the one the glue gives
/// ([`crate::lower::stand_in`]).
fn stand_in(f: &crate::ast::Field) -> Value {
    use crate::lower::words::{Val, step};
    match (crate::lower::stand_in(f), &f.ty) {
        (_, crate::ast::TypeExpr::Bool) => Value::Bool(false),
        (Val::Day(d), _) => Value::Date(crate::lex::date_text(d)),
        (Val::Num(r), crate::ast::TypeExpr::Num(u)) => Value::Int(r.div(step(u)).num),
        (Val::Num(r), _) => Value::Int(r.num),
    }
}

/// A value of the field the request names `field` (its ASCII name) as a lowered file takes it: a
/// choice's value by the name the lowered files write it under; any other as it is.
fn lowered_value(s: &Scope, lf: Option<&LFile>, field: &str, v: &Value) -> Value {
    match (lf, v) {
        (Some(_), Value::Enum(e)) => match s.fields().iter().find(|f| f.name.ascii() == field && matches!(f.ty, crate::ast::TypeExpr::Choice)) {
            Some(f) => Value::Enum(crate::lower::words::choice_value(s, f, e)),
            None => v.clone(),
        },
        _ => v.clone(),
    }
}

/// The values, with whether each field a condition of the lowered file `lf` asks of is filled in
/// (`cover is none`), by the input's name in the file: yes or no, as the glue gives it.
fn with_given(lf: &LFile, vals: &Values) -> Values {
    let mut out = vals.clone();
    // the fields at the top of the form; a cell of a row is given with its row ([`pick`])
    out.extend(lf.given.iter().filter(|(_, field)| !field.contains('.')).map(|(col, field)| (col.clone(), Value::Bool(vals.iter().any(|(n, v)| n == field && *v != Value::None)))));
    out
}

/// The output `out` of the rule the request uses by `alias`, for the values.
pub fn rule_output(s: &Scope, suite: &Suite, alias: &str, out: &str, vals: &Values) -> Result<Option<Value>, Unrouted> {
    let Some(rules) = &suite.rules else { return Err(Unrouted::Unjoined("rulec")) };
    let Some(rp) = s.other_path(alias) else { return Ok(None) };
    let ins = pick(s, None, rules.as_ref(), rp, vals)?;
    let got = rules.eval(rp, &ins).map_err(|e| Unrouted::Rule(e, tr!("規則 `{alias}`", "the rule `{alias}`")))?;
    Ok(got.into_iter().find(|(n, _)| n == out).map(|(_, v)| v))
}

/// The outputs of the lowered form for the values `given` (by the lowered names of the fields): its
/// computations, and whether each field is shown and asked for (`<field>_shown`,
/// `<field>_required`), by the names the file gives them. None where the request has no lowered
/// form.
pub fn form_outs(s: &Scope, suite: &Suite, lowered: &Lowered, given: &Values) -> Result<Option<Values>, Unrouted> {
    let Some(rules) = &suite.rules else { return Err(Unrouted::Unjoined("rulec")) };
    let Some(ff) = lowered.files.iter().find(|f| f.part == Part::Form) else { return Ok(None) };
    held_where_there(s, rules.as_ref(), lowered, given)?;
    let p = lowered.path(ff);
    let ins = pick(s, Some(ff), rules.as_ref(), &p, &with_given(ff, given))?;
    // a value the form works out outside its range, by what rulec's evaluator found: the refusal
    // in the words of the request (DESIGN 4.4)
    let vf = lowered.files.iter().find(|f| f.part == Part::Values);
    rules.eval_outside(&p, &ins).map(Some).map_err(|(e, o)| match o.and_then(|o| outside_said(s, ff, vf, &o)) {
        Some((t, at)) => Unrouted::Outside(t, at),
        None => Unrouted::Rule(e, tr!("フォーム", "the form")),
    })
}

/// The outputs of the copy of the lowered form that the generated code and the screens read
/// (`values.rule`) for the values `given`, by the names the file gives them: its totals run from no
/// rows, so it works out the rows there so far when they are fewer than their field asks for, where
/// `form.rule` refuses at its door a total below its range (DESIGN 4.4, 13.7). None where the request
/// has no copy.
pub fn values_outs(s: &Scope, suite: &Suite, lowered: &Lowered, given: &Values) -> Result<Option<Values>, Unrouted> {
    let Some(rules) = &suite.rules else { return Err(Unrouted::Unjoined("rulec")) };
    let Some(vf) = lowered.files.iter().find(|f| f.part == Part::Values) else { return Ok(None) };
    let p = lowered.path(vf);
    let ins = pick(s, Some(vf), rules.as_ref(), &p, &with_given(vf, given))?;
    rules.eval(&p, &ins).map(Some).map_err(|e| Unrouted::Rule(e, tr!("フォーム", "the form")))
}

/// Whether a computed field has a value for the values given: a computation that reads a field that
/// may be empty has none where one of those is empty (DESIGN 4.9).
pub fn has_value(s: &Scope, f: &crate::ast::Field, given: &Values) -> bool {
    crate::lower::empty_reads(s, f).iter().all(|g| given.iter().any(|(n, v)| n == g.name.ascii() && *v != Value::None))
}

/// For a form some computation of which may have no value, which `form.rule` holds to no range at
/// its door (DESIGN 4.9): each computed number held to its range where it has a value, in the order
/// the door holds the others and the glue holds them all ([`crate::lower::form::held`]), read from
/// the copy (`values.rule`). The first outside is refused, as the door refuses it.
fn held_where_there(s: &Scope, rules: &dyn ritsu_ports::Rules, lowered: &Lowered, given: &Values) -> Result<(), Unrouted> {
    let none_some = |f: &crate::ast::Field| crate::lower::may_be_none(s, f);
    let rows_some = |f: &crate::ast::Field| f.children.iter().any(|c| c.computed.is_some() && !crate::lower::empty_reads_of_row(f, c).is_empty());
    if !s.fields().iter().any(|f| (f.computed.is_some() && none_some(f)) || rows_some(f)) {
        return Ok(());
    }
    let Some(vf) = lowered.files.iter().find(|f| f.part == Part::Values) else { return Ok(()) };
    let p = lowered.path(vf);
    let worked = |given: &Values| -> Result<Values, Unrouted> {
        let ins = pick(s, Some(vf), rules, &p, &with_given(vf, given))?;
        rules.eval(&p, &ins).map_err(|e| Unrouted::Rule(e, tr!("フォーム", "the form")))
    };
    let got = worked(given)?;
    let held = crate::lower::form::held(s, vf);
    let mut i = 0;
    while i < held.len() {
        let h = &held[i];
        // the computations of a row, of the same rows, the door holds on each row in turn
        let n = match &h.rows {
            Some(r) => held[i..].iter().take_while(|g| g.rows.as_ref() == Some(r)).count(),
            None => 1,
        };
        let group = &held[i..i + n];
        i += n;
        if h.rows.is_none() && s.fields().iter().find(|f| f.name.ascii() == h.field).is_some_and(|f| !has_value(s, f, given)) {
            continue;
        }
        if !group.iter().any(|g| past(g, &got)) {
            continue;
        }
        let Some(r) = &h.rows else { return Err(Unrouted::Outside(h.said.clone(), h.field.clone())) };
        // the first row a computation of it is outside its range on, and the first such computation
        // of the row, as the door finds it: the copy worked out over that row alone, which its
        // computations read nothing else of
        let Some((_, Value::List(rows))) = given.iter().find(|(k, _)| k == r) else { continue };
        for (k, row) in rows.iter().enumerate() {
            let one: Values = given.iter().map(|(n, v)| if n == r { (n.clone(), Value::List(vec![row.clone()])) } else { (n.clone(), v.clone()) }).collect();
            let got = worked(&one)?;
            if let Some(g) = group.iter().find(|g| past(g, &got)) {
                let (t, at) = g.said_at(k);
                return Err(Unrouted::Outside(t, at));
            }
        }
    }
    Ok(())
}

/// Whether the copy's outputs `got` put the value `h` holds outside its range: its value, or for a
/// computation of a row, its most or its least over the rows.
fn past(h: &crate::lower::form::Held, got: &Values) -> bool {
    let num = |n: &str| got.iter().find(|(k, _)| k == n).and_then(|(_, v)| if let Value::Int(i) = v { Some(*i) } else { None });
    let step = match &h.ty {
        crate::ast::TypeExpr::Num(u) => crate::lower::words::step(u),
        _ => ritsu_units::Rat::int(1),
    };
    let wire = |r: ritsu_units::Rat| r.div(step);
    let below = h.lo.as_ref().is_some_and(|(n, l)| num(n).is_some_and(|v| ritsu_units::Rat::int(v).cmp_to(wire(*l)) == std::cmp::Ordering::Less));
    let above = h.hi.as_ref().is_some_and(|(n, hi)| num(n).is_some_and(|v| ritsu_units::Rat::int(v).cmp_to(wire(*hi)) == std::cmp::Ordering::Greater));
    below || above
}

/// The form computed and the route decided for the values `given` (by the lowered names of the
/// fields), read with the lowered files of `lowered` laid over the files ([`overlay`]).
pub fn routed(s: &Scope, suite: &Suite, lowered: &Lowered, given: &Values) -> Result<Routed, Unrouted> {
    let Some(rules) = &suite.rules else { return Err(Unrouted::Unjoined("rulec")) };
    let mut vals = given.clone();
    let mut r = Routed::default();
    // the computations of the form: numbers, and whether each field is shown and asked for
    if let Some(ff) = lowered.files.iter().find(|f| f.part == Part::Form) {
        let got = form_outs(s, suite, lowered, &vals)?.unwrap_or_default();
        for (n, v) in got {
            let want = asked(ff, &n);
            if let Some(f) = s.fields().iter().find(|f| f.name.ascii() == want && f.computed.is_some()) {
                // a computation that reads a field that is empty has no value (DESIGN 4.9)
                let v = if has_value(s, f, given) { v.clone() } else { Value::None };
                r.computed.push((f.name.ascii().to_string(), v.clone()));
                vals.push((f.name.ascii().to_string(), v));
            }
            r.outs.push((n, v));
        }
    }
    // the dates the form counts in business days
    if let Some(df) = lowered.files.iter().find(|f| f.part == Part::FormDates) {
        let Some(dates) = &suite.dates else { return Err(Unrouted::Unjoined("koyomi")) };
        let p = lowered.path(df);
        let facts = dates.facts(&p).map_err(|e| Unrouted::Unread(e.first().map(|x| x.message.clone()).unwrap_or_default()))?;
        let mut ins = Vec::new();
        // the inputs that are not filled in, each given the least of its range, which no date counted
        // from it reads: a date counted from a value that is empty has none (DESIGN 4.9)
        let mut empty: Vec<&str> = Vec::new();
        for i in &facts.inputs {
            let want = asked(df, &i.name);
            match vals.iter().find(|(n, _)| n == want).map(|(_, v)| v) {
                Some(Value::Date(d)) if ritsu_ports::day_of(d).is_some() => ins.extend(ritsu_ports::day_of(d).map(|d| (i.name.clone(), d))),
                Some(Value::Int(k)) => ins.push((i.name.clone(), *k as i64)),
                _ => {
                    empty.push(&i.name);
                    ins.push((i.name.clone(), i.min));
                }
            }
        }
        let counts = |f: &ritsu_ports::DateFunction| !f.params.iter().any(|p| empty.contains(&p.as_str()));
        if facts.functions.iter().any(counts) {
            let whose = |n: &str| s.fields().iter().find(|f| f.name.ascii() == asked(df, n) && f.computed.is_some());
            let first = facts.functions.iter().find(|f| counts(f)).and_then(|f| whose(&f.name)).map(|f| f.name.text.clone()).unwrap_or_default();
            let got = dates.eval(&p, &ins).map_err(|e| Unrouted::Date(first.clone(), e.first().map(|x| Text::new(df.named_back(&x.message.ja), df.named_back(&x.message.en))).unwrap_or_default()))?;
            for (n, v) in got {
                let Some(f) = whose(&n) else { continue };
                if facts.functions.iter().any(|g| g.name == n && !counts(g)) {
                    continue;
                }
                match v {
                    DateValue::Day(d) => {
                        let v = Value::Date(crate::lex::date_text(d));
                        r.computed.push((f.name.ascii().to_string(), v.clone()));
                        vals.push((f.name.ascii().to_string(), v));
                    }
                    DateValue::Stopped(t) => return Err(Unrouted::Date(f.name.text.clone(), Text::new(df.named_back(&t.ja), df.named_back(&t.en)))),
                }
            }
        }
    }
    let Some(rf) = lowered.files.iter().find(|f| f.part == Part::Route) else { return Ok(r) };
    let names = lowered.route.clone().unwrap_or_default();
    let p = lowered.path(rf);
    // the inputs the route reads that may be none: a field that may be empty, a computation that may
    // have no value, a rule's output that may be none (DESIGN 4.9)
    let facts = rules.facts(&p).map_err(|e| Unrouted::Unread(e.first().map(|x| x.message.clone()).unwrap_or_default()))?;
    let optional = |col: &str| facts.inputs.iter().any(|i| i.name == col && matches!(i.ty, ritsu_ports::ColumnType::Opt(_)));
    // the inputs the route reads: a field, whether a field is filled in, a rule's output
    for (reads, col, _) in &names.inputs {
        // a value the route works out itself (a rule it applies), or one given already
        if names.worked.contains(col) || vals.iter().any(|(n, _)| n == col) {
            continue;
        }
        let v = match reads.as_slice() {
            [one] if one.ends_with('?') => {
                let f = one.trim_end_matches('?');
                let ascii = s.field(f).map(|x| x.name.ascii().to_string()).unwrap_or_else(|| f.to_string());
                Some(Value::Bool(vals.iter().any(|(n, v)| *n == ascii && *v != Value::None)))
            }
            [one] => {
                let ascii = s.field(one).map(|x| x.name.ascii().to_string()).unwrap_or_else(|| one.clone());
                vals.iter().find(|(n, _)| *n == ascii).map(|(_, v)| v.clone()).filter(|v| *v != Value::None)
            }
            [alias, out] => rule_output(s, suite, alias, out, &vals)?,
            _ => None,
        };
        match v {
            Some(v) => vals.push((col.clone(), v)),
            None if optional(col) => vals.push((col.clone(), Value::None)),
            None => return Err(Unrouted::Lacks { name: reads.join("."), route: true }),
        }
    }
    let ins = pick(s, Some(rf), rules.as_ref(), &p, &vals)?;
    let got = rules.eval(&p, &ins).map_err(|e| Unrouted::Rule(e, tr!("経路", "the route")))?;
    r.stages = names
        .stages
        .iter()
        .map(|(stage, output, _)| {
            let v = got.iter().find(|(n, _)| n == output).and_then(|(_, v)| match v {
                Value::Enum(e) => Some(asked(rf, e).to_string()),
                _ => None,
            });
            (stage.clone(), v)
        })
        .collect();
    Ok(r)
}

/// The refusal of a value the lowered form works out outside the range its field writes, in the
/// words of the request (DESIGN 4.4), by what rulec's evaluator found ([`Outside`]): the total or
/// the computation by its field's name and the range as written, else what it can reach, with the
/// place it names (`at`): the field, or a computation of a row at the row the door refuses it on
/// (`lines[1].fee`, the rows counted from 0), and the fields it is worked out from as the kind's
/// description says them, by the lowered `values.rule` (`vf`). None for a value that is no field's.
fn outside_said(s: &Scope, ff: &LFile, vf: Option<&LFile>, o: &Outside) -> Option<(Text, String)> {
    // the name the lowered file gives what the request names `want`
    let given = |want: &str| ff.renamed.iter().find(|(_, w)| w == want).map(|(g, _)| g.clone()).unwrap_or_else(|| want.to_string());
    match o {
        Outside::Define { name, at: None } | Outside::Summary { name } => {
            let f = s.fields().iter().find(|f| f.computed.is_some() && given(f.name.ascii()) == *name)?;
            Some((crate::lower::form::out_of_range_said(s, vf, f, None, &f.name.text), f.name.ascii().to_string()))
        }
        Outside::Define { name, at: Some((over, k)) } => {
            let rows = s.fields().iter().find(|f| given(f.name.ascii()) == *over)?;
            let c = rows.children.iter().find(|c| c.computed.is_some() && given(c.name.ascii()) == *name)?;
            let whole = format!("{}[{k}].{}", rows.name.text, c.name.text);
            Some((crate::lower::form::out_of_range_said(s, vf, c, Some(rows), &whole), format!("{}[{k}].{}", rows.name.ascii(), c.name.ascii())))
        }
    }
}
