//! What a form takes (DESIGN 10.2): one reading of a value written for a field, and one check of a
//! value against its field, for every place the values of a form are given — a cell of the
//! examples, a scenario's `form`, `simulate --form`, and a ticket's `submit`, `resubmit` and
//! `edit`. The type, the value of a choice, the unit's step, the range, the length, the row of a
//! master, the person and the team, the open day of `open in`, the number of rows, and what must be
//! filled in are each decided here and nowhere else, so that the check, `simulate` and the ticket
//! machine refuse the same values with the same words.

use crate::ast::*;
use crate::engine::part::PartRow;
use crate::names::Scope;
use crate::org::Version;
use ritsu_base::text::Text;
use ritsu_ports::{CalendarFacts, Value, Values};
use ritsu_units::Rat;
use std::rc::Rc;

/// Why a value is not taken, and the code the check says it with: E009 for a value of another
/// type, E004 for a value a choice does not have, E601 for a value of the field's type the form
/// does not take.
#[derive(Clone, Debug)]
pub struct No {
    pub code: &'static str,
    pub text: Text,
}

/// What the values are read against besides the form.
pub struct When<'x> {
    /// The day the request is made on; None where the values have no day (the examples): a row of
    /// a master of any day is taken then.
    pub day: Option<i64>,
    /// The versions of the organization a person or a team is looked for in: the one in effect on
    /// the day, or every version where the values have no day. None where the request reads no
    /// organization's data: a person and a team are then taken as written.
    pub versions: Option<Vec<&'x Version>>,
    /// The calendars the request uses, by alias, for `open in`.
    pub calendars: &'x [(String, Rc<CalendarFacts>)],
    /// The rows of the masters in effect on the day, where they are given with the event (the
    /// ticket machine); None to read them from the masters' data.
    pub rows: Option<&'x [PartRow]>,
}

/// A value written for the field `f`, as the lowered rules take it: a number counted in the
/// field's step, a choice's value by its ASCII name, a person and a team by their IDs.
pub fn read(f: &Field, e: &Expr) -> Result<Value, No> {
    let n = &f.name.text;
    let bad = || No { code: "E009", text: tr!("`{n}` の値が、フィールドの型に合いません", "the value of `{n}` does not fit the field's type") };
    Ok(match (&f.ty, &e.kind) {
        (_, ExprKind::None) => Value::None,
        (TypeExpr::Choice, ExprKind::Path(p)) if p.len() == 1 => {
            let v = &p[0].text;
            let o = f.options.iter().find(|o| &o.name.text == v || o.name.ascii() == v).ok_or_else(|| No { code: "E004", text: tr!("`{n}` に `{v}` という値はありません", "`{n}` has no value `{v}`") })?;
            Value::Enum(o.name.ascii().to_string())
        }
        (TypeExpr::ChoiceFrom(_), ExprKind::Path(p)) if p.len() == 1 => Value::Enum(p[0].text.clone()),
        (TypeExpr::ChoiceFrom(_) | TypeExpr::Text | TypeExpr::LongText | TypeExpr::Attachment, ExprKind::Str(x)) => Value::Str(x.clone()),
        (TypeExpr::Person, ExprKind::Person(id)) => Value::Str(id.clone()),
        (TypeExpr::Team, ExprKind::Team(w)) => Value::Str(w.text.clone()),
        (TypeExpr::Person, ExprKind::Str(x)) => {
            return Err(No { code: "E009", text: tr!("`{n}` は人のフィールドです。値は `person \"{x}\"` と書きます", "`{n}` is a field of a person: its value is written `person \"{x}\"`") });
        }
        (TypeExpr::Team, ExprKind::Str(x)) => {
            return Err(No { code: "E009", text: tr!("`{n}` はチームのフィールドです。値は `team {x}` と書きます", "`{n}` is a field of a team: its value is written `team {x}`") });
        }
        (TypeExpr::Num(u), ExprKind::Num(_) | ExprKind::Neg(_)) => {
            let Some(x) = crate::lower::words::constant_in(e, &f.ty).map(|v| v.rat()) else { return Err(bad()) };
            let st = crate::lower::words::step(u);
            let r = x.div(st);
            if !r.is_int() {
                let (w, sw) = (crate::lower::words::number(x, u), crate::lower::words::number(st, u));
                return Err(No { code: "E601", text: off_step(&w, n, &sw) });
            }
            Value::Int(r.num)
        }
        (TypeExpr::Date, ExprKind::Date(d)) => Value::Date(crate::lex::date_text(*d)),
        (TypeExpr::Bool, ExprKind::Bool(b)) => Value::Bool(*b),
        (TypeExpr::Rows, ExprKind::List(rows)) => {
            let mut out = Vec::new();
            for r in rows {
                let ExprKind::Record(fs) = &r.kind else { return Err(bad()) };
                let mut row = Vec::new();
                for (k, x) in fs {
                    let Some(c) = f.children.iter().find(|c| c.name.text == k.text || c.name.ascii() == k.text) else {
                        let w = &k.text;
                        return Err(No { code: "E004", text: tr!("`{n}` の行に `{w}` はありません", "a row of `{n}` has no `{w}`") });
                    };
                    row.push((c.name.ascii().to_string(), read(c, x)?));
                }
                out.push(row);
            }
            Value::List(out)
        }
        _ => return Err(bad()),
    })
}

fn off_step(w: &str, n: &str, sw: &str) -> Text {
    tr!("`{w}` は、`{n}` の刻み（`{sw}`）の倍数ではありません", "`{w}` is not a multiple of the step of `{n}` (`{sw}`)")
}

/// A value of a field as the language writes it: `1_200_000JPY`, `2026-11-16`, `"text"`, a value of
/// a choice by its name as written (`役務`, not its name in the JSON `service`).
pub fn shown(f: &Field, v: &Value) -> String {
    match (&f.ty, v) {
        (TypeExpr::Num(u), Value::Int(n)) => crate::lower::words::number(Rat::int(*n).mul(crate::lower::words::step(u)), u),
        (TypeExpr::Person, Value::Str(s)) => format!("person \"{s}\""),
        (TypeExpr::Team, Value::Str(s)) => format!("team {s}"),
        (TypeExpr::Choice, Value::Enum(e)) => f.options.iter().find(|o| o.name.ascii() == e).map_or_else(|| e.clone(), |o| o.name.text.clone()),
        _ => plain(v),
    }
}

/// A value with no field to read it by.
pub fn plain(v: &Value) -> String {
    match v {
        Value::Bool(b) => b.to_string(),
        Value::Int(n) => crate::lower::words::grouped(*n),
        Value::Str(s) => format!("\"{s}\""),
        Value::Date(d) => d.clone(),
        Value::Enum(e) => e.clone(),
        Value::List(l) => format!("[{} rows]", l.len()),
        Value::None => "none".into(),
    }
}

/// Whether `x` is below `lo` or above `hi`, where they are given.
fn outside(x: Rat, lo: Option<Rat>, hi: Option<Rat>) -> bool {
    use std::cmp::Ordering::{Greater, Less};
    lo.is_some_and(|lo| x.cmp_to(lo) == Less) || hi.is_some_and(|hi| x.cmp_to(hi) == Greater)
}

/// The ends of a `count`, both in.
pub(crate) fn count_ends(b: &Bounds) -> (Option<Rat>, Option<Rat>) {
    let end = |x: &Option<Bound>, inward: i128| {
        let x = x.as_ref()?;
        let c = crate::lower::words::constant(&x.value)?.rat();
        Some(if x.inclusive { c } else { c.add(Rat::int(inward)) })
    };
    (end(&b.lo, 1), end(&b.hi, -1))
}

/// Why the field `f` does not take the value `v` (of its type, as [`read`] reads it), when it does
/// not. `w` is the value as it is shown: as it was written, where it was.
pub fn holds(s: &Scope, f: &Field, v: &Value, w: &str, when: &When) -> Option<Text> {
    holds_at(s, f, v, w, when, &f.name.text)
}

/// [`holds`], the field named `n` in the words: a field of a row by its place, the rows counted
/// from 0 as `at` counts them (`items[1].price`, DESIGN 12.4).
fn holds_at(s: &Scope, f: &Field, v: &Value, w: &str, when: &When, n: &str) -> Option<Text> {
    use crate::lower::words;
    match (&f.ty, v) {
        (_, Value::None) => None,
        (TypeExpr::Num(u), Value::Int(k)) => {
            let x = Rat::int(*k).mul(words::step(u));
            // the ends the lowered rule reads the field with: a computed field's, what it can
            // come to within its range
            let (lo, hi) = crate::lower::route::reach_or_range(s, f);
            if !outside(x, lo, hi) {
                return None;
            }
            let r = crate::lower::route::range_of(f, lo, hi);
            Some(if r == words::range_text(f) {
                tr!("`{w}` は、`{n}` の範囲（`{r}`）の外です", "`{w}` is outside the range of `{n}` (`{r}`)")
            } else {
                tr!("`{w}` は、`{n}` の計算がとりうる値（`{r}`）の外です", "`{w}` is outside what the computation of `{n}` can come to (`{r}`)")
            })
        }
        (TypeExpr::Date, Value::Date(d)) => {
            let day = ritsu_ports::day_of(d)?;
            let (lo, hi) = words::ends(f);
            let r = words::range_text(f);
            if outside(Rat::int(day as i128), lo, hi) {
                return Some(tr!("`{w}` は、`{n}` の範囲（`{r}`）の外です", "`{w}` is outside the range of `{n}` (`{r}`)"));
            }
            // `open in office`: a day the calendar is open on
            let cal = f.open_in.as_ref()?;
            let (_, facts) = when.calendars.iter().find(|(a, _)| *a == cal.text)?;
            let c = &cal.text;
            if day < facts.data.0 || day > facts.data.1 {
                let (a, b) = (crate::lex::date_text(facts.data.0), crate::lex::date_text(facts.data.1));
                return Some(tr!("`{w}` は、カレンダー `{c}` のデータ（{a} から {b}）の外です", "`{w}` is outside the data of the calendar `{c}` ({a} to {b})"));
            }
            facts.closed.contains(&day).then(|| tr!("`{w}` は、カレンダー `{c}` の休みの日です。`{n}` は `open in {c}` です", "`{w}` is a day the calendar `{c}` is closed on, and `{n}` is `open in {c}`"))
        }
        (TypeExpr::Text | TypeExpr::LongText, Value::Str(t)) => {
            let b = f.count.as_ref()?;
            let len = Rat::int(t.chars().count() as i128);
            let (lo, hi) = count_ends(b);
            let (l, c) = (t.chars().count(), crate::types::written(s.src, b.span));
            let long = if l == 1 { "1 character long".to_string() } else { format!("{l} characters long") };
            if outside(len, None, hi) {
                return Some(tr!("`{w}` は{l}文字で、`{n}` の上限（`{c}`）を超えます", "`{w}` is {long}, past the limit of `{n}` (`{c}`)"));
            }
            outside(len, lo, None).then(|| tr!("`{w}` は{l}文字で、`{n}` の下限（`{c}`）に足りません", "`{w}` is {long}, short of the least of `{n}` (`{c}`)"))
        }
        (TypeExpr::ChoiceFrom(p), Value::Str(key) | Value::Enum(key)) if p.len() == 2 => {
            // the values of a rule's enum are the types' to say; a master's rows are read here
            if let Some(rows) = when.rows {
                let m = format!("{}.{}", p[0].text, p[1].text);
                if s.ringi(&p[0].text, UseKind::Masters).is_none() || rows.iter().any(|r| r.master == m && &r.key == key) {
                    return None;
                }
                let name = &p[1].text;
                let d = when.day.map(crate::lex::date_text).unwrap_or_default();
                return Some(tr!("`{n}` の `{key}` は、{d} に選べるマスタ `{name}` の行ではありません", "`{key}` of `{n}` is not a row of the master `{name}` to choose on {d}"));
            }
            let md = s.masters_data(&p[0].text)?.master(&p[1].text)?;
            let m = &md.name;
            match when.day {
                None => (!md.rows.iter().any(|r| &r.key == key)).then(|| tr!("`{w}` は、`{n}` が選ぶマスタ `{m}` の行ではありません", "`{w}` is not a row of the master `{m}`, which `{n}` chooses from")),
                Some(day) => {
                    let d = crate::lex::date_text(day);
                    (!md.rows.iter().any(|r| &r.key == key && r.on(day))).then(|| tr!("`{n}` の `{key}` は、{d} に選べるマスタ `{m}` の行ではありません", "`{key}` of `{n}` is not a row of the master `{m}` to choose on {d}"))
                }
            }
        }
        (TypeExpr::Person, Value::Str(id)) => {
            let vs = when.versions.as_ref()?;
            if vs.iter().any(|v| v.person(id).is_some()) {
                return None;
            }
            Some(match (when.day, vs.as_slice()) {
                (Some(_), [v]) => {
                    let d = v.day();
                    tr!("`{n}` の {id} は、{d} からの組織にいません", "{id} of `{n}` is not in the organization from {d}")
                }
                _ => tr!("`{n}` の {id} は、組織のどの版にもいません", "{id} of `{n}` is in no version of the organization"),
            })
        }
        (TypeExpr::Team, Value::Str(id)) => {
            let vs = when.versions.as_ref()?;
            if vs.iter().any(|v| v.team(id).is_some()) {
                return None;
            }
            Some(match (when.day, vs.as_slice()) {
                (Some(_), [v]) => {
                    let d = v.day();
                    tr!("`{n}` のチーム `{id}` は、{d} からの組織にありません", "the team `{id}` of `{n}` is not in the organization from {d}")
                }
                _ => tr!("`{n}` のチーム `{id}` は、組織のどの版にもありません", "the team `{id}` of `{n}` is in no version of the organization"),
            })
        }
        (TypeExpr::Rows, Value::List(rows)) => {
            if let Some(b) = &f.count {
                let (lo, hi) = count_ends(b);
                let (l, c) = (rows.len(), crate::types::written(s.src, b.span));
                let len = Rat::int(l as i128);
                let many = if l == 1 { "1 row".to_string() } else { format!("{l} rows") };
                if outside(len, None, hi) {
                    return Some(tr!("`{n}` は{l}行で、行の数の上限（`{c}`）を超えます", "`{n}` has {many}, past the limit of its rows (`{c}`)"));
                }
                if outside(len, lo, None) {
                    return Some(tr!("`{n}` は{l}行で、行の数の下限（`{c}`）に足りません", "`{n}` has {many}, short of the least of its rows (`{c}`)"));
                }
            }
            // a field of a row is said at its place, as the place the refusal names (`at_of`)
            for (i, row) in rows.iter().enumerate() {
                let place = |c: &Field| format!("{n}[{i}].{}", c.name.text);
                for (k, x) in row {
                    let Some(c) = f.children.iter().find(|c| c.name.ascii() == k) else { continue };
                    if c.computed.is_some() {
                        let p = place(c);
                        return Some(tr!("`{p}` は計算するフィールドなので、渡せません", "`{p}` is computed, and is not given"));
                    }
                    if let Some(t) = holds_at(s, c, x, &shown(c, x), when, &place(c)) {
                        return Some(t);
                    }
                }
                // a field of the row that is not filled in
                for c in f.children.iter().filter(|c| c.computed.is_none() && c.optional.is_none() && c.ty != TypeExpr::Attachment) {
                    if !row.iter().any(|(k, x)| k == c.name.ascii() && *x != Value::None) {
                        let p = place(c);
                        return Some(tr!("`{p}` がありません。必須のフィールドなので、入れてください", "`{p}` is not filled in, and the form asks for it"));
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// The place of the form a refusal of the value `v` of `f` names, as the contract writes it (`at`,
/// DESIGN 12.4): the field by its ASCII name, or, for a field of rows, the field of the first row
/// [`holds`] refuses (`items[1].price`, the rows counted from 0), in the order it reads them. The
/// field itself where the count of its rows is refused.
pub fn at_of(s: &Scope, f: &Field, v: &Value, when: &When) -> String {
    let n = f.name.ascii();
    let (TypeExpr::Rows, Value::List(rows)) = (&f.ty, v) else { return n.to_string() };
    for (i, row) in rows.iter().enumerate() {
        for (k, x) in row {
            let Some(c) = f.children.iter().find(|c| c.name.ascii() == k) else { continue };
            if c.computed.is_some() || holds(s, c, x, &shown(c, x), when).is_some() {
                return format!("{n}[{i}].{}", c.name.ascii());
            }
        }
        for c in f.children.iter().filter(|c| c.computed.is_none() && c.optional.is_none() && c.ty != TypeExpr::Attachment) {
            if !row.iter().any(|(k, x)| k == c.name.ascii() && *x != Value::None) {
                return format!("{n}[{i}].{}", c.name.ascii());
            }
        }
    }
    n.to_string()
}

/// Why a cell of the examples is not a value the field `f` takes, when it is not: a number off the
/// unit's step, and what [`holds`] says of a value of the field's type. A cell of another type is
/// the types' to say.
pub fn cell(s: &Scope, f: &Field, e: &Expr, when: &When) -> Option<Text> {
    let w = crate::types::written(s.src, e.span);
    match read(f, e) {
        Ok(v) => holds(s, f, &v, &w, when),
        Err(No { code: "E601", .. }) => {
            let TypeExpr::Num(u) = &f.ty else { return None };
            Some(off_step(&w, &f.name.text, &crate::lower::words::number(crate::lower::words::step(u), u)))
        }
        Err(_) => None,
    }
}

/// The values of a form as they are written (a scenario's `form`, `simulate --form`), by their
/// lowered names: each read for its field and held to it. `src` is the text they are written in.
/// Err with the first value the form does not take, and where it is written.
pub fn form(s: &Scope, src: &str, written: &[(Word, Expr)], when: &When) -> Result<Values, (Span, No)> {
    let mut out = Values::new();
    for (n, e) in written {
        let Some(f) = s.field(&n.text).or_else(|| s.fields().iter().find(|f| f.name.ascii() == n.text)) else {
            let w = &n.text;
            return Err((n.span, No { code: "E004", text: tr!("フォームに `{w}` というフィールドはありません", "the form has no field `{w}`") }));
        };
        if f.computed.is_some() {
            let w = &n.text;
            return Err((n.span, No { code: "E601", text: tr!("`{w}` は計算するフィールドなので、渡せません", "`{w}` is computed, and is not given") }));
        }
        let v = read(f, e).map_err(|no| (e.span, no))?;
        let w = crate::types::written(src, e.span);
        if let Some(t) = holds(s, f, &v, &w, when) {
            return Err((e.span, No { code: "E601", text: t }));
        }
        out.push((f.name.ascii().to_string(), v));
    }
    Ok(out)
}

/// Who gives the values: the requester when the request is made, or an approver editing it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Giver {
    Requester,
    Approver,
}

/// Why the form does not take the values `given` (by their lowered names), each of its field's
/// type: a field the form does not have, one the giver does not fill in, a value its field does
/// not take ([`holds`]). A value of none is not given, as `null` in the JSON is not (DESIGN 10.4).
pub fn given(s: &Scope, given: &Values, giver: Giver, editable: &[String], when: &When) -> Option<Said> {
    for (n, v) in given.iter().filter(|(_, v)| *v != Value::None) {
        let Some(f) = s.fields().iter().find(|f| f.name.ascii() == n || &f.name.text == n) else {
            return Some(Said::of(tr!("フォームに `{n}` というフィールドはありません", "the form has no field `{n}`")));
        };
        let (fnm, at) = (&f.name.text, || f.name.ascii().to_string());
        if f.computed.is_some() {
            return Some(Said::at(tr!("`{fnm}` は計算するフィールドなので、渡せません", "`{fnm}` is computed, and is not given"), at()));
        }
        if let Some(t) = not_theirs(f, giver, editable) {
            return Some(Said::at(t, at()));
        }
        if !fits(f, v) {
            return Some(Said::at(tr!("`{fnm}` の値が、フィールドの型に合いません", "the value of `{fnm}` does not fit the field's type"), at()));
        }
        if let Some(t) = holds(s, f, v, &shown(f, v), when) {
            return Some(Said::at(t, at_of(s, f, v, when)));
        }
    }
    None
}

/// A refusal of the values given, with the place of the form it names (`at`, DESIGN 12.4): None
/// for one that names no field of the form.
#[derive(Clone, Debug)]
pub struct Said {
    pub text: Text,
    pub at: Option<String>,
}

impl Said {
    pub fn of(text: Text) -> Said {
        Said { text, at: None }
    }

    pub fn at(text: Text, at: String) -> Said {
        Said { text, at: Some(at) }
    }
}

/// Why the event does not make empty the fields `clear` names (DESIGN 7.3), each by its lowered
/// name, in the order the list names them: a field the form does not have, one it computes, one the
/// giver does not fill in (each refused as a value given for it is: [`given`]), one the event gives
/// a value too (`given`), and one the form asks for on every request (no `optional`, no condition,
/// not the approvers'), which it refuses empty as it refuses one not filled in ([`missing`]).
pub fn cleared(s: &Scope, clear: &[String], given: &Values, giver: Giver, editable: &[String]) -> Option<Said> {
    for n in clear {
        let Some(f) = s.fields().iter().find(|f| f.name.ascii() == n || &f.name.text == n) else {
            return Some(Said::of(tr!("フォームに `{n}` というフィールドはありません", "the form has no field `{n}`")));
        };
        let fnm = &f.name.text;
        let said = |t: Text| Some(Said::at(t, f.name.ascii().to_string()));
        if f.computed.is_some() {
            return said(tr!("`{fnm}` は計算するフィールドなので、渡せません", "`{fnm}` is computed, and is not given"));
        }
        if let Some(t) = not_theirs(f, giver, editable) {
            return said(t);
        }
        if given.iter().any(|(k, v)| (k == f.name.ascii() || k == fnm) && *v != Value::None) {
            return said(tr!("`{fnm}` に値を渡していて、`clear` にも挙げています", "`{fnm}` is given a value, and is in `clear` too"));
        }
        if f.optional.is_none() && f.required_when.is_none() && f.shown_when.is_none() && f.approvers_only.is_none() {
            return said(tr!("`{fnm}` がありません。必須のフィールドなので、入れてください", "`{fnm}` is not filled in, and the form asks for it"));
        }
    }
    None
}

/// Why the giver does not fill in the field: an approvers' field the requester gives, or one of
/// the requester's that `approvers edit` (`editable`) does not name, which an approver gives.
fn not_theirs(f: &Field, giver: Giver, editable: &[String]) -> Option<Text> {
    let fnm = &f.name.text;
    match giver {
        Giver::Requester if f.approvers_only.is_some() => Some(tr!("`{fnm}` は承認者だけのフィールドなので、申請者は入れません", "`{fnm}` is a field of the approvers only, which the requester does not fill in")),
        Giver::Approver if f.approvers_only.is_none() && !editable.iter().any(|e| e == fnm) => {
            Some(tr!("`{fnm}` は、承認者が直せるフィールドではありません（`approvers edit` に無い）", "`{fnm}` is not a field approvers may edit (it is not in `approvers edit`)"))
        }
        _ => None,
    }
}

/// Whether a value is of the field's type, as [`read`] writes one.
fn fits(f: &Field, v: &Value) -> bool {
    match (&f.ty, v) {
        (_, Value::None) => true,
        (TypeExpr::Choice, Value::Enum(e)) => f.options.iter().any(|o| o.name.ascii() == e),
        (TypeExpr::ChoiceFrom(_), Value::Enum(_) | Value::Str(_)) => true,
        (TypeExpr::Text | TypeExpr::LongText | TypeExpr::Attachment | TypeExpr::Person | TypeExpr::Team, Value::Str(_)) => true,
        (TypeExpr::Num(_), Value::Int(_)) => true,
        (TypeExpr::Date, Value::Date(d)) => ritsu_ports::day_of(d).is_some(),
        (TypeExpr::Bool, Value::Bool(_)) => true,
        (TypeExpr::Rows, Value::List(rows)) => rows.iter().all(|r| r.iter().all(|(k, x)| f.children.iter().find(|c| c.name.ascii() == k).is_some_and(|c| fits(c, x)))),
        _ => false,
    }
}

/// The first field the requester must fill in and did not (DESIGN 4.2): one with no `optional`,
/// shown, and not the approvers'; one whose `required when` holds. `outs` are the outputs of the
/// lowered form (`<field>_shown`, `<field>_required`), by the names `named` gives them. A value
/// of a field that is not shown is refused too (an approvers' field's as well): it is not sent, and
/// a field hidden by an event that does not give it a value has its value dropped first (DESIGN
/// 7.3).
pub fn missing(s: &Scope, vals: &Values, outs: &Values, named: &dyn Fn(&str) -> String) -> Option<Said> {
    let flag = |name: String| outs.iter().find(|(n, _)| *n == named(&name)).and_then(|(_, v)| if let Value::Bool(b) = v { Some(*b) } else { None });
    for f in s.fields().iter().filter(|f| f.computed.is_none()) {
        let n = &f.name.text;
        let has = vals.iter().any(|(k, v)| k == f.name.ascii() && *v != Value::None);
        let shown = if f.shown_when.is_some() { flag(format!("{}_shown", f.name.ascii())).unwrap_or(true) } else { true };
        if !shown {
            if has {
                return Some(Said::at(not_shown(n), f.name.ascii().to_string()));
            }
            continue;
        }
        if has || f.approvers_only.is_some() {
            continue;
        }
        let asked = match (&f.required_when, f.optional.is_some()) {
            (Some(_), _) => flag(format!("{}_required", f.name.ascii())).unwrap_or(false),
            (None, optional) => !optional,
        };
        if asked {
            let t = match &f.required_when {
                Some(_) => tr!("`{n}` がありません。この申請では必須です（`required when` が成り立つ）", "`{n}` is not filled in, and this request must have it (its `required when` holds)"),
                None => tr!("`{n}` がありません。必須のフィールドなので、入れてください", "`{n}` is not filled in, and the form asks for it"),
            };
            return Some(Said::at(t, f.name.ascii().to_string()));
        }
    }
    None
}

/// The first field of `vals` given a value that its `shown when` hides for them, as [`missing`]
/// refuses it: what `simulate` refuses of `--form`, in the machine's words (DESIGN 4.2), without
/// asking for the fields the form asks for (`--form` gives the fields the route reads).
pub fn hidden(s: &Scope, vals: &Values, outs: &Values, named: &dyn Fn(&str) -> String) -> Option<Said> {
    let flag = |name: String| outs.iter().find(|(n, _)| *n == named(&name)).and_then(|(_, v)| if let Value::Bool(b) = v { Some(*b) } else { None });
    s.fields().iter().filter(|f| f.computed.is_none() && f.shown_when.is_some()).find_map(|f| {
        let has = vals.iter().any(|(k, v)| k == f.name.ascii() && *v != Value::None);
        (has && flag(format!("{}_shown", f.name.ascii())) == Some(false)).then(|| Said::at(not_shown(&f.name.text), f.name.ascii().to_string()))
    })
}

/// The refusal of a value of a field its `shown when` hides.
fn not_shown(n: &str) -> Text {
    tr!("`{n}` は、この申請では見せないフィールドです（`shown when` が成り立たない）。値は送りません", "`{n}` is not shown for this request (its `shown when` does not hold), and takes no value")
}
