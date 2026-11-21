//! What a form works out, for values given from outside the ticket machine (DESIGN 13.7, 4.9,
//! 12.4): the answer of a kind's `worked` entry. Given a form in the contract's shape, filled in so
//! far (`form`), and the values an approver gives or corrects in it (`edited`, the shape of an
//! edit's `fields`, where it is given), it answers whether each field shown on a condition is shown
//! (`shown`), whether each field asked for on a condition is asked for (`required`), and the value
//! of each field the form works out (`value`; of a computation of a row, for each row, `rows`).
//! `true`, `false` or a value where it is decided, null where it is not.
//!
//! What the screens work out is decided as the screens decide it (`a2ui/functions.js`): each output
//! of the copy of the form (`values.rule`) is worked out from what it reads alone, each argument it
//! does not read given a value of its range, which cannot change it. It is not decided while a
//! field it reads that the requester has to fill in is empty, while a value it reads is outside its
//! field's range or no value of its field, or while the rows it reads that are filled in are more
//! than their field takes (the code generated from the glue gives the copy no more rows than the
//! field takes, and the check of the rows' count stops the form). A field the form hides (its
//! condition does not hold, or is not decided) is read as empty, as the machine reads it; a field
//! that may be empty is read as none (DESIGN 4.9); a yes or no is read as no where it is not given,
//! as a screen's box that is not ticked; a total reads the rows filled in so far, fewer than the
//! field asks for too (a row whose field it reads that the requester has to fill in is empty is not
//! there yet). A computation has no value while a field that may be empty it reads is empty or
//! hidden.
//!
//! What the screens do not work out is worked out as the machine works it out: a computation of a
//! row by the copy over that row alone (no value while a field of the row it reads is empty, or
//! outside its range), a date the form counts by `form.cal` from the values it is counted from (no
//! value while one of them is empty, hidden or outside the range `form.cal` reads it in).
//!
//! The value an approver's edit gives replaces the form's, of a field the approver gives or
//! corrects (an approvers' field, or one `approvers edit` names), as an edit replaces it: a field of
//! `edited` that is null or left out keeps the form's value, and any other field of it is not read.

use super::Machine;
use crate::ast::{ExprKind, Field, TicketLine, TypeExpr};
use crate::lower::{LFile, Part};
use crate::screens::{Empty, Param, Worked};
use ritsu_base::json::Json;
use ritsu_base::text::Text;
use ritsu_ports::{DateValue, RuleFacts, Value, Values};
use std::cell::RefCell;
use std::collections::HashMap;

/// A value of a field as the screens read it ([`Form::wire`]): none for one that is empty, or hidden.
type Held = Option<Json>;

/// What a kind's `worked` answers for the input `input`, `{"form": …, "edited": … | null}`. Err for
/// an input whose `form` is not an object, or a value of another type than its field's (a number
/// that is no whole number of int64, a date that is no day, a text that is no string), which the
/// generated code's entry refuses too.
pub fn answer(m: &Machine, input: &Json) -> Result<Json, Text> {
    let s = m.s;
    let Some(form) = input.get("form").filter(|f| !f.is_null()) else { return Err(tr!("入力に `form` がありません", "the input has no `form`")) };
    let mut values = read(s, form, "form")?;
    if let Some(e) = input.get("edited").filter(|e| !e.is_null()) {
        let edits = read(s, e, "edited")?;
        let editable = editable(s);
        for (k, v) in edits {
            if !editable.iter().any(|f| f.name.ascii() == k) {
                continue;
            }
            match values.iter_mut().find(|(n, _)| *n == k) {
                Some(at) => at.1 = v,
                None => values.push((k, v)),
            }
        }
    }
    let f = Form::new(m, values);
    Ok(Json::obj([("fields", f.fields())]))
}

/// The fields of the form a person gives at its top (none it works out), as the contract names
/// them.
fn given_fields<'s>(s: &'s crate::names::Scope) -> impl Iterator<Item = &'s Field> {
    s.fields().iter().filter(|f| f.computed.is_none())
}

/// The fields an approver gives or corrects: the approvers' and those `approvers edit` names.
fn editable<'s>(s: &'s crate::names::Scope) -> Vec<&'s Field> {
    let named: Vec<String> = s.file.ticket.iter().flat_map(|t| &t.lines).find_map(|l| if let TicketLine::ApproversEdit { fields, .. } = l { Some(fields.iter().map(|w| w.text.clone()).collect()) } else { None }).unwrap_or_default();
    given_fields(s).filter(|f| f.approvers_only.is_some() || named.contains(&f.name.text)).collect()
}

/// The values of the object `j` (named `what` in the input) by the ASCII names of the fields a person
/// gives, each held to the type the contract gives it; a key of no such field is not read, and one
/// that is null is not given.
fn read(s: &crate::names::Scope, j: &Json, what: &str) -> Result<Vec<(String, Json)>, Text> {
    let Some(o) = j.as_obj() else { return Err(tr!("`{what}` がオブジェクトではありません", "`{what}` is not an object")) };
    let mut out = Vec::new();
    for f in given_fields(s) {
        let Some(v) = o.iter().find(|(k, _)| k == f.name.ascii()).map(|(_, v)| v).filter(|v| !v.is_null()) else { continue };
        let at = format!("{what}.{}", f.name.ascii());
        if f.ty != TypeExpr::Rows {
            typed(f, v, &at)?;
            out.push((f.name.ascii().to_string(), v.clone()));
            continue;
        }
        let Some(rows) = v.as_arr() else { return Err(tr!("`{at}` が行の配列ではありません", "`{at}` is not a list of rows")) };
        let mut kept = Vec::new();
        for (k, r) in rows.iter().enumerate() {
            let Some(cells) = r.as_obj() else { return Err(tr!("`{at}[{k}]` がオブジェクトではありません", "`{at}[{k}]` is not an object")) };
            let mut row = Vec::new();
            for c in f.children.iter().filter(|c| c.computed.is_none()) {
                let Some(x) = cells.iter().find(|(n, _)| n == c.name.ascii()).map(|(_, x)| x).filter(|x| !x.is_null()) else { continue };
                typed(c, x, &format!("{at}[{k}].{}", c.name.ascii()))?;
                row.push((c.name.ascii().to_string(), x.clone()));
            }
            kept.push(Json::Obj(row));
        }
        out.push((f.name.ascii().to_string(), Json::Arr(kept)));
    }
    Ok(out)
}

/// Whether `v` is a value of the type the contract gives the field `f`: a whole number of int64 (the
/// steps of its unit), a day `YYYY-MM-DD`, yes or no, or a string.
fn typed(f: &Field, v: &Json, at: &str) -> Result<(), Text> {
    let fits = match &f.ty {
        TypeExpr::Num(_) => v.as_int().is_some_and(|n| i64::try_from(n).is_ok()),
        TypeExpr::Date => v.as_str().and_then(ritsu_ports::day_of).is_some(),
        TypeExpr::Bool => v.as_bool().is_some(),
        TypeExpr::Rows => false,
        _ => v.as_str().is_some(),
    };
    if fits { Ok(()) } else { Err(tr!("`{at}` の値が、フィールドの型に合いません", "the value of `{at}` does not fit the field's type")) }
}

/// The form as it is read: its values after the edit, and what the copy of its computations is.
struct Form<'x, 's, 'a> {
    m: &'x Machine<'s, 'a>,
    values: Vec<(String, Json)>,
    worked: Option<Worked>,
    /// `values.rule`, with what rulec knows of it.
    copy: Option<(&'x LFile, RuleFacts)>,
    /// Whether a field is shown, by the field and the fields whose condition is being worked out.
    shown: RefCell<HashMap<(String, Vec<String>), bool>>,
}

impl<'x, 's, 'a> Form<'x, 's, 'a> {
    fn new(m: &'x Machine<'s, 'a>, values: Vec<(String, Json)>) -> Self {
        let copy = m.lowered.files.iter().find(|f| f.part == Part::Values).and_then(|lf| Some((lf, m.fs(|| m.suite.rules.as_ref()?.facts(&m.lowered.path(lf)).ok())?)));
        let worked = copy.as_ref().and_then(|_| crate::screens::Screen::of(m).worked);
        Form { m, values, worked, copy, shown: RefCell::new(HashMap::new()) }
    }

    fn field(&self, ascii: &str) -> Option<&'s Field> {
        self.m.s.fields().iter().find(|f| f.name.ascii() == ascii)
    }

    /// The answer: each field with a condition or a value worked out, in the form's order.
    fn fields(&self) -> Json {
        let s = self.m.s;
        let dates = self.dates();
        let mut out = Vec::new();
        for f in s.fields() {
            let n = f.name.ascii();
            let mut one = Vec::new();
            if f.shown_when.is_some() {
                one.push(("shown".to_string(), self.decided_shown(f)));
            }
            if f.required_when.is_some() {
                one.push(("required".to_string(), self.decided_required(f)));
            }
            if f.computed.is_some() {
                let v = match &f.ty {
                    TypeExpr::Date => dates.iter().find(|(d, _)| d == n).map(|(_, v)| v.clone()).unwrap_or(Json::Null),
                    _ => self.computed(f),
                };
                one.push(("value".to_string(), v));
            }
            if f.ty == TypeExpr::Rows && f.children.iter().any(|c| c.computed.is_some()) {
                one.push(("rows".to_string(), self.rows_worked(f)));
            }
            if !one.is_empty() {
                out.push((n.to_string(), Json::Obj(one)));
            }
        }
        Json::Obj(out)
    }

    /// The value of a field as the screens take it from the form, given or not: a text without the
    /// white space at its ends, a choice, a person, a team and an attachment that are empty strings,
    /// none; a yes or no that is not given, no; rows that are not given, none of them, and of each
    /// row its cells as such values.
    fn wire(f: &Field, v: Option<&Json>) -> Held {
        match &f.ty {
            TypeExpr::Bool => return Some(Json::Bool(v.and_then(Json::as_bool) == Some(true))),
            TypeExpr::Rows => {
                let rows = v.and_then(Json::as_arr).unwrap_or_default();
                let one = |r: &Json| {
                    let cells = f.children.iter().filter(|c| c.computed.is_none()).filter_map(|c| Some((c.name.ascii().to_string(), Self::wire(c, r.get(c.name.ascii()))?)));
                    Json::Obj(cells.collect())
                };
                return Some(Json::Arr(rows.iter().map(one).collect()));
            }
            _ => {}
        }
        let v = v.filter(|v| !v.is_null())?;
        match (&f.ty, v.as_str()) {
            (TypeExpr::Text | TypeExpr::LongText, Some(t)) => Some(t.trim()).filter(|t| !t.is_empty()).map(Json::str),
            (_, Some("")) => None,
            _ => Some(v.clone()),
        }
    }

    /// What the field `f` holds as the rules read it: none while the form hides it (`seen` holds
    /// the fields whose condition is being worked out, so that none is worked out from itself).
    fn held(&self, f: &Field, seen: &[String]) -> Held {
        if f.shown_when.is_some() && !self.shown_in(f, seen) {
            return None;
        }
        Self::wire(f, self.values.iter().find(|(n, _)| n == f.name.ascii()).map(|(_, v)| v))
    }

    fn shown_in(&self, f: &Field, seen: &[String]) -> bool {
        let n = f.name.ascii().to_string();
        if seen.contains(&n) {
            return false;
        }
        let mut more = seen.to_vec();
        more.push(n.clone());
        more.sort();
        if let Some(b) = self.shown.borrow().get(&(n.clone(), more.clone())) {
            return *b;
        }
        let b = self.code(&format!("{n}_shown")).is_some_and(|code| self.output(&code, &more, None) == Some(Value::Bool(true)));
        self.shown.borrow_mut().insert((n, more), b);
        b
    }

    /// The name in the code of the copy's output the request names `name` (`trip_to_shown`).
    fn code(&self, name: &str) -> Option<String> {
        self.worked.as_ref()?.outputs.iter().find(|(n, _)| n == name).map(|(_, c)| c.clone())
    }

    /// `shown` of a field shown on a condition: whether its condition holds, null while it cannot be
    /// worked out.
    fn decided_shown(&self, f: &Field) -> Json {
        let n = f.name.ascii().to_string();
        match self.code(&format!("{n}_shown")).and_then(|code| self.output(&code, &[n], None)) {
            Some(Value::Bool(b)) => Json::Bool(b),
            _ => Json::Null,
        }
    }

    /// `required` of a field asked for on a condition: whether its condition holds, null while it
    /// cannot be worked out; no for an approvers' field, which the form never asks the requester for.
    fn decided_required(&self, f: &Field) -> Json {
        if f.approvers_only.is_some() {
            return Json::Bool(false);
        }
        match self.code(&format!("{}_required", f.name.ascii())).and_then(|code| self.output(&code, &[], None)) {
            Some(Value::Bool(b)) => Json::Bool(b),
            _ => Json::Null,
        }
    }

    /// The value of a computed number at the top of the form: none while a field that may be empty
    /// it reads is empty or hidden, or while it cannot be worked out.
    fn computed(&self, f: &Field) -> Json {
        let Some(code) = self.code(f.name.ascii()) else { return Json::Null };
        let w = self.worked.as_ref().expect("an output is the copy's");
        let when = w.value_when.iter().find(|(c, _)| *c == code).map(|(_, fs)| fs.as_slice()).unwrap_or_default();
        if when.iter().any(|n| self.field(n).is_none_or(|g| self.held(g, &[]).is_none())) {
            return Json::Null;
        }
        match self.output(&code, &[], None) {
            Some(Value::Int(n)) => Json::int(n),
            _ => Json::Null,
        }
    }

    /// The output `code` of the copy, worked out from what it reads alone (as the screens'
    /// `outputOf`): None while it cannot be worked out. `row` gives the rows field it names the one
    /// row in place of the form's.
    fn output(&self, code: &str, seen: &[String], row: Option<(&str, &Json)>) -> Option<Value> {
        let w = self.worked.as_ref()?;
        let (lf, facts) = self.copy.as_ref()?;
        let reads: &[(usize, Vec<String>)] = w.reads.iter().find(|(c, _)| c == code).map(|(_, r)| r.as_slice()).unwrap_or_default();
        let mut ins = Values::new();
        for (i, p) in w.params.iter().enumerate() {
            let name = param_name(facts, i)?;
            let stand = w.stand_ins.get(i).cloned().unwrap_or(Json::Null);
            let Some((_, cols)) = reads.iter().find(|(j, _)| *j == i) else {
                ins.push((name, self.stand_in(facts, i, p, &stand)));
                continue;
            };
            match p {
                Param::Given(f) => {
                    let g = self.field(f)?;
                    ins.push((name, Value::Bool(self.held(g, seen).is_some())));
                }
                Param::Rows(f, taken) => {
                    let g = self.field(f)?;
                    let rows = match row {
                        Some((r, one)) if r == f.as_str() => Some(Json::Arr(vec![one.clone()])),
                        _ => self.held(g, seen),
                    };
                    let rows = rows.as_ref().and_then(Json::as_arr).unwrap_or_default();
                    ins.push((name, Value::List(self.rows_for(facts, i, g, taken, &stand, cols, rows)?)));
                }
                Param::Value(f, _, empty) => {
                    let g = self.field(f)?;
                    match self.held(g, seen) {
                        Some(x) => ins.push((name, self.for_rule(g, &x)?)),
                        None => match empty {
                            Empty::Wait => return None,
                            Empty::None => ins.push((name, Value::None)),
                            Empty::StandIn => ins.push((name, self.stand_in(facts, i, p, &stand))),
                        },
                    }
                }
            }
        }
        let rules = self.m.suite.rules.as_ref()?;
        let got = self.m.fs(|| rules.eval(&self.m.lowered.path(lf), &ins)).ok()?;
        let out = facts.typescript.outputs.iter().find(|o| o.alias == code)?;
        got.into_iter().find(|(n, _)| *n == out.name).map(|(_, v)| v)
    }

    /// The rows the copy is given for an output that reads the fields `cols` of them (by their names
    /// in the code): each row of the form with the value of each, a row whose field the requester
    /// has to fill in is empty left out (it is not there yet); a field that may be empty is given
    /// with whether it is filled in, and a field the output does not read as the value of its range
    /// the screens give it. None where a value it reads of a row there is outside its field's range,
    /// or where the rows there are more than the field takes.
    #[allow(clippy::too_many_arguments)]
    fn rows_for(&self, facts: &RuleFacts, i: usize, f: &Field, taken: &[(String, String, bool)], stand: &Json, cols: &[String], rows: &[Json]) -> Option<Vec<Values>> {
        let (_, _, columns) = element(facts, i)?;
        let col = |code: &str| columns.iter().find(|c| column_code(c) == code).map(|c| c.name.clone());
        let mut out = Vec::new();
        for r in rows {
            let mut one = Values::new();
            let mut there = true;
            let mut bad = false;
            for (code, child, given) in taken {
                let c = f.children.iter().find(|c| c.name.ascii() == child)?;
                let x = r.get(child).filter(|x| !x.is_null());
                let standing = || if *given { Value::Bool(false) } else { stand.get("cells").and_then(|cs| cs.get(code)).and_then(|v| self.for_rule(c, v)).unwrap_or(Value::None) };
                let v = if !cols.contains(code) {
                    standing()
                } else if *given {
                    Value::Bool(x.is_some())
                } else if let Some(x) = x {
                    match self.for_rule(c, x) {
                        Some(v) => v,
                        None => {
                            bad = true;
                            Value::None
                        }
                    }
                } else if taken.iter().any(|(_, n, g)| *g && n == child) {
                    standing()
                } else {
                    there = false;
                    Value::None
                };
                one.push((col(code)?, v));
            }
            if there && bad {
                return None;
            }
            if there {
                out.push(one);
            }
        }
        if crate::ports::field(f).hi.is_some_and(|most| out.len() as i128 > most) {
            return None;
        }
        Some(out)
    }

    /// What an argument the output called does not read is given (as the screens' `standIn`): the
    /// value of its range the description holds, no for whether a field is filled in, and for rows
    /// as many rows as the field asks for at least, each of such values.
    fn stand_in(&self, facts: &RuleFacts, i: usize, p: &Param, stand: &Json) -> Value {
        match p {
            Param::Given(_) => Value::Bool(false),
            Param::Value(f, ..) => match (self.field(f), stand) {
                (_, Json::Null) | (None, _) => Value::None,
                (Some(g), v) => self.for_rule(g, v).unwrap_or(Value::None),
            },
            Param::Rows(f, taken) => {
                let Some(g) = self.field(f) else { return Value::List(Vec::new()) };
                let columns = element(facts, i).map(|(_, _, c)| c.clone()).unwrap_or_default();
                let mut row = Values::new();
                for (code, child, given) in taken {
                    let Some(name) = columns.iter().find(|c| column_code(c) == code).map(|c| c.name.clone()) else { continue };
                    let v = if *given {
                        Value::Bool(false)
                    } else {
                        let c = g.children.iter().find(|c| c.name.ascii() == child);
                        stand.get("cells").and_then(|cs| cs.get(code)).zip(c).and_then(|(v, c)| self.for_rule(c, v)).unwrap_or(Value::None)
                    };
                    row.push((name, v));
                }
                let count = stand.get("count").and_then(Json::as_int).unwrap_or(0).max(0);
                Value::List((0..count).map(|_| row.clone()).collect())
            }
        }
    }

    /// A value of the field `f` as the copy takes it: a number as the whole number of its steps, a
    /// date as its day, a choice by the name the lowered files write its value under. None for one
    /// outside the field's range, or no value of it (a choice it does not have), which the copy does
    /// not take.
    fn for_rule(&self, f: &Field, v: &Json) -> Option<Value> {
        let s = self.m.s;
        match &f.ty {
            TypeExpr::Num(_) => {
                let n = v.as_int()?;
                let r = crate::ports::field(f);
                (r.lo.is_none_or(|l| n >= l) && r.hi.is_none_or(|h| n <= h)).then_some(Value::Int(n))
            }
            TypeExpr::Date => {
                let d = ritsu_ports::day_of(v.as_str()?)?;
                let (lo, hi) = crate::lower::words::ends(f);
                let r = ritsu_units::Rat::int(i128::from(d));
                let inside = lo.is_none_or(|l| r.cmp_to(l) != std::cmp::Ordering::Less) && hi.is_none_or(|h| r.cmp_to(h) != std::cmp::Ordering::Greater);
                inside.then(|| Value::Date(crate::lex::date_text(d)))
            }
            TypeExpr::Bool => Some(Value::Bool(v.as_bool()?)),
            TypeExpr::Choice => {
                let x = v.as_str()?;
                f.options.iter().any(|o| o.name.ascii() == x).then(|| Value::Enum(crate::lower::words::choice_value(s, f, x)))
            }
            // a rule's enum; a master's row is any string
            TypeExpr::ChoiceFrom(p) if p.len() == 2 && s.rule_facts(&p[0].text).is_some() => {
                let x = v.as_str()?;
                let e = s.rule_facts(&p[0].text).and_then(|r| r.enums.iter().find(|e| e.name == p[1].text || e.public == p[1].text))?;
                e.values.iter().any(|y| y.public == x).then(|| Value::Enum(x.to_string()))
            }
            _ => Some(Value::Str(v.as_str()?.to_string())),
        }
    }

    /// For each row of the rows field `f` the form holds, the value of each computation of the row:
    /// the copy over that row alone, as the machine holds the row (DESIGN 4.4). None of a row while a
    /// field of it the computation reads is empty or outside its range, while the form hides the
    /// rows, and where the copy says nothing of it alone.
    fn rows_worked(&self, f: &Field) -> Json {
        let held = self.held(f, &[]);
        let given = Self::wire(f, self.values.iter().find(|(n, _)| n == f.name.ascii()).map(|(_, v)| v));
        let rows = given.as_ref().and_then(Json::as_arr).unwrap_or_default();
        let mut out = Vec::new();
        for r in rows {
            let mut one = Vec::new();
            for c in f.children.iter().filter(|c| c.computed.is_some()) {
                let v = match held {
                    Some(_) => self.row_value(f, c, r),
                    None => Json::Null,
                };
                one.push((c.name.ascii().to_string(), Json::obj([("value", v)])));
            }
            out.push(Json::Obj(one));
        }
        Json::arr(out)
    }

    /// The value of the computation `c` of the row `r` of the rows `f`.
    fn row_value(&self, f: &Field, c: &Field, r: &Json) -> Json {
        let s = self.m.s;
        let Some(e) = &c.computed else { return Json::Null };
        // each field of the row it reads, through the row's computations it reads, filled in and
        // within its range
        for read in crate::lower::form_reads(s, e, Some(f)) {
            let Some((_, cell)) = read.split_once('.') else { continue };
            let Some(g) = f.children.iter().find(|g| g.name.ascii() == cell) else { return Json::Null };
            match r.get(cell) {
                Some(x) if self.for_rule(g, x).is_some() => {}
                _ => return Json::Null,
            }
        }
        let Some(code) = self.row_output(f, c) else { return Json::Null };
        match self.output(&code, &[], Some((f.name.ascii(), r))) {
            Some(Value::Int(n)) => Json::int(n),
            _ => Json::Null,
        }
    }

    /// The output of the copy that is the computation `c` of the rows `f` where the copy is given one
    /// row: its most or its least over the rows (which the copy works out where the computation is
    /// held to a range), else a total of it alone. None where the copy has neither.
    fn row_output(&self, f: &Field, c: &Field) -> Option<String> {
        let (lf, facts) = self.copy.as_ref()?;
        let s = self.m.s;
        let code_of = |name: &str| facts.typescript.outputs.iter().find(|o| o.name == name).map(|o| o.alias.clone());
        let held = crate::lower::form::held(s, lf);
        let h = held.iter().find(|h| h.rows.as_deref() == Some(f.name.ascii()) && h.field == c.name.ascii());
        if let Some(code) = h.and_then(|h| h.hi.as_ref().or(h.lo.as_ref())).and_then(|(n, _)| code_of(n)) {
            return Some(code);
        }
        let total = s.fields().iter().find(|g| matches!(&g.computed, Some(e) if matches!(&e.kind, ExprKind::Sum(p) if p.len() == 2 && p[0].text == f.name.text && p[1].text == c.name.text)))?;
        self.code(total.name.ascii())
    }

    /// The dates the form counts (`form.cal`), by their fields' ASCII names: each counted from the
    /// values it reads, in the order the file writes them, as the machine counts it; none where a
    /// value it is counted from is empty or hidden, or outside the range `form.cal` reads it in, or
    /// where it cannot be counted.
    fn dates(&self) -> Vec<(String, Json)> {
        let m = self.m;
        let Some(df) = m.lowered.files.iter().find(|f| f.part == Part::FormDates) else { return Vec::new() };
        let Some(dates) = m.suite.dates.as_ref() else { return Vec::new() };
        let p = m.lowered.path(df);
        let Ok(facts) = m.fs(|| dates.facts(&p)) else { return Vec::new() };
        let mut counted: Vec<(String, Json)> = Vec::new();
        for fun in &facts.functions {
            let want = asked(df, &fun.name);
            let mut ins = Vec::new();
            let mut whole = true;
            for i in &facts.inputs {
                let from = asked(df, &i.name);
                let v = match self.field(&from) {
                    Some(g) if g.computed.is_some() && g.ty == TypeExpr::Date => counted.iter().find(|(n, _)| *n == from).map(|(_, v)| v.clone()).filter(|v| !v.is_null()),
                    Some(g) if g.computed.is_some() => Some(self.computed(g)).filter(|v| !v.is_null()),
                    Some(g) => self.held(g, &[]).filter(|x| self.for_rule(g, x).is_some()),
                    None => None,
                };
                let n = v.and_then(|v| match &v {
                    Json::Int(n) => i64::try_from(*n).ok(),
                    Json::Str(d) => ritsu_ports::day_of(d),
                    _ => None,
                });
                let n = n.filter(|n| *n >= i.min && *n <= i.max);
                if n.is_none() && fun.params.contains(&i.name) {
                    whole = false;
                }
                ins.push((i.name.clone(), n.unwrap_or(i.min)));
            }
            let v = if whole {
                match m.fs(|| dates.eval(&p, &ins)) {
                    Ok(got) => match got.into_iter().find(|(n, _)| *n == fun.name).map(|(_, v)| v) {
                        Some(DateValue::Day(d)) => Json::str(crate::lex::date_text(d)),
                        _ => Json::Null,
                    },
                    Err(_) => Json::Null,
                }
            } else {
                Json::Null
            };
            counted.push((want, v));
        }
        counted
    }
}

/// The name the request gives an output or an input of a lowered file (`days` for `days_value`).
fn asked(lf: &LFile, given: &str) -> String {
    lf.renamed.iter().find(|(g, _)| g == given).map(|(_, w)| w.clone()).unwrap_or_else(|| given.to_string())
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

/// The vectors of a kind's entry `worked` (DESIGN 10.4), a line of JSON each: what it is given
/// (`input`), what the reference answers (`answer`), and where the input comes from (`from`). The
/// forms are those the kind's vectors (`lines`) submit, each as it is, and the forms they edit with
/// the values the edits give; then, from the first form submitted, the form with nothing filled in,
/// and each field left empty, given each value of a choice and a value that is none of them, its
/// least and its most and a value past each (a number's and a date's), yes and no, a text of spaces
/// alone; rows with none, with one more than the field takes, and with the first row's fields each
/// left empty, at its ends and past them; and an edit of each field an approver gives or corrects,
/// giving each such value, none (which keeps the form's), and an edit that gives no field.
pub fn vectors(m: &Machine, lines: &str) -> String {
    let s = m.s;
    let mut bases: Vec<(String, Json)> = Vec::new();
    let mut inputs: Vec<(String, Json)> = Vec::new();
    let mut add = |from: String, form: Json, edited: Json| {
        let input = Json::obj([("form", form), ("edited", edited)]);
        if !inputs.iter().any(|(_, i)| *i == input) {
            inputs.push((from, input));
        }
    };
    let mut edits: Vec<(String, Json, Json)> = Vec::new();
    for line in lines.lines() {
        let Ok(v) = ritsu_base::json::parse_nested(line, 4096) else { continue };
        let from = v.get("from").and_then(Json::as_str).unwrap_or_default().to_string();
        let Some(ev) = v.get("event") else { continue };
        if let Some(form) = ev.get("submit").and_then(|x| x.get("form")) {
            let f = given_only(s, form);
            if !bases.iter().any(|(_, b)| *b == f) {
                bases.push((from.clone(), f));
            }
        }
        if let (Some(e), Some(form)) = (ev.get("edit"), v.get("ticket").and_then(|t| t.get("form"))) {
            edits.push((from, given_only(s, form), e.get("fields").cloned().unwrap_or(Json::Null)));
        }
    }
    for (from, b) in &bases {
        add(from.clone(), b.clone(), Json::Null);
    }
    for (from, f, e) in edits {
        add(from, f, e);
    }
    if let Some((from, base)) = bases.first() {
        add("a form with nothing filled in".to_string(), Json::Obj(Vec::new()), Json::Null);
        for f in given_fields(s) {
            let n = f.name.ascii();
            for (what, v) in values_of(f, base.get(n)) {
                add(format!("{from}; `{n}` {what}"), with_value(base, n, v), Json::Null);
            }
        }
        let editable = editable(s);
        if !editable.is_empty() {
            add(format!("{from}; an edit that gives no field"), base.clone(), Json::Obj(Vec::new()));
        }
        for f in editable {
            let n = f.name.ascii();
            for (what, v) in values_of(f, base.get(n)) {
                add(format!("{from}; an edit of `{n}` {what}"), base.clone(), Json::Obj(vec![(n.to_string(), v.unwrap_or(Json::Null))]));
            }
        }
    }
    let mut out = String::new();
    for (from, input) in inputs {
        let Ok(answer) = answer(m, &input) else { continue };
        out.push_str(&format!("{}\n", Json::obj([("from", Json::str(&from)), ("input", input), ("answer", answer)]).compact()));
    }
    out
}

/// The values of a form a person gives (the fields it does not work out), from the form of a
/// ticket or of what is submitted.
fn given_only(s: &crate::names::Scope, form: &Json) -> Json {
    let mut out = Vec::new();
    for f in given_fields(s) {
        let Some(v) = form.get(f.name.ascii()) else { continue };
        let v = match (&f.ty, v.as_arr()) {
            (TypeExpr::Rows, Some(rows)) => Json::arr(rows.iter().map(|r| {
                let cells = f.children.iter().filter(|c| c.computed.is_none()).filter_map(|c| r.get(c.name.ascii()).map(|x| (c.name.ascii().to_string(), x.clone())));
                Json::Obj(cells.collect())
            })),
            _ => v.clone(),
        };
        out.push((f.name.ascii().to_string(), v));
    }
    Json::Obj(out)
}

/// The form `base` with the value of `name` given as `v`, or left out where `v` is None.
fn with_value(base: &Json, name: &str, v: Option<Json>) -> Json {
    let mut o: Vec<(String, Json)> = base.as_obj().map(|o| o.to_vec()).unwrap_or_default();
    o.retain(|(k, _)| k != name);
    if let Some(v) = v {
        o.push((name.to_string(), v));
    }
    Json::Obj(o)
}

/// The values a vector gives the field `f` in place of `now` (its value in the form), each with
/// what it is: None for the field left empty.
fn values_of(f: &Field, now: Option<&Json>) -> Vec<(String, Option<Json>)> {
    let mut out: Vec<(String, Option<Json>)> = Vec::new();
    match &f.ty {
        TypeExpr::Rows => {
            out.push(("with no row".to_string(), Some(Json::arr([]))));
            let rows = now.and_then(Json::as_arr).unwrap_or_default();
            let Some(first) = rows.first() else { return out };
            if let Some(most) = crate::ports::field(f).hi.filter(|h| (0..64).contains(h)) {
                out.push(("with one row more than it takes".to_string(), Some(Json::arr((0..=most).map(|_| first.clone())))));
            }
            out.push(("with one row".to_string(), Some(Json::arr([first.clone()]))));
            for c in f.children.iter().filter(|c| c.computed.is_none()) {
                let cn = c.name.ascii();
                for (what, v) in values_of(c, first.get(cn)) {
                    if matches!(c.ty, TypeExpr::Choice | TypeExpr::ChoiceFrom(_)) && v.is_some() {
                        continue;
                    }
                    let mut all = rows.to_vec();
                    all[0] = with_value(first, cn, v);
                    out.push((format!("with `{cn}` of the first row {what}"), Some(Json::Arr(all))));
                }
            }
            return out;
        }
        TypeExpr::Bool => {
            out.push(("not given".to_string(), None));
            out.push(("yes".to_string(), Some(Json::Bool(true))));
            out.push(("no".to_string(), Some(Json::Bool(false))));
            return out;
        }
        _ => out.push(("left empty".to_string(), None)),
    }
    match &f.ty {
        TypeExpr::Num(_) => {
            let r = crate::ports::field(f);
            let fits = |n: i128| i64::try_from(n).is_ok();
            if let Some(lo) = r.lo {
                out.push(("at its least".to_string(), Some(Json::int(lo))));
                if fits(lo - 1) {
                    out.push(("below its range".to_string(), Some(Json::int(lo - 1))));
                }
            }
            if let Some(hi) = r.hi {
                out.push(("at its most".to_string(), Some(Json::int(hi))));
                if fits(hi + 1) {
                    out.push(("above its range".to_string(), Some(Json::int(hi + 1))));
                }
            }
        }
        TypeExpr::Date => {
            let (lo, hi) = crate::lower::words::ends(f);
            let day = |r: ritsu_units::Rat, by: i64| Json::str(crate::lex::date_text(r.num as i64 + by));
            if let Some(lo) = lo.filter(|r| r.is_int()) {
                out.push(("at its first day".to_string(), Some(day(lo, 0))));
                out.push(("the day before its range".to_string(), Some(day(lo, -1))));
            }
            if let Some(hi) = hi.filter(|r| r.is_int()) {
                out.push(("at its last day".to_string(), Some(day(hi, 0))));
                out.push(("the day after its range".to_string(), Some(day(hi, 1))));
            }
        }
        TypeExpr::Choice => {
            for o in &f.options {
                out.push((format!("`{}`", o.name.ascii()), Some(Json::str(o.name.ascii()))));
            }
            out.push(("a value that is none of its values".to_string(), Some(Json::str("none_of_them"))));
        }
        TypeExpr::Text | TypeExpr::LongText => out.push(("of spaces alone".to_string(), Some(Json::str("   ")))),
        _ => {}
    }
    // a value the form had, given again, is no other vector
    out.retain(|(_, v)| v.as_ref() != now || now.is_none());
    out
}
