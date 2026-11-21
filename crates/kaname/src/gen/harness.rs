//! A program that runs a package's code of kaname on inputs as `vectors` writes them, for the tests
//! that hold the code to the reference interpreter (DESIGN 16.4). It is not part of a package: the
//! tests write it beside one.
//!
//! It reads a JSON array of cases (`{"m": <module's alias>, "f": <function's index>, "in": <the
//! arguments on the wire>}`, the file named by its first argument), calls each function as a host
//! does, and prints a line for each: `{"out": <the answer on the wire>}`, or `{"fails": {"kind",
//! "module", "line", "message"}}` for a `KanameError`, or `{"error": …}` for anything else. The
//! values on the wire are those of `vectors` (DESIGN 9): an amount in its unit, a rate as a count of
//! its step, a date `YYYY-MM-DD`, an enum's value by its name, a record by its fields' names.
//!
//! Rust's harness is the package's door instead (DESIGN 36.9): a `main` that gives each line of a file
//! to `glue::door::answer` and prints the line it answers. Each line is a call as the package's Wasm
//! takes it ([`call_lines`]), and [`compare_lines`] holds the answers to the reference interpreter's.

use super::Target;
use super::plan::{field_name, fn_name, go_package, type_name, variant_name};
use crate::check::Checked;
use crate::suite::Suite;
use crate::ty::{Owner, RecId, Ty};
use crate::num::Rat;
use crate::value::{Program, Value};
use ritsu_base::json::Json;
use ritsu_base::text::Lang;
use ritsu_ports::{ColumnType, RuleFacts};
use std::collections::BTreeMap;
use std::rc::Rc;

/// A case of the harness and what the reference interpreter answers for it (`out` or `fails`, as
/// `vectors` writes it); None where it does not stop (DESIGN 8: not held to the generated code).
#[derive(Clone, Debug)]
pub struct Case {
    pub module: String,
    pub function: String,
    pub input: Json,
    pub expected: Option<Json>,
    /// The targets it runs in: all of them, but for an input a target's values cannot hold (DESIGN
    /// 22.4): a number past int64 (Go's and Rust's `int64`), a day outside 0001-01-01 to 9999-12-31
    /// and a day of no calendar (Python's `datetime.date`).
    pub targets: Vec<Target>,
    /// Of a vector, the line `vectors` writes for it without its `why`: Rust's door answers it to the
    /// letter (DESIGN 36.11).
    pub line: Option<String>,
}

impl Case {
    fn of(module: String, function: String, input: Json, expected: Option<Json>) -> Case {
        Case { module, function, input, expected, targets: Target::ALL.to_vec(), line: None }
    }
}

/// The cases a target runs.
pub fn of_target(cases: &[Case], t: Target) -> Vec<Case> {
    cases.iter().filter(|c| c.targets.contains(&t)).cloned().collect()
}

/// Every vector of every function of the modules, run in the reference interpreter with the
/// languages of `suite` and the messages in `lang` (DESIGN 9); and how many do not stop.
pub fn cases(modules: &[Rc<Checked>], suite: &Suite, lang: Lang) -> (Vec<Case>, usize) {
    let mut out = Vec::new();
    let mut diverged = 0;
    for c in modules {
        let p = Program::of(c);
        for f in 0..c.decls.fns.len() {
            let (vs, _) = crate::vectors::choose(&p, c, f);
            for v in &vs {
                let line = crate::vectors::line(&p, suite, c, f, v, false, lang);
                let expected = if let Some(x) = line.get("out") {
                    Some(Json::obj([("out", x.clone())]))
                } else if let Some(x) = line.get("fails") {
                    Some(Json::obj([("fails", x.clone())]))
                } else {
                    diverged += 1;
                    None
                };
                let mut case = Case::of(c.module.name.ascii().to_string(), f.to_string(), line.get("in").cloned().unwrap_or(Json::Null), expected);
                if let Json::Obj(fs) = &line {
                    case.line = Some(Json::Obj(fs.iter().filter(|(k, _)| k != "why").cloned().collect()).compact());
                }
                out.push(case);
            }
        }
    }
    (out, diverged)
}

/// Inputs a test writes itself, each `(module, function, input)` by the ASCII names of the module and
/// the function, the input as the cases write it (`{"p": {"weight": 40000, "fragile": false}}`), run
/// in the reference interpreter: its answer, the function's or the entry's failure `input` (DESIGN
/// 4.4), is what each target is held to. An input of a module that is not among `modules` is left
/// out (a test that runs modules of its own); one that names no function of its module, or that is no
/// value of its parameters' types, stops the test.
pub fn given_cases(modules: &[Rc<Checked>], suite: &Suite, lang: Lang, given: &[(&str, &str, &str)]) -> Vec<Case> {
    given
        .iter()
        .filter_map(|(module, function, input)| {
            let c = modules.iter().find(|c| c.module.name.ascii() == *module)?;
            let f = c.decls.fns.iter().position(|x| x.name.ascii() == *function).unwrap_or_else(|| panic!("no function {function} in {module}"));
            let info = &c.decls.fns[f];
            let p = Program::of(c);
            let j = ritsu_base::json::parse(input).unwrap_or_else(|e| panic!("{module}.{function}: {input}: {e:?}"));
            let values: Vec<Value> = info
                .params
                .iter()
                .map(|prm| {
                    let x = j.get(&prm.name.text).cloned().unwrap_or(Json::Null);
                    p.from_json(c, &x, &prm.slot.ty, &prm.name.text).unwrap_or_else(|e| panic!("{module}.{function}: {input}: {}", e.why.get(Lang::En)))
                })
                .collect();
            let line = crate::vectors::line(&p, suite, c, f, &crate::vectors::Vector { input: values, why: None }, false, lang);
            let expected = line.get("fails").map(|x| Json::obj([("fails", x.clone())])).or_else(|| line.get("out").map(|x| Json::obj([("out", x.clone())])));
            Some(Case::of(c.module.name.ascii().to_string(), f.to_string(), line.get("in").cloned().unwrap_or(Json::Null), expected))
        })
        .collect()
}

/// Inputs outside what a function's parameters declare, each at one part of one parameter (a step
/// past an end of a number's range, a day past a date's, one element past a list's count, one short
/// of it), the other parts at their base values, run in the reference interpreter: its answer is the
/// entry's failure `input` (DESIGN 4.4), which the vectors never reach (every vector is inside what
/// the parameters declare) and the code's public function is held to as well. Where a range is not
/// written, past the type's ends (DESIGN 22.4): 2⁶³ and −2⁶³ − 1 for a number, run in TypeScript and
/// Python; 10000-01-01 and 0000-12-31 for a date, and 2026-13-01, a day of no calendar (the
/// reference's answer is that of 10000-01-01, the day written so), run in TypeScript, Go and Rust, and
/// in Python too where the date is a field of an element of a rule's list, which every harness reads
/// as the rule's count of days and refuses where there is none (DESIGN 4.4, 22.5). A string
/// one code point past its count, or one short of it; and a string that is not well formed, which the
/// reference interpreter cannot hold, so its answer is the entry's failure `input` made here (DESIGN
/// 25.8): each target makes its own of the marker the case writes (`{"bad": "string"}`), a lone
/// surrogate in TypeScript and Python, a byte that is no UTF-8 in Go, and the escape of a lone surrogate
/// in the JSON Rust's door reads.
pub fn outside_cases(modules: &[Rc<Checked>], suite: &Suite, lang: Lang) -> Vec<Case> {
    use crate::vectors::{Path, Step, Vector, base_at, set_at, value_at};
    /// A day the cases write where a date's range is not written, of no calendar.
    const NO_DAY: &str = "2026-13-01";
    /// The string the cases put where they write the marker of a string that is not well formed.
    const BAD: &str = "\u{fdd0}kaname: no well-formed string\u{fdd0}";
    let mut out = Vec::new();
    for c in modules {
        let p = Program::of(c);
        for (f, info) in c.decls.fns.iter().enumerate() {
            // a generic function has no entry a host calls (DESIGN 21.3)
            if info.inner() {
                continue;
            }
            // every part of every parameter, with what it declares
            let mut parts: Vec<(Path, crate::ty::Slot)> = Vec::new();
            for (i, prm) in info.params.iter().enumerate() {
                walk(&p, (i, Vec::new()), prm.slot.clone(), &mut parts, 0, &[]);
            }
            let mut inputs: Vec<(Vec<Value>, Vec<Target>)> = Vec::new();
            let mut past_days: Vec<Vec<Value>> = Vec::new();
            let mut bad: Vec<Case> = Vec::new();
            for (at, slot) in &parts {
                // the base values, with the value of a sum the part is in (DESIGN 23.6)
                let base = base_at(&p, c, f, at);
                let mut ty = slot.ty.clone();
                while let Ty::Opt(x) = ty {
                    ty = *x;
                }
                let mut put = |v: Value, targets: &[Target]| -> Option<Vec<Value>> {
                    let i = set_at(&base, at, v)?;
                    if inputs.iter().any(|(x, _)| *x == i) {
                        return None;
                    }
                    inputs.push((i.clone(), targets.to_vec()));
                    Some(i)
                };
                match &ty {
                    Ty::Num(n) => {
                        let step = if n.is_rate() { n.step.unwrap_or(Rat::new(1, 100)) } else { Rat::int(1) };
                        let written = match (&slot.range, n.is_rate() && n.step.is_some()) {
                            (Some(r), _) => Some(r.clone()),
                            (None, true) => Some(crate::ty::Range { min: Some(Rat::zero()), max: Some(Rat::int(1)) }),
                            _ => None,
                        };
                        let whole = crate::ty::whole_range(&ty).unwrap_or_default();
                        // the value as it travels: a rate as a count of its step (DESIGN 9)
                        let travels = |v: Rat| crate::num::fits(if n.is_rate() { v.div(step) } else { v });
                        for (end, step) in [
                            (written.as_ref().and_then(|r| r.max), step),
                            (written.as_ref().and_then(|r| r.min), Rat::zero().sub(step)),
                            (written.as_ref().is_none_or(|r| r.max.is_none()).then_some(whole.max).flatten(), Rat::int(1)),
                            (written.as_ref().is_none_or(|r| r.min.is_none()).then_some(whole.min).flatten(), Rat::int(-1)),
                        ] {
                            let Some(v) = end.map(|e| e.add(step)) else { continue };
                            if travels(v) {
                                put(Value::Num(v), &Target::ALL);
                            } else {
                                // past int64 as it travels, whether the end is written or not: Go's and
                                // Rust's int64 cannot hold it (DESIGN 44)
                                put(Value::Num(v), &[Target::TypeScript, Target::Python]);
                            }
                        }
                    }
                    Ty::Date => {
                        if let Some(r) = &slot.range {
                            for d in [r.max.map(|hi| hi.num as i64 + 1), r.min.map(|lo| lo.num as i64 - 1)].into_iter().flatten() {
                                if (crate::date::MIN..=crate::date::MAX).contains(&d) {
                                    put(Value::Date(d), &Target::ALL);
                                }
                            }
                        }
                        // past the days of the calendar, where an end is not written: Python's date
                        // cannot hold them, so its entry is not reached. A field of an element of a
                        // rule's list is read by every harness before the entry, and refused there as
                        // the reference's entry refuses it (DESIGN 4.4, 22.5): every target runs those
                        let open = slot.range.as_ref().is_none_or(|r| r.min.is_none() || r.max.is_none());
                        if open {
                            let element = at.1.split_last().is_some_and(|(last, before)| {
                                matches!(last, Step::Field(_)) && crate::vectors::slot_at(&p, info, &(at.0, before.to_vec())).is_some_and(|s| matches!(s.ty.present(), Ty::Record(RecId::Element(..))))
                            });
                            let past: &[Target] = if element { &Target::ALL } else { &[Target::TypeScript, Target::Go, Target::Rust] };
                            if let Some(i) = put(Value::Date(crate::date::MAX + 1), past) {
                                past_days.push(i);
                            }
                            put(Value::Date(crate::date::MIN - 1), past);
                        }
                    }
                    _ => {}
                }
                // a dict or a set a pair or a member past its count, or one short of it; one whose
                // first value or member is past its range (DESIGN 24.4)
                if let Ty::Dict(_, _) | Ty::Set(_) = &ty {
                    let (kt, vt) = match &ty {
                        Ty::Dict(k, v) => ((**k).clone(), Some((**v).clone())),
                        Ty::Set(x) => ((**x).clone(), None),
                        _ => (Ty::Error, None),
                    };
                    let kslot = match &vt {
                        Some(_) => crate::ty::Slot::of(kt.clone()),
                        None => crate::ty::Slot { ty: kt.clone(), range: slot.range.clone(), count: None },
                    };
                    let val = vt.as_ref().map(|v| crate::vectors::base_of(&p, &crate::ty::Slot { ty: v.clone(), range: slot.range.clone(), count: None }));
                    let made = |n: usize| -> Option<Value> {
                        let keys = crate::vectors::keys_of(&p, &kslot, &kt, n);
                        if keys.len() != n {
                            return None;
                        }
                        Some(match &val {
                            Some(v) => Value::Dict(p.dict_of(&kt, keys.into_iter().map(|k| (k, v.clone())).collect())),
                            None => Value::Set(p.set_of(&kt, keys)),
                        })
                    };
                    if let Some(count) = &slot.count {
                        if let Some(hi) = count.max
                            && let Some(v) = made(hi.num.max(0) as usize + 1)
                        {
                            put(v, &Target::ALL);
                        }
                        if let Some(lo) = count.min.filter(|lo| lo.num > 0)
                            && let Some(v) = made(lo.num as usize - 1)
                        {
                            put(v, &Target::ALL);
                        }
                    }
                    let elem = vt.clone().unwrap_or(kt.clone());
                    if let (Some(r), Ty::Num(n), Some(whole)) = (&slot.range, &elem, value_at(&base, at).cloned()) {
                        let step = if n.is_rate() { n.step.unwrap_or(Rat::new(1, 100)) } else { Rat::int(1) };
                        for end in [r.max.map(|h| h.add(step)), r.min.map(|l| l.sub(step))].into_iter().flatten() {
                            let changed = match &whole {
                                Value::Dict(ps) if !ps.is_empty() => {
                                    let mut qs = ps.clone();
                                    qs[0].1 = Value::Num(end);
                                    Some(Value::Dict(qs))
                                }
                                Value::Set(ms) if !ms.is_empty() && !ms.contains(&Value::Num(end)) => {
                                    let mut xs = ms.clone();
                                    xs[0] = Value::Num(end);
                                    Some(Value::Set(p.set_of(&kt, xs)))
                                }
                                _ => None,
                            };
                            // as it travels: a rate as a count of its step; past int64 so, to the
                            // targets whose numbers hold it alone, as a number's (DESIGN 44)
                            if let Some(v) = changed {
                                if crate::num::fits(if n.is_rate() { end.div(step) } else { end }) {
                                    put(v, &Target::ALL);
                                } else {
                                    put(v, &[Target::TypeScript, Target::Python]);
                                }
                            }
                        }
                    }
                }
                // a list one element past its count, or one short of it: the base list, its first
                // element repeated
                if let (Ty::List(_), Some(count), Some(Value::List(xs))) = (&ty, &slot.count, value_at(&base, at)) {
                    let first = xs.first().cloned();
                    if let (Some(hi), Some(x)) = (count.max, first) {
                        put(Value::List(vec![x; hi.num.max(0) as usize + 1]), &Target::ALL);
                    }
                    if let Some(lo) = count.min.filter(|lo| lo.num > 0) {
                        put(Value::List(xs.iter().take(lo.num as usize - 1).cloned().collect()), &Target::ALL);
                    }
                }
                // a string one code point past its count, or one short of it (DESIGN 25.8)
                if let (Ty::Str, Some(count)) = (&ty, &slot.count) {
                    if let Some(hi) = count.max {
                        put(Value::Str("a".repeat(hi.num.max(0) as usize + 1)), &Target::ALL);
                    }
                    if let Some(lo) = count.min.filter(|lo| lo.num > 0) {
                        put(Value::Str("a".repeat(lo.num as usize - 1)), &Target::ALL);
                    }
                }
                // a string that is not well formed: the base input, the string a marker each target
                // makes its own of
                if matches!(ty, Ty::Str)
                    && let Some(i) = set_at(&base, at, Value::Str(BAD.into()))
                {
                    let input = Json::obj(info.params.iter().zip(&i).map(|(prm, x)| (prm.name.text.clone(), p.to_json(x, &prm.slot.ty))));
                    let input = marked(&input);
                    // the place as the check writes it: a field of a value of a sum after the value's
                    // name (`o.placed.on`), an element by its place
                    let mut path = info.params[at.0].name.text.clone();
                    let mut v = i.get(at.0);
                    for (k, st) in at.1.iter().enumerate() {
                        // the value a result holds, after `ok` or `err` alone (`r.ok`, DESIGN 26.3)
                        let result = crate::vectors::slot_at(&p, info, &(at.0, at.1[..k].to_vec())).is_some_and(|s| matches!(s.ty.present(), Ty::Result(..)));
                        match (st, v) {
                            (Step::Field(f), Some(Value::Variant(tag, fs))) => {
                                path = if result { format!("{path}.{tag}") } else { format!("{path}.{tag}.{f}") };
                                v = fs.iter().find(|(n, _)| n == f).map(|(_, x)| x);
                            }
                            (Step::Field(f), Some(Value::Record(fs))) => {
                                path = format!("{path}.{f}");
                                v = fs.iter().find(|(n, _)| n == f).map(|(_, x)| x);
                            }
                            (Step::Elem, Some(Value::List(xs))) => {
                                path = format!("{path}[0]");
                                v = xs.first();
                            }
                            _ => {}
                        }
                    }
                    let fun = &info.name.text;
                    let msg = tr!("関数 `{fun}` の引数 `{path}` は、Unicode の文字列として正しくありません", "The argument `{path}` of `{fun}` is no well-formed string of Unicode");
                    let fails = Json::obj([
                        ("kind", Json::str("input")),
                        ("module", Json::str(c.module.name.text.clone())),
                        ("line", Json::int(info.line as i128)),
                        ("message", Json::str(ritsu_base::text::spaced(&msg, lang))),
                        ("path", Json::str(path.clone())),
                    ]);
                    bad.push(Case::of(c.module.name.ascii().to_string(), f.to_string(), input, Some(Json::obj([("fails", fails)]))));
                }
            }
            for (input, targets) in inputs {
                let past = past_days.contains(&input);
                let line = crate::vectors::line(&p, suite, c, f, &Vector { input, why: None }, false, lang);
                let expected = line.get("fails").map(|x| Json::obj([("fails", x.clone())])).or_else(|| line.get("out").map(|x| Json::obj([("out", x.clone())])));
                let given = line.get("in").cloned().unwrap_or(Json::Null);
                if past {
                    // the same input with a day of no calendar written where 10000-01-01 is: the
                    // reference's failure the same, the day it says written so
                    let day = crate::date::text(crate::date::MAX + 1);
                    let mut case = Case::of(c.module.name.ascii().to_string(), f.to_string(), retext(&given, &day, NO_DAY), expected.as_ref().map(|e| retext(e, &day, NO_DAY)));
                    case.targets = targets.clone();
                    out.push(case);
                }
                let mut case = Case::of(c.module.name.ascii().to_string(), f.to_string(), given, expected);
                case.targets = targets;
                out.push(case);
            }
            out.extend(bad);
        }
    }
    return out;

    /// The input with the string [`BAD`] written as the marker of a string that is not well formed.
    fn marked(j: &Json) -> Json {
        match j {
            Json::Str(s) if s == BAD => Json::obj([("bad", Json::str("string"))]),
            Json::Arr(xs) => Json::Arr(xs.iter().map(marked).collect()),
            Json::Obj(fs) => Json::Obj(fs.iter().map(|(k, v)| (k.clone(), marked(v))).collect()),
            x => x.clone(),
        }
    }

    /// Every part of a parameter at `at` that declares a range or a count, or holds a number or a
    /// date (whose range not written is the type's, DESIGN 22.1): the part, then its fields and its
    /// first element.
    fn walk(p: &Program, at: Path, slot: crate::ty::Slot, parts: &mut Vec<(Path, crate::ty::Slot)>, depth: usize, nest: &[Ty]) {
        if depth > 8 {
            return;
        }
        let mut ty = slot.ty.clone();
        while let Ty::Opt(x) = ty {
            ty = *x;
        }
        // a value of a type that holds itself is walked once on a path: the base values hold no other
        // inside it (DESIGN 28.1)
        let mut nest = nest.to_vec();
        if let Some(k) = p.nest_key(&ty) {
            if nest.contains(&k) {
                return;
            }
            nest.push(k);
        }
        let nest = nest.as_slice();
        if slot.range.is_some() || slot.count.is_some() || matches!(&ty, Ty::Num(_) | Ty::Date | Ty::Str) {
            parts.push((at.clone(), slot.clone()));
        }
        match ty {
            Ty::List(x) => {
                let mut next = at.1.clone();
                next.push(Step::Elem);
                walk(p, (at.0, next), crate::ty::Slot { ty: *x, range: slot.range.clone(), count: None }, parts, depth + 1, nest);
            }
            Ty::Record(id) => {
                for (name, s) in p.fields(&id) {
                    let mut next = at.1.clone();
                    next.push(Step::Field(name.text.clone()));
                    walk(p, (at.0, next), s, parts, depth + 1, nest);
                }
            }
            // a result: the value of each of `ok` and `err` (`r.ok`, DESIGN 26.3)
            Ty::Result(a, e) => {
                for (tag, field, inner) in [("ok", "value", a), ("err", "error", e)] {
                    let mut next = at.1.clone();
                    next.push(Step::Value(tag.into()));
                    next.push(Step::Field(field.into()));
                    walk(p, (at.0, next), crate::ty::Slot::of(*inner), parts, depth + 1, nest);
                }
            }
            // a sum: the fields of each of its values (`o.placed.on`)
            Ty::Enum(id) if p.is_sum(&id) => {
                for v in p.values(&id).0 {
                    let mut next = at.1.clone();
                    next.push(Step::Value(v.text.clone()));
                    let Some(r) = crate::ty::RecId::variant(&id, v.text.clone()) else { return };
                    let t = Ty::Record(r);
                    walk(p, (at.0, next), crate::ty::Slot::of(t), parts, depth + 1, nest);
                }
            }
            _ => {}
        }
    }

    /// JSON with `from` written `to` in every string: a day in an input, and in a message.
    fn retext(j: &Json, from: &str, to: &str) -> Json {
        match j {
            Json::Str(s) => Json::Str(s.replace(from, to)),
            Json::Arr(xs) => Json::Arr(xs.iter().map(|x| retext(x, from, to)).collect()),
            Json::Obj(fs) => Json::Obj(fs.iter().map(|(k, v)| (k.clone(), retext(v, from, to))).collect()),
            x => x.clone(),
        }
    }
}

/// The cases as the harness reads them: those the interpreter answers.
pub fn cases_json(cases: &[Case]) -> String {
    Json::arr(cases.iter().filter(|c| c.expected.is_some()).map(|c| Json::obj([("m", Json::str(c.module.clone())), ("f", Json::int(c.function.parse::<i128>().unwrap_or(0))), ("in", c.input.clone())]))).compact()
}

/// The cases as Rust's harness reads them, those the interpreter answers: each a line of the call the
/// package's door takes (`{"module","function","in"}`, each name as the module writes it). The marker
/// of a string that is not well formed is written as the escape of a lone surrogate (`"\ud800"`),
/// which the door reads as such a string (DESIGN 36.11).
pub fn call_lines(modules: &[Rc<Checked>], cases: &[Case]) -> String {
    const SENTINEL: &str = "\u{fdd0}kaname: a lone surrogate\u{fdd0}";
    fn unmarked(j: &Json) -> Json {
        match j {
            Json::Obj(fs) if fs.len() == 1 && fs[0].0 == "bad" && fs[0].1 == Json::str("string") => Json::str(SENTINEL),
            Json::Arr(xs) => Json::Arr(xs.iter().map(unmarked).collect()),
            Json::Obj(fs) => Json::Obj(fs.iter().map(|(k, v)| (k.clone(), unmarked(v))).collect()),
            x => x.clone(),
        }
    }
    let mut out = String::new();
    for c in cases.iter().filter(|c| c.expected.is_some()) {
        let m = modules.iter().find(|m| m.module.name.ascii() == c.module);
        let info = m.and_then(|m| c.function.parse::<usize>().ok().and_then(|f| m.decls.fns.get(f)));
        let (Some(m), Some(info)) = (m, info) else {
            out.push_str("{}\n");
            continue;
        };
        let line = Json::obj([("module", Json::str(m.module.name.text.clone())), ("function", Json::str(info.name.text.clone())), ("in", unmarked(&c.input))]).compact();
        out.push_str(&line.replace(&ritsu_base::json::quote(SENTINEL), "\"\\ud800\""));
        out.push('\n');
    }
    out
}

/// Rust's harness's lines held to the cases: the number that agree, and each that does not (the first
/// twenty). A line's `out` or `fails` is held to the reference's as [`compare`] holds them, and a
/// vector's whole line to the line `vectors` writes, to the letter.
pub fn compare_lines(cases: &[Case], stdout: &str) -> (usize, Vec<String>) {
    let run: Vec<&Case> = cases.iter().filter(|c| c.expected.is_some()).collect();
    let lines: Vec<&str> = stdout.lines().collect();
    let mut wrong = Vec::new();
    let mut agree = 0;
    if lines.len() != run.len() {
        wrong.push(format!("the harness printed {} lines for {} cases", lines.len(), run.len()));
    }
    for (c, line) in run.iter().zip(&lines) {
        let Some(want) = c.expected.as_ref() else { continue };
        let depth = json_depth(want).max(json_depth(&c.input) + 1).max(ritsu_base::json::MAX_DEPTH) + 2;
        let answer = ritsu_base::json::parse_nested(line, depth).ok().map(|g| match (g.get("out"), g.get("fails")) {
            (Some(x), _) => Json::obj([("out", x.clone())]),
            (None, Some(x)) => Json::obj([("fails", x.clone())]),
            _ => g,
        });
        let letter = c.line.as_ref().is_none_or(|l| l == line);
        match answer {
            Some(a) if same(&a, want) && letter => agree += 1,
            _ => {
                if wrong.len() < 20 {
                    let shown = c.line.clone().unwrap_or_else(|| want.compact());
                    wrong.push(format!("{}.{} {}:\n  reference: {shown}\n  generated: {line}", c.module, c.function, c.input.compact()));
                }
            }
        }
    }
    (agree, wrong)
}

/// Rust's harness: a `main` beside the package's crate `name` that answers each line of the file its
/// first argument names through the door, a line each. A call that panics, which no call the door
/// answers should, is said as an error, and the next one is run.
fn rust_program(name: &str) -> String {
    format!(
        "//! The harness of kaname's tests: each call through the package's door, a line of JSON each.

fn main() {{
    let path = std::env::args().nth(1).expect(\"the file of the calls\");
    let text = std::fs::read_to_string(path).expect(\"the file of the calls\");
    for line in text.lines() {{
        match std::panic::catch_unwind(|| {name}::glue::door::answer(line)) {{
            Ok(answer) => println!(\"{{answer}}\"),
            Err(_) => println!(\"{{{{\\\"error\\\":\\\"the call panicked\\\"}}}}\"),
        }}
    }}
}}
"
    )
}

/// Two values of JSON alike, an object's keys in any order.
fn same(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::Obj(x), Json::Obj(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.iter().any(|(k2, v2)| k == k2 && same(v, v2))),
        (Json::Arr(x), Json::Arr(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q)),
        _ => a == b,
    }
}

/// A value of a record or an enum that holds itself, nested as deep as a host may give one (200), and
/// one deeper, which the entry fails (DESIGN 28.1): each parameter of each function a host calls that
/// can hold one, the other parameters at their base values, run in the reference interpreter. A case
/// whose answer does not stop (a function that walks a value 200 deep, called from another, goes past
/// the depth of the calls) has no answer and is not run, as a vector that does not stop. An answer
/// that is a value 200 deep is held to the code as any other (`compare` reads a line as deep as the
/// answer it holds it to).
pub fn nested_cases(modules: &[Rc<Checked>], suite: &Suite, lang: Lang) -> Vec<Case> {
    let mut out = Vec::new();
    for c in modules {
        let p = Program::of(c);
        for (f, info) in c.decls.fns.iter().enumerate() {
            if info.inner() {
                continue;
            }
            let mut inputs: Vec<Vec<Value>> = Vec::new();
            for (i, prm) in info.params.iter().enumerate() {
                for n in [crate::interp::NESTED, crate::interp::NESTED + 1] {
                    let Some(v) = crate::vectors::nested_value(&p, &prm.slot.ty, n) else { continue };
                    let mut input = crate::vectors::base_input(&p, c, f);
                    input[i] = v;
                    if !inputs.contains(&input) {
                        inputs.push(input);
                    }
                }
            }
            for input in inputs {
                let line = crate::vectors::line(&p, suite, c, f, &crate::vectors::Vector { input, why: None }, false, lang);
                let expected = line.get("fails").map(|x| Json::obj([("fails", x.clone())])).or_else(|| line.get("out").map(|x| Json::obj([("out", x.clone())])));
                out.push(Case::of(c.module.name.ascii().to_string(), f.to_string(), line.get("in").cloned().unwrap_or(Json::Null), expected));
            }
        }
    }
    out
}

/// How deep JSON nests its arrays and objects.
fn json_depth(j: &Json) -> usize {
    match j {
        Json::Arr(xs) => 1 + xs.iter().map(json_depth).max().unwrap_or(0),
        Json::Obj(fs) => 1 + fs.iter().map(|(_, v)| json_depth(v)).max().unwrap_or(0),
        _ => 0,
    }
}

/// The harness's lines held to the cases: the number that agree, and each that does not (the first
/// twenty). A line is read as deep as the answer it is held to nests (a value 200 deep, DESIGN 28.1),
/// and no less deep than ritsu's reader reads.
pub fn compare(cases: &[Case], stdout: &str) -> (usize, Vec<String>) {
    let run: Vec<&Case> = cases.iter().filter(|c| c.expected.is_some()).collect();
    let lines: Vec<&str> = stdout.lines().collect();
    let mut wrong = Vec::new();
    let mut agree = 0;
    if lines.len() != run.len() {
        wrong.push(format!("the harness printed {} lines for {} cases", lines.len(), run.len()));
    }
    for (c, line) in run.iter().zip(&lines) {
        let Some(want) = c.expected.as_ref() else { continue };
        match ritsu_base::json::parse_nested(line, json_depth(want).max(ritsu_base::json::MAX_DEPTH)) {
            Ok(got) if same(&got, want) => agree += 1,
            got => {
                if wrong.len() < 20 {
                    let got = got.map(|g| g.compact()).unwrap_or_else(|_| line.to_string());
                    wrong.push(format!("{}.{} {}:\n  reference: {}\n  generated: {got}", c.module, c.function, c.input.compact(), want.compact()));
                }
            }
        }
    }
    (agree, wrong)
}

/// The program, in `t`, for the modules (their package's Python package is `name`, and Rust's crate;
/// Go's import path of the package's directory is `go_module`). Rust's reads lines of [`call_lines`].
/// What its readers refuse (a day of a rule's element no date holds, DESIGN 22.5) is the failure
/// `input` of the function, said in `lang`, the language of the package, as the package's entries say
/// it.
pub fn program(modules: &[Rc<Checked>], t: Target, name: &str, go_module: &str, lang: Lang) -> String {
    if t == Target::Rust {
        return rust_program(name);
    }
    let mut h = H::new(modules, t, name, go_module, lang, true);
    let mut cases = Vec::new();
    for (mi, c) in modules.iter().enumerate() {
        let p = Program::of(c);
        for (fi, info) in c.decls.fns.iter().enumerate() {
            if info.inner() {
                continue;
            }
            let args: Vec<String> = info.params.iter().map(|prm| h.decode_in(&p, c, &prm.slot.ty, prm.slot.range.as_ref(), &h.at_key(&prm.name.text), &h.lit(&prm.name.text))).collect();
            let call = h.call(c, mi, fi, &args);
            let out = h.encode(&p, c, &info.ret.ty, "r");
            let fails = c.analysis.get().and_then(|a| a.fns.get(fi)).is_some_and(|x| !x.effect.fails.is_empty());
            let entry = Entry { fun: h.lit(&info.name.text), module: h.lit(&c.module.name.text), line: info.line };
            cases.push((c.module.name.ascii().to_string(), fi, call, out, fails, entry));
        }
    }
    h.finish(&cases)
}

/// A function of the harness as its failure `input` names it: its name and its module's, written as
/// the target writes a string, and the line it is declared on.
struct Entry {
    fun: String,
    module: String,
    line: usize,
}

/// A function of a package's modules a host calls through JSON: the function `function` of the
/// module `module` (its index among the modules), named `name` in the file of [`json_entries`].
#[derive(Clone, Debug)]
pub struct JsonEntry {
    pub module: usize,
    pub function: String,
    pub name: String,
}

/// Whether a parameter of an entry that takes JSON is an integer, given as one: a number with no
/// unit, whole. Any other is given as a JSON text.
fn integer_param(t: &Ty) -> bool {
    matches!(t, Ty::Num(nt) if nt.is_number() && nt.step == Some(Rat::int(1)))
}

/// A file of a package in `t` (TypeScript, Python or Go) with an entry for each of `entries` that
/// takes and answers JSON, as the vectors write it (DESIGN 9): a function named by the entry, which
/// reads each parameter of a whole number with no unit as an integer and each other one from a JSON
/// text, calls the function as a host does, and answers its value as a JSON text. The readings and
/// the writings are the harness's ([`program`]). A value out of the range its function declares
/// fails as the function's call fails (a `KanameError` of the kind `input`), and so does JSON that is
/// no value of its type, which the reading finds before the function is called (DESIGN 16.4): the
/// place of the value from the argument's name (`submit.form.days`, `organization.people[0]`, a
/// value of a sum by its name, `event.approve.by`), what is there and what it must be, in `lang`:
/// "The argument `submit.form.days` of `start` is "three", and must be an integer". The file is
/// `head` (its comment, and Go's `package` line), then its imports, the readings and the entries,
/// then `tail`; it sits at the root of the package (Python's package is `name`, Go's import path
/// `go_module`).
#[allow(clippy::too_many_arguments)]
pub fn json_entries(modules: &[Rc<Checked>], t: Target, name: &str, go_module: &str, entries: &[JsonEntry], head: &str, tail: &str, lang: Lang) -> String {
    assert!(t != Target::Rust, "Rust's entries answer through the door");
    let mut h = H::new(modules, t, name, go_module, lang, false);
    let mut fns = String::new();
    let mut used: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    for e in entries {
        let c = &modules[e.module];
        let Some(fi) = c.decls.fns.iter().position(|f| f.name.text == e.function && !f.inner()) else { continue };
        used.insert(e.module);
        let p = Program::of(c);
        let info = &c.decls.fns[fi];
        let fails = c.analysis.get().and_then(|a| a.fns.get(fi)).is_some_and(|x| !x.effect.fails.is_empty());
        let mut params = Vec::new();
        // each argument, read off its JSON, or the integer as it is given
        let mut args = Vec::new();
        let mut read = Vec::new();
        for prm in &info.params {
            let n = prm.name.ascii().to_string();
            if integer_param(&prm.slot.ty) {
                let (decl, arg) = match t {
                    Target::TypeScript => (format!("{n}: bigint | number"), format!("BigInt({n})")),
                    Target::Python => (format!("{n}: int"), n.clone()),
                    _ => (format!("{n} int64"), n.clone()),
                };
                params.push(decl);
                args.push(arg);
                continue;
            }
            let wire = h.lit(&prm.name.text);
            let at = match t {
                Target::TypeScript => format!("parse({n}, {wire})"),
                Target::Python => format!("_parse({n}, {wire})"),
                _ => format!("jsonValue({n}, {wire})"),
            };
            let d = h.decode_in(&p, c, &prm.slot.ty, prm.slot.range.as_ref(), &at, &wire);
            params.push(match t {
                Target::TypeScript => format!("{n}: string"),
                Target::Python => format!("{n}: str"),
                _ => format!("{n} json.RawMessage"),
            });
            // TypeScript and Python read the arguments first, in their order, and call after; Go reads
            // them in the call, in their order too
            if t == Target::Go {
                args.push(d);
            } else {
                args.push(format!("a[{}]", read.len()));
                read.push(d);
            }
        }
        let call = match t {
            Target::Python => {
                let kw: Vec<String> = info.params.iter().zip(&args).map(|(p, a)| format!("{}={a}", p.name.ascii())).collect();
                format!("m{}.{}({})", e.module, fn_name(t, c, fi), kw.join(", "))
            }
            Target::TypeScript => format!("m{}.{}({})", e.module, fn_name(t, c, fi), args.join(", ")),
            _ => format!("m{}.{}({})", e.module, fn_name(t, c, fi), args.join(", ")),
        };
        let out = h.encode(&p, c, &info.ret.ty, "r");
        let (en, f) = (&e.name, &info.name.text);
        let m = c.module.name.ascii();
        let (fun, module, line) = (h.lit(f), h.lit(&c.module.name.text), info.line);
        fns.push_str(&match t {
            Target::TypeScript => {
                let reads = if read.is_empty() { String::new() } else { format!("  let a: any[];\n  try {{\n    a = [{}];\n  }} catch (e) {{\n    throw entered(e, {fun}, {module}, {line});\n  }}\n", read.join(", ")) };
                format!("/** The function `{f}` of the module `{m}`, through JSON. */\nfunction {en}({}): string {{\n{reads}  const r = {call};\n  return show({out});\n}}\n\n", params.join(", "))
            }
            Target::Python => {
                let reads = if read.is_empty() { String::new() } else { format!("    try:\n        a = [{}]\n    except _Wrong as w:\n        raise kaname.KanameError(\"input\", {module}, {line}, _said({fun}, w), w.path) from None\n", read.join(", ")) };
                format!("def {en}({}) -> str:\n    \"\"\"The function `{f}` of the module `{m}`, through JSON.\"\"\"\n{reads}    r = {call}\n    return json.dumps({out}, ensure_ascii=False)\n\n\n", params.join(", "))
            }
            _ => {
                let got = if fails { format!("r, err := {call}\n\tif err != nil {{\n\t\treturn nil, err\n\t}}\n") } else { format!("r := {call}\n") };
                format!(
                    "// {en} is the function `{f}` of the module `{m}`, through JSON. JSON that is no value of its\n// type, or a value out of the range it declares, is answered with an error.\nfunc {en}({}) (out json.RawMessage, err error) {{\n\tdefer func() {{\n\t\tif r := recover(); r != nil {{\n\t\t\tout = nil\n\t\t\terr = entryError(r, {fun}, {module}, {line})\n\t\t}}\n\t}}()\n\t{got}\treturn json.Marshal({out})\n}}\n\n",
                    params.join(", ")
                )
            }
        });
    }
    let defs = h.defs.join("\n");
    let named = h.named.borrow().clone();
    let wanted: Vec<usize> = (0..modules.len()).filter(|i| used.contains(i) || named.contains(i)).collect();
    let mut s = String::from(head);
    match t {
        Target::TypeScript => {
            s.push_str("import * as kaname from \"./glue/kaname\";\n");
            for i in &wanted {
                s.push_str(&format!("import * as m{i} from \"./glue/{}\";\n", modules[*i].module.name.ascii()));
            }
            for line in h.imports.values() {
                s.push_str(line);
                s.push('\n');
            }
            s.push_str(TS_IO);
            s.push_str(&ts_read(lang, true));
            s.push_str(&defs);
            s.push('\n');
            s.push_str(&fns);
        }
        Target::Python => {
            s.push_str(&format!("import datetime as _dt\nimport json\nfrom typing import Any\n\nfrom {name}.glue import kaname\n"));
            for i in &wanted {
                s.push_str(&format!("from {name}.glue import {} as m{i}\n", modules[*i].module.name.ascii()));
            }
            for line in h.imports.values() {
                s.push_str(line);
                s.push('\n');
            }
            s.push_str("\n\n");
            s.push_str(&py_read(lang, true));
            for d in &h.defs {
                s.push_str(d);
                s.push_str("\n\n");
            }
            s.push_str(&fns);
        }
        _ => {
            let mut imports: Vec<String> = vec![format!("\"{go_module}/glue/kaname\"")];
            if h.units {
                imports.push(format!("\"{go_module}/units\""));
            }
            // each module by a name of its own, as in TypeScript and Python: a module may have the name
            // of an import or of a function of the file (a request named `order`, `pairs` or `strings`)
            for i in &wanted {
                imports.push(format!("m{i} \"{go_module}/glue/{}\"", go_package(modules[*i].module.name.ascii())));
            }
            imports.extend(h.imports.values().cloned());
            imports.sort_by(|a, b| a.split('"').nth(1).cmp(&b.split('"').nth(1)));
            imports.dedup();
            s.push_str(&format!("import (\n\t\"bytes\"\n\t\"encoding/json\"\n\t\"fmt\"\n\t\"io\"\n\t\"slices\"\n\t\"strconv\"\n\t\"strings\"\n\t\"unicode/utf8\"\n\n{}\n)\n\n", imports.iter().map(|i| format!("\t{i}")).collect::<Vec<_>>().join("\n")));
            s.push_str(GO_LIB_IO);
            s.push_str(&go_read(lang));
            s.push_str(&defs);
            s.push('\n');
            s.push_str(&fns);
        }
    }
    s.push_str(tail);
    s
}

struct H {
    t: Target,
    /// Each codec made, by its key: its function's name.
    codecs: BTreeMap<String, String>,
    defs: Vec<String>,
    modules: Vec<Rc<Checked>>,
    name: String,
    go_module: String,
    /// What the program imports beyond the modules: a rule's module, a book's types.
    imports: BTreeMap<String, String>,
    /// Whether the program names a type of the package's `units` (Go imports it then).
    units: bool,
    /// The modules whose types the codecs name (Go imports them, whether or not a host calls them).
    named: std::cell::RefCell<std::collections::BTreeSet<usize>>,
    /// The language the readers say what is wrong in (DESIGN 16.4).
    lang: Lang,
    /// Whether a string's reader takes the marker of a string that is not well formed (`{"bad":
    /// "string"}`, the harness's alone, [`outside_cases`]).
    marker: bool,
}

impl H {
    fn new(modules: &[Rc<Checked>], t: Target, name: &str, go_module: &str, lang: Lang, marker: bool) -> H {
        H { t, codecs: BTreeMap::new(), defs: Vec::new(), modules: modules.to_vec(), name: name.to_string(), go_module: go_module.to_string(), imports: BTreeMap::new(), units: false, named: std::cell::RefCell::default(), lang, marker }
    }

    /// A string of the target: `s` written as its literal.
    fn lit(&self, s: &str) -> String {
        match self.t {
            Target::Go => ritsu_emit::lit::go(s),
            _ => ritsu_emit::lit::json(s),
        }
    }

    /// The place of the key `k` after the place `at` (`at + ".form"`), as the target joins them.
    fn key_at(&self, at: &str, k: &str) -> String {
        match self.t {
            Target::Go => format!("{at}+{}", self.lit(&format!(".{k}"))),
            _ => format!("{at} + {}", self.lit(&format!(".{k}"))),
        }
    }

    /// The place of the element `i` after the place `at` (`at + "[" + i + "]"`), an argument of a call.
    fn elem_at(&self, at: &str, i: &str) -> String {
        match self.t {
            Target::TypeScript => format!("{at} + \"[\" + {i} + \"]\""),
            Target::Python => format!("f\"{{{at}}}[{{{i}}}]\""),
            _ => format!("{at}+\"[\"+strconv.Itoa({i})+\"]\""),
        }
    }

    /// The place `at` with `tail` after it (`a + "[0]"`), an argument of a call.
    fn fixed_at(&self, at: &str, tail: &str) -> String {
        match self.t {
            Target::Go => format!("{at}+{}", self.lit(tail)),
            _ => format!("{at} + {}", self.lit(tail)),
        }
    }

    fn at_key(&self, k: &str) -> String {
        let q = ritsu_emit::lit::json(k);
        match self.t {
            Target::TypeScript => format!("c.in[{q}]"),
            Target::Python => format!("c[\"in\"][{q}]"),
            Target::Go => format!("c.In[{}]", ritsu_emit::lit::go(k)),
            Target::Rust => unreachable!("Rust's harness answers through the door"),
        }
    }

    /// The module of kaname a type is of, as the program names its import.
    fn mod_ref(&self, file: &std::path::Path) -> String {
        let i = self.modules.iter().position(|m| m.canon == file).unwrap_or(0);
        self.named.borrow_mut().insert(i);
        format!("m{i}")
    }

    /// The name the program imports a rule's code by, the import made.
    fn rule_ref(&mut self, p: &Program, f: &std::path::Path) -> String {
        let (key, line) = self.rule_import(p, f);
        self.imports.insert(key.clone(), line);
        key
    }

    /// The name the program imports a rule's code by, and the line that imports it.
    fn rule_import(&self, p: &Program, f: &std::path::Path) -> (String, String) {
        let facts = p.rules.get(f).expect("a rule the module reads");
        match self.t {
            Target::TypeScript => {
                let m = facts.typescript.module.trim_end_matches(".ts");
                (format!("r_{}", facts.alias), format!("import * as r_{} from \"./rules/{m}\";", facts.alias))
            }
            Target::Python => (format!("r_{}", facts.alias), format!("from {}.rules import {} as r_{}", self.name, facts.python.module, facts.alias)),
            // by a name of its own, as a book's is: a module of kaname may have the package's name (the
            // example `catalog` beside the `enums` file `catalog` a module reads as a rule)
            Target::Go => (format!("r{}", facts.go.module), format!("r{} \"{}/rules/{}\"", facts.go.module, self.go_module, facts.go.module)),
            Target::Rust => unreachable!("Rust's harness answers through the door"),
        }
    }

    /// The type this target's code of a rule gives one element of the list `seq` it walks, as the
    /// parameter of the list names it (`readonly Element[]`, `list[Element]`, `[]Element`).
    fn element_name(&self, facts: &RuleFacts, seq: &str) -> String {
        let call = match self.t {
            Target::TypeScript => &facts.typescript,
            Target::Python => &facts.python,
            Target::Go => &facts.go,
            Target::Rust => unreachable!("Rust's harness answers through the door"),
        };
        let ty = call.params.iter().find(|x| x.name == seq).map(|x| x.ty.clone()).unwrap_or_default();
        let s = ty.trim();
        let s = s.strip_prefix("readonly ").unwrap_or(s);
        s.strip_prefix("list[").and_then(|x| x.strip_suffix(']')).or_else(|| s.strip_prefix("[]")).or_else(|| s.strip_suffix("[]")).unwrap_or(s).to_string()
    }

    /// The field of a rule's element read into `v` as kaname's types hold it, as the rule's code holds
    /// it: the lines that make it (Go's of a field that may be none), and the expression. A date is its
    /// count of days, a rate Go's type of the rule's; any other as it is (the package's type of a unit
    /// is the rule's and kaname's).
    fn to_rule_col(&self, v: &str, col: &ColumnType, r: &str) -> (String, String) {
        let mut inner = col;
        while let ColumnType::Opt(x) = inner {
            inner = x;
        }
        let rate = matches!(inner, ColumnType::Num { unit: Some(u), .. } if u.dim == ritsu_units::Dim::Rate);
        let conv = |x: &str| -> String {
            match (inner, self.t) {
                (ColumnType::Date { .. }, Target::TypeScript) => format!("BigInt(kaname.dayOf({x}))"),
                (ColumnType::Date { .. }, Target::Python) => format!("kaname.day_of({x})"),
                (ColumnType::Date { .. }, _) => format!("{x}.Days()"),
                (_, Target::Go) if rate => format!("{r}.Rate({x})"),
                _ => x.to_string(),
            }
        };
        let converts = matches!(inner, ColumnType::Date { .. }) || (rate && self.t == Target::Go);
        if !converts {
            return (String::new(), v.to_string());
        }
        if !matches!(col, ColumnType::Opt(_)) {
            return (String::new(), conv(v));
        }
        match self.t {
            Target::TypeScript => (String::new(), format!("{v} === null ? null : {}", conv(v))),
            Target::Python => (String::new(), format!("None if {v} is None else {}", conv(v))),
            _ => {
                let gt = if matches!(inner, ColumnType::Date { .. }) { "int64".to_string() } else { format!("{r}.Rate") };
                let w = format!("{v}r");
                (format!("\tvar {w} *{gt}\n\tif {v} != nil {{\n\t\t{w} = kaname.Some({})\n\t}}\n", conv(&format!("(*{v})"))), w)
            }
        }
    }

    fn book_ref(&mut self, p: &Program, f: &std::path::Path) -> String {
        let b = p.books.get(f).expect("a book the module reads");
        let (key, line) = match self.t {
            Target::TypeScript => (format!("b_{}", b.name), format!("import type * as b_{} from \"./glue/books/{}\";", b.name, b.name)),
            Target::Python => (format!("b_{}", b.name), format!("from {}.glue.books import {} as b_{}", self.name, b.name, b.name)),
            Target::Go => (format!("b{}", b.go.module), format!("b{} \"{}/glue/books/{}\"", b.go.module, self.go_module, b.go.module)),
            Target::Rust => unreachable!("Rust's harness answers through the door"),
        };
        self.imports.insert(key.clone(), line);
        key
    }

    /// The call of function `fi` of module `mi`, as a host calls it, with the arguments decoded.
    fn call(&self, c: &Checked, mi: usize, fi: usize, args: &[String]) -> String {
        let f = fn_name(self.t, c, fi);
        match self.t {
            Target::TypeScript => format!("m{mi}.{f}({})", args.join(", ")),
            Target::Python => {
                let kw: Vec<String> = c.decls.fns[fi].params.iter().zip(args).map(|(p, a)| format!("{}={a}", p.name.ascii())).collect();
                format!("m{mi}.{f}({})", kw.join(", "))
            }
            Target::Go => format!("m{mi}.{f}({})", args.join(", ")),
            Target::Rust => unreachable!("Rust's harness answers through the door"),
        }
    }

    /// A codec's function, made once by its key.
    fn codec(&mut self, key: String, make: impl FnOnce(&mut Self, &str) -> String) -> String {
        if let Some(n) = self.codecs.get(&key) {
            return n.clone();
        }
        let n = format!("c{}", self.codecs.len() + 1);
        self.codecs.insert(key, n.clone());
        let def = make(self, &n);
        self.defs.push(def);
        n
    }

    // ── From the wire ────────────────────────────────────────────────────

    /// The value of `t` as a host gives it, from the JSON at `x`, which is at the place `at` (an
    /// expression of the target, a string: `"submit"`, `at + ".form"`). JSON that is no value of `t`
    /// stops the reading with the place, what is there and what it must be (DESIGN 16.4).
    fn decode(&mut self, p: &Program, c: &Checked, t: &Ty, x: &str, at: &str) -> String {
        self.decode_in(p, c, t, None, x, at)
    }

    /// The same, of a place that declares `range`: Python's reader of a date says that a day its date
    /// cannot hold (a year before 1 or after 9999, a day of no calendar) is outside the range, the
    /// ends not written the type's, as the entry of the other targets says it (DESIGN 22.5).
    fn decode_in(&mut self, p: &Program, c: &Checked, t: &Ty, range: Option<&crate::ty::Range>, x: &str, at: &str) -> String {
        let ranged = self.t == Target::Python && dated(t);
        let key = if ranged { format!("dec|{t:?}|{}", day_range(range)) } else { format!("dec|{t:?}") };
        let range = if ranged { range.cloned() } else { None };
        let f = self.codec(key, |h, n| h.decoder(p, c, t, range.as_ref(), n));
        format!("{f}({x}, {at})")
    }

    /// The value of the key `k` of the object `o`, which the readers take as missing where it has
    /// none.
    fn got(&self, o: &str, k: &str) -> String {
        match self.t {
            Target::Python => format!("_got({o}, {})", self.lit(k)),
            Target::Go => format!("keyValue({o}, {})", self.lit(k)),
            _ => format!("got({o}, {})", self.lit(k)),
        }
    }

    /// What a value must be, as the target's string.
    fn need(&self, n: Need) -> String {
        self.lit(needs(self.lang, n))
    }

    fn decoder(&mut self, p: &Program, c: &Checked, t: &Ty, range: Option<&crate::ty::Range>, n: &str) -> String {
        let tt = self.t;
        let lang = self.lang;
        // a function of TypeScript or Python with the lines of `body`
        let ts = |body: &str| format!("function {n}(x: any, at: string): any {{\n{}}}\n", indented(body, 2));
        let py = |body: &str| format!("def {n}(x: Any, at: str) -> Any:\n{}", indented(body, 4));
        match t {
            Ty::Opt(inner) => {
                let d = self.decode_in(p, c, inner, range, "x", "at");
                match tt {
                    Target::TypeScript => ts(&format!("return x === null || x === undefined ? null : {d};\n")),
                    Target::Python => py(&format!("return None if x is None or x is _MISSING else {d}\n")),
                    Target::Go => {
                        let gt = self.go_type(p, inner);
                        format!("func {n}(x any, at string) *{gt} {{\n\tswitch x.(type) {{\n\tcase nil, keyAbsent:\n\t\treturn nil\n\t}}\n\tv := {d}\n\treturn &v\n}}\n")
                    }
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                }
            }
            Ty::List(inner) => {
                let d = self.decode_in(p, c, inner, range, "e", &self.elem_at("at", "i"));
                match tt {
                    Target::TypeScript => ts(&format!("return array(x, at).map((e: any, i: number) => {d});\n")),
                    Target::Python => py(&format!("return [{d} for i, e in enumerate(_array(x, at))]\n")),
                    Target::Go => {
                        let gt = self.go_type(p, inner);
                        format!("func {n}(x any, at string) []{gt} {{\n\tout := []{gt}{{}}\n\tfor i, e := range jsonArray(x, at) {{\n\t\tout = append(out, {d})\n\t}}\n\treturn out\n}}\n")
                    }
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                }
            }
            // a dict from its pairs, a set from its members, on the wire in any order (DESIGN 24.4); a
            // pair's key read before its value
            Ty::Dict(k, v) => {
                let dk = self.decode(p, c, k, "q[0]", &self.fixed_at("a", "[0]"));
                let dv = self.decode_in(p, c, v, range, "q[1]", &self.fixed_at("a", "[1]"));
                let ai = self.elem_at("at", "i");
                match tt {
                    Target::TypeScript => ts(&format!("return new Map<any, any>(array(x, at).map((p: any, i: number): [any, any] => {{\n  const a = {ai};\n  const q = pair(p, a);\n  const k = {dk};\n  return [k, {dv}];\n}}));\n")),
                    Target::Python => py(&format!("out: dict[Any, Any] = {{}}\nfor i, p in enumerate(_array(x, at)):\n    a = {ai}\n    q = _pair(p, a)\n    k = {dk}\n    out[k] = {dv}\nreturn out\n")),
                    Target::Go => {
                        let gt = self.go_type(p, t);
                        // the place a statement of its own, which gofmt writes with spaces
                        format!("func {n}(x any, at string) {gt} {{\n\tout := {gt}{{}}\n\tfor i, p := range jsonArray(x, at) {{\n\t\ta := at + \"[\" + strconv.Itoa(i) + \"]\"\n\t\tq := jsonPair(p, a)\n\t\tk := {dk}\n\t\tout[k] = {dv}\n\t}}\n\treturn out\n}}\n")
                    }
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                }
            }
            Ty::Set(inner) => {
                let d = self.decode_in(p, c, inner, range, "e", &self.elem_at("at", "i"));
                match tt {
                    Target::TypeScript => ts(&format!("return new Set(array(x, at).map((e: any, i: number) => {d}));\n")),
                    Target::Python => py(&format!("return frozenset({d} for i, e in enumerate(_array(x, at)))\n")),
                    Target::Go => {
                        let gt = self.go_type(p, t);
                        format!("func {n}(x any, at string) {gt} {{\n\tout := {gt}{{}}\n\tfor i, e := range jsonArray(x, at) {{\n\t\tout[{d}] = struct{{}}{{}}\n\t}}\n\treturn out\n}}\n")
                    }
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                }
            }
            Ty::Record(RecId::Entry(k, v)) => {
                let dk = self.decode(p, c, k, &self.got("o", "key"), &self.key_at("at", "key"));
                let dv = self.decode(p, c, v, &self.got("o", "value"), &self.key_at("at", "value"));
                match tt {
                    Target::TypeScript => ts(&format!("const o = object(x, at);\nreturn {{ key: {dk}, value: {dv} }};\n")),
                    Target::Python => py(&format!("o = _object(x, at)\nreturn kaname.Entry(key={dk}, value={dv})\n")),
                    Target::Go => {
                        let gt = self.go_type(p, t);
                        format!("func {n}(x any, at string) {gt} {{\n\to := jsonObject(x, at)\n\treturn {gt}{{Key: {dk}, Value: {dv}}}\n}}\n")
                    }
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                }
            }
            Ty::Record(RecId::Indexed(x0)) => {
                let di = self.decode(p, c, &Ty::Num(crate::ty::NumTy::number()), &self.got("o", "index"), &self.key_at("at", "index"));
                let dx = self.decode(p, c, x0, &self.got("o", "item"), &self.key_at("at", "item"));
                match tt {
                    Target::TypeScript => ts(&format!("const o = object(x, at);\nreturn {{ index: {di}, item: {dx} }};\n")),
                    Target::Python => py(&format!("o = _object(x, at)\nreturn kaname.Indexed(index={di}, item={dx})\n")),
                    Target::Go => {
                        let gt = self.go_type(p, t);
                        format!("func {n}(x any, at string) {gt} {{\n\to := jsonObject(x, at)\n\treturn {gt}{{Index: {di}, Item: {dx}}}\n}}\n")
                    }
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                }
            }
            // a running total a host gives (a parameter of `running[T, W]`): the item, and the three
            // weights, in the order declared
            Ty::Record(RecId::Running(item, w)) => {
                let di = self.decode(p, c, item, &self.got("o", "item"), &self.key_at("at", "item"));
                let mut dw = Vec::new();
                for k in ["before", "upto", "total"] {
                    dw.push(self.decode(p, c, w, &self.got("o", k), &self.key_at("at", k)));
                }
                let (b, u, l) = (&dw[0], &dw[1], &dw[2]);
                match tt {
                    Target::TypeScript => ts(&format!("const o = object(x, at);\nreturn {{ item: {di}, before: {b}, upto: {u}, total: {l} }};\n")),
                    Target::Python => py(&format!("o = _object(x, at)\nreturn kaname.Running(item={di}, before={b}, upto={u}, total={l})\n")),
                    Target::Go => {
                        let gt = self.go_type(p, t);
                        format!("func {n}(x any, at string) {gt} {{\n\to := jsonObject(x, at)\n\treturn {gt}{{Item: {di}, Before: {b}, Upto: {u}, Total: {l}}}\n}}\n")
                    }
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                }
            }
            // an element of a list a rule walks (DESIGN 4.4): each field read in its order as kaname's
            // types hold it, then put as the rule's code holds it (a date as its count of days, a rate
            // in Go's type of it), as kaname's code puts it where it calls the rule. A day of no
            // calendar (2026-13-01, 10000-01-01) has no count of days: the entry fails `input` on it
            // here, as the check of a date says one outside its range (Python's reader fails so on
            // every day its date cannot hold, DESIGN 22.5)
            Ty::Record(RecId::Element(file, seq)) => {
                let Some(facts) = p.rules.get(file).cloned() else { return String::new() };
                let Some((_, _, cols)) = facts.elements.iter().find(|(x, _, _)| x == seq).cloned() else { return String::new() };
                let r = self.rule_ref(p, file);
                let elem = self.element_name(&facts, seq);
                let fields = p.fields(&RecId::Element(file.clone(), seq.clone()));
                let mut lines = String::new();
                let mut parts: Vec<(String, String)> = Vec::new();
                for (k, ((f, slot), col)) in fields.iter().zip(&cols).enumerate() {
                    let fat = self.key_at("at", &f.text);
                    let d = self.decode_in(p, c, &slot.ty, slot.range.as_ref(), &self.got("o", &f.text), &fat);
                    let v = format!("f{}", k + 1);
                    lines.push_str(&match tt {
                        Target::TypeScript => format!("const {v} = {d};\n"),
                        Target::Python => format!("{v} = {d}\n"),
                        _ => format!("\t{v} := {d}\n"),
                    });
                    if matches!(slot.ty.present(), Ty::Date) && tt != Target::Python {
                        let rt = self.lit(&day_range(slot.range.as_ref()));
                        let opt = matches!(slot.ty, Ty::Opt(_));
                        lines.push_str(&match tt {
                            Target::TypeScript => {
                                let there = if opt { format!("{v} !== null && ") } else { String::new() };
                                format!("if ({there}!kaname.isDate({v})) throw new Wrong({fat}, {v}, {rt}, 3);\n")
                            }
                            _ => {
                                let there = if opt { format!("{v} != nil && ") } else { String::new() };
                                // in a field of a composite literal, gofmt spaces the `+`
                                let path = format!("at + {}", self.lit(&format!(".{}", f.text)));
                                format!("\tif {there}!{v}.Valid() {{\n\t\tpanic(wrongJSON{{path: {path}, given: {v}.String(), needed: {rt}, kind: 3}})\n\t}}\n")
                            }
                        });
                    }
                    let name = if tt == Target::Go { ritsu_emit::ident::pascal(&col.alias) } else { col.alias.clone() };
                    let (more, e) = self.to_rule_col(&v, &col.ty, &r);
                    lines.push_str(&more);
                    parts.push((name, e));
                }
                match tt {
                    Target::TypeScript => ts(&format!("const o = object(x, at);\n{lines}return {{ {} }};\n", parts.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join(", "))),
                    Target::Python => py(&format!("o = _object(x, at)\n{lines}return {r}.{elem}({})\n", parts.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(", "))),
                    Target::Go => format!(
                        "func {n}(x any, at string) {r}.{elem} {{\n\to := jsonObject(x, at)\n{lines}\treturn {r}.{elem}{{{}}}\n}}\n",
                        parts.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join(", ")
                    ),
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                }
            }
            Ty::Num(nt) => match tt {
                Target::TypeScript => ts("return int(x, at);\n"),
                Target::Python => py("return _int(x, at)\n"),
                Target::Go => {
                    let gt = if nt.is_rate() { "int64".to_string() } else { self.unit_go(&nt.unit) };
                    format!("func {n}(x any, at string) {gt} {{\n\treturn {gt}(jsonInt(x, at))\n}}\n")
                }
                Target::Rust => unreachable!("Rust's harness answers through the door"),
            },
            Ty::Date => match tt {
                Target::TypeScript => ts("return day(x, at);\n"),
                Target::Python => py(&format!("return _day(x, at, {})\n", self.lit(&day_range(range)))),
                Target::Go => format!("func {n}(x any, at string) kaname.Date {{\n\treturn jsonDate(x, at)\n}}\n"),
                Target::Rust => unreachable!("Rust's harness answers through the door"),
            },
            Ty::Bool => {
                let need = self.need(Need::Bool);
                match tt {
                    Target::TypeScript => ts(&format!("if (typeof x === \"boolean\") return x;\nthrow wrong(at, x, {need});\n")),
                    Target::Python => py(&format!("if isinstance(x, bool):\n    return x\nraise _wrong(at, x, {need})\n")),
                    Target::Go => format!("func {n}(x any, at string) bool {{\n\tif b, ok := x.(bool); ok {{\n\t\treturn b\n\t}}\n\tpanic(wrongAt(at, x, {need}))\n}}\n"),
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                }
            }
            // a string; in the harness, the marker of one that is not well formed made a lone
            // surrogate, a byte that is no UTF-8 (DESIGN 25.8)
            Ty::Str => {
                let need = self.need(Need::Str);
                let marker = self.marker;
                match tt {
                    Target::TypeScript => {
                        let m = if marker { "if (isObject(x) && Object.keys(x).length === 1 && x.bad === \"string\") return String.fromCharCode(0xd800);\n" } else { "" };
                        ts(&format!("if (typeof x === \"string\") return x;\n{m}throw wrong(at, x, {need});\n"))
                    }
                    Target::Python => {
                        let m = if marker { "if x == {\"bad\": \"string\"}:\n    return chr(0xD800)\n" } else { "" };
                        py(&format!("if isinstance(x, str):\n    return x\n{m}raise _wrong(at, x, {need})\n"))
                    }
                    Target::Go => {
                        let m = if marker { "\tif m, ok := x.(map[string]any); ok && len(m) == 1 && m[\"bad\"] == \"string\" {\n\t\treturn string([]byte{0xff})\n\t}\n" } else { "" };
                        format!("func {n}(x any, at string) string {{\n\tif s, ok := x.(string); ok {{\n\t\treturn s\n\t}}\n{m}\tpanic(wrongAt(at, x, {need}))\n}}\n")
                    }
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                }
            }
            Ty::Enum(id @ crate::ty::EnumId { owner: Owner::Module(file), .. }) if p.is_sum(id) => self.sum_decoder(p, c, id, file, n),
            // a value of a sum of the one value the record is of
            Ty::Record(r @ RecId::Variant { file, name, value, .. }) => {
                let Some(id) = r.sum() else { return String::new() };
                let d = self.decode(p, c, &Ty::Enum(id), "x", "at");
                let need = self.lit(&one_key_of(lang, std::slice::from_ref(value)));
                match tt {
                    Target::TypeScript => ts(&format!("const v = {d};\nif (v.kind === {}) return v;\nthrow new Wrong(at, keyed(x), {need}, 0);\n", self.lit(value))),
                    Target::Python => {
                        let m = p.module(file).clone();
                        let Some(e) = m.decls.enums.iter().find(|e| e.name.text == *name).cloned() else { return String::new() };
                        let va = e.values.iter().find(|v| v.text == *value).map(|v| v.ascii().to_string()).unwrap_or_default();
                        let vt = format!("{}.{}", self.mod_ref(file), variant_name(tt, e.name.ascii(), &va));
                        py(&format!("v = {d}\nif isinstance(v, {vt}):\n    return v\nraise _Wrong(at, _keyed(x), {need}, 0)\n"))
                    }
                    Target::Go => {
                        let gt = self.go_type(p, t);
                        format!("func {n}(x any, at string) {gt} {{\n\tif v, ok := {d}.({gt}); ok {{\n\t\treturn v\n\t}}\n\tpanic(wrongJSON{{path: at, given: keyedText(x), needed: {need}}})\n}}\n")
                    }
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                }
            }
            // a value of an enum by its name, one of the names it declares
            Ty::Enum(id) => {
                let names: Vec<String> = p.values(id).0.iter().map(|v| v.text.clone()).collect();
                let need = self.lit(&one_of(lang, &names));
                let listed = names.iter().map(|v| self.lit(v)).collect::<Vec<_>>().join(", ");
                match (&id.owner, tt) {
                    (_, Target::TypeScript) => ts(&format!("if (typeof x === \"string\" && [{listed}].includes(x)) return x;\nthrow wrong(at, x, {need});\n")),
                    (Owner::Module(f), Target::Python) => {
                        let m = p.module(f);
                        let alias = m.decls.enums.iter().find(|e| e.name.text == id.name).map(|e| e.name.ascii().to_string()).unwrap_or_default();
                        let ty = format!("{}.{}", self.mod_ref(f), type_name(tt, &alias));
                        py(&format!("if isinstance(x, str) and x in [{listed}]:\n    return {ty}(x)\nraise _wrong(at, x, {need})\n"))
                    }
                    (Owner::Rule(f), Target::Python) => {
                        let r = self.rule_ref(p, f);
                        let alias = rule_enum_alias(p.rules.get(f).map(|r| &r.python), &id.name);
                        py(&format!("if isinstance(x, str) and x in [{listed}]:\n    return {r}.{alias}(x)\nraise _wrong(at, x, {need})\n"))
                    }
                    (Owner::Module(f), Target::Go) => {
                        let m = p.module(f);
                        let alias = m.decls.enums.iter().find(|e| e.name.text == id.name).map(|e| e.name.ascii().to_string()).unwrap_or_default();
                        let ty = format!("{}.{}", self.mod_ref(f), type_name(tt, &alias));
                        format!("func {n}(x any, at string) {ty} {{\n\tif s, ok := x.(string); ok {{\n\t\tswitch s {{\n\t\tcase {listed}:\n\t\t\treturn {ty}(s)\n\t\t}}\n\t}}\n\tpanic(wrongAt(at, x, {need}))\n}}\n")
                    }
                    (Owner::Rule(f), Target::Go) => {
                        let r = self.rule_ref(p, f);
                        let alias = rule_enum_alias(p.rules.get(f).map(|r| &r.go), &id.name);
                        format!("func {n}(x any, at string) {r}.{alias} {{\n\tif s, ok := x.(string); ok {{\n\t\tif v, ok := {r}.Parse{alias}(s); ok {{\n\t\t\treturn v\n\t\t}}\n\t}}\n\tpanic(wrongAt(at, x, {need}))\n}}\n")
                    }
                    (_, Target::Rust) => unreachable!("Rust's harness answers through the door"),
                }
            }
            // a result from its one pair, `ok` or `err` (DESIGN 26.4)
            Ty::Result(a, e) => {
                let names = ["ok".to_string(), "err".to_string()];
                let (need_key, need) = (self.lit(&one_key_of(lang, &names)), self.lit(&sum_need(lang, &[], &names)));
                let (x_ok, x_err) = match tt {
                    Target::Go => ("v", "v"),
                    _ => ("x[\"ok\"]", "x[\"err\"]"),
                };
                let (da, de) = (self.decode(p, c, a, x_ok, &self.key_at("at", "ok")), self.decode(p, c, e, x_err, &self.key_at("at", "err")));
                match tt {
                    Target::TypeScript => ts(&format!(
                        "if (isObject(x)) {{\n  const ks = Object.keys(x);\n  if (ks.length === 1 && ks[0] === \"ok\") return {{ kind: \"ok\", value: {da} }};\n  if (ks.length === 1 && ks[0] === \"err\") return {{ kind: \"err\", error: {de} }};\n  throw new Wrong(at, keyed(x), {need_key}, 0);\n}}\nthrow wrong(at, x, {need});\n"
                    )),
                    Target::Python => py(&format!(
                        "if isinstance(x, dict):\n    if len(x) == 1 and \"ok\" in x:\n        return kaname.Ok(value={da})\n    if len(x) == 1 and \"err\" in x:\n        return kaname.Err(error={de})\n    raise _Wrong(at, _keyed(x), {need_key}, 0)\nraise _wrong(at, x, {need})\n"
                    )),
                    Target::Go => {
                        let (ga, ge) = (self.go_type(p, a), self.go_type(p, e));
                        format!(
                            "func {n}(x any, at string) kaname.Result[{ga}, {ge}] {{\n\tif m, ok := x.(map[string]any); ok {{\n\t\tif v, ok := m[\"ok\"]; ok && len(m) == 1 {{\n\t\t\treturn kaname.Ok[{ga}, {ge}]{{Value: {da}}}\n\t\t}}\n\t\tif v, ok := m[\"err\"]; ok && len(m) == 1 {{\n\t\t\treturn kaname.Err[{ga}, {ge}]{{Error: {de}}}\n\t\t}}\n\t\tpanic(wrongJSON{{path: at, given: keyedText(x), needed: {need_key}}})\n\t}}\n\tpanic(wrongAt(at, x, {need}))\n}}\n"
                        )
                    }
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                }
            }
            Ty::Record(rid @ RecId::Module { file, name, .. }) => {
                let m = p.module(file).clone();
                let Some(r) = m.decls.records.iter().find(|r| r.name.text == *name).cloned() else { return String::new() };
                let ty = match tt {
                    Target::Go => self.go_type(p, t),
                    _ => format!("{}.{}", self.mod_ref(file), type_name(tt, r.name.ascii())),
                };
                let mut fields = Vec::new();
                // a generic record's fields with its type arguments put in (DESIGN 26.1)
                for (f, slot) in p.fields(rid) {
                    let d = self.decode_in(p, c, &slot.ty, slot.range.as_ref(), &self.got("o", &f.text), &self.key_at("at", &f.text));
                    fields.push((field_name(tt, f.ascii()), d));
                }
                let none = fields.is_empty();
                match tt {
                    Target::TypeScript => {
                        let head = if none { "object(x, at);\n" } else { "const o = object(x, at);\n" };
                        ts(&format!("{head}return {{ {} }};\n", fields.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join(", ")))
                    }
                    Target::Python => {
                        let head = if none { "_object(x, at)\n" } else { "o = _object(x, at)\n" };
                        py(&format!("{head}return {ty}({})\n", fields.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(", ")))
                    }
                    Target::Go => {
                        let head = if none { "jsonObject(x, at)" } else { "o := jsonObject(x, at)" };
                        format!("func {n}(x any, at string) {ty} {{\n\t{head}\n\treturn {ty}{{{}}}\n}}\n", fields.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join(", "))
                    }
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                }
            }
            _ => match tt {
                Target::TypeScript => ts("return x;\n"),
                Target::Python => py("return x\n"),
                Target::Go => format!("func {n}(x any, at string) any {{\n\treturn x\n}}\n"),
                Target::Rust => unreachable!("Rust's harness answers through the door"),
            },
        }
    }

    /// The decoder of a value of a sum (DESIGN 23.6): a value with no fields by its name, a value with
    /// fields as an object of one pair, its name and its fields (a value with no fields that way too,
    /// as Rust's door reads it). The place of a field of a value is after the value's name
    /// (`event.approve.by`).
    fn sum_decoder(&mut self, p: &Program, c: &Checked, id: &crate::ty::EnumId, file: &std::path::Path, n: &str) -> String {
        let tt = self.t;
        let lang = self.lang;
        let m = p.module(file).clone();
        let Some(e) = m.decls.enums.iter().find(|e| e.name.text == id.name).cloned() else { return String::new() };
        let mref = self.mod_ref(file);
        let args = if tt == Target::Go { self.go_args(p, &id.args) } else { String::new() };
        let mut plain: Vec<(String, String)> = Vec::new();
        let mut with: Vec<(String, String)> = Vec::new();
        for v in &e.values {
            // a generic sum's fields with its type arguments put in (DESIGN 26.1)
            let fs = p.value_fields(id, &v.text);
            let vt = format!("{mref}.{}{args}", variant_name(tt, e.name.ascii(), v.ascii()));
            if fs.is_empty() {
                let built = match tt {
                    Target::TypeScript => format!("{{ kind: {} }}", ritsu_emit::lit::json(&v.text)),
                    Target::Python => format!("{vt}()"),
                    Target::Go => format!("{vt}{{}}"),
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                };
                plain.push((v.text.clone(), built));
                continue;
            }
            let mut fields = Vec::new();
            for (f, slot) in &fs {
                let d = self.decode_in(p, c, &slot.ty, slot.range.as_ref(), &self.got("o", &f.text), &self.key_at("a", &f.text));
                fields.push((field_name(tt, f.ascii()), d));
            }
            let built = match tt {
                Target::TypeScript => format!("{{ kind: {}, {} }}", ritsu_emit::lit::json(&v.text), fields.iter().map(|(k, d)| format!("{k}: {d}")).collect::<Vec<_>>().join(", ")),
                Target::Python => format!("{vt}({})", fields.iter().map(|(k, d)| format!("{k}={d}")).collect::<Vec<_>>().join(", ")),
                Target::Go => format!("{vt}{{{}}}", fields.iter().map(|(k, d)| format!("{k}: {d}")).collect::<Vec<_>>().join(", ")),
                Target::Rust => unreachable!("Rust's harness answers through the door"),
            };
            with.push((v.text.clone(), built));
        }
        let all: Vec<String> = e.values.iter().map(|v| v.text.clone()).collect();
        let names: Vec<String> = plain.iter().map(|(v, _)| v.clone()).collect();
        // what the place must be, every name of the sum's values listed: said of a string that is
        // no name of a value of no fields too (`"approve"` for an event, whose approval is an object
        // of the one key `approve`), and of what is neither a string nor an object; an object of
        // other keys is said by its keys, and what the object must have
        let need_key = self.lit(&one_key_of(lang, &all));
        let need = self.lit(&sum_need(lang, &names, &all));
        let q = |v: &str| if tt == Target::Go { ritsu_emit::lit::go(v) } else { ritsu_emit::lit::json(v) };
        match tt {
            Target::TypeScript => {
                let mut s = format!("function {n}(x: any, at: string): any {{\n");
                if !plain.is_empty() {
                    s.push_str("  if (typeof x === \"string\") {\n    switch (x) {\n");
                    for (v, b) in &plain {
                        s.push_str(&format!("      case {}:\n        return {b};\n", q(v)));
                    }
                    s.push_str(&format!("    }}\n    throw wrong(at, x, {need});\n  }}\n"));
                }
                s.push_str("  if (isObject(x)) {\n    const ks = Object.keys(x);\n    if (ks.length === 1) {\n");
                if !with.is_empty() {
                    s.push_str("      const f = x[ks[0]];\n      const a = at + \".\" + ks[0];\n");
                }
                s.push_str("      switch (ks[0]) {\n");
                for (v, b) in &plain {
                    s.push_str(&format!("        case {}:\n          return {b};\n", q(v)));
                }
                for (v, b) in &with {
                    s.push_str(&format!("        case {}: {{\n          const o = object(f, a);\n          return {b};\n        }}\n", q(v)));
                }
                s.push_str(&format!("      }}\n    }}\n    throw new Wrong(at, keyed(x), {need_key}, 0);\n  }}\n  throw wrong(at, x, {need});\n}}\n"));
                s
            }
            Target::Python => {
                let mut s = format!("def {n}(x: Any, at: str) -> Any:\n");
                if !plain.is_empty() {
                    s.push_str("    if isinstance(x, str):\n");
                    for (v, b) in &plain {
                        s.push_str(&format!("        if x == {}:\n            return {b}\n", q(v)));
                    }
                    s.push_str(&format!("        raise _wrong(at, x, {need})\n"));
                }
                s.push_str("    if isinstance(x, dict):\n        if len(x) == 1:\n            k = next(iter(x))\n");
                if !with.is_empty() {
                    s.push_str("            f = x[k]\n            a = at + \".\" + k\n");
                }
                for (v, b) in &plain {
                    s.push_str(&format!("            if k == {}:\n                return {b}\n", q(v)));
                }
                for (v, b) in &with {
                    s.push_str(&format!("            if k == {}:\n                o = _object(f, a)\n                return {b}\n", q(v)));
                }
                s.push_str(&format!("        raise _Wrong(at, _keyed(x), {need_key}, 0)\n    raise _wrong(at, x, {need})\n"));
                s
            }
            Target::Go => {
                let gt = self.go_type(p, &Ty::Enum(id.clone()));
                let mut s = format!("func {n}(x any, at string) {gt} {{\n");
                if !plain.is_empty() {
                    s.push_str("\tif s, ok := x.(string); ok {\n\t\tswitch s {\n");
                    for (v, b) in &plain {
                        s.push_str(&format!("\t\tcase {}:\n\t\t\treturn {b}\n", q(v)));
                    }
                    s.push_str(&format!("\t\t}}\n\t\tpanic(wrongAt(at, x, {need}))\n\t}}\n"));
                }
                if with.is_empty() {
                    s.push_str("\tif xm, ok := x.(map[string]any); ok {\n\t\tif len(xm) == 1 {\n\t\t\tfor k := range xm {\n\t\t\t\tswitch k {\n");
                } else {
                    s.push_str("\tif xm, ok := x.(map[string]any); ok {\n\t\tif len(xm) == 1 {\n\t\t\tfor k, f := range xm {\n\t\t\t\ta := at + \".\" + k\n\t\t\t\tswitch k {\n");
                }
                for (v, b) in &plain {
                    s.push_str(&format!("\t\t\t\tcase {}:\n\t\t\t\t\treturn {b}\n", q(v)));
                }
                for (v, b) in &with {
                    s.push_str(&format!("\t\t\t\tcase {}:\n\t\t\t\t\to := jsonObject(f, a)\n\t\t\t\t\treturn {b}\n", q(v)));
                }
                s.push_str(&format!("\t\t\t\t}}\n\t\t\t}}\n\t\t}}\n\t\tpanic(wrongJSON{{path: at, given: keyedText(x), needed: {need_key}}})\n\t}}\n\tpanic(wrongAt(at, x, {need}))\n}}\n"));
                s
            }
            Target::Rust => unreachable!("Rust's harness answers through the door"),
        }
    }

    /// The encoder of a value of a sum: a value with no fields by its name, a value with fields as an
    /// object of one pair.
    fn sum_encoder(&mut self, p: &Program, c: &Checked, id: &crate::ty::EnumId, file: &std::path::Path, n: &str) -> String {
        let tt = self.t;
        let m = p.module(file).clone();
        let Some(e) = m.decls.enums.iter().find(|e| e.name.text == id.name).cloned() else { return String::new() };
        let mref = self.mod_ref(file);
        let args = if tt == Target::Go { self.go_args(p, &id.args) } else { String::new() };
        let mut arms: Vec<(String, String, String)> = Vec::new();
        for v in &e.values {
            let fs = p.value_fields(id, &v.text);
            let vt = format!("{mref}.{}{args}", variant_name(tt, e.name.ascii(), v.ascii()));
            let x = if tt == Target::Go { "v" } else { "x" };
            let out = if fs.is_empty() {
                match tt {
                    Target::Go => ritsu_emit::lit::go(&v.text),
                    _ => ritsu_emit::lit::json(&v.text),
                }
            } else {
                let mut parts = Vec::new();
                for (f, slot) in &fs {
                    let at = format!("{x}.{}", field_name(tt, f.ascii()));
                    let enc = self.encode(p, c, &slot.ty, &at);
                    let key = match tt {
                        Target::Go => ritsu_emit::lit::go(&f.text),
                        _ => ritsu_emit::lit::json(&f.text),
                    };
                    parts.push(format!("{key}: {enc}"));
                }
                match tt {
                    Target::TypeScript => format!("{{ {}: {{ {} }} }}", ritsu_emit::lit::json(&v.text), parts.join(", ")),
                    Target::Python => format!("{{{}: {{{}}}}}", ritsu_emit::lit::json(&v.text), parts.join(", ")),
                    Target::Go => format!("map[string]any{{{}: map[string]any{{{}}}}}", ritsu_emit::lit::go(&v.text), parts.join(", ")),
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                }
            };
            arms.push((v.text.clone(), vt, out));
        }
        match tt {
            Target::TypeScript => {
                let mut s = format!("function {n}(x: any): any {{\n  switch (x.kind) {{\n");
                for (v, _, out) in &arms {
                    s.push_str(&format!("    case {}:\n      return {out};\n", ritsu_emit::lit::json(v)));
                }
                s.push_str("  }\n  return null;\n}\n");
                s
            }
            Target::Python => {
                let mut s = format!("def {n}(x: Any) -> Any:\n");
                for (_, vt, out) in &arms {
                    s.push_str(&format!("    if isinstance(x, {vt}):\n        return {out}\n"));
                }
                s.push_str("    return None\n");
                s
            }
            Target::Go => {
                let gt = self.go_type(p, &Ty::Enum(id.clone()));
                let fields = e.fields.iter().any(|f| !f.is_empty());
                let mut s = format!("func {n}(x {gt}) any {{\n\tswitch {}x.(type) {{\n", if fields { "v := " } else { "" });
                for (_, vt, out) in &arms {
                    s.push_str(&format!("\tcase {vt}:\n\t\treturn {out}\n"));
                }
                s.push_str("\t}\n\treturn nil\n}\n");
                s
            }
            Target::Rust => unreachable!("Rust's harness answers through the door"),
        }
    }

    fn unit_go(&mut self, u: &ritsu_units::Unit) -> String {
        match u.type_name() {
            Some(n) => {
                self.units = true;
                format!("units.{n}")
            }
            None => "int64".into(),
        }
    }

    /// The type of the target a value of `t` travels as, for Go's codecs.
    fn go_type(&mut self, p: &Program, t: &Ty) -> String {
        match t {
            Ty::Num(n) if n.is_rate() => "int64".into(),
            Ty::Num(n) => self.unit_go(&n.unit),
            Ty::Bool => "bool".into(),
            Ty::Str => "string".into(),
            Ty::Date => "kaname.Date".into(),
            Ty::Opt(x) => format!("*{}", self.go_type(p, x)),
            Ty::List(x) => format!("[]{}", self.go_type(p, x)),
            Ty::Dict(k, v) => format!("map[{}]{}", self.go_type(p, k), self.go_type(p, v)),
            Ty::Set(x) => format!("map[{}]struct{{}}", self.go_type(p, x)),
            Ty::Record(RecId::Entry(k, v)) => format!("kaname.Entry[{}, {}]", self.go_type(p, k), self.go_type(p, v)),
            Ty::Record(RecId::Indexed(x)) => format!("kaname.Indexed[{}]", self.go_type(p, x)),
            Ty::Record(RecId::Running(x, w)) => format!("kaname.Running[{}, {}]", self.go_type(p, x), self.go_type(p, w)),
            Ty::Record(RecId::Element(f, seq)) => {
                let r = self.rule_ref(p, f);
                let facts = p.rules.get(f).cloned().expect("a rule the module reads");
                format!("{r}.{}", self.element_name(&facts, seq))
            }
            // a rule's answer as the rule's code gives it: the struct of its outputs, or one output's
            // value itself, which names the rule's package only where it is an enum or a rate of the
            // rule's (a package imported and not named is an error of Go's)
            Ty::Record(RecId::Outputs(f)) => {
                let facts = p.rules.get(f).cloned().expect("a rule the module reads");
                match facts.outputs.as_slice() {
                    [one] if !names_rule_go(&one.ty) => self.col_go("", &one.ty, &facts),
                    [one] => {
                        let r = self.rule_ref(p, f);
                        self.col_go(&r, &one.ty, &facts)
                    }
                    _ => format!("{}.Output", self.rule_ref(p, f)),
                }
            }
            Ty::Enum(id) => match &id.owner {
                Owner::Module(f) => {
                    let m = p.module(f);
                    let alias = m.decls.enums.iter().find(|e| e.name.text == id.name).map(|e| e.name.ascii().to_string()).unwrap_or_default();
                    let args = if p.is_sum(id) { self.go_args(p, &id.args) } else { String::new() };
                    format!("{}.{}{args}", self.mod_ref(f), type_name(Target::Go, &alias))
                }
                Owner::Rule(f) => {
                    let r = self.rule_ref(p, f);
                    format!("{r}.{}", rule_enum_alias(p.rules.get(f).map(|r| &r.go), &id.name))
                }
            },
            Ty::Record(RecId::Module { file, name, args }) => {
                let m = p.module(file);
                let alias = m.decls.records.iter().find(|r| r.name.text == *name).map(|r| r.name.ascii().to_string()).unwrap_or_default();
                let args = self.go_args(p, args);
                format!("{}.{}{args}", self.mod_ref(file), type_name(Target::Go, &alias))
            }
            Ty::Record(RecId::Variant { file, name, value, args }) => {
                let m = p.module(file);
                let Some(e) = m.decls.enums.iter().find(|e| e.name.text == *name) else { return "any".into() };
                let va = e.values.iter().find(|v| v.text == *value).map(|v| v.ascii().to_string()).unwrap_or_default();
                let args = self.go_args(p, args);
                format!("{}.{}{args}", self.mod_ref(file), variant_name(Target::Go, e.name.ascii(), &va))
            }
            Ty::Result(a, e) => format!("kaname.Result[{}, {}]", self.go_type(p, a), self.go_type(p, e)),
            Ty::Transfer(id) => {
                let b = self.book_ref(p, &id.book);
                format!("{b}.{}", super::books::op_type(Target::Go, &id.transfer, &id.op))
            }
            _ => "any".into(),
        }
    }

    /// Go's type arguments of a generic record or sum (`[int64]`); nothing for none.
    fn go_args(&mut self, p: &Program, args: &[Ty]) -> String {
        if args.is_empty() {
            return String::new();
        }
        let xs: Vec<String> = args.iter().map(|a| self.go_type(p, a)).collect();
        format!("[{}]", xs.join(", "))
    }

    // ── To the wire ──────────────────────────────────────────────────────

    /// The JSON on the wire of the value at `v` of `t`.
    fn encode(&mut self, p: &Program, c: &Checked, t: &Ty, v: &str) -> String {
        let key = format!("enc|{t:?}");
        let f = self.codec(key, |h, n| h.encoder(p, c, t, n));
        format!("{f}({v})")
    }

    fn encoder(&mut self, p: &Program, c: &Checked, t: &Ty, n: &str) -> String {
        let tt = self.t;
        let gt = if tt == Target::Go { self.go_type(p, t) } else { String::new() };
        let body: String = match t {
            Ty::Opt(inner) => {
                let e = self.encode(p, c, inner, if tt == Target::Go { "*x" } else { "x" });
                match tt {
                    Target::TypeScript => format!("x === null ? null : {e}"),
                    Target::Python => format!("None if x is None else {e}"),
                    Target::Go => return format!("func {n}(x {gt}) any {{\n\tif x == nil {{\n\t\treturn nil\n\t}}\n\treturn {e}\n}}\n"),
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                }
            }
            Ty::List(inner) => {
                let e = self.encode(p, c, inner, "e");
                match tt {
                    Target::TypeScript => format!("x.map((e: any) => {e})"),
                    Target::Python => format!("[{e} for e in x]"),
                    Target::Go => return format!("func {n}(x {gt}) any {{\n\tout := []any{{}}\n\tfor _, e := range x {{\n\t\tout = append(out, {e})\n\t}}\n\treturn out\n}}\n"),
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                }
            }
            // a dict as its pairs and a set as its members: in the order TypeScript's `Map` and `Set`
            // and Python's `dict` go round in, which the code makes the order of the keys; Python's set
            // and Go's map and set, which have none, sorted (DESIGN 24.4)
            Ty::Dict(k, v) => {
                let ek = self.encode(p, c, k, "k");
                let ev = self.encode(p, c, v, "v");
                match tt {
                    Target::TypeScript => format!("[...x.entries()].map(([k, v]: any) => [{ek}, {ev}])"),
                    Target::Python => format!("[[{ek}, {ev}] for k, v in x.items()]"),
                    Target::Go => {
                        let rank = self.rank_go(p, k);
                        let (kt, vt) = (self.go_type(p, k), self.go_type(p, v));
                        return format!("func {n}(x {gt}) any {{\n\treturn pairs(x, func(k {kt}) any {{ return {ek} }}, func(v {vt}) any {{ return {ev} }}, {rank})\n}}\n");
                    }
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                }
            }
            Ty::Set(inner) => {
                let e = self.encode(p, c, inner, "e");
                match tt {
                    Target::TypeScript => format!("[...x].map((e: any) => {e})"),
                    Target::Python => {
                        let key = self.rank_py(p, inner);
                        format!("sorted([{e} for e in x], key={key})")
                    }
                    Target::Go => {
                        let rank = self.rank_go(p, inner);
                        let et = self.go_type(p, inner);
                        return format!("func {n}(x {gt}) any {{\n\treturn members(x, func(e {et}) any {{ return {e} }}, {rank})\n}}\n");
                    }
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                }
            }
            Ty::Record(RecId::Entry(k, v)) => {
                let (fk, fv) = if tt == Target::Go { ("x.Key", "x.Value") } else { ("x.key", "x.value") };
                let fields = vec![("key".to_string(), self.encode(p, c, k, fk)), ("value".to_string(), self.encode(p, c, v, fv))];
                return self.object(&fields, n, &gt);
            }
            Ty::Record(RecId::Indexed(x0)) => {
                let (fi, fx) = if tt == Target::Go { ("x.Index", "x.Item") } else { ("x.index", "x.item") };
                let fields = vec![("index".to_string(), self.encode(p, c, &Ty::Num(crate::ty::NumTy::number()), fi)), ("item".to_string(), self.encode(p, c, x0, fx))];
                return self.object(&fields, n, &gt);
            }
            Ty::Num(_) => match tt {
                Target::Go => return format!("func {n}(x {gt}) any {{\n\treturn int64(x)\n}}\n"),
                _ => "x".into(),
            },
            Ty::Date => match tt {
                Target::TypeScript => "x".into(),
                Target::Python => "x.isoformat()".into(),
                Target::Go => return format!("func {n}(x kaname.Date) any {{\n\treturn x.String()\n}}\n"),
                Target::Rust => unreachable!("Rust's harness answers through the door"),
            },
            Ty::Enum(id @ crate::ty::EnumId { owner: Owner::Module(file), .. }) if p.is_sum(id) => return self.sum_encoder(p, c, id, file, n),
            Ty::Record(r @ RecId::Variant { .. }) => {
                let Some(id) = r.sum() else { return String::new() };
                let e = self.encode(p, c, &Ty::Enum(id), "x");
                match tt {
                    Target::Go => return format!("func {n}(x {gt}) any {{\n\treturn {e}\n}}\n"),
                    _ => e,
                }
            }
            Ty::Enum(id) => match (tt, &id.owner) {
                (Target::TypeScript, _) => "x".into(),
                (Target::Python, _) => "x.value".into(),
                (Target::Go, Owner::Module(_)) => return format!("func {n}(x {gt}) any {{\n\treturn string(x)\n}}\n"),
                (Target::Go, Owner::Rule(_)) => return format!("func {n}(x {gt}) any {{\n\treturn x.String()\n}}\n"),
                (Target::Rust, _) => unreachable!("Rust's harness answers through the door"),
            },
            Ty::Record(rid @ RecId::Module { .. }) => {
                let mut fields = Vec::new();
                for (f, slot) in p.fields(rid) {
                    let at = format!("x.{}", field_name(tt, f.ascii()));
                    fields.push((f.text.clone(), self.encode(p, c, &slot.ty, &at)));
                }
                return self.object(&fields, n, &gt);
            }
            // a result as its one pair, `ok` or `err` (DESIGN 26.4)
            Ty::Result(a, e) => {
                let (at_ok, at_err) = if tt == Target::Go { ("v.Value", "v.Error") } else { ("x.value", "x.error") };
                let (ea, ee) = (self.encode(p, c, a, at_ok), self.encode(p, c, e, at_err));
                match tt {
                    Target::TypeScript => format!("x.kind === \"ok\" ? {{ ok: {ea} }} : {{ err: {ee} }}"),
                    Target::Python => format!("{{\"ok\": {ea}}} if isinstance(x, kaname.Ok) else {{\"err\": {ee}}}"),
                    Target::Go => {
                        let (ga, ge) = (self.go_type(p, a), self.go_type(p, e));
                        return format!(
                            "func {n}(x {gt}) any {{\n\tswitch v := x.(type) {{\n\tcase kaname.Ok[{ga}, {ge}]:\n\t\treturn map[string]any{{\"ok\": {ea}}}\n\tcase kaname.Err[{ga}, {ge}]:\n\t\treturn map[string]any{{\"err\": {ee}}}\n\t}}\n\treturn nil\n}}\n"
                        );
                    }
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                }
            }
            // a rule's answer: an object of its outputs by their names, each a column of the rule's.
            // Go's type of it (`gt`) imports the rule's package where the code names it
            Ty::Record(RecId::Outputs(f)) => {
                let facts = p.rules.get(f).cloned().expect("a rule the module reads");
                let r = self.rule_import(p, f).0;
                let call = match tt {
                    Target::TypeScript => &facts.typescript,
                    Target::Python => &facts.python,
                    Target::Go => &facts.go,
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                };
                let mut fields = Vec::new();
                for (k, col) in facts.outputs.iter().enumerate() {
                    let at = if facts.outputs.len() == 1 { "x".to_string() } else { format!("x.{}", call.outputs.get(k).map(|o| o.alias.clone()).unwrap_or_default()) };
                    fields.push((col.name.clone(), self.encode_col(&facts, &r, &col.ty, &at)));
                }
                return self.object(&fields, n, &gt);
            }
            // an element of a list a rule walks, as the rule's code holds it (DESIGN 4.4), on the wire
            // as the reference writes it: an object of its fields by their names in the rule, each a
            // column of the rule's (a date from its count of days, an enum by its value's name). Go's
            // type of it (`gt`) imports the rule's package
            Ty::Record(RecId::Element(file, seq)) => {
                let facts = p.rules.get(file).cloned().expect("a rule the module reads");
                let Some((_, _, cols)) = facts.elements.iter().find(|(x, _, _)| x == seq).cloned() else { return String::new() };
                let r = self.rule_import(p, file).0;
                let mut fields = Vec::new();
                for ((f, _), col) in p.fields(&RecId::Element(file.clone(), seq.clone())).iter().zip(&cols) {
                    let name = if tt == Target::Go { ritsu_emit::ident::pascal(&col.alias) } else { col.alias.clone() };
                    fields.push((f.text.clone(), self.encode_col(&facts, &r, &col.ty, &format!("x.{name}"))));
                }
                return self.object(&fields, n, &gt);
            }
            Ty::Record(RecId::Running(item, w)) => {
                let k = |s: &str| if tt == Target::Go { format!("x.{}{}", s[..1].to_uppercase(), &s[1..]) } else { format!("x.{s}") };
                let fields = vec![
                    ("item".to_string(), self.encode(p, c, item, &k("item"))),
                    ("before".to_string(), self.encode(p, c, w, &k("before"))),
                    ("upto".to_string(), self.encode(p, c, w, &k("upto"))),
                    ("total".to_string(), self.encode(p, c, w, &k("total"))),
                ];
                let gt = if tt == Target::Go { format!("kaname.Running[{}, {}]", self.go_type(p, item), self.go_type(p, w)) } else { gt.clone() };
                return self.object(&fields, n, &gt);
            }
            Ty::Transfer(id) => {
                let book = p.books.get(&id.book).cloned().expect("a book the module reads");
                let tr = book.transfers.iter().find(|x| x.name == id.transfer).cloned().expect("a transfer of the book");
                let names = super::books::param_names(tt, &book, &id.transfer);
                let b = self.book_ref(p, &id.book);
                let gt = if tt == Target::Go { format!("{b}.{}", super::books::op_type(Target::Go, &id.transfer, &id.op)) } else { gt.clone() };
                let ps: Vec<(String, String, bool)> = tr
                    .params
                    .iter()
                    .zip(&names)
                    .filter(|(prm, _)| match id.op.as_str() {
                        "do" | "hold" => true,
                        "post" => tr.key.contains(&prm.name) || prm.unit.is_some(),
                        _ => tr.key.contains(&prm.name),
                    })
                    .map(|(prm, n)| (prm.name.clone(), n.clone(), id.op == "post" && prm.unit.is_some() && !tr.key.contains(&prm.name)))
                    .collect();
                let q = ritsu_emit::lit::json;
                match tt {
                    Target::TypeScript => "x".into(),
                    Target::Python => {
                        let args: Vec<String> = ps.iter().map(|(wire, py, _)| format!("({}, x.{py})", q(wire))).collect();
                        format!("{{\"book\": x.book, \"transfer\": x.transfer, \"op\": x.op, \"args\": {{k: v for k, v in [{}] if v is not None}}}}", args.join(", "))
                    }
                    Target::Go => {
                        let mut s = format!("func {n}(x {gt}) any {{\n\targs := map[string]any{{}}\n");
                        for (wire, gname, optional) in &ps {
                            if *optional {
                                s.push_str(&format!("\tif x.{gname} != nil {{\n\t\targs[{}] = int64(*x.{gname})\n\t}}\n", ritsu_emit::lit::go(wire)));
                            } else {
                                let is_amount = tr.params.iter().find(|p| p.name == *wire).is_some_and(|p| p.unit.is_some());
                                if is_amount {
                                    s.push_str(&format!("\targs[{}] = int64(x.{gname})\n", ritsu_emit::lit::go(wire)));
                                } else {
                                    s.push_str(&format!("\targs[{}] = x.{gname}\n", ritsu_emit::lit::go(wire)));
                                }
                            }
                        }
                        s.push_str(&format!(
                            "\treturn map[string]any{{\"book\": {}, \"transfer\": {}, \"op\": {}, \"args\": args}}\n}}\n",
                            ritsu_emit::lit::go(&book.name),
                            ritsu_emit::lit::go(&id.transfer),
                            ritsu_emit::lit::go(&id.op)
                        ));
                        return s;
                    }
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                }
            }
            _ => match tt {
                Target::Go => return format!("func {n}(x any) any {{\n\treturn x\n}}\n"),
                _ => "x".into(),
            },
        };
        match tt {
            Target::TypeScript => format!("function {n}(x: any): any {{\n  return {body};\n}}\n"),
            Target::Python => format!("def {n}(x: Any) -> Any:\n    return {body}\n"),
            Target::Go => format!("func {n}(x {gt}) any {{\n\treturn {body}\n}}\n"),
            Target::Rust => unreachable!("Rust's harness answers through the door"),
        }
    }

    /// Go's order of the keys of `t` on the wire: nil for their own order (a number, a string, a date
    /// written `YYYY-MM-DD`, `false` before `true`), the places of an enum's values by their names.
    fn rank_go(&mut self, p: &Program, t: &Ty) -> String {
        match t {
            Ty::Enum(id) => {
                let names: Vec<String> = p.values(id).0.iter().map(|v| ritsu_emit::lit::go(&v.text)).collect();
                format!("func(v any) int {{ return slices.Index([]string{{{}}}, v.(string)) }}", names.join(", "))
            }
            _ => "nil".into(),
        }
    }

    /// Python's key of the keys of `t` on the wire.
    fn rank_py(&mut self, p: &Program, t: &Ty) -> String {
        match t {
            Ty::Enum(id) => {
                let names: Vec<String> = p.values(id).0.iter().map(|v| ritsu_emit::lit::json(&v.text)).collect();
                format!("[{}].index", names.join(", "))
            }
            _ => "None".into(),
        }
    }

    /// A rule's column, as its code holds it, on the wire.
    fn encode_col(&mut self, facts: &RuleFacts, r: &str, t: &ColumnType, at: &str) -> String {
        let tt = self.t;
        match t {
            ColumnType::Opt(x) => {
                let inner = self.encode_col(facts, r, x, if tt == Target::Go { "(*v)" } else { "v" });
                match tt {
                    Target::TypeScript => format!("(((v: any) => v === null ? null : {inner})({at}))"),
                    Target::Python => format!("((lambda v: None if v is None else {inner})({at}))"),
                    // a function of its own, as gofmt writes it (a closure on one line is not)
                    Target::Go => {
                        let gt = self.col_go(r, t, facts);
                        let f = self.codec(format!("enccol|{gt}|{inner}"), |_, n| format!("func {n}(v {gt}) any {{\n\tif v == nil {{\n\t\treturn nil\n\t}}\n\treturn {inner}\n}}\n"));
                        format!("{f}({at})")
                    }
                    Target::Rust => unreachable!("Rust's harness answers through the door"),
                }
            }
            ColumnType::Date { .. } => match tt {
                Target::TypeScript => format!("kaname.dateOf(Number({at}))"),
                Target::Python => format!("kaname.date_of({at}).isoformat()"),
                Target::Go => format!("kaname.DateOf(int64({at})).String()"),
                Target::Rust => unreachable!("Rust's harness answers through the door"),
            },
            ColumnType::Enum { .. } => match tt {
                Target::TypeScript => at.to_string(),
                Target::Python => format!("{at}.value"),
                Target::Go => format!("{at}.String()"),
                Target::Rust => unreachable!("Rust's harness answers through the door"),
            },
            ColumnType::Num { .. } => match tt {
                Target::Go => format!("int64({at})"),
                _ => at.to_string(),
            },
            _ => at.to_string(),
        }
    }

    /// An object of the wire from fields already encoded.
    fn object(&self, fields: &[(String, String)], n: &str, gt: &str) -> String {
        match self.t {
            Target::TypeScript => format!("function {n}(x: any): any {{\n  return {{ {} }};\n}}\n", fields.iter().map(|(k, v)| format!("{}: {v}", ritsu_emit::lit::json(k))).collect::<Vec<_>>().join(", ")),
            Target::Python => format!("def {n}(x: Any) -> Any:\n    return {{{}}}\n", fields.iter().map(|(k, v)| format!("{}: {v}", ritsu_emit::lit::json(k))).collect::<Vec<_>>().join(", ")),
            Target::Go => format!("func {n}(x {gt}) any {{\n\treturn map[string]any{{{}}}\n}}\n", fields.iter().map(|(k, v)| format!("{}: {v}", ritsu_emit::lit::go(k))).collect::<Vec<_>>().join(", ")),
            Target::Rust => unreachable!("Rust's harness answers through the door"),
        }
    }

    // ── The program ──────────────────────────────────────────────────────

    /// The program of the cases: each case's call and the encoding of its answer, whether it can fail,
    /// and its function as a refusal of its readers (a day no date holds, DESIGN 22.5) names it, the
    /// failure `input` of the function, said as the package's entries say it.
    fn finish(&self, cases: &[(String, usize, String, String, bool, Entry)]) -> String {
        let defs = self.defs.join("\n");
        match self.t {
            Target::TypeScript => {
                let mut s = String::from("// The harness of kaname's tests: each case through the generated code, a JSON line each.\nimport { readFileSync } from \"node:fs\";\nimport * as kaname from \"./glue/kaname\";\n");
                for (i, m) in self.modules.iter().enumerate() {
                    s.push_str(&format!("import * as m{i} from \"./glue/{}\";\n", m.module.name.ascii()));
                }
                for line in self.imports.values() {
                    s.push_str(line);
                    s.push('\n');
                }
                s.push_str(TS_IO);
                s.push_str(&ts_read(self.lang, false));
                s.push_str(&defs);
                s.push_str("\nconst cases: any[] = parse(readFileSync(process.argv[2], \"utf8\"));\nfor (const c of cases) {\n  let entry: [string, string, number] = [\"\", \"\", 0];\n  try {\n    let r: any;\n    let out: any;\n    switch (c.m + \".\" + String(c.f)) {\n");
                for (m, f, call, out, _, e) in cases {
                    s.push_str(&format!("      case {}:\n        entry = [{}, {}, {}];\n        r = {call};\n        out = {out};\n        break;\n", ritsu_emit::lit::json(&format!("{m}.{f}")), e.fun, e.module, e.line));
                }
                s.push_str("      default:\n        throw new Error(\"no such function\");\n    }\n    console.log(show({ out }));\n  } catch (e) {\n    const x = entered(e, entry[0], entry[1], entry[2]);\n    if (x instanceof kaname.KanameError) console.log(show({ fails: x.path === undefined ? { kind: x.kind, module: x.module, line: BigInt(x.line), message: x.message } : { kind: x.kind, module: x.module, line: BigInt(x.line), message: x.message, path: x.path } }));\n    else console.log(show({ error: String(x) }));\n  }\n}\n");
                s
            }
            Target::Python => {
                let mut s = format!("\"\"\"The harness of kaname's tests: each case through the generated code, a JSON line each.\"\"\"\n\nimport datetime as _dt\nimport json\nimport sys\nfrom typing import Any\n\nfrom {n}.glue import kaname\n", n = self.name);
                for (i, m) in self.modules.iter().enumerate() {
                    s.push_str(&format!("from {}.glue import {} as m{i}\n", self.name, m.module.name.ascii()));
                }
                for line in self.imports.values() {
                    s.push_str(line);
                    s.push('\n');
                }
                s.push_str("\n\n");
                s.push_str(&py_read(self.lang, false));
                s.push_str(&defs);
                s.push_str("\n\ndef main() -> None:\n    with open(sys.argv[1], encoding=\"utf-8\") as f:\n        cases = json.load(f)\n    for c in cases:\n        r: Any = None\n        out: Any = None\n        entry: tuple[str, str, int] = (\"\", \"\", 0)\n        try:\n            key = c[\"m\"] + \".\" + str(c[\"f\"])\n");
                let mut first = true;
                for (m, f, call, out, _, e) in cases {
                    let kw = if first { "if" } else { "elif" };
                    first = false;
                    s.push_str(&format!("            {kw} key == {}:\n                entry = ({}, {}, {})\n                r = {call}\n                out = {out}\n", ritsu_emit::lit::json(&format!("{m}.{f}")), e.fun, e.module, e.line));
                }
                if cases.is_empty() {
                    s.push_str("            raise ValueError(\"no such function\")\n");
                } else {
                    s.push_str("            else:\n                raise ValueError(\"no such function\")\n");
                }
                s.push_str("            print(json.dumps({\"out\": out}, ensure_ascii=False))\n        except (kaname.KanameError, _Wrong) as x:\n            k = x if isinstance(x, kaname.KanameError) else kaname.KanameError(\"input\", entry[1], entry[2], _said(entry[0], x), x.path)\n            fails: dict[str, object] = {\"kind\": k.kind, \"module\": k.module, \"line\": k.line, \"message\": k.message}\n            if k.path is not None:\n                fails[\"path\"] = k.path\n            print(json.dumps({\"fails\": fails}, ensure_ascii=False))\n        except Exception as e:  # noqa: BLE001\n            print(json.dumps({\"error\": repr(e)}, ensure_ascii=False))\n\n\nmain()\n");
                s
            }
            Target::Go => {
                let mut imports: Vec<String> = vec![format!("\"{}/glue/kaname\"", self.go_module)];
                if self.units {
                    imports.push(format!("\"{}/units\"", self.go_module));
                }
                // a module a host calls (not one of generic functions alone), or whose types the codecs name
                let named = self.named.borrow();
                for (i, m) in self.modules.iter().enumerate().filter(|(i, m)| m.decls.fns.iter().any(|f| !f.inner()) || named.contains(i)) {
                    let p = go_package(m.module.name.ascii());
                    imports.push(format!("m{i} \"{}/glue/{p}\"", self.go_module));
                }
                for line in self.imports.values() {
                    imports.push(line.clone());
                }
                // in the order gofmt keeps: by the path
                imports.sort_by(|a, b| a.split('"').nth(1).cmp(&b.split('"').nth(1)));
                imports.dedup();
                let mut s = format!("// The harness of kaname's tests: each case through the generated code, a JSON line each.\npackage main\n\nimport (\n\t\"bytes\"\n\t\"encoding/json\"\n\t\"errors\"\n\t\"fmt\"\n\t\"io\"\n\t\"os\"\n\t\"slices\"\n\t\"strconv\"\n\t\"strings\"\n\t\"unicode/utf8\"\n\n{}\n)\n\n", imports.iter().map(|i| format!("\t{i}")).collect::<Vec<_>>().join("\n"));
                s.push_str(GO_IO);
                s.push_str(&go_read(self.lang));
                s.push_str(&defs);
                s.push_str("\nfunc run(c kase) (out any, err error) {\n\tfun, module, line := \"\", \"\", 0\n\tdefer func() {\n\t\tif r := recover(); r != nil {\n\t\t\terr = entryError(r, fun, module, line)\n\t\t}\n\t}()\n\tswitch c.M + \".\" + strconv.Itoa(c.F) {\n");
                for (m, f, call, out, fails, e) in cases {
                    s.push_str(&format!("\tcase {}:\n\t\tfun, module, line = {}, {}, {}\n", ritsu_emit::lit::go(&format!("{m}.{f}")), e.fun, e.module, e.line));
                    if *fails {
                        s.push_str(&format!("\t\tr, err := {call}\n\t\tif err != nil {{\n\t\t\treturn nil, err\n\t\t}}\n\t\treturn {out}, nil\n"));
                    } else {
                        s.push_str(&format!("\t\tr := {call}\n\t\treturn {out}, nil\n"));
                    }
                }
                s.push_str("\t}\n\treturn nil, errors.New(\"no such function\")\n}\n\nfunc main() {\n\tcases := read(os.Args[1])\n\tfor _, c := range cases {\n\t\tout, err := run(c)\n\t\tvar k *kaname.KanameError\n\t\tswitch {\n\t\tcase errors.As(err, &k):\n\t\t\tfails := map[string]any{\"kind\": k.Kind, \"module\": k.Module, \"line\": k.Line, \"message\": k.Message}\n\t\t\tif k.Path != \"\" {\n\t\t\t\tfails[\"path\"] = k.Path\n\t\t\t}\n\t\t\tsay(map[string]any{\"fails\": fails})\n\t\tcase err != nil:\n\t\t\tsay(map[string]any{\"error\": err.Error()})\n\t\tdefault:\n\t\t\tsay(map[string]any{\"out\": out})\n\t\t}\n\t}\n}\n");
                s
            }
            Target::Rust => unreachable!("Rust's harness answers through the door"),
        }
    }
}

/// A rule's enum, as one language's code names its type.
fn rule_enum_alias(call: Option<&ritsu_ports::Call>, name: &str) -> String {
    call.and_then(|c| c.enums.iter().find(|e| e.name == name)).map(|e| e.alias.clone()).unwrap_or_default()
}

/// Whether Go's type of a rule's column ([`H::col_go`]) names the rule's package: an enum of the
/// rule's, a rate.
fn names_rule_go(t: &ColumnType) -> bool {
    match t {
        ColumnType::Enum { .. } => true,
        ColumnType::Num { unit: Some(u), .. } => u.dim == ritsu_units::Dim::Rate,
        ColumnType::Opt(x) => names_rule_go(x),
        _ => false,
    }
}

impl H {
    /// The Go type of a rule's column.
    fn col_go(&mut self, r: &str, t: &ColumnType, facts: &RuleFacts) -> String {
        match t {
            ColumnType::Bool => "bool".into(),
            ColumnType::Str => "string".into(),
            ColumnType::Date { .. } => "int64".into(),
            ColumnType::Enum { name: n, .. } => format!("{r}.{}", rule_enum_alias(Some(&facts.go), n)),
            ColumnType::Num { unit: Some(u), .. } if u.dim == ritsu_units::Dim::Rate => format!("{r}.Rate"),
            ColumnType::Num { unit: Some(u), .. } => self.unit_go(u),
            ColumnType::Num { unit: None, .. } => "int64".into(),
            ColumnType::Opt(x) => format!("*{}", self.col_go(r, x, facts)),
        }
    }
}

/// What every TypeScript harness reads and writes JSON with: an integer as a `bigint`, to the digit, a
/// number written with a point or an exponent as it is written ([`ts_read`]'s `Real`). Where the text
/// is no value of JSON, an entry's `parse` stops with the place `at`.
const TS_IO: &str = r#"
function parse(text: string, at?: string): any {
  try {
    return JSON.parse(text, (_k: string, v: any, ctx?: { source?: string }) => (typeof v === "number" && ctx?.source !== undefined ? number(ctx.source) : v));
  } catch (e) {
    if (at === undefined) throw e;
    throw new Wrong(at, "", "", 1);
  }
}

function number(source: string): any {
  return /^-?(0|[1-9][0-9]*)$/.test(source) ? BigInt(source) : new Real(source);
}

function show(v: any): string {
  if (v === null || v === undefined) return "null";
  if (typeof v === "bigint") return v.toString();
  if (typeof v === "number") return String(v);
  if (typeof v === "string" || typeof v === "boolean") return JSON.stringify(v);
  if (Array.isArray(v)) return "[" + v.map(show).join(",") + "]";
  return "{" + Object.entries(v).filter(([, x]) => x !== undefined).map(([k, x]) => JSON.stringify(k) + ":" + show(x)).join(",") + "}";
}

"#;

/// What a file of Go entries that take JSON writes it with ([`json_entries`]): the orders of a dict's
/// and a set's members. It reads JSON with [`go_read`].
const GO_LIB_IO: &str = r#"// order is the order of two keys on the wire: by rank where it is given, else a number's, a
// string's (a date written YYYY-MM-DD too), false before true.
func order(a, b any, rank func(any) int) int {
	if rank != nil {
		return rank(a) - rank(b)
	}
	switch x := a.(type) {
	case int64:
		y := b.(int64)
		if x < y {
			return -1
		}
		if x > y {
			return 1
		}
		return 0
	case string:
		return strings.Compare(x, b.(string))
	case bool:
		y := b.(bool)
		if x == y {
			return 0
		}
		if !x {
			return -1
		}
		return 1
	}
	return 0
}

// pairs is a dict on the wire: each pair [key, value], in the order of the keys.
func pairs[K comparable, V any](m map[K]V, ek func(K) any, ev func(V) any, rank func(any) int) any {
	ks := make([]K, 0, len(m))
	for k := range m {
		ks = append(ks, k)
	}
	slices.SortFunc(ks, func(a, b K) int { return order(ek(a), ek(b), rank) })
	out := []any{}
	for _, k := range ks {
		out = append(out, []any{ek(k), ev(m[k])})
	}
	return out
}

// members is a set on the wire: its members, in their order.
func members[T comparable](s map[T]struct{}, e func(T) any, rank func(any) int) any {
	xs := []any{}
	for x := range s {
		xs = append(xs, e(x))
	}
	slices.SortFunc(xs, func(a, b any) int { return order(a, b, rank) })
	return xs
}

"#;

/// What every Go harness reads its cases with and writes its lines with: numbers as json.Number, to
/// the digit. It reads the values with [`go_read`].
const GO_IO: &str = r#"type kase struct {
	M  string         `json:"m"`
	F  int            `json:"f"`
	In map[string]any `json:"in"`
}

func read(path string) []kase {
	data, err := os.ReadFile(path)
	if err != nil {
		panic(err)
	}
	var raw []map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		panic(err)
	}
	var out []kase
	for _, r := range raw {
		var c kase
		_ = json.Unmarshal(r["m"], &c.M)
		_ = json.Unmarshal(r["f"], &c.F)
		d := json.NewDecoder(bytes.NewReader(r["in"]))
		d.UseNumber()
		if err := d.Decode(&c.In); err != nil {
			panic(err)
		}
		out = append(out, c)
	}
	return out
}

func say(v any) {
	b, err := json.Marshal(v)
	if err != nil {
		panic(err)
	}
	fmt.Println(string(b))
}

// order is the order of two keys on the wire: by rank where it is given, else a number's, a
// string's (a date written YYYY-MM-DD too), false before true.
func order(a, b any, rank func(any) int) int {
	if rank != nil {
		return rank(a) - rank(b)
	}
	switch x := a.(type) {
	case int64:
		y := b.(int64)
		if x < y {
			return -1
		}
		if x > y {
			return 1
		}
		return 0
	case string:
		return strings.Compare(x, b.(string))
	case bool:
		y := b.(bool)
		if x == y {
			return 0
		}
		if !x {
			return -1
		}
		return 1
	}
	return 0
}

// pairs is a dict on the wire: each pair [key, value], in the order of the keys.
func pairs[K comparable, V any](m map[K]V, ek func(K) any, ev func(V) any, rank func(any) int) any {
	ks := make([]K, 0, len(m))
	for k := range m {
		ks = append(ks, k)
	}
	slices.SortFunc(ks, func(a, b K) int { return order(ek(a), ek(b), rank) })
	out := []any{}
	for _, k := range ks {
		out = append(out, []any{ek(k), ev(m[k])})
	}
	return out
}

// members is a set on the wire: its members, in their order.
func members[T comparable](s map[T]struct{}, e func(T) any, rank func(any) int) any {
	xs := []any{}
	for x := range s {
		xs = append(xs, e(x))
	}
	slices.SortFunc(xs, func(a, b any) int { return order(a, b, rank) })
	return xs
}

"#;

// ── What the readers say (DESIGN 16.4) ───────────────────────────────────────

/// What a value must be, as a reader says it.
#[derive(Clone, Copy)]
pub(super) enum Need {
    Str,
    Int,
    Bool,
    Date,
    Array,
    Object,
    Pair,
}

/// What a value of `n` must be, in `lang`: the words after "and must" of the English sentence, those
/// before "が要ります" of the Japanese.
pub(super) fn needs(lang: Lang, n: Need) -> &'static str {
    match lang {
        Lang::En => match n {
            Need::Str => "be a string",
            Need::Int => "be an integer",
            Need::Bool => "be true or false",
            Need::Date => "be a date written YYYY-MM-DD",
            Need::Array => "be an array",
            Need::Object => "be an object",
            Need::Pair => "be a pair [key, value]",
        },
        Lang::Ja => match n {
            Need::Str => "文字列",
            Need::Int => "整数",
            Need::Bool => "true か false",
            Need::Date => "YYYY-MM-DD と書いた日付",
            Need::Array => "配列",
            Need::Object => "オブジェクト",
            Need::Pair => "[キー, 値] の組",
        },
    }
}

/// Names as the sentences list them: each as JSON writes it, in the order they are declared.
fn listed(lang: Lang, names: &[String]) -> String {
    let sep = match lang {
        Lang::En => ", ",
        Lang::Ja => "、",
    };
    names.iter().map(|v| ritsu_base::json::quote(v)).collect::<Vec<_>>().join(sep)
}

/// What a value of an enum must be: one of its names.
pub(super) fn one_of(lang: Lang, names: &[String]) -> String {
    match (lang, names.len()) {
        (Lang::En, 1) => format!("be {}", listed(lang, names)),
        (Lang::En, _) => format!("be one of {}", listed(lang, names)),
        (Lang::Ja, 1) => listed(lang, names),
        (Lang::Ja, _) => format!("{} のどれか", listed(lang, names)),
    }
}

/// An object of one key, one of `names`.
fn keyed_by(lang: Lang, names: &[String]) -> String {
    match (lang, names.len()) {
        (Lang::En, 1) => format!("an object with the one key {}", listed(lang, names)),
        (Lang::En, _) => format!("an object with one key of {}", listed(lang, names)),
        (Lang::Ja, 1) => format!("キーを {} 一つだけ持つオブジェクト", listed(lang, names)),
        (Lang::Ja, _) => format!("キーを {} のどれか一つだけ持つオブジェクト", listed(lang, names)),
    }
}

/// What an object given for a value of a sum must have: one key, one of `names`.
pub(super) fn one_key_of(lang: Lang, names: &[String]) -> String {
    match (lang, names.len()) {
        (Lang::En, 1) => format!("have the one key {}", listed(lang, names)),
        (Lang::En, _) => format!("have one key of {}", listed(lang, names)),
        (Lang::Ja, _) => keyed_by(lang, names),
    }
}

/// What a value of a sum must be: the name of one of its values with no fields (`plain`), or an
/// object of one key, the name of any of its values (`all`).
pub(super) fn sum_need(lang: Lang, plain: &[String], all: &[String]) -> String {
    match (lang, plain.is_empty()) {
        (Lang::En, true) => format!("be {}", keyed_by(lang, all)),
        (Lang::En, false) => format!("{}, or {}", one_of(lang, plain), keyed_by(lang, all)),
        (Lang::Ja, true) => keyed_by(lang, all),
        (Lang::Ja, false) => format!("{}、または{}", one_of(lang, plain), keyed_by(lang, all)),
    }
}

/// Whether the decoder of `t` reads a date with the range of its place: a date, or one inside a value
/// that may be none, a list, a set, or a dict's values (the places a slot's range reaches).
fn dated(t: &Ty) -> bool {
    match t {
        Ty::Date => true,
        Ty::Opt(x) | Ty::List(x) | Ty::Set(x) | Ty::Dict(_, x) => dated(x),
        _ => false,
    }
}

/// The range of a date's place as the entry's check says it: the ends written, the type's where one
/// is not (DESIGN 22.5).
pub(crate) fn day_range(range: Option<&crate::ty::Range>) -> String {
    match crate::ty::filled(range, crate::ty::whole_range(&Ty::Date)) {
        Some(r) => crate::value::range_text(&r, &Ty::Date),
        None => String::new(),
    }
}

/// The lines of `body`, each with `n` spaces before it.
fn indented(body: &str, n: usize) -> String {
    let pad = " ".repeat(n);
    body.lines().map(|l| if l.is_empty() { "\n".to_string() } else { format!("{pad}{l}\n") }).collect()
}

/// The words of a reader's sentences in `lang`, as each target's code writes them: `w` is the error
/// (its `path`, `given` and `needed`) and `fun` the function's name.
struct Said {
    /// What is at a place, written as JSON writes it (a string, a number, `true`, `null`), from `s`.
    lit: &'static str,
    an_array: &'static str,
    an_object: &'static str,
    no_key: &'static str,
    /// An object of one key (`k`, written as JSON writes it), and of `n` keys (an expression of a count
    /// as text).
    one_key: (&'static str, &'static str),
    keys: (&'static str, &'static str),
    json: &'static str,
    int64: &'static str,
    missing: &'static str,
    wrong: &'static str,
    /// A day that no date holds (Python's, or a count of days of a rule's element): what is there
    /// (`given`) is outside the range of its place (`needed`), as the function's check says it.
    day: &'static str,
}

fn said_in(lang: Lang) -> Said {
    match lang {
        Lang::En => Said {
            lit: "s",
            an_array: "\"an array\"",
            an_object: "\"an object\"",
            no_key: "\"an object with no key\"",
            one_key: ("\"an object with the key \" + ", ""),
            keys: ("\"an object with \" + ", " + \" keys\""),
            json: "\"The argument `\" + w.path + \"` of `\" + fun + \"` is not one value of JSON\"",
            int64: "\"The argument `\" + w.path + \"` of `\" + fun + \"` is outside int64\"",
            missing: "\"The argument `\" + w.path + \"` of `\" + fun + \"` is missing, and must \" + w.needed",
            wrong: "\"The argument `\" + w.path + \"` of `\" + fun + \"` is \" + w.given + \", and must \" + w.needed",
            day: "\"The argument `\" + w.path + \"` of `\" + fun + \"` is \" + w.given + \", outside its range \" + w.needed",
        },
        Lang::Ja => Said {
            lit: "\" \" + s + \" \"",
            an_array: "\"配列\"",
            an_object: "\"オブジェクト\"",
            no_key: "\"キーの無いオブジェクト\"",
            one_key: ("\"キー \" + ", " + \" のオブジェクト\""),
            keys: ("\"キーを \" + ", " + \" 個持つオブジェクト\""),
            json: "\"関数 `\" + fun + \"` の引数 `\" + w.path + \"` は、一つの JSON の値ではありません\"",
            int64: "\"関数 `\" + fun + \"` の引数 `\" + w.path + \"` が int64 の外にあります\"",
            missing: "\"関数 `\" + fun + \"` の引数 `\" + w.path + \"` がありません。\" + w.needed + \"が要ります\"",
            wrong: "\"関数 `\" + fun + \"` の引数 `\" + w.path + \"` が\" + w.given + \"です。\" + w.needed + \"が要ります\"",
            day: "\"関数 `\" + fun + \"` の引数 `\" + w.path + \"` の \" + w.given + \" は、範囲 \" + w.needed + \" の外です\"",
        },
    }
}

/// `template` with each `@NAME@` put in: the words of [`Said`] and what values must be, as the target
/// writes a string (`lit`).
fn put_words(template: &str, lang: Lang, lit: impl Fn(&str) -> String, one_key: &str, keys: &str) -> String {
    let w = said_in(lang);
    let mut s = template.to_string();
    for (k, v) in [
        ("@LIT@", w.lit.to_string()),
        ("@AN_ARRAY@", w.an_array.to_string()),
        ("@AN_OBJECT@", w.an_object.to_string()),
        ("@NO_KEY@", w.no_key.to_string()),
        ("@ONE_KEY@", format!("{}{one_key}{}", w.one_key.0, w.one_key.1)),
        ("@KEYS@", format!("{}{keys}{}", w.keys.0, w.keys.1)),
        ("@JSON@", w.json.to_string()),
        ("@INT64@", w.int64.to_string()),
        ("@MISSING@", w.missing.to_string()),
        ("@WRONG@", w.wrong.to_string()),
        ("@DAY@", w.day.to_string()),
        ("@NEED_STR@", lit(needs(lang, Need::Str))),
        ("@NEED_INT@", lit(needs(lang, Need::Int))),
        ("@NEED_DATE@", lit(needs(lang, Need::Date))),
        ("@NEED_ARRAY@", lit(needs(lang, Need::Array))),
        ("@NEED_OBJECT@", lit(needs(lang, Need::Object))),
        ("@NEED_PAIR@", lit(needs(lang, Need::Pair))),
    ] {
        s = s.replace(k, &v);
    }
    s
}

/// What TypeScript's readers read with and say in `lang` (DESIGN 16.4); in a file of entries
/// ([`json_entries`]), an integer past int64 is refused, as Go's and Rust's readers refuse it, where
/// the harness hands it to the function, whose check it is the harness's to try (DESIGN 22.4).
fn ts_read(lang: Lang, entries: bool) -> String {
    let past = if entries { "  if (typeof x === \"bigint\" && (x < -(2n ** 63n) || x > 2n ** 63n - 1n)) throw new Wrong(at, \"\", \"\", 2);\n" } else { "" };
    put_words(TS_READ, lang, ritsu_emit::lit::json, "quoted(ks[0])", "ks.length").replace("@INT64_PAST@", past)
}

/// What Python's readers read with and say in `lang`; in a file of entries, an integer past int64 is
/// refused, as in TypeScript's.
fn py_read(lang: Lang, entries: bool) -> String {
    let past = if entries { "    if isinstance(x, int) and not isinstance(x, bool) and not -(2**63) <= x <= 2**63 - 1:\n        raise _Wrong(at, \"\", \"\", 2)\n" } else { "" };
    put_words(PY_READ, lang, ritsu_emit::lit::json, "_quoted(next(iter(x)))", "str(len(x))").replace("@INT64_PAST@", past)
}

/// What Go's readers read with and say in `lang`.
fn go_read(lang: Lang) -> String {
    put_words(GO_READ, lang, ritsu_emit::lit::go, "quotedText(k)", "strconv.Itoa(len(m))")
}

const TS_READ: &str = r#"/** A number JSON writes with a point or an exponent, as it is written. */
class Real {
  readonly text: string;
  constructor(text: string) {
    this.text = text;
  }
}

/**
 * JSON that is no value of the type of its place (kaname's DESIGN 16.4): the place, what is there
 * and what it must be; `kind` 1 where the text is no value of JSON, 2 where a whole number is outside
 * int64, 3 where the date of a field of a rule's element is no day of the calendar (outside its range,
 * which `needed` is), 4 where the key is missing.
 */
class Wrong {
  readonly path: string;
  readonly given: string;
  readonly needed: string;
  readonly kind: number;
  constructor(path: string, given: string, needed: string, kind: number) {
    this.path = path;
    this.given = given;
    this.needed = needed;
    this.kind = kind;
  }
}

function isObject(x: any): boolean {
  return typeof x === "object" && x !== null && !Array.isArray(x) && !(x instanceof Real);
}

/** A string as the sentences show it: its first 40 characters, as JSON writes them. */
function quoted(s: string): string {
  const cs = Array.from(s, (c) => (c.length === 1 && c >= "\ud800" && c <= "\udfff" ? "�" : c));
  return JSON.stringify(cs.slice(0, 40).join("") + (cs.length > 40 ? "…" : ""));
}

function lit(s: string): string {
  return @LIT@;
}

/** What is at a place, as the sentences say it. */
function given(x: any): string {
  if (x === null) return lit("null");
  if (typeof x === "boolean" || typeof x === "bigint") return lit(String(x));
  if (x instanceof Real) return lit(x.text);
  if (typeof x === "string") return lit(quoted(x));
  if (Array.isArray(x)) return @AN_ARRAY@;
  return @AN_OBJECT@;
}

/** An object given for a value of a sum, as the sentences say it: by its keys. */
function keyed(x: any): string {
  if (!isObject(x)) return given(x);
  const ks = Object.keys(x);
  if (ks.length === 0) return @NO_KEY@;
  if (ks.length === 1) return @ONE_KEY@;
  return @KEYS@;
}

function wrong(at: string, x: any, needed: string): Wrong {
  return x === undefined ? new Wrong(at, "", needed, 4) : new Wrong(at, given(x), needed, 0);
}

/** The sentence of `w`, of an argument of the function `fun`. */
function said(fun: string, w: Wrong): string {
  switch (w.kind) {
    case 1:
      return @JSON@;
    case 2:
      return @INT64@;
    case 3:
      return @DAY@;
    case 4:
      return @MISSING@;
    default:
      return @WRONG@;
  }
}

/** What a call of an entry throws for what stopped it: JSON no value of its type is the failure `input`. */
function entered(e: unknown, fun: string, module: string, line: number): unknown {
  return e instanceof Wrong ? new kaname.KanameError("input", module, line, said(fun, e), undefined, e.path) : e;
}

/** The value of the key `k` of the object `o`; undefined where it has none. */
function got(o: any, k: string): any {
  return Object.prototype.hasOwnProperty.call(o, k) ? o[k] : undefined;
}

function array(x: any, at: string): any[] {
  if (Array.isArray(x)) return x;
  throw wrong(at, x, @NEED_ARRAY@);
}

function object(x: any, at: string): any {
  if (isObject(x)) return x;
  throw wrong(at, x, @NEED_OBJECT@);
}

/** A pair of a dict on the wire, `[key, value]`. */
function pair(x: any, at: string): any[] {
  if (Array.isArray(x) && x.length === 2) return x;
  throw wrong(at, x, @NEED_PAIR@);
}

/** A whole number: an integer as it is written, or one written with a point or an exponent (`20.0`, `2e1`) within int64. */
function int(x: any, at: string): bigint {
@INT64_PAST@  if (typeof x === "bigint") return x;
  if (x instanceof Real) {
    const [v, w] = whole(x.text);
    if (w === 0) return v;
    if (w === 2) throw new Wrong(at, "", "", 2);
  }
  throw wrong(at, x, @NEED_INT@);
}

/** The int64 a number written with a point or an exponent is, and 0; 1 where it is no whole number, 2 where it is outside int64. */
function whole(text: string): [bigint, number] {
  const neg = text.startsWith("-");
  let s = neg ? text.slice(1) : text;
  let exp = 0;
  const k = s.search(/[eE]/);
  if (k >= 0) {
    const e = s.slice(k + 1);
    s = s.slice(0, k);
    const ds = e.replace(/^[+-]/, "");
    exp = (e.startsWith("-") ? -1 : 1) * (ds.length > 18 ? 1e18 : Number(ds));
  }
  const p = s.indexOf(".");
  let digits = p < 0 ? s : s.slice(0, p) + s.slice(p + 1);
  if (p >= 0) exp -= s.length - p - 1;
  if (exp < 0) {
    const t = digits.replace(/0+$/, "");
    if (digits.length - t.length >= -exp) {
      digits = digits.slice(0, digits.length + exp);
      exp = 0;
    } else if (t === "") {
      digits = "";
      exp = 0;
    } else {
      return [0n, 1];
    }
  }
  digits = digits.replace(/^0+/, "");
  if (digits === "") return [0n, 0];
  if (digits.length + exp > 19) return [0n, 2];
  const v = BigInt(digits) * 10n ** BigInt(exp) * (neg ? -1n : 1n);
  return v < -(2n ** 63n) || v > 2n ** 63n - 1n ? [0n, 2] : [v, 0];
}

/** A date written YYYY-MM-DD (four digits or more, then two and two, a year int32 holds), of any month and day: the function holds it to the days of the calendar. */
function day(x: any, at: string): string {
  if (typeof x === "string" && /^[0-9]{4,}-[0-9]{2}-[0-9]{2}$/.test(x) && BigInt(x.slice(0, -6)) <= 2147483647n) return x;
  throw wrong(at, x, @NEED_DATE@);
}

"#;

const PY_READ: &str = r#"_MISSING: Any = object()


class _Real:
    """A number JSON writes with a point or an exponent, or an integer of more digits than int64
    holds, as it is written."""

    __slots__ = ("text",)

    def __init__(self, text: str) -> None:
        self.text = text


class _Wrong(Exception):
    """JSON that is no value of the type of its place (kaname's DESIGN 16.4): the place, what is
    there and what it must be; `kind` 1 where the text is no value of JSON, 2 where a whole number
    is outside int64, 3 where a day is outside what a date of Python holds, 4 where the key is
    missing."""

    def __init__(self, path: str, given: str, needed: str, kind: int) -> None:
        super().__init__(path)
        self.path = path
        self.given = given
        self.needed = needed
        self.kind = kind


def _constant(s: str) -> Any:
    raise ValueError(s)


def _whole_text(s: str) -> Any:
    # an integer of more digits than int64 holds as it is written, as Python's int takes none of more
    # than 4300 digits from a text
    return _Real(s) if len(s) > 20 else int(s)


def _parse(text: str, at: str) -> Any:
    try:
        return json.loads(text, parse_float=_Real, parse_int=_whole_text, parse_constant=_constant)
    except (ValueError, RecursionError):
        raise _Wrong(at, "", "", 1) from None


def _lit(s: str) -> str:
    return @LIT@


def _quoted(s: str) -> str:
    t = "".join("�" if 0xD800 <= ord(c) <= 0xDFFF else c for c in s[:40])
    return json.dumps(t + ("…" if len(s) > 40 else ""), ensure_ascii=False)


def _given(x: Any) -> str:
    if x is None:
        return _lit("null")
    if isinstance(x, bool):
        return _lit("true" if x else "false")
    if isinstance(x, int):
        return _lit(str(x))
    if isinstance(x, _Real):
        return _lit(x.text)
    if isinstance(x, str):
        return _lit(_quoted(x))
    if isinstance(x, list):
        return @AN_ARRAY@
    return @AN_OBJECT@


def _keyed(x: Any) -> str:
    if not isinstance(x, dict):
        return _given(x)
    if not x:
        return @NO_KEY@
    if len(x) == 1:
        return @ONE_KEY@
    return @KEYS@


def _wrong(at: str, x: Any, needed: str) -> _Wrong:
    if x is _MISSING:
        return _Wrong(at, "", needed, 4)
    return _Wrong(at, _given(x), needed, 0)


def _said(fun: str, w: _Wrong) -> str:
    if w.kind == 1:
        return @JSON@
    if w.kind == 2:
        return @INT64@
    if w.kind == 3:
        return @DAY@
    if w.kind == 4:
        return @MISSING@
    return @WRONG@


def _got(o: Any, k: str) -> Any:
    return o.get(k, _MISSING)


def _array(x: Any, at: str) -> Any:
    if isinstance(x, list):
        return x
    raise _wrong(at, x, @NEED_ARRAY@)


def _object(x: Any, at: str) -> Any:
    if isinstance(x, dict):
        return x
    raise _wrong(at, x, @NEED_OBJECT@)


def _pair(x: Any, at: str) -> Any:
    if isinstance(x, list) and len(x) == 2:
        return x
    raise _wrong(at, x, @NEED_PAIR@)


def _int(x: Any, at: str) -> int:
@INT64_PAST@    if isinstance(x, int) and not isinstance(x, bool):
        return x
    if isinstance(x, _Real):
        v, w = _whole(x.text)
        if w == 0:
            return v
        if w == 2:
            raise _Wrong(at, "", "", 2)
    raise _wrong(at, x, @NEED_INT@)


def _whole(text: str) -> tuple[int, int]:
    neg = text.startswith("-")
    s = text[1:] if neg else text
    exp = 0
    k = max(s.find("e"), s.find("E"))
    if k >= 0:
        e = s[k + 1 :]
        s = s[:k]
        ds = e.lstrip("+-")
        exp = (-1 if e.startswith("-") else 1) * (10**18 if len(ds) > 18 else int(ds))
    p = s.find(".")
    digits = s if p < 0 else s[:p] + s[p + 1 :]
    if p >= 0:
        exp -= len(s) - p - 1
    if exp < 0:
        t = digits.rstrip("0")
        if len(digits) - len(t) >= -exp:
            digits = digits[: len(digits) + exp]
            exp = 0
        elif not t:
            digits, exp = "", 0
        else:
            return 0, 1
    digits = digits.lstrip("0")
    if not digits:
        return 0, 0
    if len(digits) + exp > 19:
        return 0, 2
    v = int(digits) * 10**exp * (-1 if neg else 1)
    if v < -(2**63) or v > 2**63 - 1:
        return 0, 2
    return v, 0


def _day(x: Any, at: str, rt: str) -> _dt.date:
    if isinstance(x, str):
        p = x.split("-")
        if len(p) == 3 and len(p[0]) >= 4 and len(p[1]) == 2 and len(p[2]) == 2 and all(q.isascii() and q.isdigit() for q in p):
            y = p[0].lstrip("0") or "0"
            if len(y) <= 10 and int(y) <= 2147483647:
                try:
                    return _dt.date(int(y), int(p[1]), int(p[2]))
                except ValueError:
                    raise _Wrong(at, x, rt, 3) from None
    raise _wrong(at, x, @NEED_DATE@)


"#;

const GO_READ: &str = r#"// keyAbsent is the value of a key an object leaves out.
type keyAbsent struct{}

// wrongJSON is JSON that is no value of the type of its place (kaname's DESIGN 16.4): the place,
// what is there and what it must be; kind 1 where the text is no value of JSON, 2 where a whole
// number is outside int64, 3 where the date of a field of a rule's element is no day of the
// calendar (outside its range, which needed is), 4 where the key is missing.
type wrongJSON struct {
	path   string
	given  string
	needed string
	kind   int
}

// jsonValue is the value of a JSON text, its numbers to the digit. A text that is no UTF-8 is no
// JSON; an object's key written twice is the later, as JavaScript's JSON.parse reads it; and a
// string written with a lone surrogate (`"\ud800"`), which Go's reader reads as U+FFFD, holds the
// surrogate, as a string of TypeScript or Python does, for the function's check to refuse.
func jsonValue(raw json.RawMessage, at string) any {
	if !utf8.Valid(raw) {
		panic(wrongJSON{path: at, kind: 1})
	}
	d := json.NewDecoder(bytes.NewReader(raw))
	d.UseNumber()
	v, ok := tokenValue(d, raw, 0)
	if !ok {
		panic(wrongJSON{path: at, kind: 1})
	}
	if _, err := d.Token(); err != io.EOF {
		panic(wrongJSON{path: at, kind: 1})
	}
	return v
}

// tokenValue is the value of the JSON the decoder reads next, token by token, as deep as Go's
// reader reads a value (10000); false where it is no value of JSON.
func tokenValue(d *json.Decoder, raw []byte, depth int) (any, bool) {
	if depth > 10000 {
		return nil, false
	}
	start := d.InputOffset()
	t, err := d.Token()
	if err != nil {
		return nil, false
	}
	switch x := t.(type) {
	case json.Delim:
		switch x {
		case '[':
			a := []any{}
			for d.More() {
				v, ok := tokenValue(d, raw, depth+1)
				if !ok {
					return nil, false
				}
				a = append(a, v)
			}
			_, err := d.Token()
			return a, err == nil
		case '{':
			o := map[string]any{}
			for d.More() {
				k, err := d.Token()
				key, isKey := k.(string)
				if err != nil || !isKey {
					return nil, false
				}
				v, ok := tokenValue(d, raw, depth+1)
				if !ok {
					return nil, false
				}
				o[key] = v
			}
			_, err := d.Token()
			return o, err == nil
		}
		return nil, false
	case string:
		return textOf(raw[start:d.InputOffset()], x), true
	}
	return t, true
}

// textOf is the string a JSON string reads as, lit its text after what comes before it (white
// space, a comma, a colon) and s the string Go reads: s, or where an escape is a lone surrogate, the
// string with the surrogate's three bytes in its place (as WTF-8 writes it), which is no UTF-8.
func textOf(lit []byte, s string) string {
	i := bytes.IndexByte(lit, '"')
	if i < 0 || !bytes.Contains(lit, []byte(`\u`)) {
		return s
	}
	var b []byte
	lone := false
	for j := i + 1; j < len(lit) && lit[j] != '"'; j++ {
		if lit[j] != '\\' {
			b = append(b, lit[j])
			continue
		}
		j++
		switch lit[j] {
		case 'b':
			b = append(b, '\b')
		case 'f':
			b = append(b, '\f')
		case 'n':
			b = append(b, '\n')
		case 'r':
			b = append(b, '\r')
		case 't':
			b = append(b, '\t')
		case 'u':
			r := hex4(lit[j+1 : j+5])
			j += 4
			if r >= 0xd800 && r < 0xdc00 && j+6 < len(lit) && lit[j+1] == '\\' && lit[j+2] == 'u' {
				if lo := hex4(lit[j+3 : j+7]); lo >= 0xdc00 && lo < 0xe000 {
					b = utf8.AppendRune(b, 0x10000+(r-0xd800)<<10+(lo-0xdc00))
					j += 6
					continue
				}
			}
			if r >= 0xd800 && r < 0xe000 {
				lone = true
				b = append(b, 0xe0|byte(r>>12), 0x80|byte(r>>6&0x3f), 0x80|byte(r&0x3f))
				continue
			}
			b = utf8.AppendRune(b, r)
		default:
			b = append(b, lit[j])
		}
	}
	if !lone {
		return s
	}
	return string(b)
}

// hex4 is the code unit four hex digits write.
func hex4(h []byte) rune {
	n, _ := strconv.ParseUint(string(h), 16, 32)
	return rune(n)
}

func givenLit(s string) string {
	return @LIT@
}

// quotedText is a string as the sentences show it: its first 40 characters, as JSON writes them, a
// lone surrogate (its three bytes, as jsonValue reads it) or a byte that is no UTF-8 as U+FFFD.
func quotedText(s string) string {
	var b strings.Builder
	b.WriteByte('"')
	n := 0
	for k := 0; k < len(s); {
		c, w := utf8.DecodeRuneInString(s[k:])
		if c == utf8.RuneError && w == 1 && k+2 < len(s) && s[k] == 0xed && s[k+1] >= 0xa0 && s[k+1] <= 0xbf && s[k+2] >= 0x80 && s[k+2] <= 0xbf {
			w = 3
		}
		k += w
		if n == 40 {
			b.WriteString("…")
			break
		}
		n++
		switch c {
		case '"':
			b.WriteString(`\"`)
		case '\\':
			b.WriteString(`\\`)
		case '\b':
			b.WriteString(`\b`)
		case '\t':
			b.WriteString(`\t`)
		case '\n':
			b.WriteString(`\n`)
		case '\f':
			b.WriteString(`\f`)
		case '\r':
			b.WriteString(`\r`)
		default:
			if c < 0x20 {
				fmt.Fprintf(&b, `\u%04x`, c)
			} else {
				b.WriteRune(c)
			}
		}
	}
	b.WriteByte('"')
	return b.String()
}

// givenText is what is at a place, as the sentences say it.
func givenText(x any) string {
	switch v := x.(type) {
	case nil:
		return givenLit("null")
	case bool:
		return givenLit(strconv.FormatBool(v))
	case json.Number:
		if v == "-0" {
			return givenLit("0")
		}
		return givenLit(string(v))
	case string:
		return givenLit(quotedText(v))
	case []any:
		return @AN_ARRAY@
	}
	return @AN_OBJECT@
}

// keyedText is an object given for a value of a sum, as the sentences say it: by its keys.
func keyedText(x any) string {
	m, ok := x.(map[string]any)
	if !ok {
		return givenText(x)
	}
	switch len(m) {
	case 0:
		return @NO_KEY@
	case 1:
		for k := range m {
			return @ONE_KEY@
		}
	}
	return @KEYS@
}

func wrongAt(at string, x any, needed string) wrongJSON {
	if _, ok := x.(keyAbsent); ok {
		return wrongJSON{path: at, needed: needed, kind: 4}
	}
	return wrongJSON{path: at, given: givenText(x), needed: needed}
}

// wrongSaid is the sentence of w, of an argument of the function fun.
func wrongSaid(fun string, w wrongJSON) string {
	switch w.kind {
	case 1:
		return @JSON@
	case 2:
		return @INT64@
	case 3:
		return @DAY@
	case 4:
		return @MISSING@
	}
	return @WRONG@
}

// entryError is the error of a call of an entry for what stopped it: JSON no value of its type is
// the failure input, as the function's own.
func entryError(r any, fun, module string, line int) error {
	switch v := r.(type) {
	case wrongJSON:
		return inputError(module, line, v.path, wrongSaid(fun, v))
	case *kaname.KanameError:
		return v
	}
	return fmt.Errorf("%v", r)
}

// inputError is the failure of the kind input at line of module, the argument not taken at path,
// its message as kaname writes it.
func inputError(module string, line int, path, message string) (err error) {
	defer kaname.Recover(&err)
	kaname.FailInput(module, line, path, message)
	return nil
}

// keyValue is the value of the key k of the object o; keyAbsent where it has none.
func keyValue(o map[string]any, k string) any {
	if v, ok := o[k]; ok {
		return v
	}
	return keyAbsent{}
}

func jsonArray(x any, at string) []any {
	if a, ok := x.([]any); ok {
		return a
	}
	panic(wrongAt(at, x, @NEED_ARRAY@))
}

func jsonObject(x any, at string) map[string]any {
	if o, ok := x.(map[string]any); ok {
		return o
	}
	panic(wrongAt(at, x, @NEED_OBJECT@))
}

// jsonPair is a pair of a dict on the wire, [key, value].
func jsonPair(x any, at string) []any {
	if a, ok := x.([]any); ok && len(a) == 2 {
		return a
	}
	panic(wrongAt(at, x, @NEED_PAIR@))
}

// jsonInt is a whole number of int64, written as JSON may write it: 20, 20.0, 2e1.
func jsonInt(x any, at string) int64 {
	if n, ok := x.(json.Number); ok {
		v, w := wholeOf(string(n))
		switch w {
		case 0:
			return v
		case 2:
			panic(wrongJSON{path: at, kind: 2})
		}
	}
	panic(wrongAt(at, x, @NEED_INT@))
}

// wholeOf is the int64 a number of JSON is, and 0; 1 where it is no whole number, 2 where it is
// outside int64.
func wholeOf(text string) (int64, int) {
	neg := strings.HasPrefix(text, "-")
	s := strings.TrimPrefix(text, "-")
	exp := int64(0)
	if k := strings.IndexAny(s, "eE"); k >= 0 {
		e := s[k+1:]
		s = s[:k]
		ds := strings.TrimLeft(e, "+-")
		n := int64(1e18)
		if len(ds) <= 18 {
			n, _ = strconv.ParseInt(ds, 10, 64)
		}
		if strings.HasPrefix(e, "-") {
			n = -n
		}
		exp = n
	}
	digits := s
	if p := strings.IndexByte(s, '.'); p >= 0 {
		digits = s[:p] + s[p+1:]
		exp -= int64(len(s) - p - 1)
	}
	if exp < 0 {
		t := strings.TrimRight(digits, "0")
		switch {
		case int64(len(digits)-len(t)) >= -exp:
			digits = digits[:int64(len(digits))+exp]
			exp = 0
		case t == "":
			digits, exp = "", 0
		default:
			return 0, 1
		}
	}
	digits = strings.TrimLeft(digits, "0")
	if digits == "" {
		return 0, 0
	}
	if int64(len(digits))+exp > 19 {
		return 0, 2
	}
	v, _ := strconv.ParseUint(digits, 10, 64)
	for ; exp > 0; exp-- {
		v *= 10
	}
	switch {
	case !neg && v <= 9223372036854775807:
		return int64(v), 0
	case neg && v <= 9223372036854775808:
		return -int64(v), 0
	}
	return 0, 2
}

// jsonDate is a date written YYYY-MM-DD (four digits or more, then two and two, a year int32
// holds), of any month and day, which the code holds to the days of the calendar.
func jsonDate(x any, at string) kaname.Date {
	if s, ok := x.(string); ok {
		p := strings.Split(s, "-")
		if len(p) == 3 && len(p[0]) >= 4 && len(p[1]) == 2 && len(p[2]) == 2 && allDigits(p[0]+p[1]+p[2]) {
			y, err := strconv.ParseInt(p[0], 10, 64)
			if err == nil && y <= 2147483647 {
				m, _ := strconv.Atoi(p[1])
				d, _ := strconv.Atoi(p[2])
				return kaname.Date{Year: int(y), Month: m, Day: d}
			}
		}
	}
	panic(wrongAt(at, x, @NEED_DATE@))
}

func allDigits(s string) bool {
	for _, c := range s {
		if c < '0' || c > '9' {
			return false
		}
	}
	return true
}

"#;
