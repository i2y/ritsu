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
    /// 22.4): a number past int64 (Go's `int64`), a day outside 0001-01-01 to 9999-12-31 and a day of
    /// no calendar (Python's `datetime.date`).
    pub targets: Vec<Target>,
}

impl Case {
    fn of(module: String, function: String, input: Json, expected: Option<Json>) -> Case {
        Case { module, function, input, expected, targets: Target::ALL.to_vec() }
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
                out.push(Case::of(c.module.name.ascii().to_string(), f.to_string(), line.get("in").cloned().unwrap_or(Json::Null), expected));
            }
        }
    }
    (out, diverged)
}

/// Inputs outside what a function's parameters declare, each at one part of one parameter (a step
/// past an end of a number's range, a day past a date's, one element past a list's count, one short
/// of it), the other parts at their base values, run in the reference interpreter: its answer is the
/// entry's failure `input` (DESIGN 4.4), which the vectors never reach (every vector is inside what
/// the parameters declare) and the code's public function is held to as well. Where a range is not
/// written, past the type's ends (DESIGN 22.4): 2⁶³ and −2⁶³ − 1 for a number, run in TypeScript and
/// Python; 10000-01-01 and 0000-12-31 for a date, and 2026-13-01, a day of no calendar (the
/// reference's answer is that of 10000-01-01, the day written so), run in TypeScript and Go. A string
/// one code point past its count, or one short of it; and a string that is not well formed, which the
/// reference interpreter cannot hold, so its answer is the entry's failure `input` made here (DESIGN
/// 25.8): each target makes its own of the marker the case writes (`{"bad": "string"}`), a lone
/// surrogate in TypeScript and Python and a byte that is no UTF-8 in Go.
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
                        for (end, step, mine) in [
                            (written.as_ref().and_then(|r| r.max), step, true),
                            (written.as_ref().and_then(|r| r.min), Rat::zero().sub(step), true),
                            (written.as_ref().is_none_or(|r| r.max.is_none()).then_some(whole.max).flatten(), Rat::int(1), false),
                            (written.as_ref().is_none_or(|r| r.min.is_none()).then_some(whole.min).flatten(), Rat::int(-1), false),
                        ] {
                            let Some(v) = end.map(|e| e.add(step)) else { continue };
                            if crate::num::fits(v) {
                                put(Value::Num(v), &Target::ALL);
                            } else if !mine {
                                // past int64, where the range is not written: Go's int64 cannot hold it
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
                        // cannot hold them
                        let open = slot.range.as_ref().is_none_or(|r| r.min.is_none() || r.max.is_none());
                        if open {
                            if let Some(i) = put(Value::Date(crate::date::MAX + 1), &[Target::TypeScript, Target::Go]) {
                                past_days.push(i);
                            }
                            put(Value::Date(crate::date::MIN - 1), &[Target::TypeScript, Target::Go]);
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
                            if let Some(v) = changed
                                && crate::num::fits(end)
                            {
                                put(v, &Target::ALL);
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

/// The program, in `t`, for the modules (their package's Python package is `name`; Go's import path
/// of the package's directory is `go_module`).
pub fn program(modules: &[Rc<Checked>], t: Target, name: &str, go_module: &str) -> String {
    let mut h = H { t, codecs: BTreeMap::new(), defs: Vec::new(), modules: modules.to_vec(), name: name.to_string(), go_module: go_module.to_string(), imports: BTreeMap::new(), units: false, named: std::cell::RefCell::default() };
    let mut cases = Vec::new();
    for (mi, c) in modules.iter().enumerate() {
        let p = Program::of(c);
        for (fi, info) in c.decls.fns.iter().enumerate() {
            if info.inner() {
                continue;
            }
            let args: Vec<String> = info.params.iter().map(|prm| h.decode(&p, c, &prm.slot.ty, &h.at_key(&prm.name.text))).collect();
            let call = h.call(c, mi, fi, &args);
            let out = h.encode(&p, c, &info.ret.ty, "r");
            cases.push((c.module.name.ascii().to_string(), fi, call, out, c.analysis.get().and_then(|a| a.fns.get(fi)).is_some_and(|x| !x.effect.fails.is_empty())));
        }
    }
    h.finish(&cases)
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
}

impl H {
    fn at_key(&self, k: &str) -> String {
        let q = ritsu_emit::lit::json(k);
        match self.t {
            Target::TypeScript => format!("c.in[{q}]"),
            Target::Python => format!("c[\"in\"][{q}]"),
            Target::Go => format!("c.In[{}]", ritsu_emit::lit::go(k)),
        }
    }

    /// The module of kaname a type is of, as the program names its import.
    fn mod_ref(&self, file: &std::path::Path) -> String {
        let i = self.modules.iter().position(|m| m.canon == file).unwrap_or(0);
        self.named.borrow_mut().insert(i);
        match self.t {
            Target::Go => go_package(self.modules[i].module.name.ascii()),
            _ => format!("m{i}"),
        }
    }

    fn rule_ref(&mut self, p: &Program, f: &std::path::Path) -> String {
        let facts = p.rules.get(f).expect("a rule the module reads");
        let (key, line) = match self.t {
            Target::TypeScript => {
                let m = facts.typescript.module.trim_end_matches(".ts");
                (format!("r_{}", facts.alias), format!("import * as r_{} from \"./rules/{m}\";", facts.alias))
            }
            Target::Python => (format!("r_{}", facts.alias), format!("from {}.rules import {} as r_{}", self.name, facts.python.module, facts.alias)),
            Target::Go => (facts.go.module.clone(), format!("\"{}/rules/{}\"", self.go_module, facts.go.module)),
        };
        self.imports.insert(key.clone(), line);
        key
    }

    fn book_ref(&mut self, p: &Program, f: &std::path::Path) -> String {
        let b = p.books.get(f).expect("a book the module reads");
        let (key, line) = match self.t {
            Target::TypeScript => (format!("b_{}", b.name), format!("import type * as b_{} from \"./glue/books/{}\";", b.name, b.name)),
            Target::Python => (format!("b_{}", b.name), format!("from {}.glue.books import {} as b_{}", self.name, b.name, b.name)),
            Target::Go => (format!("b{}", b.go.module), format!("b{} \"{}/glue/books/{}\"", b.go.module, self.go_module, b.go.module)),
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
            Target::Go => format!("{}.{f}({})", go_package(c.module.name.ascii()), args.join(", ")),
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

    /// The value of `t` as a host gives it, from the JSON at `x`.
    fn decode(&mut self, p: &Program, c: &Checked, t: &Ty, x: &str) -> String {
        let key = format!("dec|{t:?}");
        let tt = self.t;
        let f = self.codec(key, |h, n| h.decoder(p, c, t, n));
        let _ = tt;
        format!("{f}({x})")
    }

    fn decoder(&mut self, p: &Program, c: &Checked, t: &Ty, n: &str) -> String {
        let tt = self.t;
        let body = match t {
            Ty::Opt(inner) => {
                let d = self.decode(p, c, inner, "x");
                match tt {
                    Target::TypeScript => format!("x === null ? null : {d}"),
                    Target::Python => format!("None if x is None else {d}"),
                    Target::Go => {
                        let gt = self.go_type(p, inner);
                        return format!("func {n}(x any) *{gt} {{\n\tif x == nil {{\n\t\treturn nil\n\t}}\n\tv := {d}\n\treturn &v\n}}\n");
                    }
                }
            }
            Ty::List(inner) => {
                let d = self.decode(p, c, inner, "e");
                match tt {
                    Target::TypeScript => format!("(x as any[]).map((e: any) => {d})"),
                    Target::Python => format!("[{d} for e in x]"),
                    Target::Go => {
                        let gt = self.go_type(p, inner);
                        return format!("func {n}(x any) []{gt} {{\n\tout := []{gt}{{}}\n\tfor _, e := range x.([]any) {{\n\t\tout = append(out, {d})\n\t}}\n\treturn out\n}}\n");
                    }
                }
            }
            // a dict from its pairs, a set from its members, on the wire in any order (DESIGN 24.4)
            Ty::Dict(k, v) => {
                let dk = self.decode(p, c, k, "q[0]");
                let dv = self.decode(p, c, v, "q[1]");
                match tt {
                    Target::TypeScript => format!("new Map((x as any[]).map((q: any) => [{dk}, {dv}]))"),
                    Target::Python => format!("{{{dk}: {dv} for q in x}}"),
                    Target::Go => {
                        let gt = self.go_type(p, t);
                        return format!("func {n}(x any) {gt} {{\n\tout := {gt}{{}}\n\tfor _, p := range x.([]any) {{\n\t\tq := p.([]any)\n\t\tout[{dk}] = {dv}\n\t}}\n\treturn out\n}}\n");
                    }
                }
            }
            Ty::Set(inner) => {
                let d = self.decode(p, c, inner, "e");
                match tt {
                    Target::TypeScript => format!("new Set((x as any[]).map((e: any) => {d}))"),
                    Target::Python => format!("frozenset({d} for e in x)"),
                    Target::Go => {
                        let gt = self.go_type(p, t);
                        return format!("func {n}(x any) {gt} {{\n\tout := {gt}{{}}\n\tfor _, e := range x.([]any) {{\n\t\tout[{d}] = struct{{}}{{}}\n\t}}\n\treturn out\n}}\n");
                    }
                }
            }
            Ty::Record(RecId::Entry(k, v)) => {
                let (at_k, at_v) = match tt {
                    Target::Go => ("x.(map[string]any)[\"key\"]", "x.(map[string]any)[\"value\"]"),
                    _ => ("x[\"key\"]", "x[\"value\"]"),
                };
                let (dk, dv) = (self.decode(p, c, k, at_k), self.decode(p, c, v, at_v));
                match tt {
                    Target::TypeScript => format!("({{ key: {dk}, value: {dv} }})"),
                    Target::Python => format!("kaname.Entry(key={dk}, value={dv})"),
                    Target::Go => {
                        let gt = self.go_type(p, t);
                        return format!("func {n}(x any) {gt} {{\n\treturn {gt}{{Key: {dk}, Value: {dv}}}\n}}\n");
                    }
                }
            }
            Ty::Record(RecId::Indexed(x0)) => {
                let (at_i, at_x) = match tt {
                    Target::Go => ("x.(map[string]any)[\"index\"]", "x.(map[string]any)[\"item\"]"),
                    _ => ("x[\"index\"]", "x[\"item\"]"),
                };
                let di = self.decode(p, c, &Ty::Num(crate::ty::NumTy::number()), at_i);
                let dx = self.decode(p, c, x0, at_x);
                match tt {
                    Target::TypeScript => format!("({{ index: {di}, item: {dx} }})"),
                    Target::Python => format!("kaname.Indexed(index={di}, item={dx})"),
                    Target::Go => {
                        let gt = self.go_type(p, t);
                        return format!("func {n}(x any) {gt} {{\n\treturn {gt}{{Index: {di}, Item: {dx}}}\n}}\n");
                    }
                }
            }
            Ty::Num(nt) => match tt {
                Target::TypeScript => "x".into(),
                Target::Python => "int(x)".into(),
                Target::Go => {
                    let gt = if nt.is_rate() { "int64".to_string() } else { self.unit_go(&nt.unit) };
                    return format!("func {n}(x any) {gt} {{\n\treturn {gt}(num(x))\n}}\n");
                }
            },
            Ty::Date => match tt {
                Target::TypeScript => "x".into(),
                Target::Python => "_dt.date.fromisoformat(x)".into(),
                Target::Go => return format!("func {n}(x any) kaname.Date {{\n\treturn date(x)\n}}\n"),
            },
            Ty::Bool => match tt {
                Target::Go => return format!("func {n}(x any) bool {{\n\treturn x.(bool)\n}}\n"),
                _ => "x".into(),
            },
            // a string, or the marker of one that is not well formed: a lone surrogate, a byte that is
            // no UTF-8 (DESIGN 25.8)
            Ty::Str => match tt {
                Target::TypeScript => "typeof x === \"string\" ? x : String.fromCharCode(0xd800)".into(),
                Target::Python => "x if isinstance(x, str) else chr(0xD800)".into(),
                Target::Go => return format!("func {n}(x any) string {{\n\tif s, ok := x.(string); ok {{\n\t\treturn s\n\t}}\n\treturn string([]byte{{0xff}})\n}}\n"),
            },
            Ty::Enum(id @ crate::ty::EnumId { owner: Owner::Module(file), .. }) if p.is_sum(id) => return self.sum_decoder(p, c, id, file, n),
            Ty::Record(r @ RecId::Variant { .. }) => {
                let Some(id) = r.sum() else { return String::new() };
                let d = self.decode(p, c, &Ty::Enum(id), "x");
                match tt {
                    Target::Go => {
                        let gt = self.go_type(p, t);
                        return format!("func {n}(x any) {gt} {{\n\treturn {d}.({gt})\n}}\n");
                    }
                    _ => d,
                }
            }
            Ty::Enum(id) => match (&id.owner, tt) {
                (_, Target::TypeScript) => "x".into(),
                (Owner::Module(f), Target::Python) => {
                    let m = p.module(f);
                    let alias = m.decls.enums.iter().find(|e| e.name.text == id.name).map(|e| e.name.ascii().to_string()).unwrap_or_default();
                    format!("{}.{}(x)", self.mod_ref(f), type_name(tt, &alias))
                }
                (Owner::Rule(f), Target::Python) => {
                    let r = self.rule_ref(p, f);
                    let alias = rule_enum_alias(p.rules.get(f).map(|r| &r.python), &id.name);
                    format!("{r}.{alias}(x)")
                }
                (Owner::Module(f), Target::Go) => {
                    let m = p.module(f);
                    let alias = m.decls.enums.iter().find(|e| e.name.text == id.name).map(|e| e.name.ascii().to_string()).unwrap_or_default();
                    let ty = format!("{}.{}", self.mod_ref(f), type_name(tt, &alias));
                    return format!("func {n}(x any) {ty} {{\n\treturn {ty}(x.(string))\n}}\n");
                }
                (Owner::Rule(f), Target::Go) => {
                    let r = self.rule_ref(p, f);
                    let alias = rule_enum_alias(p.rules.get(f).map(|r| &r.go), &id.name);
                    return format!("func {n}(x any) {r}.{alias} {{\n\tv, ok := {r}.Parse{alias}(x.(string))\n\tif !ok {{\n\t\tpanic(x)\n\t}}\n\treturn v\n}}\n");
                }
            },
            // a result from its one pair, `ok` or `err` (DESIGN 26.4)
            Ty::Result(a, e) => {
                let (at_ok, at_err) = match tt {
                    Target::Go => ("v", "m[\"err\"]"),
                    _ => ("x[\"ok\"]", "x[\"err\"]"),
                };
                let (da, de) = (self.decode(p, c, a, at_ok), self.decode(p, c, e, at_err));
                match tt {
                    Target::TypeScript => format!("\"ok\" in x ? {{ kind: \"ok\", value: {da} }} : {{ kind: \"err\", error: {de} }}"),
                    Target::Python => format!("kaname.Ok(value={da}) if \"ok\" in x else kaname.Err(error={de})"),
                    Target::Go => {
                        let (ga, ge) = (self.go_type(p, a), self.go_type(p, e));
                        return format!(
                            "func {n}(x any) kaname.Result[{ga}, {ge}] {{\n\tm := x.(map[string]any)\n\tif v, ok := m[\"ok\"]; ok {{\n\t\treturn kaname.Ok[{ga}, {ge}]{{Value: {da}}}\n\t}}\n\treturn kaname.Err[{ga}, {ge}]{{Error: {de}}}\n}}\n"
                        );
                    }
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
                    let at = match tt {
                        Target::TypeScript => format!("x[{}]", ritsu_emit::lit::json(&f.text)),
                        Target::Python => format!("x[{}]", ritsu_emit::lit::json(&f.text)),
                        Target::Go => format!("x.(map[string]any)[{}]", ritsu_emit::lit::go(&f.text)),
                    };
                    // a field that may be none may be left out of the JSON
                    let at = match (tt, &slot.ty) {
                        (Target::TypeScript, Ty::Opt(_)) => format!("({at} ?? null)"),
                        (Target::Python, Ty::Opt(_)) => format!("x.get({})", ritsu_emit::lit::json(&f.text)),
                        _ => at,
                    };
                    let d = self.decode(p, c, &slot.ty, &at);
                    fields.push((field_name(tt, f.ascii()), d));
                }
                match tt {
                    Target::TypeScript => format!("({{ {} }})", fields.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join(", ")),
                    Target::Python => format!("{ty}({})", fields.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(", ")),
                    Target::Go => return format!("func {n}(x any) {ty} {{\n\treturn {ty}{{{}}}\n}}\n", fields.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join(", ")),
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
            Target::Go => format!("func {n}(x any) any {{\n\treturn {body}\n}}\n"),
        }
    }

    /// The decoder of a value of a sum (DESIGN 23.6): a value with no fields by its name, a value with
    /// fields as an object of one pair, its name and its fields.
    fn sum_decoder(&mut self, p: &Program, c: &Checked, id: &crate::ty::EnumId, file: &std::path::Path, n: &str) -> String {
        let tt = self.t;
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
                };
                plain.push((v.text.clone(), built));
                continue;
            }
            let mut fields = Vec::new();
            for (f, slot) in &fs {
                let at = match tt {
                    Target::Go => format!("f[{}]", ritsu_emit::lit::go(&f.text)),
                    _ => format!("f[{}]", ritsu_emit::lit::json(&f.text)),
                };
                let at = match (tt, &slot.ty) {
                    (Target::TypeScript, Ty::Opt(_)) => format!("({at} ?? null)"),
                    (Target::Python, Ty::Opt(_)) => format!("f.get({})", ritsu_emit::lit::json(&f.text)),
                    _ => at,
                };
                let d = self.decode(p, c, &slot.ty, &at);
                fields.push((field_name(tt, f.ascii()), d));
            }
            let built = match tt {
                Target::TypeScript => format!("{{ kind: {}, {} }}", ritsu_emit::lit::json(&v.text), fields.iter().map(|(k, d)| format!("{k}: {d}")).collect::<Vec<_>>().join(", ")),
                Target::Python => format!("{vt}({})", fields.iter().map(|(k, d)| format!("{k}={d}")).collect::<Vec<_>>().join(", ")),
                Target::Go => format!("{vt}{{{}}}", fields.iter().map(|(k, d)| format!("{k}: {d}")).collect::<Vec<_>>().join(", ")),
            };
            with.push((v.text.clone(), built));
        }
        match tt {
            Target::TypeScript => {
                let mut s = format!("function {n}(x: any): any {{\n  if (typeof x === \"string\") {{\n    switch (x) {{\n");
                for (v, b) in &plain {
                    s.push_str(&format!("      case {}:\n        return {b};\n", ritsu_emit::lit::json(v)));
                }
                s.push_str("    }\n    throw new Error(\"no such value\");\n  }\n  const k = Object.keys(x)[0];\n  const f = x[k];\n  switch (k) {\n");
                for (v, b) in &with {
                    s.push_str(&format!("    case {}:\n      return {b};\n", ritsu_emit::lit::json(v)));
                }
                s.push_str("  }\n  throw new Error(\"no such value\");\n}\n");
                s
            }
            Target::Python => {
                let mut s = format!("def {n}(x: Any) -> Any:\n    if isinstance(x, str):\n");
                for (v, b) in &plain {
                    s.push_str(&format!("        if x == {}:\n            return {b}\n", ritsu_emit::lit::json(v)));
                }
                s.push_str("        raise ValueError(x)\n    k = next(iter(x))\n    f = x[k]\n");
                for (v, b) in &with {
                    s.push_str(&format!("    if k == {}:\n        return {b}\n", ritsu_emit::lit::json(v)));
                }
                s.push_str("    raise ValueError(x)\n");
                s
            }
            Target::Go => {
                let gt = self.go_type(p, &Ty::Enum(id.clone()));
                let mut s = format!("func {n}(x any) {gt} {{\n\tif s, ok := x.(string); ok {{\n");
                if !plain.is_empty() {
                    s.push_str("\t\tswitch s {\n");
                    for (v, b) in &plain {
                        s.push_str(&format!("\t\tcase {}:\n\t\t\treturn {b}\n", ritsu_emit::lit::go(v)));
                    }
                    s.push_str("\t\t}\n");
                }
                s.push_str("\t\tpanic(s)\n\t}\n\tfor k, v := range x.(map[string]any) {\n\t\tf := v.(map[string]any)\n\t\tswitch k {\n");
                for (v, b) in &with {
                    s.push_str(&format!("\t\tcase {}:\n\t\t\treturn {b}\n", ritsu_emit::lit::go(v)));
                }
                s.push_str("\t\t}\n\t}\n\tpanic(x)\n}\n");
                s
            }
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
                }
            }
            Ty::List(inner) => {
                let e = self.encode(p, c, inner, "e");
                match tt {
                    Target::TypeScript => format!("x.map((e: any) => {e})"),
                    Target::Python => format!("[{e} for e in x]"),
                    Target::Go => return format!("func {n}(x {gt}) any {{\n\tout := []any{{}}\n\tfor _, e := range x {{\n\t\tout = append(out, {e})\n\t}}\n\treturn out\n}}\n"),
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
                }
            }
            Ty::Record(RecId::Outputs(f)) => {
                let facts = p.rules.get(f).cloned().expect("a rule the module reads");
                let r = self.rule_ref(p, f);
                let call = match tt {
                    Target::TypeScript => &facts.typescript,
                    Target::Python => &facts.python,
                    Target::Go => &facts.go,
                };
                let mut fields = Vec::new();
                for (k, col) in facts.outputs.iter().enumerate() {
                    let at = if facts.outputs.len() == 1 { "x".to_string() } else { format!("x.{}", call.outputs.get(k).map(|o| o.alias.clone()).unwrap_or_default()) };
                    fields.push((col.name.clone(), self.encode_col(&facts, &r, &col.ty, &at)));
                }
                let gt = if tt == Target::Go { if facts.outputs.len() == 1 { self.col_go(&r, &facts.outputs[0].ty, &facts) } else { format!("{r}.Output") } } else { gt.clone() };
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
                    Target::Go => {
                        let gt = self.col_go(r, t, facts);
                        format!("func(v {gt}) any {{ if v == nil {{ return nil }}; return {inner} }}({at})")
                    }
                }
            }
            ColumnType::Date => match tt {
                Target::TypeScript => format!("kaname.dateOf(Number({at}))"),
                Target::Python => format!("kaname.date_of({at}).isoformat()"),
                Target::Go => format!("kaname.DateOf(int64({at})).String()"),
            },
            ColumnType::Enum { .. } => match tt {
                Target::TypeScript => at.to_string(),
                Target::Python => format!("{at}.value"),
                Target::Go => format!("{at}.String()"),
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
        }
    }

    // ── The program ──────────────────────────────────────────────────────

    fn finish(&self, cases: &[(String, usize, String, String, bool)]) -> String {
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
                s.push_str(&defs);
                s.push_str("\nconst cases: any[] = parse(readFileSync(process.argv[2], \"utf8\"));\nfor (const c of cases) {\n  try {\n    let r: any;\n    let out: any;\n    switch (c.m + \".\" + String(c.f)) {\n");
                for (m, f, call, out, _) in cases {
                    s.push_str(&format!("      case {}:\n        r = {call};\n        out = {out};\n        break;\n", ritsu_emit::lit::json(&format!("{m}.{f}"))));
                }
                s.push_str("      default:\n        throw new Error(\"no such function\");\n    }\n    console.log(show({ out }));\n  } catch (e) {\n    if (e instanceof kaname.KanameError) console.log(show({ fails: { kind: e.kind, module: e.module, line: BigInt(e.line), message: e.message } }));\n    else console.log(show({ error: String(e) }));\n  }\n}\n");
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
                s.push('\n');
                s.push_str(&defs);
                s.push_str("\n\ndef main() -> None:\n    with open(sys.argv[1], encoding=\"utf-8\") as f:\n        cases = json.load(f)\n    for c in cases:\n        r: Any = None\n        out: Any = None\n        try:\n            key = c[\"m\"] + \".\" + str(c[\"f\"])\n");
                let mut first = true;
                for (m, f, call, out, _) in cases {
                    let kw = if first { "if" } else { "elif" };
                    first = false;
                    s.push_str(&format!("            {kw} key == {}:\n                r = {call}\n                out = {out}\n", ritsu_emit::lit::json(&format!("{m}.{f}"))));
                }
                if cases.is_empty() {
                    s.push_str("            raise ValueError(\"no such function\")\n");
                } else {
                    s.push_str("            else:\n                raise ValueError(\"no such function\")\n");
                }
                s.push_str("            print(json.dumps({\"out\": out}, ensure_ascii=False))\n        except kaname.KanameError as e:\n            print(json.dumps({\"fails\": {\"kind\": e.kind, \"module\": e.module, \"line\": e.line, \"message\": e.message}}, ensure_ascii=False))\n        except Exception as e:  # noqa: BLE001\n            print(json.dumps({\"error\": repr(e)}, ensure_ascii=False))\n\n\nmain()\n");
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
                    let _ = i;
                    let p = go_package(m.module.name.ascii());
                    imports.push(format!("\"{}/glue/{p}\"", self.go_module));
                }
                for line in self.imports.values() {
                    imports.push(line.clone());
                }
                // in the order gofmt keeps: by the path
                imports.sort_by(|a, b| a.split('"').nth(1).cmp(&b.split('"').nth(1)));
                imports.dedup();
                let mut s = format!("// The harness of kaname's tests: each case through the generated code, a JSON line each.\npackage main\n\nimport (\n\t\"bytes\"\n\t\"encoding/json\"\n\t\"errors\"\n\t\"fmt\"\n\t\"os\"\n\t\"slices\"\n\t\"strconv\"\n\t\"strings\"\n\n{}\n)\n\n", imports.iter().map(|i| format!("\t{i}")).collect::<Vec<_>>().join("\n"));
                s.push_str(GO_IO);
                s.push_str(&defs);
                s.push_str("\nfunc run(c kase) (out any, err error) {\n\tdefer func() {\n\t\tif r := recover(); r != nil {\n\t\t\tif k, ok := r.(*kaname.KanameError); ok {\n\t\t\t\terr = k\n\t\t\t\treturn\n\t\t\t}\n\t\t\terr = fmt.Errorf(\"%v\", r)\n\t\t}\n\t}()\n\tswitch c.M + \".\" + strconv.Itoa(c.F) {\n");
                for (m, f, call, out, fails) in cases {
                    s.push_str(&format!("\tcase {}:\n", ritsu_emit::lit::go(&format!("{m}.{f}"))));
                    if *fails {
                        s.push_str(&format!("\t\tr, err := {call}\n\t\tif err != nil {{\n\t\t\treturn nil, err\n\t\t}}\n\t\treturn {out}, nil\n"));
                    } else {
                        s.push_str(&format!("\t\tr := {call}\n\t\treturn {out}, nil\n"));
                    }
                }
                s.push_str("\t}\n\treturn nil, errors.New(\"no such function\")\n}\n\nfunc main() {\n\tcases := read(os.Args[1])\n\tfor _, c := range cases {\n\t\tout, err := run(c)\n\t\tvar k *kaname.KanameError\n\t\tswitch {\n\t\tcase errors.As(err, &k):\n\t\t\tsay(map[string]any{\"fails\": map[string]any{\"kind\": k.Kind, \"module\": k.Module, \"line\": k.Line, \"message\": k.Message}})\n\t\tcase err != nil:\n\t\t\tsay(map[string]any{\"error\": err.Error()})\n\t\tdefault:\n\t\t\tsay(map[string]any{\"out\": out})\n\t\t}\n\t}\n}\n");
                s
            }
        }
    }
}

/// A rule's enum, as one language's code names its type.
fn rule_enum_alias(call: Option<&ritsu_ports::Call>, name: &str) -> String {
    call.and_then(|c| c.enums.iter().find(|e| e.name == name)).map(|e| e.alias.clone()).unwrap_or_default()
}

impl H {
    /// The Go type of a rule's column.
    fn col_go(&mut self, r: &str, t: &ColumnType, facts: &RuleFacts) -> String {
        match t {
            ColumnType::Bool => "bool".into(),
            ColumnType::Str => "string".into(),
            ColumnType::Date => "int64".into(),
            ColumnType::Enum { name: n, .. } => format!("{r}.{}", rule_enum_alias(Some(&facts.go), n)),
            ColumnType::Num { unit: Some(u), .. } if u.dim == ritsu_units::Dim::Rate => format!("{r}.Rate"),
            ColumnType::Num { unit: Some(u), .. } => self.unit_go(u),
            ColumnType::Num { unit: None, .. } => "int64".into(),
            ColumnType::Opt(x) => format!("*{}", self.col_go(r, x, facts)),
        }
    }
}

/// What every TypeScript harness reads and writes JSON with: numbers as `bigint`, to the digit.
const TS_IO: &str = r#"
function parse(text: string): any {
  return JSON.parse(text, (_k: string, v: any, ctx?: { source?: string }) => (typeof v === "number" && ctx?.source !== undefined ? BigInt(ctx.source) : v));
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

/// What every Go harness reads and writes JSON with: numbers as json.Number, to the digit.
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

func num(x any) int64 {
	n, err := strconv.ParseInt(string(x.(json.Number)), 10, 64)
	if err != nil {
		panic(err)
	}
	return n
}

// date is the date written YYYY-MM-DD, as a host builds one: of any year, month and day, which the
// code is to hold to the days of the calendar.
func date(x any) kaname.Date {
	s := x.(string)
	i := strings.LastIndex(s, "-")
	j := strings.LastIndex(s[:i], "-")
	y, e1 := strconv.Atoi(s[:j])
	m, e2 := strconv.Atoi(s[j+1 : i])
	d, e3 := strconv.Atoi(s[i+1:])
	if e1 != nil || e2 != nil || e3 != nil {
		panic(s)
	}
	return kaname.Date{Year: y, Month: m, Day: d}
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
