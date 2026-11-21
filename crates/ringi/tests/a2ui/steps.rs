//! The screens work out each condition and each computation of a form from the fields it reads
//! alone (DESIGN 4.9, 13.7), as a person fills the form in. For every kind of the examples, and for
//! the machine's tests whose conditions and computations read fields that may be empty, a total and
//! a computation of one, rates, and a field shown on a field shown on a condition
//! (`tests/machine/`), the form is filled in one value at a time in its order: each field given its
//! least, one more, its most and a value the vectors give it (a choice each of its values), rows
//! added empty and filled in one field at a time, the fields of a row given their values, a second
//! row added and filled in, the first taken away; then, with the form full, each field given its
//! values again, each emptied and filled in again, and all the rows taken away and put back.
//!
//! At each step, what the screens say of each condition and computation is what the reference says:
//! the reference interpreter is given the fields it reads (`reads` in `types/<name>.json`) as the
//! form holds them, a field the form hides as empty, rows whose field it reads that the requester
//! fills in is empty left out (they are not there yet), and a value of the vectors' of every other
//! field the requester fills in. The rows there so far are given as they are, fewer than the field
//! asks for too: a total of the rows filled in so far is shown, and a condition reads it. Where the
//! lowered form refuses so few rows at its door (a total below its range), what the screens say is
//! held to what the copy of it they read (`values.rule`) works out, by the same evaluator. While a
//! field it reads that the requester has to fill in is empty, a condition does not hold and a
//! computation shows no value. The check of a computation held to the range written for it says what
//! the reference says of the range, once the rows it reads are as many as their field asks for; it
//! passes before. A step the reference refuses for something else is not compared, and counted. A
//! field an output does not read, through the computations it reads and the conditions the fields it
//! reads are shown on, changes nothing the output says when it is emptied or filled in.
//!
//! The functions with the fields one output reads made one fewer, or one more field the requester
//! has to fill in, which holds it back, give themselves away. One more field that may be empty
//! changes nothing the output says, nor does one more its answers change with already through a
//! condition, nor one more field of rows, whose rows the rule is given as they are there; none is
//! such a change.

use super::forms::{Field, Seed, Ty, fields_of, held, sample, seeds, values_of};
use super::{at, file, made_of, node};
use ringi::check::{Options, Run};
use ritsu_base::json::{self, Json};
use ritsu_ports::Value;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

/// The machine's tests the steps are taken on beside the examples.
const MACHINE: [&str; 4] = ["tests/machine/absent.ringi", "tests/machine/ranges.ringi", "tests/machine/steps.ringi", "tests/machine/supplies.ringi"];

#[derive(Clone, Copy, Debug, PartialEq)]
enum What {
    Shown,
    Required,
    Value,
}

/// A condition or a computation the screens work out: the function of the catalog that answers it
/// (`expense.shown_trip_to`), the reference's output (`trip_to_shown`), what it is, its field, and
/// what it reads.
#[derive(Debug)]
struct Out {
    function: String,
    output: String,
    what: What,
    field: String,
    reads: Vec<String>,
}

fn outs_of(kind: &str, fields: &[Field]) -> Vec<Out> {
    let mut out = Vec::new();
    for f in fields {
        if let Some((w, r)) = &f.shown {
            out.push(Out { function: format!("{kind}.shown_{}", f.name), output: w.clone(), what: What::Shown, field: f.name.clone(), reads: r.clone() });
        }
        if let Some((w, r)) = &f.required {
            out.push(Out { function: format!("{kind}.required_{}", f.name), output: w.clone(), what: What::Required, field: f.name.clone(), reads: r.clone() });
        }
        if f.worked {
            out.push(Out { function: format!("{kind}.{}", f.name), output: f.name.clone(), what: What::Value, field: f.name.clone(), reads: f.reads.clone() });
        }
    }
    out
}

/// A field the requester has to fill in wherever the form shows it, which no condition hides: what
/// reads it waits while it is empty.
fn waited_for(f: &Field) -> bool {
    f.given() && !f.optional && !f.shown_when && !f.required_when && !matches!(f.ty, Ty::Rows | Ty::Bool)
}

/// A field that may be empty (DESIGN 4.9): a computation that reads it has no value while it is. A
/// screen always holds yes or no.
fn may_be_empty(f: &Field) -> bool {
    !f.computed && f.ty != Ty::Bool && (f.optional || f.shown_when || f.required_when || f.approvers)
}

/// A field of a row the requester fills in in every row.
fn asked_in_row(c: &Field) -> bool {
    c.given() && !c.optional && c.ty != Ty::Attachment
}

fn filled(v: Option<&Json>) -> bool {
    match v {
        None | Some(Json::Null) => false,
        Some(Json::Str(s)) => !s.is_empty(),
        Some(Json::Arr(xs)) => !xs.is_empty(),
        Some(_) => true,
    }
}

fn put(j: &mut Json, key: &str, v: Json) {
    let Json::Obj(pairs) = j else { panic!("not an object: {}", j.compact()) };
    match pairs.iter_mut().find(|(k, _)| k == key) {
        Some((_, x)) => *x = v,
        None => pairs.push((key.to_string(), v)),
    }
}

fn take(j: &mut Json, key: &str) {
    if let Json::Obj(pairs) = j {
        pairs.retain(|(k, _)| k != key);
    }
}

/// A value of the field `c` of the rows `f` the vectors give, else one made from its description.
fn cell_sample(f: &Field, c: &Field, seeds: &[Seed]) -> Option<Json> {
    let given = seeds.iter().find_map(|s| [&s.form, &s.gives].into_iter().find_map(|j| j.get(&f.name).and_then(Json::as_arr).and_then(|rows| rows.iter().find_map(|r| r.get(&c.name).filter(|v| **v != Json::Null).cloned()))));
    given.or_else(|| sample(c, &[]))
}

/// The values a step gives the field `f`, the one it keeps last: its least, one more than its least
/// (a number) and its most where it writes them, each of a choice's values, both of yes or no, and
/// `last`.
fn values(f: &Field, last: Option<Json>) -> Vec<(String, Json)> {
    let mut out: Vec<(String, Json)> = Vec::new();
    match f.ty {
        Ty::Number | Ty::Date => {
            for (word, v) in [("its least", &f.min), ("its most", &f.max)] {
                if *v != Json::Null {
                    out.push((format!("{word}, {}", v.compact()), v.clone()));
                }
            }
            if let Some(lo) = f.min.as_int().filter(|lo| f.max.as_int().is_none_or(|hi| lo + 1 < hi)) {
                out.insert(1, (format!("one more than its least, {}", lo + 1), Json::Int(lo + 1)));
            }
        }
        Ty::Choice => out.extend(f.choices.iter().map(|c| (c.clone(), Json::str(c)))),
        Ty::Bool => out.extend([("yes".to_string(), Json::Bool(true)), ("no".to_string(), Json::Bool(false))]),
        _ => {}
    }
    if let Some(v) = last {
        out.push((format!("{} (as the vectors give it)", v.compact()), v));
    }
    out
}

/// The forms a person makes as they fill in the form of `fields`, each with what was done last, as
/// the API takes a form: each field in the form's order given its values and kept at a value of the
/// vectors'; rows added empty, filled in one field at a time, each field of the row then given its
/// values, a second row added empty, filled in and its fields given their values, the first taken
/// away, a field of the row that may be empty emptied; then, with the form full, each field given
/// its values again, each emptied and filled in again, and all the rows taken away and put back.
fn steps(fields: &[Field], seeds: &[Seed]) -> Vec<(String, Json)> {
    let mut out: Vec<(String, Json)> = Vec::new();
    let mut form = Json::Obj(Vec::new());
    out.push(("nothing filled in".into(), form.clone()));
    let given: Vec<&Field> = fields.iter().filter(|f| f.given() && !matches!(f.ty, Ty::Master | Ty::Person)).collect();
    for f in &given {
        if f.ty != Ty::Rows {
            for (label, v) in values(f, sample(f, seeds)) {
                put(&mut form, &f.name, v);
                out.push((format!("{}: {label}", f.name), form.clone()));
            }
            continue;
        }
        let cells: Vec<&Field> = f.fields.iter().filter(|c| c.given() && !matches!(c.ty, Ty::Master | Ty::Person)).collect();
        let mut rows = vec![Json::Obj(Vec::new())];
        let mut step = |rows: &[Json], form: &mut Json, what: String| {
            put(form, &f.name, Json::arr(rows.to_vec()));
            out.push((what, form.clone()));
        };
        step(&rows, &mut form, format!("{}: a row added", f.name));
        for c in &cells {
            let Some(v) = cell_sample(f, c, seeds) else { continue };
            put(&mut rows[0], &c.name, v.clone());
            step(&rows, &mut form, format!("{}[0].{}: {}", f.name, c.name, v.compact()));
        }
        for c in &cells {
            for (label, v) in values(c, cell_sample(f, c, seeds)) {
                put(&mut rows[0], &c.name, v);
                step(&rows, &mut form, format!("{}[0].{}: {label}", f.name, c.name));
            }
        }
        rows.push(Json::Obj(Vec::new()));
        step(&rows, &mut form, format!("{}: a second row added", f.name));
        for c in &cells {
            let Some(v) = cell_sample(f, c, seeds) else { continue };
            put(&mut rows[1], &c.name, v.clone());
            step(&rows, &mut form, format!("{}[1].{}: {}", f.name, c.name, v.compact()));
        }
        for c in &cells {
            for (label, v) in values(c, cell_sample(f, c, seeds)) {
                put(&mut rows[1], &c.name, v);
                step(&rows, &mut form, format!("{}[1].{}: {label}", f.name, c.name));
            }
        }
        rows.remove(0);
        step(&rows, &mut form, format!("{}: the first row taken away", f.name));
        for c in cells.iter().filter(|c| !asked_in_row(c)) {
            let Some(v) = rows[0].get(&c.name).cloned() else { continue };
            take(&mut rows[0], &c.name);
            step(&rows, &mut form, format!("{}[0].{}: emptied", f.name, c.name));
            put(&mut rows[0], &c.name, v);
            put(&mut form, &f.name, Json::arr(rows.clone()));
        }
    }
    // with the form full, each field given its values again
    for f in given.iter().filter(|f| f.ty != Ty::Rows) {
        let Some(keep) = form.get(&f.name).cloned() else { continue };
        for (label, v) in values(f, None) {
            put(&mut form, &f.name, v);
            out.push((format!("{} changed to {label}", f.name), form.clone()));
        }
        put(&mut form, &f.name, keep);
    }
    // each field emptied, and filled in again; all the rows taken away, and put back
    for f in &given {
        let Some(keep) = form.get(&f.name).cloned() else { continue };
        match f.ty {
            Ty::Rows => {
                put(&mut form, &f.name, Json::arr([]));
                out.push((format!("{}: all the rows taken away", f.name), form.clone()));
            }
            _ => {
                take(&mut form, &f.name);
                out.push((format!("{} emptied", f.name), form.clone()));
            }
        }
        put(&mut form, &f.name, keep);
    }
    out
}

/// What the reference says of an output on a form.
#[derive(Clone, Debug)]
enum Expect {
    /// A field it reads that the requester has to fill in is empty.
    Waits,
    Is(Json),
    /// The reference refuses the form it is given.
    Refused,
}

/// The form as the machine is given it: without the fields the form hides, yes or no as no where
/// nothing is put in it (a screen sends no then).
fn machine_view(fields: &[Field], form: &Json, hidden: &[String]) -> Json {
    let mut out = form.clone();
    for h in hidden {
        take(&mut out, h);
    }
    for f in fields.iter().filter(|f| f.given() && f.ty == Ty::Bool && !hidden.contains(&f.name)) {
        if out.get(&f.name).is_none() {
            put(&mut out, &f.name, Json::Bool(false));
        }
    }
    out
}

fn value_json(v: &Value) -> Json {
    match v {
        Value::Bool(b) => Json::Bool(*b),
        Value::Int(n) => Json::Int(*n),
        Value::None => Json::Null,
        other => Json::str(format!("{other:?}")),
    }
}

/// The form the reference is given for what reads `reads` on the form `pm` (as the machine is given
/// it): the fields it reads as they are, rows whose field it reads that the requester fills in is
/// empty left out (the rows there so far, fewer than the field asks for too), and a value of the
/// vectors' for every other field the requester fills in. None while it waits: a field it reads that
/// the requester has to fill in is empty.
fn ready(fields: &[Field], reads: &[String], pm: &Json, seeds: &[Seed]) -> Option<Json> {
    let field = |n: &str| fields.iter().find(|f| f.name == n);
    let top: Vec<&str> = reads.iter().filter(|r| !r.contains('.')).map(String::as_str).collect();
    if top.iter().any(|r| field(r).is_some_and(waited_for) && !filled(pm.get(r))) {
        return None;
    }
    let mut q = pm.clone();
    for f in fields.iter().filter(|f| f.given()) {
        if f.ty == Ty::Rows {
            let prefix = format!("{}.", f.name);
            let read: Vec<&str> = reads.iter().filter_map(|r| r.strip_prefix(&prefix)).collect();
            let mut rows: Vec<Json> = q.get(&f.name).and_then(Json::as_arr).unwrap_or_default().to_vec();
            if read.is_empty() {
                while (rows.len() as i128) < f.count_min.unwrap_or(0) {
                    rows.push(Json::Obj(Vec::new()));
                }
            } else {
                // a row whose field it reads that the requester fills in is empty is not there yet
                rows.retain(|r| there(f, &read, r));
            }
            for r in rows.iter_mut() {
                for c in f.fields.iter().filter(|c| asked_in_row(c) && !read.contains(&c.name.as_str())) {
                    if !filled(r.get(&c.name))
                        && let Some(v) = cell_sample(f, c, seeds)
                    {
                        put(r, &c.name, v);
                    }
                }
            }
            put(&mut q, &f.name, Json::arr(rows));
        } else if waited_for(f) && !top.contains(&f.name.as_str()) && !filled(q.get(&f.name))
            && let Some(v) = sample(f, seeds)
        {
            put(&mut q, &f.name, v);
        }
    }
    Some(q)
}

/// Whether the row `r` of the rows `f` is there for what reads its fields `read`: each of them the
/// requester fills in is filled in.
fn there(f: &Field, read: &[&str], r: &Json) -> bool {
    f.fields.iter().filter(|c| read.contains(&c.name.as_str()) && asked_in_row(c)).all(|c| filled(r.get(&c.name)))
}

/// Whether the rows of a field of rows that `reads` reads, those there on the form `pm`, are fewer
/// than the field asks for: what is worked out of them is not held to its range until they are as
/// many (DESIGN 13.7).
fn short(fields: &[Field], reads: &[String], pm: &Json) -> bool {
    fields.iter().filter(|f| f.given() && f.ty == Ty::Rows).any(|f| {
        let prefix = format!("{}.", f.name);
        let read: Vec<&str> = reads.iter().filter_map(|r| r.strip_prefix(&prefix)).collect();
        let rows = pm.get(&f.name).and_then(Json::as_arr).unwrap_or_default();
        !read.is_empty() && (rows.iter().filter(|r| there(f, &read, r)).count() as i128) < f.count_min.unwrap_or(0)
    })
}

/// Whether a computation that reads `reads` has no value on the form `pm`: a field that may be
/// empty it reads is empty (DESIGN 4.9).
fn no_value(fields: &[Field], reads: &[String], pm: &Json) -> bool {
    reads.iter().filter(|r| !r.contains('.')).any(|r| fields.iter().find(|f| f.name == *r).is_some_and(may_be_empty) && !filled(pm.get(r)))
}

/// The reference's outputs of the form for the form `q`, with the lowered files laid over the files
/// as the machine reads them.
fn outs_for(mach: &ringi::engine::Machine, fields: &[Field], q: &Json) -> Result<ritsu_ports::Values, ringi::engine::eval::Unrouted> {
    let laid = ringi::engine::eval::overlay(mach.lowered);
    let got = ritsu_base::fs::with(laid, || ringi::engine::eval::form_outs(mach.s, mach.suite, mach.lowered, &values_of(fields, q)));
    got.map(|o| o.expect("the request has no lowered form"))
}

/// What the reference says of the output `o` on the form `pm`, as the machine is given it.
fn decide(mach: &ringi::engine::Machine, fields: &[Field], o: &Out, pm: &Json, seeds: &[Seed]) -> Expect {
    let Some(q) = ready(fields, &o.reads, pm, seeds) else { return Expect::Waits };
    if o.what == What::Value && no_value(fields, &o.reads, pm) {
        return Expect::Is(Json::Null);
    }
    match outs_for(mach, fields, &q) {
        Ok(outs) => {
            // the output by the name the lowered form gives it (`over_value` for `over`)
            let lf = mach.lowered.files.iter().find(|f| f.part == ringi::lower::Part::Form).expect("no lowered form");
            let named = lf.renamed.iter().find(|(_, w)| *w == o.output).map(|(g, _)| g.as_str()).unwrap_or(&o.output);
            match outs.iter().find(|(n, _)| n == named) {
                Some((_, v)) => Expect::Is(value_json(v)),
                None => panic!("the reference's form has no output {named}"),
            }
        }
        // rows fewer than their field asks for, which `form.rule` refuses at its door: what the copy
        // the screens read (`values.rule`) works out of the rows there so far (DESIGN 13.7)
        Err(_) if short(fields, &o.reads, pm) => match values_for(mach, fields, &q) {
            Ok(Some(outs)) => {
                let vf = mach.lowered.files.iter().find(|f| f.part == ringi::lower::Part::Values).expect("no copy of the lowered form");
                let named = vf.renamed.iter().find(|(_, w)| *w == o.output).map(|(g, _)| g.as_str()).unwrap_or(&o.output);
                match outs.iter().find(|(n, _)| n == named) {
                    Some((_, v)) => Expect::Is(value_json(v)),
                    None => panic!("the copy of the reference's form has no output {named}"),
                }
            }
            _ => Expect::Refused,
        },
        Err(_) => Expect::Refused,
    }
}

/// What the copy of the lowered form the screens read (`values.rule`) works out on the form `q`,
/// as [`outs_for`] asks the lowered form.
fn values_for(mach: &ringi::engine::Machine, fields: &[Field], q: &Json) -> Result<Option<ritsu_ports::Values>, ringi::engine::eval::Unrouted> {
    let laid = ringi::engine::eval::overlay(mach.lowered);
    ritsu_base::fs::with(laid, || ringi::engine::eval::values_outs(mach.s, mach.suite, mach.lowered, &values_of(fields, q)))
}

/// What the reference says of a computation held to its range (`amount`, `lines.subtotal`, reading
/// `reads`) on the form `pm`: whether it passes, as the screens' check `<x>.worked` says; it passes
/// while it waits, has no value, or reads rows fewer than their field asks for. None where the
/// reference refuses the form for something else.
fn hold_of(mach: &ringi::engine::Machine, fields: &[Field], x: &str, reads: &[String], pm: &Json, seeds: &[Seed]) -> Option<bool> {
    if short(fields, reads, pm) {
        return Some(true);
    }
    let Some(q) = ready(fields, reads, pm, seeds) else { return Some(true) };
    if !x.contains('.') && no_value(fields, reads, pm) {
        return Some(true);
    }
    match outs_for(mach, fields, &q) {
        Ok(_) => Some(true),
        Err(ringi::engine::eval::Unrouted::Outside(_, at)) => names(&at, x).then_some(false),
        Err(_) => None,
    }
}

/// Whether the place a refusal names (`at`) is the computation `x` (`amount`, `lines.subtotal`): the
/// field, or a computation of a row at a row of it (`lines[1].subtotal`).
fn names(at: &str, x: &str) -> bool {
    match x.split_once('.') {
        None => at == x,
        Some((rows, field)) => at
            .strip_prefix(rows)
            .and_then(|a| a.strip_prefix('['))
            .and_then(|a| a.strip_suffix(&format!("].{field}")))
            .is_some_and(|k| !k.is_empty() && k.bytes().all(|b| b.is_ascii_digit())),
    }
}

/// The fields a form hides: each shown on a condition that is not worked out to hold, the form
/// read without the ones hidden so far until nothing changes (a condition reads a hidden field as
/// empty).
fn hidden_of(mach: &ringi::engine::Machine, fields: &[Field], outs: &[Out], form: &Json, seeds: &[Seed]) -> Vec<String> {
    let mut hidden: Vec<String> = outs.iter().filter(|o| o.what == What::Shown).map(|o| o.field.clone()).collect();
    for _ in 0..8 {
        let pm = machine_view(fields, form, &hidden);
        let now: Vec<String> = outs.iter().filter(|o| o.what == What::Shown).filter(|o| !matches!(decide(mach, fields, o, &pm, seeds), Expect::Is(Json::Bool(true)))).map(|o| o.field.clone()).collect();
        if now == hidden {
            return hidden;
        }
        hidden = now;
    }
    panic!("the fields hidden do not settle on {}", form.compact())
}

/// The fields a screen's answer for `o` changes with: the fields it reads (rows by their field), and
/// the fields the conditions they are shown on read.
fn related(outs: &[Out], o: &Out) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut todo: Vec<String> = o.reads.iter().map(|r| r.split('.').next().unwrap_or(r).to_string()).collect();
    while let Some(r) = todo.pop() {
        if out.contains(&r) {
            continue;
        }
        if let Some(c) = outs.iter().find(|x| x.what == What::Shown && x.field == r) {
            todo.extend(c.reads.iter().map(|x| x.split('.').next().unwrap_or(x).to_string()));
        }
        out.push(r);
    }
    out.sort();
    out
}

/// A form as a screen holds it.
fn shown_form(fields: &[Field], form: &Json) -> Json {
    Json::obj(form.as_obj().unwrap_or_default().iter().map(|(k, v)| (k.clone(), fields.iter().find(|f| f.name == *k).map(|f| held(f, v)).unwrap_or_else(|| v.clone()))))
}

/// The module with what one output of the kind `kind` reads changed by `change`.
fn mutated(functions: &str, kind: &str, change: &dyn Fn(&mut Json)) -> String {
    let start = functions.find("const KINDS = ").expect("no KINDS in the functions") + "const KINDS = ".len();
    let end = start + functions[start..].find(";\n\nfunction ").expect("no end of KINDS");
    let mut kinds = json::parse(&functions[start..end]).unwrap();
    change(at(at(&mut kinds, kind), "worked"));
    format!("{}{}{}", &functions[..start], kinds.pretty(), &functions[end..])
}

/// The changes to what each output of a kind reads that make it one fewer, or one more field the
/// requester has to fill in, which holds it back, each with its words. `same(output, field)` is
/// whether the answers that read the output change with the field already (through the condition a
/// field they read is shown on): one more such field changes nothing they say, and is no change; nor
/// is one more field of rows, whose rows the rule is given as they are there.
fn mutants(worked: &Json, same: &dyn Fn(&str, &str) -> bool) -> Vec<(String, Box<dyn Fn(&mut Json)>)> {
    let params = worked.get("params").and_then(Json::as_arr).unwrap_or_default().to_vec();
    let reads = worked.get("reads").and_then(Json::as_obj).unwrap_or_default().to_vec();
    let name = |i: usize| params[i].as_arr().and_then(|p| p.get(1)).and_then(Json::as_str).unwrap_or_default().to_string();
    let mut out: Vec<(String, Box<dyn Fn(&mut Json)>)> = Vec::new();
    for (code, rs) in reads {
        let rs = rs.as_arr().unwrap_or_default().to_vec();
        for (e, r) in rs.iter().enumerate() {
            let i = r.as_arr().and_then(|x| x.first()).and_then(Json::as_int).unwrap() as usize;
            let cols: Vec<Json> = r.as_arr().and_then(|x| x.get(1)).and_then(Json::as_arr).unwrap_or_default().to_vec();
            let c = code.clone();
            out.push((
                format!("{code} reads no {}", name(i)),
                Box::new(move |w: &mut Json| {
                    let Json::Arr(list) = at(at(w, "reads"), &c) else { panic!() };
                    list.remove(e);
                }),
            ));
            if cols.len() > 1 {
                for (ci, col) in cols.iter().enumerate() {
                    let c = code.clone();
                    out.push((
                        format!("{code} reads no {}.{}", name(i), col.as_str().unwrap_or_default()),
                        Box::new(move |w: &mut Json| {
                            let Json::Arr(list) = at(at(w, "reads"), &c) else { panic!() };
                            let Json::Arr(entry) = &mut list[e] else { panic!() };
                            let Json::Arr(cs) = &mut entry[1] else { panic!() };
                            cs.remove(ci);
                        }),
                    ));
                }
            }
        }
        let read: Vec<usize> = rs.iter().filter_map(|r| r.as_arr().and_then(|x| x.first()).and_then(Json::as_int)).map(|i| i as usize).collect();
        for (i, p) in params.iter().enumerate().filter(|(i, _)| !read.contains(i) && !same(&code, &name(*i))) {
            let p = p.as_arr().unwrap_or_default();
            let kind = p.first().and_then(Json::as_str).unwrap_or_default();
            let cols = match kind {
                "value" if p.get(3).and_then(Json::as_str) == Some("wait") => Json::arr([]),
                _ => continue,
            };
            let c = code.clone();
            out.push((
                format!("{code} reads {} too", name(i)),
                Box::new(move |w: &mut Json| {
                    let Json::Arr(list) = at(at(w, "reads"), &c) else { panic!() };
                    list.push(Json::arr([Json::int(i as i128), cols.clone()]));
                }),
            ));
        }
    }
    out
}

/// What runs the functions over the steps of each kind: for each run (the module as written, or one
/// changed), each output's answer on each step against the reference's, and against its answer with
/// each field it does not read emptied or filled in.
const DRIVER: &str = r#"import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { join } from "node:path";

const dir = process.argv[2];
const { kinds, runs } = JSON.parse(readFileSync(join(dir, "steps.json"), "utf8"));
const canon = (v) => JSON.stringify(v === undefined ? null : v);
const filled = (v) => !(v === null || v === undefined || v === "" || v === false || (Array.isArray(v) && v.length === 0));
const out = [];
for (const run of runs) {
  const { functions } = await import(pathToFileURL(join(dir, run.module)).href);
  const failures = [];
  let compared = 0, skipped = 0, kept = 0;
  for (const name of run.kinds) {
    const k = kinds[name];
    const call = (fn, form) => {
      const [f, code] = fn.split(" ");
      if (!functions[f]) {
        failures.push(`${f} is not defined`);
        return null;
      }
      return code === undefined ? functions[f]({ form }) : functions[f]({ form, code });
    };
    for (const s of k.steps) {
      for (const o of k.outs) {
        const got = call(o.function, s.form);
        const want = s.expect[o.function];
        if (want === "skip") skipped++;
        else {
          compared++;
          if (canon(got) !== canon(want)) failures.push(`${name}, ${s.what}: ${o.function} is ${canon(got)}, and the reference's is ${canon(want)}`);
        }
        for (const g of k.fields) {
          if (o.related.includes(g)) continue;
          const alt = Object.assign({}, s.form);
          if (filled(alt[g])) delete alt[g];
          else if (k.alt[g] !== undefined) alt[g] = k.alt[g];
          else continue;
          kept++;
          const again = call(o.function, alt);
          if (canon(again) !== canon(got)) failures.push(`${name}, ${s.what}: ${o.function} is ${canon(got)}, and ${canon(again)} with ${g} ${filled(s.form[g]) ? "emptied" : "filled in"}, which it does not read`);
        }
      }
    }
  }
  out.push({ id: run.id, compared, skipped, kept, failures: failures.length, first: failures.slice(0, 8) });
}
console.log(JSON.stringify(out));
"#;

#[test]
fn each_condition_and_computation_is_worked_out_from_what_it_reads_as_the_form_is_filled_in() {
    if !node() {
        return;
    }
    let mut paths: Vec<PathBuf> = crate::common::example_files().into_iter().filter(|f| !f.starts_with("examples/shared/")).map(PathBuf::from).collect();
    paths.extend(MACHINE.iter().map(PathBuf::from));
    let m = made_of(&paths);
    let functions = file(&m, "a2ui/functions.js").to_string();
    let start = functions.find("const KINDS = ").unwrap() + "const KINDS = ".len();
    let end = start + functions[start..].find(";\n\nfunction ").unwrap();
    let all_kinds = json::parse(&functions[start..end]).unwrap();
    let mut kinds: Vec<(String, Json)> = Vec::new();
    let mut runs: Vec<Json> = vec![Json::obj([("id", Json::str("as written")), ("module", Json::str("functions.js")), ("kinds", Json::arr(m.kinds.iter().map(|k| Json::str(&k.named.name))))])];
    let mut written: Vec<(String, String)> = Vec::new();
    let mut table = String::new();
    for k in &m.kinds {
        let name = k.named.name.clone();
        let types = json::parse(file(&m, &format!("types/{name}.json"))).unwrap();
        let fields = fields_of(types.get("form").unwrap());
        let outs = outs_of(&name, &fields);
        // the computations held to their ranges, by the screens' checks, each with what it reads
        let spec = all_kinds.get(&name).and_then(|s| s.get("worked")).filter(|w| **w != Json::Null);
        let holds: Vec<(String, String, Vec<String>, Vec<String>)> = spec
            .and_then(|w| w.get("held"))
            .and_then(Json::as_arr)
            .unwrap_or_default()
            .iter()
            .filter_map(|h| {
                let code = h.get("code").and_then(Json::as_str)?.to_string();
                let x = code.strip_suffix(".worked")?.to_string();
                let f = match x.split_once('.') {
                    Some((r, c)) => fields.iter().find(|f| f.name == r)?.fields.iter().find(|f| f.name == c)?,
                    None => fields.iter().find(|f| f.name == x)?,
                };
                // the outputs of the rule it holds to its ends
                let ends: Vec<String> = ["lo", "hi"].iter().filter_map(|e| h.get(e).and_then(Json::as_arr).and_then(|a| a.first()).and_then(Json::as_str).map(str::to_string)).collect();
                Some((code, x, f.reads.clone(), ends))
            })
            .collect();
        // what each output of the rule is read for, by the fields each answer that reads it changes
        // with
        let code_of = |o: &Out| spec.and_then(|w| w.get("outputs")).and_then(|x| x.get(&o.output)).and_then(Json::as_str).unwrap_or(&o.output).to_string();
        let mut read_for: Vec<(String, Vec<String>)> = outs.iter().map(|o| (code_of(o), related(&outs, o))).collect();
        for (_, x, reads, ends) in &holds {
            let o = Out { function: String::new(), output: x.clone(), what: What::Value, field: x.clone(), reads: reads.clone() };
            read_for.extend(ends.iter().map(|e| (e.clone(), related(&outs, &o))));
        }
        let run = Run::with(crate::common::joined(), Options { unlisted: true, unrun: true, ..Options::default() });
        let o = run.file(&k.file).unwrap_or_else(|e| panic!("{}: {e}", k.file));
        let (steps_json, count, refused, alt) = ringi::engine::with(&o, &run, |mach, org| {
            let seeds = seeds(mach, org);
            let mut list = Vec::new();
            let mut refused = 0;
            let all = steps(&fields, &seeds);
            for (what, form) in &all {
                let hidden = hidden_of(mach, &fields, &outs, form, &seeds);
                let pm = machine_view(&fields, form, &hidden);
                let mut expect: Vec<(String, Json)> = Vec::new();
                for (code, x, reads, _) in &holds {
                    let want = match hold_of(mach, &fields, x, reads, &pm, &seeds) {
                        Some(b) => Json::Bool(b),
                        None => {
                            refused += 1;
                            Json::str("skip")
                        }
                    };
                    expect.push((format!("{name}.check {code}"), want));
                }
                for o in &outs {
                    let e = decide(mach, &fields, o, &pm, &seeds);
                    let field = fields.iter().find(|f| f.name == o.field).unwrap();
                    let want = match (o.what, e) {
                        (_, Expect::Refused) => {
                            refused += 1;
                            Json::str("skip")
                        }
                        (What::Required, _) if !field.given() => Json::Bool(false),
                        (What::Shown | What::Required, Expect::Waits) => Json::Bool(false),
                        (What::Value, Expect::Waits) => Json::Null,
                        (_, Expect::Is(v)) => v,
                    };
                    expect.push((o.function.clone(), want));
                }
                list.push(Json::obj([("what", Json::str(what)), ("form", shown_form(&fields, form)), ("expect", Json::obj(expect))]));
            }
            // what a field an output does not read is filled in with, as a screen holds it
            let alt = Json::obj(fields.iter().filter(|f| f.given()).filter_map(|f| {
                let v = match f.ty {
                    Ty::Rows => Json::arr([Json::obj(f.fields.iter().filter(|c| c.given()).filter_map(|c| Some((c.name.clone(), held(c, &cell_sample(f, c, &seeds)?)))))]),
                    _ => held(f, &sample(f, &seeds)?),
                };
                Some((f.name.clone(), v))
            }));
            (list, all.len(), refused, alt)
        })
        .unwrap_or_else(|t| panic!("{}: {}", k.file, t.en));
        table.push_str(&format!("{name}: {} outputs, {count} steps, {refused} answers the reference refuses\n", outs.len()));
        let given: Vec<Json> = fields.iter().filter(|f| f.given()).map(|f| Json::str(&f.name)).collect();
        let mut outs_json: Vec<Json> = outs.iter().map(|o| Json::obj([("function", Json::str(&o.function)), ("related", Json::arr(related(&outs, o).iter().map(Json::str)))])).collect();
        for (code, x, reads, _) in &holds {
            let o = Out { function: format!("{name}.check {code}"), output: x.clone(), what: What::Value, field: x.clone(), reads: reads.clone() };
            outs_json.push(Json::obj([("function", Json::str(&o.function)), ("check", Json::str(code)), ("related", Json::arr(related(&outs, &o).iter().map(Json::str)))]));
        }
        let outs_json = Json::arr(outs_json);
        kinds.push((name.clone(), Json::obj([("steps", Json::arr(steps_json)), ("outs", outs_json), ("fields", Json::arr(given)), ("alt", alt)])));
        // the functions with what one output reads changed: each gives itself away on the kind's steps
        if let Some(w) = spec {
            let same = |code: &str, field: &str| {
                let users: Vec<&Vec<String>> = read_for.iter().filter(|(c, _)| c == code).map(|(_, r)| r).collect();
                !users.is_empty() && users.iter().all(|r| r.iter().any(|x| x == field))
            };
            for (said, change) in mutants(w, &same) {
                let module = format!("m{}.js", written.len());
                written.push((module.clone(), mutated(&functions, &name, change.as_ref())));
                runs.push(Json::obj([("id", Json::str(format!("{name}: {said}"))), ("module", Json::str(module)), ("kinds", Json::arr([Json::str(&name)]))]));
            }
        }
    }
    let t = ritsu_testkit::TempDir::new("ringi-a2ui-steps");
    t.write("functions.js", &functions);
    for (module, text) in &written {
        t.write(module, text);
    }
    t.write("package.json", "{\"type\": \"module\"}\n");
    t.write("steps.json", Json::obj([("kinds", Json::obj(kinds)), ("runs", Json::arr(runs.clone()))]).compact());
    t.write("driver.mjs", DRIVER);
    let r = ritsu_testkit::run::finish(Command::new("node").arg(t.path().join("driver.mjs")).arg(t.path()), Duration::from_secs(600), "node, running the functions over the steps");
    assert!(r.ok, "{}", r.both());
    let got = json::parse(r.stdout.trim()).unwrap_or_else(|e| panic!("{e:?}: {}", r.stdout));
    let got = got.as_arr().unwrap();
    let first = &got[0];
    let n = |j: &Json, k: &str| j.get(k).and_then(Json::as_int).unwrap_or(-1);
    println!("{table}as written: {} answers compared, {} not ({}), {} kept with a field the output does not read changed", n(first, "compared"), n(first, "skipped"), "the reference refuses the form", n(first, "kept"));
    assert_eq!(n(first, "failures"), 0, "{}", first.get("first").map(Json::pretty).unwrap_or_default());
    assert!(n(first, "compared") > 500 && n(first, "kept") > 5000, "{}", first.compact());
    // every change gives itself away
    let mut kept_alive = Vec::new();
    for g in &got[1..] {
        if n(g, "failures") == 0 {
            kept_alive.push(g.get("id").and_then(Json::as_str).unwrap_or_default().to_string());
        }
    }
    println!("{} changes to what an output reads, each found", got.len() - 1);
    assert!(got.len() > 20, "{} changes", got.len() - 1);
    assert!(kept_alive.is_empty(), "the changes not found:\n{}", kept_alive.join("\n"));
}
