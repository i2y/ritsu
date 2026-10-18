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
use super::plan::{field_name, fn_name, go_package, type_name};
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
pub struct Case {
    pub module: String,
    pub function: String,
    pub input: Json,
    pub expected: Option<Json>,
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
                out.push(Case { module: c.module.name.ascii().to_string(), function: f.to_string(), input: line.get("in").cloned().unwrap_or(Json::Null), expected });
            }
        }
    }
    (out, diverged)
}

/// Inputs outside what a function's parameters declare, each at one part of one parameter (a step
/// past an end of a number's range, a day past a date's, one element past a list's count, one short
/// of it), the other parts at their base values, run in the reference interpreter: its answer is the
/// entry's failure `input` (DESIGN 4.4), which the vectors never reach (every vector is inside what
/// the parameters declare) and the code's public function is held to as well.
pub fn outside_cases(modules: &[Rc<Checked>], suite: &Suite, lang: Lang) -> Vec<Case> {
    use crate::vectors::{Path, Step, Vector, base_input, set_at, value_at};
    let mut out = Vec::new();
    for c in modules {
        let p = Program::of(c);
        for (f, info) in c.decls.fns.iter().enumerate() {
            let base = base_input(&p, c, f);
            // every part of every parameter, with what it declares
            let mut parts: Vec<(Path, crate::ty::Slot)> = Vec::new();
            for (i, prm) in info.params.iter().enumerate() {
                walk(&p, (i, Vec::new()), prm.slot.clone(), &mut parts, 0);
            }
            let mut inputs: Vec<Vec<Value>> = Vec::new();
            for (at, slot) in &parts {
                let mut ty = slot.ty.clone();
                while let Ty::Opt(x) = ty {
                    ty = *x;
                }
                let mut put = |v: Value| {
                    if let Some(i) = set_at(&base, at, v)
                        && !inputs.contains(&i)
                    {
                        inputs.push(i);
                    }
                };
                match &ty {
                    Ty::Num(n) => {
                        let step = if n.is_rate() { n.step.unwrap_or(Rat::new(1, 100)) } else { Rat::int(1) };
                        let range = match (&slot.range, n.is_rate() && n.step.is_some()) {
                            (Some(r), _) => Some(r.clone()),
                            (None, true) => Some(crate::ty::Range { min: Some(Rat::zero()), max: Some(Rat::int(1)) }),
                            _ => None,
                        };
                        let Some(r) = range else { continue };
                        for v in [r.max.map(|hi| hi.add(step)), r.min.map(|lo| lo.sub(step))].into_iter().flatten() {
                            if crate::num::fits(v) {
                                put(Value::Num(v));
                            }
                        }
                    }
                    Ty::Date => {
                        let Some(r) = &slot.range else { continue };
                        for d in [r.max.map(|hi| hi.num as i64 + 1), r.min.map(|lo| lo.num as i64 - 1)].into_iter().flatten() {
                            if (crate::date::MIN..=crate::date::MAX).contains(&d) {
                                put(Value::Date(d));
                            }
                        }
                    }
                    _ => {}
                }
                // a list one element past its count, or one short of it: the base list, its first
                // element repeated
                if let (Ty::List(_), Some(count), Some(Value::List(xs))) = (&ty, &slot.count, value_at(&base, at)) {
                    let first = xs.first().cloned();
                    if let (Some(hi), Some(x)) = (count.max, first) {
                        put(Value::List(vec![x; hi.num.max(0) as usize + 1]));
                    }
                    if let Some(lo) = count.min.filter(|lo| lo.num > 0) {
                        put(Value::List(xs.iter().take(lo.num as usize - 1).cloned().collect()));
                    }
                }
            }
            for input in inputs {
                let line = crate::vectors::line(&p, suite, c, f, &Vector { input, why: None }, false, lang);
                let expected = line.get("fails").map(|x| Json::obj([("fails", x.clone())])).or_else(|| line.get("out").map(|x| Json::obj([("out", x.clone())])));
                out.push(Case { module: c.module.name.ascii().to_string(), function: f.to_string(), input: line.get("in").cloned().unwrap_or(Json::Null), expected });
            }
        }
    }
    return out;

    /// Every part of a parameter at `at` that declares a range or a count: the part, then its
    /// fields and its first element.
    fn walk(p: &Program, at: Path, slot: crate::ty::Slot, parts: &mut Vec<(Path, crate::ty::Slot)>, depth: usize) {
        if depth > 8 {
            return;
        }
        let mut ty = slot.ty.clone();
        while let Ty::Opt(x) = ty {
            ty = *x;
        }
        if slot.range.is_some() || slot.count.is_some() || matches!(&ty, Ty::Num(n) if n.is_rate() && n.step.is_some()) {
            parts.push((at.clone(), slot.clone()));
        }
        match ty {
            Ty::List(x) => {
                let mut next = at.1.clone();
                next.push(Step::Elem);
                walk(p, (at.0, next), crate::ty::Slot { ty: *x, range: slot.range.clone(), count: None }, parts, depth + 1);
            }
            Ty::Record(id) => {
                for (name, s) in p.fields(&id) {
                    let mut next = at.1.clone();
                    next.push(Step::Field(name.text.clone()));
                    walk(p, (at.0, next), s, parts, depth + 1);
                }
            }
            _ => {}
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

/// The harness's lines held to the cases: the number that agree, and each that does not (the first
/// twenty).
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
        match ritsu_base::json::parse(line) {
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
    let mut h = H { t, codecs: BTreeMap::new(), defs: Vec::new(), modules: modules.to_vec(), name: name.to_string(), go_module: go_module.to_string(), imports: BTreeMap::new(), units: false };
    let mut cases = Vec::new();
    for (mi, c) in modules.iter().enumerate() {
        let p = Program::of(c);
        for (fi, info) in c.decls.fns.iter().enumerate() {
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
                Target::Go => return format!("func {n}(x any) kaname.Date {{\n\td, err := kaname.ParseDate(x.(string))\n\tif err != nil {{\n\t\tpanic(err)\n\t}}\n\treturn d\n}}\n"),
            },
            Ty::Bool | Ty::Str => match tt {
                Target::Go => {
                    let gt = if matches!(t, Ty::Bool) { "bool" } else { "string" };
                    return format!("func {n}(x any) {gt} {{\n\treturn x.({gt})\n}}\n");
                }
                _ => "x".into(),
            },
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
            Ty::Record(RecId::Module { file, name }) => {
                let m = p.module(file).clone();
                let Some(r) = m.decls.records.iter().find(|r| r.name.text == *name).cloned() else { return String::new() };
                let ty = format!("{}.{}", self.mod_ref(file), type_name(tt, r.name.ascii()));
                let mut fields = Vec::new();
                for f in &r.fields {
                    let at = match tt {
                        Target::TypeScript => format!("x[{}]", ritsu_emit::lit::json(&f.name.text)),
                        Target::Python => format!("x[{}]", ritsu_emit::lit::json(&f.name.text)),
                        Target::Go => format!("x.(map[string]any)[{}]", ritsu_emit::lit::go(&f.name.text)),
                    };
                    // a field that may be none may be left out of the JSON
                    let at = match (tt, &f.slot.ty) {
                        (Target::TypeScript, Ty::Opt(_)) => format!("({at} ?? null)"),
                        (Target::Python, Ty::Opt(_)) => format!("x.get({})", ritsu_emit::lit::json(&f.name.text)),
                        _ => at,
                    };
                    let d = self.decode(p, c, &f.slot.ty, &at);
                    fields.push((field_name(tt, f.name.ascii()), d));
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
            Ty::Enum(id) => match &id.owner {
                Owner::Module(f) => {
                    let m = p.module(f);
                    let alias = m.decls.enums.iter().find(|e| e.name.text == id.name).map(|e| e.name.ascii().to_string()).unwrap_or_default();
                    format!("{}.{}", self.mod_ref(f), type_name(Target::Go, &alias))
                }
                Owner::Rule(f) => {
                    let r = self.rule_ref(p, f);
                    format!("{r}.{}", rule_enum_alias(p.rules.get(f).map(|r| &r.go), &id.name))
                }
            },
            Ty::Record(RecId::Module { file, name }) => {
                let m = p.module(file);
                let alias = m.decls.records.iter().find(|r| r.name.text == *name).map(|r| r.name.ascii().to_string()).unwrap_or_default();
                format!("{}.{}", self.mod_ref(file), type_name(Target::Go, &alias))
            }
            Ty::Transfer(id) => {
                let b = self.book_ref(p, &id.book);
                format!("{b}.{}", super::books::op_type(Target::Go, &id.transfer, &id.op))
            }
            _ => "any".into(),
        }
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
            Ty::Num(_) => match tt {
                Target::Go => return format!("func {n}(x {gt}) any {{\n\treturn int64(x)\n}}\n"),
                _ => "x".into(),
            },
            Ty::Date => match tt {
                Target::TypeScript => "x".into(),
                Target::Python => "x.isoformat()".into(),
                Target::Go => return format!("func {n}(x kaname.Date) any {{\n\treturn x.String()\n}}\n"),
            },
            Ty::Enum(id) => match (tt, &id.owner) {
                (Target::TypeScript, _) => "x".into(),
                (Target::Python, _) => "x.value".into(),
                (Target::Go, Owner::Module(_)) => return format!("func {n}(x {gt}) any {{\n\treturn string(x)\n}}\n"),
                (Target::Go, Owner::Rule(_)) => return format!("func {n}(x {gt}) any {{\n\treturn x.String()\n}}\n"),
            },
            Ty::Record(RecId::Module { file, name }) => {
                let m = p.module(file).clone();
                let Some(r) = m.decls.records.iter().find(|r| r.name.text == *name).cloned() else { return String::new() };
                let mut fields = Vec::new();
                for f in &r.fields {
                    let at = format!("x.{}", field_name(tt, f.name.ascii()));
                    fields.push((f.name.text.clone(), self.encode(p, c, &f.slot.ty, &at)));
                }
                return self.object(&fields, n, &gt);
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
                for m in self.modules.iter().filter(|m| !m.decls.fns.is_empty()) {
                    let p = go_package(m.module.name.ascii());
                    imports.push(format!("\"{}/glue/{p}\"", self.go_module));
                }
                for line in self.imports.values() {
                    imports.push(line.clone());
                }
                // in the order gofmt keeps: by the path
                imports.sort_by(|a, b| a.split('"').nth(1).cmp(&b.split('"').nth(1)));
                imports.dedup();
                let mut s = format!("// The harness of kaname's tests: each case through the generated code, a JSON line each.\npackage main\n\nimport (\n\t\"bytes\"\n\t\"encoding/json\"\n\t\"errors\"\n\t\"fmt\"\n\t\"os\"\n\t\"strconv\"\n\n{}\n)\n\n", imports.iter().map(|i| format!("\t{i}")).collect::<Vec<_>>().join("\n"));
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

func say(v any) {
	b, err := json.Marshal(v)
	if err != nil {
		panic(err)
	}
	fmt.Println(string(b))
}

"#;
