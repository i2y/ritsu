//! `worked_out`: what the form works out for values given from outside the ticket machine, the
//! function a kind's entry `worked` calls (DESIGN 13.7, 4.9, 12.4). Given a form filled in so far
//! and the values an approver gives or corrects in it, it answers whether each field shown on a
//! condition is shown, whether each field asked for on a condition is asked for, and the value of
//! each field the form works out (of a computation of a row, for each row), none where it is not
//! decided: as ringi's reference decides it ([`crate::engine::worked`]).
//!
//! Each output of the copy (`values.rule`) is worked out from what it reads alone, as the screens
//! work it out: the copy is called with each argument the output does not read given a value of its
//! range, which cannot change it; the output has no value while a field it reads that the requester
//! has to fill in is empty, while a value it reads is outside its field's range or no value of it,
//! or while the rows it reads that are filled in are more than their field takes. A field the form
//! hides is read as empty, worked out from the fields its condition reads, none of them from itself.
//! A computation of a row is the copy over that row alone, and a date the form counts is counted by
//! `form.cal`, as the machine works them out.

use super::super::{not_yet, quoted};
use super::{Fld, FormOf, Kind, Vk, X, asked, column_type, indent, kept, kn_type, pascal, present, rate_of, rate_open, record};
use crate::ast::{ExprKind, Field, TicketLine, TypeExpr};
use crate::lower::words::{self, Names};
use crate::screens::{Empty, Param, Worked};
use ritsu_base::text::Text;
use ritsu_ports::{ColumnType, RuleFacts};

/// What a function of `worked_out` has declared so far: its `let` lines, the names they take, and
/// what each names, so that each value is worked out once.
struct Sc {
    lets: String,
    names: Names,
    /// The values of the form it reads, by field, as the screens take them ([`FormOf::wired`]).
    wired: Vec<(String, String)>,
    /// Whether a field is shown, by the field and the fields whose condition is being worked out.
    shown: Vec<((String, Vec<String>), String)>,
    /// What a field holds as the copy reads it, by the field and the same.
    held: Vec<((String, Vec<String>), String)>,
    /// The value of an output of the copy, by its name in the code and the same.
    evals: Vec<((String, Vec<String>), String)>,
    /// The rows given in place of the form's: the rows field and the list.
    row: Option<(String, String)>,
    /// The `let` that holds the value of a rate's range given an argument the output does not
    /// read, by the field ([`FormOf::stand`]).
    stands: Vec<(String, String)>,
    /// The name the values of the form are read from, and whether anything read them.
    given: String,
    read_given: bool,
}

impl Sc {
    fn new(given: &str, keep: &[String]) -> Sc {
        let mut names = Names::kaname();
        for n in keep {
            names.keep(n);
        }
        names.keep(given);
        Sc { lets: String::new(), names, wired: Vec::new(), shown: Vec::new(), held: Vec::new(), evals: Vec::new(), row: None, stands: Vec::new(), given: given.to_string(), read_given: false }
    }
}

/// The fields whose condition is being worked out, in order, with `more`.
fn with(seen: &[String], more: Option<&str>) -> Vec<String> {
    let mut all: Vec<String> = seen.to_vec();
    if let Some(m) = more {
        all.push(m.to_string());
    }
    all.sort();
    all.dedup();
    all
}

/// The name the copy takes its `i`th argument by: an input's, or the list's of a list it walks.
fn param_name(facts: &RuleFacts, i: usize) -> Option<String> {
    let p = facts.typescript.params.get(i)?;
    match facts.elements.iter().find(|(n, a, _)| *n == p.name || *a == p.name) {
        Some((n, ..)) => Some(n.clone()),
        None => Some(p.name.clone()),
    }
}

/// The list the copy walks that is its `i`th argument.
fn element(facts: &RuleFacts, i: usize) -> Option<&(String, String, Vec<ritsu_ports::Column>)> {
    let p = facts.typescript.params.get(i)?;
    facts.elements.iter().find(|(n, a, _)| *n == p.name || *a == p.name)
}

/// A field of a list's element by its name in the code, as the screens name it.
fn column_code(c: &ritsu_ports::Column) -> &str {
    if c.alias.is_empty() { &c.name } else { &c.alias }
}

/// The type of an output of the copy that may have no value, as a `let` holds it.
fn out_type(t: &ColumnType) -> String {
    match rate_of(t) {
        Some(u) => format!("{}?{}", kn_type(u), rate_open(Some(u))),
        None => format!("{}?", column_type(t)),
    }
}

/// A `let` of the value `body` (lines of kaname) under the name `n` of the type `ty`; of a type with
/// a range (a rate's), which a `let` does not write, without its type.
fn let_of(n: &str, ty: &str, body: &str) -> String {
    let ty = if ty.contains(" range ") { String::new() } else { format!(" : {ty}") };
    if body.contains('\n') { format!("let {n}{ty} =\n{}", indent(body, 2)) } else { format!("let {n}{ty} = {body}\n") }
}

impl FormOf<'_, '_, '_> {
    /// The records and the function the entry `worked` calls: `worked_out`, or, for a form that
    /// works nothing out, `worked_none`, which reads what it is given and answers it, the entry
    /// answering no field.
    pub(in crate::lower::glue) fn worked_out(&self) -> Result<String, Text> {
        let s = self.m.s;
        let mut out = record("WorkedIn", [("form".to_string(), "Given".to_string()), ("edited".to_string(), "Given?".to_string())].into_iter());
        let any = s.fields().iter().any(|f| f.shown_when.is_some() || f.required_when.is_some() || f.computed.is_some() || (f.ty == TypeExpr::Rows && f.children.iter().any(|c| c.computed.is_some())));
        if !any {
            out.push_str("fn worked_none(input: WorkedIn) -> WorkedIn total\n  description \"The form and the edit given, as the entry `worked` reads them: the form works nothing out\"\n  input\n\n");
            return Ok(out);
        }
        let w = self.values.as_ref().and_then(|_| crate::screens::Screen::of(self.m).worked);
        let mut keep: Vec<String> = self.use_names();
        keep.extend(["machine", "route", "input", "g", "y", "row", "b", "v", "r"].map(String::from));
        let mut sc = Sc::new("g", &keep);
        let mut fields: Vec<(String, String)> = Vec::new();
        let mut made: Vec<(String, String)> = Vec::new();
        let mut records = String::new();
        let mut row_fns = String::new();
        let mut dates: Option<Vec<(String, String)>> = None;
        for f in s.fields() {
            let n = f.name.ascii().to_string();
            let mut parts: Vec<(String, String)> = Vec::new();
            let mut values: Vec<String> = Vec::new();
            if f.shown_when.is_some() {
                let v = match (&w, self.code(w.as_ref(), &format!("{n}_shown"))) {
                    (Some(w), Some(code)) => self.eval(&mut sc, w, &code, &with(&[], Some(&n)))?,
                    _ => "none".to_string(),
                };
                parts.push(("shown".to_string(), "bool?".to_string()));
                values.push(format!("shown: {v}"));
            }
            if f.required_when.is_some() {
                let v = match (&w, self.code(w.as_ref(), &format!("{n}_required"))) {
                    _ if f.approvers_only.is_some() => "false".to_string(),
                    (Some(w), Some(code)) => self.eval(&mut sc, w, &code, &[])?,
                    _ => "none".to_string(),
                };
                parts.push(("required".to_string(), "bool?".to_string()));
                values.push(format!("required: {v}"));
            }
            if f.computed.is_some() {
                let (ty, v) = if f.ty == TypeExpr::Date {
                    if dates.is_none() {
                        dates = Some(self.dates(&mut sc, w.as_ref())?);
                    }
                    let d = dates.as_ref().and_then(|d| d.iter().find(|(x, _)| *x == n)).map(|(_, l)| l.clone()).unwrap_or_else(|| "none".to_string());
                    ("date?".to_string(), d)
                } else {
                    self.computed_value(&mut sc, w.as_ref(), f)?
                };
                parts.push(("value".to_string(), ty));
                values.push(format!("value: {v}"));
            }
            if f.ty == TypeExpr::Rows && f.children.iter().any(|c| c.computed.is_some()) {
                let (ty, v, text) = self.rows_worked(&mut sc, w.as_ref(), f, &mut records)?;
                row_fns.push_str(&text);
                parts.push(("rows".to_string(), ty));
                values.push(format!("rows: {v}"));
            }
            if parts.is_empty() {
                continue;
            }
            let rec = self.names.borrow_mut().take(&format!("{}Worked", pascal(&n)));
            records.push_str(&record(&rec, parts.into_iter()));
            fields.push((n.clone(), rec.clone()));
            made.push((n, format!("{rec} {{ {} }}", values.join(", "))));
        }
        out.push_str(&record("WorkedOut", [("fields".to_string(), "WorkedFields".to_string())].into_iter()));
        out.push_str(&record("WorkedFields", fields.into_iter()));
        out.push_str(&records);
        let mut body = self.merged();
        body.push_str(&sc.lets);
        let each: Vec<String> = made.iter().map(|(n, v)| format!("{n}: {v}")).collect();
        body.push_str(&format!("WorkedOut {{ fields: WorkedFields {{ {} }} }}\n", each.join(", ")));
        out.push_str(&row_fns);
        out.push_str(&format!(
            "fn worked_out(input: WorkedIn) -> WorkedOut total\n  description \"What the form works out for the values given, as its screens work it out: whether each field shown or asked for on a condition is, and the value of each field it works out; none where it is not decided\"\n{}\n",
            indent(&body, 2)
        ));
        Ok(out)
    }

    /// The fields an approver gives or corrects: the approvers' and those `approvers edit` names.
    fn editable(&self) -> Vec<&Fld> {
        let s = self.m.s;
        let named: Vec<String> = s.file.ticket.iter().flat_map(|t| &t.lines).find_map(|l| if let TicketLine::ApproversEdit { fields, .. } = l { Some(fields.iter().map(|w| w.text.clone()).collect()) } else { None }).unwrap_or_default();
        self.given().filter(|x| x.approvers || s.fields().iter().any(|f| f.name.ascii() == x.name && named.contains(&f.name.text))).collect()
    }

    /// `let g = …`: the values of the form, each value of the edit of a field an approver gives or
    /// corrects in place of the form's, as an edit gives it: one left out or none keeps the form's.
    fn merged(&self) -> String {
        let edits = self.editable();
        if edits.is_empty() {
            return "let g = input.form\n".to_string();
        }
        let mut lets = String::new();
        let mut each = Vec::new();
        for x in self.given() {
            if !edits.iter().any(|e| e.name == x.name) {
                each.push(format!("{n}: input.form.{n}", n = x.name));
                continue;
            }
            let l = format!("m_{}", x.name);
            lets.push_str(&let_of(&l, &format!("{}?{}", super::given_type(x), x.open()), &format!("match e.{n}\n  none => input.form.{n}\n  some v => v\n", n = x.name)));
            each.push(format!("{}: {l}", x.name));
        }
        let made = format!("{lets}Given {{ {} }}\n", each.join(", "));
        format!("let g =\n  match input.edited\n    none => input.form\n    some e =>\n{}", indent(&made, 6))
    }

    /// The name in the code of the copy's output the request names `name` (`trip_to_shown`).
    fn code(&self, w: Option<&Worked>, name: &str) -> Option<String> {
        w?.outputs.iter().find(|(n, _)| n == name).map(|(_, c)| c.clone())
    }

    fn ast(&self, name: &str) -> Result<&Field, Text> {
        self.m.s.fields().iter().find(|f| f.name.ascii() == name).ok_or_else(|| not_yet(format!("the field `{name}` the form works something out from")))
    }

    fn top(&self, name: &str) -> Result<&Fld, Text> {
        self.fields.iter().find(|x| x.name == name).ok_or_else(|| not_yet(format!("the field `{name}` the form works something out from")))
    }

    /// A function the glue reads a value through as the screens take it, made the first time.
    fn screen_fn(&self, key: &str, ty: &str, ret: &str, about: &str, body: &str) -> String {
        self.helper(key.to_string(), key, |n| format!("fn {n}(v: {ty}) -> {ret} total\n  description \"{about}\"\n{}\n", indent(body, 2)))
    }

    /// A value of the field `f` (`v`, as given) as the screens take it: a text without the white
    /// space at its ends, none where nothing is left; a choice, a person, a team or an attachment
    /// that is an empty string, none; a yes or no that is not given, no. The yes or no is a
    /// comparison, not a `match` that answers no for none and the value for a value: kaname's Rust
    /// writes that `match` as the body of the function, which clippy asks to be written as
    /// `unwrap_or_default` (`manual_unwrap_or_default`).
    fn screen_value(&self, f: &Field, v: &str) -> String {
        let f = match &f.ty {
            TypeExpr::Bool => self.screen_fn("worked_yes", "bool?", "bool", "A yes or no as the screens read it: no where it is not given", "v = true\n"),
            TypeExpr::Text | TypeExpr::LongText => self.screen_fn(
                "worked_text",
                "string?",
                "string?",
                "A text as the screens read it: without the white space at its ends, none where nothing is left",
                "match v\n  none => none\n  some b =>\n    if b.trim() = \"\"\n      none\n    else\n      b.trim()\n",
            ),
            TypeExpr::Num(_) | TypeExpr::Date | TypeExpr::Rows => return v.to_string(),
            _ => self.screen_fn("worked_chosen", "string?", "string?", "A choice, a person, a team or an attachment as the screens read it: none where it is empty", "match v\n  none => none\n  some b =>\n    if b = \"\"\n      none\n    else\n      b\n"),
        };
        format!("{f}(v: {v})")
    }

    /// The value of the field `name` as the screens take it from the values the form is read with:
    /// an expression, or the name of a `let` that holds it.
    fn wired(&self, sc: &mut Sc, name: &str) -> Result<String, Text> {
        if let Some((_, l)) = sc.wired.iter().find(|(n, _)| n == name) {
            return Ok(l.clone());
        }
        sc.read_given = true;
        let x = self.top(name)?;
        let f = self.ast(name)?;
        let src = format!("{}.{}", sc.given, x.name);
        let (ty, v) = match &x.kind {
            Kind::Rows { children, .. } => {
                let ty = format!("list[{}]", x.fns.given_rec);
                let mut cells = Vec::new();
                let mut same = true;
                for c in children {
                    let a = f.children.iter().find(|a| a.name.ascii() == c.name).ok_or_else(|| not_yet(format!("the field `{name}.{}`", c.name)))?;
                    let cell = format!("y.{}", c.name);
                    let v = self.screen_value(a, &cell);
                    same &= v == cell;
                    cells.push(format!("{}: {v}", c.name));
                }
                let rows = if same { "r".to_string() } else { format!("r.map(y => {} {{ {} }})", x.fns.given_rec, cells.join(", ")) };
                (ty, format!("match {src}\n  none => []\n  some r => {rows}\n"))
            }
            _ if !matches!(f.ty, TypeExpr::Num(_) | TypeExpr::Date) => (if x.vk == Vk::Bool { "bool".to_string() } else { "string?".to_string() }, self.screen_value(f, &src)),
            _ => {
                sc.wired.push((name.to_string(), src.clone()));
                return Ok(src);
            }
        };
        let l = sc.names.take(&format!("w_{name}"));
        sc.lets.push_str(&let_of(&l, &ty, &v));
        sc.wired.push((name.to_string(), l.clone()));
        Ok(l)
    }

    /// What the field `name` holds as the copy reads it while the conditions of the fields `seen`
    /// are worked out: its value, or none (no rows) while the form hides it.
    fn held(&self, sc: &mut Sc, w: &Worked, name: &str, seen: &[String]) -> Result<String, Text> {
        if let Some((rf, list)) = &sc.row
            && rf == name
        {
            return Ok(list.clone());
        }
        let f = self.ast(name)?;
        let wired = self.wired(sc, name)?;
        if f.shown_when.is_none() {
            return Ok(wired);
        }
        let key = (name.to_string(), seen.to_vec());
        if let Some((_, l)) = sc.held.iter().find(|(k, _)| *k == key) {
            return Ok(l.clone());
        }
        let x = self.top(name)?;
        let (ty, empty) = match &x.kind {
            Kind::Rows { .. } => (format!("list[{}]", x.fns.given_rec), "[]"),
            _ if x.vk == Vk::Bool => ("bool?".to_string(), "none"),
            _ => (format!("{}?{}", x.ty, x.open()), "none"),
        };
        let shown = self.shown_in(sc, w, name, seen)?;
        let l = sc.names.take(&format!("h_{name}"));
        let body = if shown == "false" { empty.to_string() } else { format!("if {shown}\n  {wired}\nelse\n  {empty}\n") };
        sc.lets.push_str(&let_of(&l, &ty, &body));
        sc.held.push((key, l.clone()));
        Ok(l)
    }

    /// Whether the field `name` is shown while the conditions of the fields `seen` are worked out:
    /// whether its condition holds, worked out with it among them (no while it cannot be worked out,
    /// or while it is worked out from itself).
    fn shown_in(&self, sc: &mut Sc, w: &Worked, name: &str, seen: &[String]) -> Result<String, Text> {
        if seen.iter().any(|n| n == name) {
            return Ok("false".to_string());
        }
        let more = with(seen, Some(name));
        let key = (name.to_string(), more.clone());
        if let Some((_, l)) = sc.shown.iter().find(|(k, _)| *k == key) {
            return Ok(l.clone());
        }
        let Some(code) = self.code(Some(w), &format!("{name}_shown")) else { return Ok("false".to_string()) };
        let o = self.eval(sc, w, &code, &more)?;
        let l = sc.names.take(&format!("s_{name}"));
        let b = sc.names.take("b");
        sc.lets.push_str(&let_of(&l, "bool", &format!("match {o}\n  none => false\n  some {b} => {b}\n")));
        sc.shown.push((key, l.clone()));
        Ok(l)
    }

    /// `v`, a value of the field `x` that may be none (of the rows `rows`, for a field of a row), as
    /// one the copy takes: none where it is outside the field's range or no value of it. None where
    /// every value is one.
    fn valid(&self, x: &Fld, rows: Option<&str>, v: &str) -> Option<String> {
        if let Some((vs, _)) = &x.choice {
            let whole = rows.map_or_else(|| x.name.clone(), |r| format!("{r}_{}", x.name));
            let cond: Vec<String> = vs.iter().map(|c| format!("b = {}", quoted(c))).collect();
            if cond.is_empty() {
                return None;
            }
            let shown = &x.shown;
            let n = self.helper(format!("one of {whole}"), &format!("one_of_{whole}"), |n| {
                format!("fn {n}(v: string?) -> string? total\n  description \"A value of `{shown}` that may be empty, one of its values\"\n  match v\n    none => none\n    some b =>\n      if {}\n        b\n      else\n        none\n\n", cond.join(" or "))
            });
            return Some(format!("{n}(v: {v})"));
        }
        match x.vk {
            Vk::Num | Vk::Date if x.range.is_some() => Some(self.in_range(x, rows, v)),
            _ => None,
        }
    }

    /// Whether the field `name` is filled in as the copy reads it (a condition's `cover is none`).
    fn filled_in(&self, sc: &mut Sc, w: &Worked, name: &str, seen: &[String]) -> Result<String, Text> {
        let x = self.top(name)?;
        if matches!(x.kind, Kind::Rows { .. }) || x.vk == Vk::Bool {
            let f = self.ast(name)?;
            return if f.shown_when.is_none() { Ok("true".to_string()) } else { self.shown_in(sc, w, name, seen) };
        }
        let h = self.held(sc, w, name, seen)?;
        Ok(present(x.unit.as_ref(), &h))
    }

    /// The value [`Self::stand_lit`] of the field `f`, as an argument: a rate's held in a `let`
    /// first, each once. kaname's Go keeps a variable it does not use where a rate is written in
    /// place in a call of a rule's code, which `go vet` refuses; a rate held in a `let` it reads.
    fn stand(&self, sc: &mut Sc, f: &Field) -> Result<String, Text> {
        let lit = self.stand_lit(f)?;
        if !matches!(&f.ty, TypeExpr::Num(u) if u.dim == ritsu_units::Dim::Rate) {
            return Ok(lit);
        }
        let name = f.name.ascii().to_string();
        if let Some((_, l)) = sc.stands.iter().find(|(n, _)| *n == name) {
            return Ok(l.clone());
        }
        let l = sc.names.take(&format!("s_{name}"));
        sc.lets.push_str(&format!("let {l} = {lit}\n"));
        sc.stands.push((name, l.clone()));
        Ok(l)
    }

    /// A value of the range of the field `f` the screens give an argument the output does not read
    /// (the one the lowered files take where it is empty), written in kaname.
    fn stand_lit(&self, f: &Field) -> Result<String, Text> {
        let s = self.m.s;
        Ok(match &f.ty {
            TypeExpr::Num(_) | TypeExpr::Date => words::written(crate::lower::stand_in(f), &f.ty),
            TypeExpr::Bool => "false".to_string(),
            TypeExpr::Choice => quoted(f.options.first().map(|o| o.name.ascii()).unwrap_or_default()),
            TypeExpr::ChoiceFrom(p) if p.len() == 2 => {
                let e = s.rule_facts(&p[0].text).and_then(|r| r.enums.iter().find(|e| e.name == p[1].text || e.public == p[1].text)).and_then(|e| e.values.first()).map(|v| v.public.clone());
                quoted(&e.ok_or_else(|| not_yet(format!("the choice `{}`, which no rule answers for", f.name.text)))?)
            }
            _ => quoted(""),
        })
    }

    /// The name of a `let` that holds the output `code` of the copy (of the type it is, or none),
    /// worked out from what it reads alone while the conditions of the fields `seen` are worked out.
    fn eval(&self, sc: &mut Sc, w: &Worked, code: &str, seen: &[String]) -> Result<String, Text> {
        let key = (code.to_string(), seen.to_vec());
        if let Some((_, l)) = sc.evals.iter().find(|(k, _)| *k == key) {
            return Ok(l.clone());
        }
        let (_, facts, _) = self.values.as_ref().ok_or_else(|| not_yet("what the form works out, with no copy of it".to_string()))?;
        let out = facts.typescript.outputs.iter().find(|o| o.alias == code).ok_or_else(|| not_yet(format!("the output `{code}` of the copy")))?;
        let ty = facts.outputs.iter().find(|o| o.name == out.name).map(|o| out_type(&o.ty)).ok_or_else(|| not_yet(format!("the output `{code}` of the copy")))?;
        let reads: Vec<(usize, Vec<String>)> = w.reads.iter().find(|(c, _)| c == code).map(|(_, r)| r.clone()).unwrap_or_default();
        let inputs = self.worked_inputs()?;
        // the conditions under which it is not decided, the values it waits for (narrowed by a
        // `match`), and the arguments of the copy
        let mut checks: Vec<String> = Vec::new();
        let mut waits: Vec<(String, String)> = Vec::new();
        let mut args: Vec<String> = Vec::new();
        for (i, p) in w.params.iter().enumerate() {
            let name = param_name(facts, i).ok_or_else(|| not_yet(format!("the argument {i} of the copy")))?;
            let read = reads.iter().find(|(j, _)| *j == i);
            match p {
                Param::Given(f) => {
                    let v = match read {
                        None => "false".to_string(),
                        Some(_) => self.filled_in(sc, w, f, seen)?,
                    };
                    args.push(format!("{name}: {v}"));
                }
                Param::Value(f, _, empty) => {
                    let (_, _, template, _) = inputs.iter().find(|(n, ..)| *n == name).ok_or_else(|| not_yet(format!("the input `{name}` of the copy")))?;
                    let x = self.top(f)?;
                    let Some(_) = read else {
                        let v = match empty {
                            Empty::None | Empty::StandIn => "none".to_string(),
                            Empty::Wait => self.stand(sc, self.ast(f)?)?,
                        };
                        args.push(format!("{name}: {}", template.replace(X, &v)));
                        continue;
                    };
                    let h = self.held(sc, w, f, seen)?;
                    let valid = self.valid(x, None, &h);
                    if let Some(v) = &valid {
                        checks.push(format!("{} and not {}", present(x.unit.as_ref(), &h), present(x.unit.as_ref(), v)));
                    }
                    let always = x.vk == Vk::Bool && self.ast(f)?.shown_when.is_none();
                    match empty {
                        Empty::Wait if always => args.push(format!("{name}: {}", template.replace(X, &h))),
                        Empty::Wait => {
                            let b = sc.names.take(&format!("a_{f}"));
                            waits.push((valid.unwrap_or_else(|| h.clone()), b.clone()));
                            args.push(format!("{name}: {}", template.replace(X, &b)));
                        }
                        _ => args.push(format!("{name}: {}", template.replace(X, &h))),
                    }
                }
                Param::Rows(f, taken) => {
                    let x = self.top(f)?;
                    let g = self.ast(f)?;
                    let Kind::Rows { children, .. } = &x.kind else { return Err(not_yet(format!("the rows `{f}` the form walks"))) };
                    let (el, _, columns) = element(facts, i).ok_or_else(|| not_yet(format!("the rows `{f}` the form walks")))?;
                    let cols: &[String] = read.map(|(_, c)| c.as_slice()).unwrap_or_default();
                    let child = |n: &str| -> Result<(&Fld, &Field), Text> {
                        let c = children.iter().find(|c| c.name == n).ok_or_else(|| not_yet(format!("the field `{f}.{n}` the form reads")))?;
                        let a = g.children.iter().find(|a| a.name.ascii() == n).ok_or_else(|| not_yet(format!("the field `{f}.{n}` the form reads")))?;
                        Ok((c, a))
                    };
                    // each field of the element: whether a field of the row is filled in, or a
                    // value of it within its range (a value of its range where it is empty, which
                    // the output does not read)
                    let mut cells = Vec::new();
                    let mut stand = Vec::new();
                    for c in columns {
                        let Some((code, cn, given)) = taken.iter().find(|(code, ..)| code == column_code(c)) else { return Err(not_yet(format!("the field `{}` of the rows `{f}`", c.name))) };
                        let (cx, ca) = child(cn)?;
                        if *given {
                            let v = if read.is_some() && cols.contains(code) { present(cx.unit.as_ref(), &format!("y.{cn}")) } else { "false".to_string() };
                            cells.push(format!("{}: {v}", c.name));
                            stand.push(format!("{}: false", c.name));
                            continue;
                        }
                        if !matches!(cx.vk, Vk::Num | Vk::Date | Vk::Bool) {
                            return Err(not_yet(format!("the field `{f}.{cn}` the form reads, which is no number, date or yes or no")));
                        }
                        cells.push(format!("{}: {}", c.name, self.filled(cx, ca, Some(f), &format!("y.{cn}"))));
                        stand.push(format!("{}: {}", c.name, self.filled(cx, ca, Some(f), "none")));
                    }
                    let one = format!("values.{el} {{ {} }}", cells.join(", "));
                    let Some(_) = read else {
                        // as many rows as the field asks for at least, each of values of the ranges
                        let count = crate::ports::field(g).lo.unwrap_or(0).max(0);
                        let row = format!("values.{el} {{ {} }}", stand.join(", "));
                        let rows: Vec<String> = (0..count).map(|_| row.clone()).collect();
                        args.push(format!("{el}: [{}]", rows.join(", ")));
                        continue;
                    };
                    let rows = self.held(sc, w, f, seen)?;
                    // a row is there while each field it reads that the requester has to fill in is
                    // filled in; a value read of a row there is within its field's range
                    let mut need = Vec::new();
                    let mut bad = Vec::new();
                    for (code, cn, given) in taken {
                        if *given || !cols.contains(code) {
                            continue;
                        }
                        let (cx, _) = child(cn)?;
                        let flagged = taken.iter().any(|(_, n, g)| *g && n == cn);
                        let cell = format!("y.{cn}");
                        if !flagged {
                            need.push(present(cx.unit.as_ref(), &cell));
                        }
                        if let Some(v) = self.valid(cx, Some(f), &cell) {
                            bad.push(if flagged { format!("({} and not {})", present(cx.unit.as_ref(), &cell), present(cx.unit.as_ref(), &v)) } else { format!("not {}", present(cx.unit.as_ref(), &v)) });
                        }
                    }
                    let there = if need.is_empty() {
                        rows
                    } else {
                        let t = sc.names.take(&format!("t_{f}"));
                        sc.lets.push_str(&format!("let {t} = {rows}.filter(y => {})\n", need.join(" and ")));
                        t
                    };
                    if !bad.is_empty() {
                        checks.push(format!("{there}.any(y => {})", bad.join(" or ")));
                    }
                    if let Some(hi) = crate::ports::field(g).hi {
                        checks.push(format!("{there}.count() > {hi}"));
                    }
                    args.push(format!("{el}: {there}.map(y => {one})"));
                }
            }
        }
        let rule = facts.outputs.iter().find(|o| o.name == out.name).map(|o| o.ty.clone());
        let got = format!("values({}).{}", args.join(", "), out.name);
        let got = match rule.as_ref().and_then(rate_of) {
            // the rule rounds a rate to its step, which kaname does not read across: rounded again,
            // it is the same value
            Some(u) => format!("down({got}, {})", words::number(words::step(u), u)),
            None => got,
        };
        let mut inner = format!("{got}\n");
        for (v, b) in waits.iter().rev() {
            inner = format!("match {v}\n  none => none\n  some {b} =>\n{}", indent(&inner, 4));
        }
        let mut body = String::new();
        for (k, c) in checks.iter().enumerate() {
            body.push_str(&format!("{} {c}\n  none\n", if k == 0 { "if" } else { "else if" }));
        }
        if checks.is_empty() {
            body = inner;
        } else {
            body.push_str("else\n");
            body.push_str(&indent(&inner, 2));
        }
        let l = sc.names.take(&format!("o_{}", out.name));
        sc.lets.push_str(&let_of(&l, &ty, &body));
        sc.evals.push((key, l.clone()));
        Ok(l)
    }

    /// The type and the value of a number the form works out at its top: none while a field that
    /// may be empty it reads is empty or hidden, or while it cannot be worked out.
    fn computed_value(&self, sc: &mut Sc, w: Option<&Worked>, f: &Field) -> Result<(String, String), Text> {
        let n = f.name.ascii();
        let (Some(w), Some(code)) = (w, self.code(w, n)) else {
            let x = self.top(n)?;
            return Ok((format!("{}?{}", x.ty, x.open()), "none".to_string()));
        };
        let (_, facts, _) = self.values.as_ref().expect("a copy");
        let ty = facts.typescript.outputs.iter().find(|o| o.alias == code).and_then(|o| facts.outputs.iter().find(|p| p.name == o.name)).map(|o| out_type(&o.ty)).unwrap_or_else(|| "number?".to_string());
        let o = self.eval(sc, w, &code, &[])?;
        let when = w.value_when.iter().find(|(c, _)| *c == code).map(|(_, fs)| fs.clone()).unwrap_or_default();
        if when.is_empty() {
            return Ok((ty, o));
        }
        let unit = self.top(n)?.unit.clone();
        let mut has = Vec::new();
        for g in &when {
            let x = self.top(g)?;
            let h = self.held(sc, w, g, &[])?;
            has.push(present(x.unit.as_ref(), &h));
        }
        Ok((ty, kept(unit.as_ref(), &o, &has.join(" and "))))
    }

    /// The rows of the rows field `f` the form works something out of: their type, their value (for
    /// each row the form holds, the value of each computation of it), and the functions that work a
    /// row out; the records of a row are written to `records`.
    fn rows_worked(&self, sc: &mut Sc, w: Option<&Worked>, f: &Field, records: &mut String) -> Result<(String, String, String), Text> {
        let n = f.name.ascii();
        let x = self.top(n)?;
        let row_rec = self.names.borrow_mut().take(&format!("{}WorkedRow", pascal(n)));
        let mut cells = Vec::new();
        let mut values = Vec::new();
        let mut fns = String::new();
        // whether the form shows the rows
        let shown = match (w, f.shown_when.is_some()) {
            (_, false) => None,
            (Some(w), true) => Some(self.shown_in(sc, w, n, &[])?),
            (None, true) => Some("false".to_string()),
        };
        for c in f.children.iter().filter(|c| c.computed.is_some()) {
            let cn = c.name.ascii();
            let rec = self.names.borrow_mut().take(&format!("{}{}Worked", pascal(n), pascal(cn)));
            let (ty, call, text) = self.row_fn(w, f, x, c, shown.is_some())?;
            fns.push_str(&text);
            records.push_str(&record(&rec, [("value".to_string(), ty)].into_iter()));
            cells.push((cn.to_string(), rec.clone()));
            let mut a = vec!["row: y".to_string()];
            if call.1 {
                a.push(format!("{0}: {0}", sc.given));
            }
            if let Some(s) = &shown {
                a.push(format!("shown: {s}"));
            }
            let v = match &call.0 {
                Some(fname) => format!("{fname}({})", a.join(", ")),
                None => "none".to_string(),
            };
            values.push(format!("{cn}: {rec} {{ value: {v} }}"));
        }
        records.push_str(&record(&row_rec, cells.into_iter()));
        let rows = self.wired(sc, n)?;
        Ok((format!("list[{row_rec}]"), format!("{rows}.map(y => {row_rec} {{ {} }})", values.join(", ")), fns))
    }

    /// The function that works out the computation `c` of a row of the rows `f` (`x`): the copy over
    /// that row alone, as the machine holds the row (DESIGN 4.4); none while a field of the row it
    /// reads is empty or outside its range, while the form hides the rows (`hidable`), and where the
    /// copy says nothing of it alone. Its type, its name (None where it has none) with whether it
    /// reads the form's other values, and its text.
    fn row_fn(&self, w: Option<&Worked>, f: &Field, x: &Fld, c: &Field, hidable: bool) -> Result<(String, (Option<String>, bool), String), Text> {
        let s = self.m.s;
        let n = f.name.ascii();
        let cn = c.name.ascii();
        let Kind::Rows { children, .. } = &x.kind else { return Err(not_yet(format!("the rows `{n}`"))) };
        let Some((lf, facts, held)) = &self.values else { return Ok(("number?".to_string(), (None, false), String::new())) };
        let code_of = |name: &str| facts.typescript.outputs.iter().find(|o| o.name == name).map(|o| o.alias.clone());
        let h = held.iter().find(|h| h.rows.as_deref() == Some(n) && h.field == cn);
        let code = match h.and_then(|h| h.hi.as_ref().or(h.lo.as_ref())).and_then(|(o, _)| code_of(o)) {
            Some(code) => Some(code),
            None => s.fields().iter().find(|g| matches!(&g.computed, Some(e) if matches!(&e.kind, ExprKind::Sum(p) if p.len() == 2 && p[0].text == f.name.text && p[1].text == c.name.text))).and_then(|t| self.code(w, t.name.ascii())),
        };
        let _ = lf;
        let (Some(w), Some(code)) = (w, code) else {
            let ty = children.iter().find(|y| y.name == cn).map(|y| format!("{}?{}", y.ty, y.open())).unwrap_or_else(|| "number?".to_string());
            return Ok((ty, (None, false), String::new()));
        };
        let ty = facts.typescript.outputs.iter().find(|o| o.alias == code).and_then(|o| facts.outputs.iter().find(|p| p.name == o.name)).map(|o| out_type(&o.ty)).unwrap_or_else(|| "number?".to_string());
        // each field of the row it reads, through the row's computations it reads, filled in and
        // within its range
        let Some(e) = &c.computed else { return Err(not_yet(format!("the computation `{n}.{cn}`"))) };
        let mut checks = Vec::new();
        if hidable {
            checks.push("not shown".to_string());
        }
        for read in crate::lower::form_reads(s, e, Some(f)) {
            let Some((_, cell)) = read.split_once('.') else { continue };
            let Some(cx) = children.iter().find(|y| y.name == cell) else { return Err(not_yet(format!("the field `{n}.{cell}` the computation `{n}.{cn}` reads"))) };
            let v = format!("row.{cell}");
            let narrowed = self.valid(cx, Some(n), &v).unwrap_or(v);
            checks.push(format!("not {}", present(cx.unit.as_ref(), &narrowed)));
        }
        let mut keep: Vec<String> = self.use_names();
        keep.extend(["machine", "route", "row", "g", "shown", "y", "b", "v", "r"].map(String::from));
        let mut inner = Sc::new("g", &keep);
        inner.row = Some((n.to_string(), "[row]".to_string()));
        let o = self.eval(&mut inner, w, &code, &[])?;
        let mut body = String::new();
        for (k, c) in checks.iter().enumerate() {
            body.push_str(&format!("{} {c}\n  none\n", if k == 0 { "if" } else { "else if" }));
        }
        let then = format!("{}{o}\n", inner.lets);
        if checks.is_empty() {
            body = then;
        } else {
            body.push_str("else\n");
            body.push_str(&indent(&then, 2));
        }
        let fname = self.names.borrow_mut().take(&format!("worked_{n}_{cn}"));
        let mut params = vec![format!("row: {}", x.fns.given_rec)];
        if inner.read_given {
            params.push("g: Given".to_string());
        }
        if hidable {
            params.push("shown: bool".to_string());
        }
        let text = format!(
            "fn {fname}({}) -> {ty} total\n  description \"The value of `{}` of a row of `{}`, as the copy works it out over that row alone\"\n{}\n",
            params.join(", "),
            c.name.text,
            f.name.text,
            indent(&body, 2)
        );
        Ok((ty, (Some(fname), inner.read_given), text))
    }

    /// The dates the form counts (`form.cal`), each by its field's name with the name of the `let`
    /// that holds it: counted from the values it reads, in the order the file writes them; none
    /// where a value it is counted from is empty, hidden, outside its field's range, or outside the
    /// range `form.cal` reads it in.
    fn dates(&self, sc: &mut Sc, w: Option<&Worked>) -> Result<Vec<(String, String)>, Text> {
        let Some((df, facts)) = &self.dates else { return Ok(Vec::new()) };
        let mut counted: Vec<(String, String)> = Vec::new();
        for fun in &facts.functions {
            let want = asked(df, &fun.name);
            let mut each: Vec<(String, String, String, String, String)> = Vec::new();
            for p in &fun.params {
                let i = facts.inputs.iter().find(|i| i.name == *p).ok_or_else(|| not_yet(format!("the date `{want}`, counted from `{p}`")))?;
                let from = asked(df, &i.name);
                let g = self.ast(&from)?;
                let x = self.top(&from)?;
                let v = match (&g.computed, &g.ty) {
                    (Some(_), TypeExpr::Date) => counted.iter().find(|(n, _)| *n == from).map(|(_, l)| l.clone()).unwrap_or_else(|| "none".to_string()),
                    (Some(_), _) => self.computed_value(sc, w, g)?.1,
                    (None, _) => match w {
                        Some(w) => {
                            let h = self.held(sc, w, &from, &[])?;
                            self.valid(x, None, &h).unwrap_or(h)
                        }
                        None => {
                            let h = self.wired(sc, &from)?;
                            self.valid(x, None, &h).unwrap_or(h)
                        }
                    },
                };
                let lit = |d: i64| if x.vk == Vk::Date { crate::lex::date_text(d) } else { words::grouped(i128::from(d)) };
                let b = sc.names.take(&format!("a_{from}"));
                each.push((p.clone(), v, lit(i.min), lit(i.max), b));
            }
            let args: Vec<String> = each.iter().map(|(p, _, _, _, b)| format!("{p}: {b}")).collect();
            let mut inner = format!("form_dates.{}({})\n", fun.name, args.join(", "));
            for (_, v, lo, hi, b) in each.iter().rev() {
                inner = format!("match {v}\n  none => none\n  some {b} =>\n    if {b} < {lo} or {b} > {hi}\n      none\n    else\n{}", indent(&inner, 6));
            }
            let l = sc.names.take(&format!("d_{want}"));
            sc.lets.push_str(&let_of(&l, "date?", &inner));
            counted.push((want, l));
        }
        Ok(counted)
    }
}
