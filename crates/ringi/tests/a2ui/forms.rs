//! The screens stop what the reference refuses (DESIGN 13.7), on forms made for it rather than on
//! the vectors alone. From each kind's description (`types/<name>.json`), for each field and each
//! kind of check, a form that breaks that check alone and one that just passes it, built on what the
//! reference takes: a request the vectors make; one sent back and made again, as the vectors make it
//! and with every field it takes holding a value; a ticket an approver edits, as the vectors make it
//! and with every field of the approvers' holding a value. Each form is put in the screen's data as a
//! renderer holds it, and the screen is read as a renderer reads it: the parts drawn (a `Show` while
//! its `when` holds), the checks of each drawn part and of the button pressed, the rows of a
//! `LineItems` and of a `List` each in its row. What the button sends is given to the reference
//! interpreter. On every form the two say the same: the screen stops it exactly when the reference
//! refuses it, and when both take it, the ticket keeps what the screen sends (the request, the
//! request made again with its changes and the fields it clears, the approver's edit) and what the
//! screen shows the form to be. The one way they may part is the white space at a text's ends, which
//! the screen does not count and the reference keeps: such a form is given to the reference as it is
//! written too, and counted. On no screen drawn is a format of the basic catalog given a null. A
//! template with a check taken out is found.
//! What only the server knows is not given: a key of a master, a person or a team that is not of
//! the vectors, a day a calendar is closed on (DESIGN 13.7).

use super::{Made, file, made_of, node};
use ringi::check::{Options, Run};
use ringi::engine::{Event, Machine, Submit, Ticket};
use ringi::org::Org;
use ringi::scenario::{self, Input};
use ritsu_base::json::{self, Json};
use ritsu_ports::{Value, Values};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

/// A request written for the shapes of a form the examples do not have: a text asked for while it
/// is shown, an optional one shown on a condition, a long text asked for on a condition of a total,
/// rows with a choice, an attachment and an optional text in them, the widest whole numbers a screen
/// holds exactly (of both signs), a rate in steps of 0.1% below zero, a field of the approvers', and
/// approvers who edit the choice a condition reads and the field it shows. Written beside the
/// examples, it reads their organization.
const GEAR: &str = "request gear v1 \"Gear request\"
description \"Gear for a desk, written for the shapes of a form the examples do not have (fictional rules)\"

use organization people from \"shared/company.ringi\"

form
  place  : choice \"Place\"
    office \"Office\"
    home   \"Home\"
  lines  : rows \"Lines\"  count >=1 <=3
    what  : text \"What\"  count >=2 <=10
    cost  : money[USD] \"Cost\"  range >=1USD <=1_000USD
    size  : choice \"Size\"
      small \"Small\"
      large \"Large\"
    photo : attachment \"Photo\"
    note  : text \"Note\"  count <=20  optional
  total  : money[USD] \"Total\" = sum(lines.cost)  range >=1USD <=2_000USD
  room   : text \"Room\"  count <=20  shown when place is office
  memo   : text \"Memo\"  count <=30  optional  shown when place is home
  reason : long text \"Reason\"  count >=3 <=50  required when total > 500USD
  big    : number \"Big\"  range >=-9_007_199_254_740_991 <=9_007_199_254_740_991
  share  : rate[step 0.1%] \"Share\"  range >=-0.5% <=100%
  owner  : text \"Owner\"  count <=20  approvers only

route
  check  finance

who
  finance = team accounting

every stage
  when empty escalate, then skip

ticket
  self approval  not allowed
  send back      to the requester, at most 3 times
  approvers edit place, room

scenario an_office_order
  requester e1201
  form place = office, lines = [{what: \"Desk\", cost: 120USD, size: small, photo: \"desk.jpg\"}], room = \"4F-12\", big = 9_007_199_254_740_991, share = 12.5%
| at               | event               | by    | -> stage | status         |
| 2026-11-16 09:00 | submit              | e1201 | check    | in progress    |
| 2026-11-16 10:00 | edit(room: \"5F-01\") | e1001 | check    | in progress    |
| 2026-11-16 11:00 | send back           | e1001 |          | with requester |
| 2026-11-16 12:00 | resubmit            | e1201 | check    | in progress    |
| 2026-11-16 13:00 | approve             | e1001 |          | approved       |

scenario a_home_order
  requester e1201
  form place = home, lines = [{what: \"Chair\", cost: 600USD, size: large}], memo = \"For the study\", reason = \"A chair for long days\", big = -9_007_199_254_740_991, share = -0.5%
| at               | event   | by    | -> stage | status      |
| 2026-11-16 09:00 | submit  | e1201 | check    | in progress |
| 2026-11-16 10:00 | approve | e1001 |          | approved    |
";

// ── The kinds' descriptions ──

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Ty {
    Text,
    Long,
    Number,
    Date,
    Choice,
    Master,
    Attachment,
    Person,
    Bool,
    Rows,
}

/// A field as `types/<name>.json` describes it.
#[derive(Clone, Debug)]
pub(super) struct Field {
    pub(super) name: String,
    pub(super) ty: Ty,
    /// A number's ends as integers on the wire, a date's as days.
    pub(super) min: Json,
    pub(super) max: Json,
    pub(super) count_min: Option<i128>,
    count_max: Option<i128>,
    pub(super) choices: Vec<String>,
    pub(super) optional: bool,
    pub(super) computed: bool,
    pub(super) approvers: bool,
    open_in: bool,
    pub(super) shown_when: bool,
    pub(super) required_when: bool,
    /// The conditions it is shown and asked for on: the output that works each out, and the fields
    /// it reads.
    pub(super) shown: Option<(String, Vec<String>)>,
    pub(super) required: Option<(String, Vec<String>)>,
    /// What a computation reads, and whether the screens work it out.
    pub(super) reads: Vec<String>,
    pub(super) worked: bool,
    pub(super) fields: Vec<Field>,
}

impl Field {
    pub(super) fn given(&self) -> bool {
        !self.computed && !self.approvers
    }
}

pub(super) fn fields_of(j: &Json) -> Vec<Field> {
    let all = j.as_arr().unwrap_or_default();
    all.iter()
        .map(|f| {
            let s = |k: &str| f.get(k).and_then(Json::as_str).unwrap_or_default().to_string();
            let yes = |k: &str| f.get(k) == Some(&Json::Bool(true));
            let some = |k: &str| f.get(k).is_some_and(|x| *x != Json::Null);
            let choices: Vec<String> = f.get("choices").and_then(Json::as_arr).unwrap_or_default().iter().map(|c| c.get("name").and_then(Json::as_str).unwrap_or_default().to_string()).collect();
            let names = |j: Option<&Json>| -> Vec<String> { j.and_then(Json::as_arr).unwrap_or_default().iter().filter_map(|x| x.as_str().map(str::to_string)).collect() };
            let cond = |k: &str| f.get(k).filter(|c| **c != Json::Null).map(|c| (c.get("worked").and_then(Json::as_str).unwrap_or_default().to_string(), names(c.get("reads"))));
            let ty = match s("type").as_str() {
                "text" => Ty::Text,
                "long text" => Ty::Long,
                "date" => Ty::Date,
                "attachment" => Ty::Attachment,
                "person" | "team" => Ty::Person,
                "rows" => Ty::Rows,
                "bool" => Ty::Bool,
                t if t.starts_with("choice") && choices.is_empty() => Ty::Master,
                t if t.starts_with("choice") => Ty::Choice,
                _ => Ty::Number,
            };
            Field {
                name: s("name"),
                ty,
                min: f.get("min").cloned().unwrap_or(Json::Null),
                max: f.get("max").cloned().unwrap_or(Json::Null),
                count_min: f.get("count_min").and_then(Json::as_int),
                count_max: f.get("count_max").and_then(Json::as_int),
                choices,
                optional: yes("optional"),
                computed: yes("computed"),
                approvers: yes("approvers_only"),
                open_in: some("open_in"),
                shown_when: some("shown_when"),
                required_when: some("required_when"),
                shown: cond("shown_when"),
                required: cond("required_when"),
                reads: names(f.get("reads")),
                worked: yes("worked_on_screen"),
                fields: f.get("fields").map(fields_of).unwrap_or_default(),
            }
        })
        .collect()
}

/// What a field holds on a screen before anything is put in it, as the screens start a field.
pub(super) fn empty(f: &Field) -> Json {
    match f.ty {
        Ty::Rows | Ty::Choice => Json::arr([]),
        Ty::Bool => Json::Bool(false),
        _ => Json::Null,
    }
}

// ── Values, as the API carries them and as the reference takes them ──

/// A value of the API's JSON as the reference is given it, by its field's type: a value that is no
/// value of the type (a number with a fraction, a text for a number, a list for a text) is kept as a
/// text, which the reference refuses as it refuses a value of another type.
fn value_of(f: &Field, j: &Json) -> Value {
    match j {
        Json::Null => Value::None,
        Json::Bool(b) => Value::Bool(*b),
        Json::Int(n) => Value::Int(*n),
        Json::Str(s) => match f.ty {
            Ty::Date => Value::Date(s.clone()),
            Ty::Choice => Value::Enum(s.clone()),
            _ => Value::Str(s.clone()),
        },
        Json::Arr(rows) if f.ty == Ty::Rows => Value::List(rows.iter().map(|r| values_of(&f.fields, r)).collect()),
        other => Value::Str(other.compact()),
    }
}

/// The values of an object of the API's JSON, each by its field (a key no field has, as a text).
pub(super) fn values_of(fields: &[Field], j: &Json) -> Values {
    let pairs = j.as_obj().unwrap_or_default();
    pairs
        .iter()
        .map(|(k, v)| {
            let x = match fields.iter().find(|f| f.name == *k) {
                Some(f) => value_of(f, v),
                None => Value::Str(v.compact()),
            };
            (k.clone(), x)
        })
        .collect()
}

fn json_of(v: &Value) -> Json {
    match v {
        Value::Bool(b) => Json::Bool(*b),
        Value::Int(n) => Json::Int(*n),
        Value::Str(s) | Value::Date(s) | Value::Enum(s) => Json::str(s),
        Value::List(rows) => Json::arr(rows.iter().map(|r| Json::obj(r.iter().map(|(k, x)| (k.clone(), json_of(x)))))),
        Value::None => Json::Null,
    }
}

/// A ticket's or a request's form as the API gives it.
fn form_json(v: &Values) -> Json {
    Json::obj(v.iter().map(|(k, x)| (k.clone(), json_of(x))))
}

/// A value as a screen holds it: a choice a ChoicePicker writes as a list of one, in a row too.
pub(super) fn held(f: &Field, v: &Json) -> Json {
    match (f.ty, v) {
        (Ty::Choice, Json::Str(_)) => Json::arr([v.clone()]),
        (Ty::Rows, Json::Arr(rows)) => Json::arr(rows.iter().map(|r| match r {
            Json::Obj(kv) => Json::obj(kv.iter().map(|(k, x)| (k.clone(), f.fields.iter().find(|c| c.name == *k).map(|c| held(c, x)).unwrap_or_else(|| x.clone())))),
            other => other.clone(),
        })),
        _ => v.clone(),
    }
}

/// The API's form `api` as a screen holds it, over what the screen starts with (`start`).
fn held_form(fields: &[Field], start: &Json, api: &Json) -> Json {
    let mut out = start.clone();
    for (k, v) in api.as_obj().unwrap_or_default() {
        if *v == Json::Null {
            continue;
        }
        let x = fields.iter().find(|f| f.name == *k).map(|f| held(f, v)).unwrap_or_else(|| v.clone());
        put(&mut out, k, x);
    }
    out
}

fn put(j: &mut Json, key: &str, v: Json) {
    let Json::Obj(pairs) = j else { panic!("not an object: {}", j.compact()) };
    match pairs.iter_mut().find(|(k, _)| k == key) {
        Some((_, x)) => *x = v,
        None => pairs.push((key.to_string(), v)),
    }
}

fn without(j: &Json, key: &str) -> Json {
    Json::obj(j.as_obj().unwrap_or_default().iter().filter(|(k, _)| k != key).cloned())
}

/// A JSON value written with every object's keys in order and its nulls left out, to compare two
/// forms: a value of none is no value (DESIGN 10.4).
pub(super) fn canon(j: &Json) -> String {
    fn sorted(j: &Json) -> Json {
        match j {
            Json::Obj(kv) => {
                let mut kv: Vec<(String, Json)> = kv.iter().filter(|(_, v)| *v != Json::Null).map(|(k, v)| (k.clone(), sorted(v))).collect();
                kv.sort_by(|a, b| a.0.cmp(&b.0));
                Json::Obj(kv)
            }
            Json::Arr(xs) => Json::arr(xs.iter().map(sorted)),
            other => other.clone(),
        }
    }
    sorted(j).compact()
}

/// The values of `form` of the fields `keep` names.
fn only(form: &Json, keep: &dyn Fn(&str) -> bool) -> Json {
    Json::obj(form.as_obj().unwrap_or_default().iter().filter(|(k, _)| keep(k)).cloned())
}

// ── What the forms are built on ──

#[derive(Clone, Copy, Debug, PartialEq)]
enum Flow {
    Submit,
    Resubmit,
    Edit,
}

impl Flow {
    /// The screen a flow is on, and the button that makes its event.
    fn screen(self) -> (&'static str, &'static str) {
        match self {
            Flow::Submit => ("form", "button.submit"),
            Flow::Resubmit => ("form", "button.resubmit"),
            Flow::Edit => ("card", "button.edit"),
        }
    }
}

/// A transition the reference takes, which forms are built on: a request made, one made again after
/// it was sent back, an approver's edit; the ticket before it, who makes it and when, the form as
/// the API gives it (the request's, or the ticket's), and the values the event gives.
pub(super) struct Seed {
    flow: Flow,
    from: String,
    before: Option<Ticket>,
    id: String,
    requester: String,
    by: String,
    now: i64,
    pub(super) form: Json,
    pub(super) gives: Json,
}

/// Every transition the vectors of the kind make that the reference takes, of the three flows: a
/// request made by the requester themselves, made again, edited.
pub(super) fn seeds(m: &Machine, org: &Org) -> Vec<Seed> {
    let rans: Vec<_> = m.s.file.scenarios.iter().map(|sc| scenario::run(m, org, sc)).collect();
    let mut out = Vec::new();
    for v in ringi::vectors::make(m, org, &rans) {
        let s = v.stepped;
        if s.answer.is_err() {
            continue;
        }
        let (flow, by, gives) = match (&s.input, &s.before) {
            (Input::Submit(sub), None) if sub.by.is_none() => {
                let form = form_json(&sub.form);
                out.push(Seed { flow: Flow::Submit, from: v.from.clone(), before: None, id: sub.id.clone(), requester: sub.requester.clone(), by: sub.requester.clone(), now: s.now, form: form.clone(), gives: form });
                continue;
            }
            (Input::Event(Event::Resubmit { by, form, .. }), Some(_)) => (Flow::Resubmit, by.clone(), form_json(form)),
            (Input::Event(Event::Edit { by, fields, .. }), Some(_)) => (Flow::Edit, by.clone(), form_json(fields)),
            _ => continue,
        };
        let t = s.before.unwrap();
        out.push(Seed { flow, from: v.from, id: t.id.clone(), requester: t.requester.clone(), by, now: s.now, form: form_json(&t.form), gives, before: Some(t) });
    }
    out
}

/// A ticket sent back to its requester again after the request made again (the first the vectors
/// make) gave a value to every field it takes one for that held none, sent back by the one who sent
/// it back before: what the forms that empty such a field, made again, are built on.
fn filled_again(m: &Machine, org: &Org, fields: &[Field], seeds: &[Seed]) -> Option<Seed> {
    let s = seeds.iter().find(|x| x.flow == Flow::Resubmit)?;
    let back_by = s.before.as_ref()?.sent_back_by.clone()?;
    let mut give: Vec<(String, Json)> = Vec::new();
    for f in fields.iter().filter(|f| f.given() && f.ty != Ty::Rows && s.form.get(&f.name).is_none_or(|v| *v == Json::Null)) {
        let Some(v) = sample(f, seeds) else { continue };
        let mut more = give.clone();
        more.push((f.name.clone(), v));
        if answer(m, org, fields, s, &Json::obj([("form", Json::obj(more.clone()))])).is_ok() {
            give = more;
        }
    }
    if give.is_empty() {
        return None;
    }
    let again = answer(m, org, fields, s, &Json::obj([("form", Json::obj(give))])).ok()?;
    let back = scenario::step_seen(m, org, Some(&again), Input::Event(Event::SendBack { by: back_by, to: None }), None, Some(again.revision()), s.now).ok()?;
    let (t, _) = back.answer.ok()?;
    let form = form_json(&t.form);
    Some(Seed { flow: Flow::Resubmit, from: format!("{}, made again with every field it takes given and sent back", s.from), id: t.id.clone(), requester: t.requester.clone(), by: s.by.clone(), now: s.now, form, gives: Json::Obj(Vec::new()), before: Some(t) })
}

/// A ticket an approver's edit (the first the vectors make) has given a value to every field the
/// approvers give or correct that held none: what the forms that empty such a field are built on.
fn filled(m: &Machine, org: &Org, fields: &[Field], edited: &[String], seeds: &[Seed]) -> Option<Seed> {
    let s = seeds.iter().find(|x| x.flow == Flow::Edit)?;
    let empty_now = |f: &&Field| s.form.get(&f.name).is_none_or(|v| *v == Json::Null);
    let give: Vec<(String, Json)> = fields.iter().filter(|f| !f.computed && (f.approvers || edited.contains(&f.name))).filter(empty_now).filter_map(|f| Some((f.name.clone(), sample(f, seeds)?))).collect();
    if give.is_empty() {
        return None;
    }
    let t = answer(m, org, fields, s, &Json::obj([("fields", Json::obj(give))])).unwrap_or_else(|e| panic!("{}: an edit that fills every field of the approvers' is refused: {e}", s.from));
    let form = form_json(&t.form);
    Some(Seed { flow: Flow::Edit, from: format!("{}, every field of the approvers' given", s.from), id: t.id.clone(), requester: t.requester.clone(), by: s.by.clone(), now: s.now, form, gives: Json::Obj(Vec::new()), before: Some(t) })
}

/// What the reference answers to what a button sends (`sent`, the event's context) on `seed`: the
/// ticket after it, or the kind of the refusal and its words.
fn answer(m: &Machine, org: &Org, fields: &[Field], seed: &Seed, sent: &Json) -> Result<Ticket, String> {
    let key = if seed.flow == Flow::Edit { "fields" } else { "form" };
    let given = values_of(fields, sent.get(key).unwrap_or(&Json::Null));
    let clear: Vec<String> = sent.get("clear").and_then(Json::as_arr).unwrap_or_default().iter().filter_map(|c| c.as_str().map(str::to_string)).collect();
    let comment = sent.get("comment").and_then(Json::as_str).map(str::to_string);
    let input = match seed.flow {
        Flow::Submit => {
            let requester = sent.get("requester").and_then(Json::as_str).unwrap_or(&seed.requester).to_string();
            Input::Submit(Submit { id: seed.id.clone(), requester, by: None, form: given, attempt: "attempt-1".into() })
        }
        Flow::Resubmit => Input::Event(Event::Resubmit { by: seed.by.clone(), form: given, clear, attempt: "attempt-2".into() }),
        Flow::Edit => Input::Event(Event::Edit { by: seed.by.clone(), fields: given, clear }),
    };
    let seen = seed.before.as_ref().map(Ticket::revision);
    let s = scenario::step_seen(m, org, seed.before.as_ref(), input, comment, seen, seed.now).map_err(|t| format!("no step: {}", t.en))?;
    s.answer.map(|(t, _)| t).map_err(|r| format!("{}: {}", r.kind, r.text.en))
}

// ── The forms ──

/// Where a text with white space at its ends is put in what the button sends, to give the reference
/// the text as it is written: a field, or a field of the first row of rows.
struct Written {
    field: String,
    rows: Option<String>,
    text: Json,
}

/// A form of one kind of check: the seed it is built on, the data the screen is given in place of
/// what it starts with (the form, and what the flow's screen is given beside it), and, for a text
/// with white space at its ends, the text as written.
struct Form {
    check: &'static str,
    label: String,
    seed: usize,
    data: Json,
    written: Option<Written>,
}

/// A value of `f` the reference takes in a form that shows it: what a form or an event of the
/// vectors gives it, else one made from its description.
pub(super) fn sample(f: &Field, seeds: &[Seed]) -> Option<Json> {
    let given = seeds.iter().find_map(|s| [&s.form, &s.gives].into_iter().find_map(|j| j.get(&f.name).filter(|v| **v != Json::Null).cloned()));
    given.or_else(|| match f.ty {
        Ty::Text | Ty::Long => Some(Json::str("b".repeat(f.count_min.unwrap_or(1).max(1) as usize))),
        Ty::Number | Ty::Date => Some(f.min.clone()),
        Ty::Choice => f.choices.first().map(Json::str),
        Ty::Attachment => Some(Json::str("file-1.pdf")),
        Ty::Bool => Some(Json::Bool(true)),
        Ty::Master | Ty::Person | Ty::Rows => None,
    })
}

/// A day `n` days after the day written `d`.
fn day_after(d: &str, n: i64) -> String {
    let day = ritsu_ports::day_of(d).unwrap_or_else(|| panic!("{d} is no day"));
    ringi::lex::date_text(day + n)
}

/// The values that break a check of `f` alone and the ones that just pass it, each with its kind of
/// check and its words: the length at both ends and past them, the range at both ends and past them
/// (a day that does not exist, a day with a time of day, a number written as a text, with a comma,
/// with a fraction), a value none of the choices has and each one it has, and the white space at a
/// text's ends. `current` is the value the form holds. The ends of a day the calendar is to be open
/// on are not asked of the screen: which days are open is the server's to know.
fn values_for(f: &Field, current: &Json) -> Vec<(&'static str, String, Json)> {
    let mut out: Vec<(&'static str, String, Json)> = Vec::new();
    let a = |x: i128| "a".repeat(x.max(0) as usize);
    match f.ty {
        Ty::Text | Ty::Long => {
            if let Some(hi) = f.count_max {
                out.push(("length", format!("{hi} characters"), Json::str(a(hi))));
                out.push(("length", format!("{hi} characters outside the BMP"), Json::str("😀".repeat(hi as usize))));
                out.push(("length", format!("{} characters", hi + 1), Json::str(a(hi + 1))));
                out.push(("white space", format!("{hi} characters within white space"), Json::str(format!(" \u{3000}{}\n", a(hi)))));
            }
            if let Some(lo) = f.count_min.filter(|n| *n > 1) {
                out.push(("length", format!("{lo} characters"), Json::str(a(lo))));
                out.push(("length", format!("{} characters", lo - 1), Json::str(a(lo - 1))));
                out.push(("white space", format!("{} characters within white space", lo - 1), Json::str(format!(" {} ", a(lo - 1)))));
            }
            out.push(("white space", "only white space".into(), Json::str(" \u{3000}\t")));
        }
        Ty::Number => {
            let (lo, hi) = (f.min.as_int(), f.max.as_int());
            if let Some(lo) = lo {
                out.push(("range", format!("the least ({lo})"), Json::Int(lo)));
                out.push(("range", format!("one below the least ({})", lo - 1), Json::Int(lo - 1)));
            }
            if let Some(hi) = hi {
                out.push(("range", format!("the most ({hi})"), Json::Int(hi)));
                out.push(("range", format!("one past the most ({})", hi + 1), Json::Int(hi + 1)));
            }
            if let Some(v) = current.as_int().or(lo) {
                out.push(("type", format!("{v} written as a text"), Json::str(v.to_string())));
                out.push(("type", format!("{v} written with a comma"), Json::str(format!("{},{:03}", v / 1000, (v % 1000).abs()))));
                out.push(("type", format!("{v} and a half"), Json::Frac(format!("{v}.5"))));
            }
        }
        Ty::Date => {
            let (lo, hi) = (f.min.as_str().map(str::to_string), f.max.as_str().map(str::to_string));
            if !f.open_in {
                if let Some(lo) = &lo {
                    out.push(("range", format!("the first day ({lo})"), Json::str(lo)));
                    out.push(("range", "the day before the first".into(), Json::str(day_after(lo, -1))));
                }
                if let Some(hi) = &hi {
                    out.push(("range", format!("the last day ({hi})"), Json::str(hi)));
                    out.push(("range", "the day after the last".into(), Json::str(day_after(hi, 1))));
                }
            }
            let year = current.as_str().or(lo.as_deref()).map(|d| d[..4].to_string()).unwrap_or_else(|| "2026".into());
            out.push(("type", format!("{year}-02-30, a day that does not exist"), Json::str(format!("{year}-02-30"))));
            if let Some(d) = current.as_str() {
                out.push(("type", format!("{d} with a time of day"), Json::str(format!("{d}T09:30:00"))));
            }
        }
        Ty::Choice => {
            out.push(("choice", "a value none of the choices has".into(), Json::str("not_a_choice")));
            for c in &f.choices {
                out.push(("choice", format!("`{c}`"), Json::str(c)));
            }
        }
        Ty::Attachment => out.push(("type", "another file".into(), Json::str("file-2.pdf"))),
        _ => {}
    }
    out
}

/// The forms of a request made: on the requests the vectors make, for each field the requester
/// gives, its value left empty where the form asks for it and where it does not, given while the
/// field is hidden, and each value of [`values_for`]; the rows at both ends of their count and past
/// them, a row that is null, each field of a row left out of a row and given each value of
/// [`values_for`]; and each computation of a column of rows at the ends of its range and past them,
/// where rows reach them. `takes` is whether the reference takes a request made with an API form.
fn request_forms(fields: &[Field], seeds: &[Seed], start: &Json, takes: &dyn Fn(usize, &Json) -> bool) -> Vec<Form> {
    let ours: Vec<usize> = (0..seeds.len()).filter(|i| seeds[*i].flow == Flow::Submit).collect();
    let mut out = Vec::new();
    let data = |form: Json| Json::obj([("form", form), ("can", Json::obj([("submit", Json::Bool(true))])), ("requester", Json::Null), ("comment", Json::str(""))]);
    let holding = |api: &Json| held_form(fields, start, api);
    let Some(&first) = ours.first() else { return out };
    out.push(Form { check: "as the vectors make it", label: seeds[first].from.clone(), seed: first, data: data(holding(&seeds[first].form)), written: None });
    for f in fields.iter().filter(|f| f.given()) {
        let has = |s: usize| seeds[s].form.get(&f.name).is_some_and(|v| *v != Json::Null);
        // left empty where the form asks for it, and where it does not
        let asked = ours.iter().copied().find(|s| has(*s) && !takes(*s, &without(&seeds[*s].form, &f.name)));
        let not_asked = ours.iter().copied().find(|s| !has(*s) || takes(*s, &without(&seeds[*s].form, &f.name)));
        let check = match (f.required_when, f.shown_when, f.optional) {
            (true, _, _) => "required when",
            (_, true, false) => "required while shown",
            (_, _, true) => "optional",
            _ => "required",
        };
        for (s, side) in [(asked, "where it is asked for"), (not_asked, "where it is not")] {
            let Some(s) = s else { continue };
            let mut form = holding(&without(&seeds[s].form, &f.name));
            put(&mut form, &f.name, empty(f));
            out.push(Form { check, label: format!("{} left empty {side} ({})", f.name, seeds[s].from), seed: s, data: data(form), written: None });
        }
        // given a value while the form hides it
        if f.shown_when
            && let Some(v) = sample(f, seeds)
        {
            let given = |s: usize| {
                let mut api = seeds[s].form.clone();
                put(&mut api, &f.name, v.clone());
                api
            };
            if let Some(s) = ours.iter().copied().find(|s| !has(*s) && !takes(*s, &given(*s))) {
                out.push(Form { check: "hidden with a value", label: format!("{} given while it is hidden ({})", f.name, seeds[s].from), seed: s, data: data(holding(&given(s))), written: None });
            }
        }
        // each value, on a request that shows it
        let Some(s) = ours.iter().copied().find(|s| has(*s)) else { continue };
        for (check, label, v) in values_for(f, seeds[s].form.get(&f.name).unwrap_or(&Json::Null)) {
            let mut form = holding(&seeds[s].form);
            put(&mut form, &f.name, held(f, &v));
            let written = (check == "white space").then(|| Written { field: f.name.clone(), rows: None, text: v.clone() });
            out.push(Form { check, label: format!("{} {label}", f.name), seed: s, data: data(form), written });
        }
        if f.ty == Ty::Rows {
            out.extend(row_forms(f, s, seeds, &|rows: Vec<Json>| {
                let mut form = holding(&seeds[s].form);
                put(&mut form, &f.name, held(f, &Json::arr(rows)));
                data(form)
            }));
            out.extend(computed_forms(fields, f, s, seeds, &|rows: Vec<Json>| {
                let mut form = holding(&seeds[s].form);
                put(&mut form, &f.name, held(f, &Json::arr(rows)));
                data(form)
            }));
        }
    }
    out
}

/// The forms of the rows `f` on the request `s`: their count at both ends and past them, a row that
/// is null, and each field of a row left out of the first row and of a second, and given each value
/// of [`values_for`] in the first. `with` is the screen's data with the rows given.
fn row_forms(f: &Field, s: usize, seeds: &[Seed], with: &dyn Fn(Vec<Json>) -> Json) -> Vec<Form> {
    let mut out = Vec::new();
    let rows: Vec<Json> = seeds[s].form.get(&f.name).and_then(Json::as_arr).unwrap_or_default().to_vec();
    let first = rows.first().cloned().unwrap_or(Json::Obj(Vec::new()));
    let mut counts = Vec::new();
    if let Some(lo) = f.count_min {
        counts.extend([lo, lo - 1]);
    }
    if let Some(hi) = f.count_max {
        counts.extend([hi, hi + 1]);
    }
    for c in counts.into_iter().filter(|c| *c >= 0) {
        out.push(Form { check: "count of rows", label: format!("{} with {c} rows", f.name), seed: s, data: with(vec![first.clone(); c as usize]), written: None });
    }
    let mut with_null = rows.clone();
    with_null.push(Json::Null);
    out.push(Form { check: "count of rows", label: format!("{} with a row that is null", f.name), seed: s, data: with(with_null), written: None });
    for c in f.fields.iter().filter(|c| c.given()) {
        let in_row = |i: usize, v: Option<&Json>| {
            let mut rows = rows.clone();
            while rows.len() <= i {
                rows.push(first.clone());
            }
            rows[i] = match v {
                Some(v) => {
                    let mut r = rows[i].clone();
                    put(&mut r, &c.name, v.clone());
                    r
                }
                None => without(&rows[i], &c.name),
            };
            rows
        };
        let check = if c.optional || c.ty == Ty::Attachment { "optional in a row" } else { "required in a row" };
        out.push(Form { check, label: format!("{}.{} left out of the first row", f.name, c.name), seed: s, data: with(in_row(0, None)), written: None });
        if f.count_max.is_none_or(|hi| hi >= 2) {
            out.push(Form { check, label: format!("{}.{} left out of a second row", f.name, c.name), seed: s, data: with(in_row(1, None)), written: None });
        }
        for (check, label, v) in values_for(c, first.get(&c.name).unwrap_or(&Json::Null)) {
            let check = match check {
                "white space" => "white space",
                "length" => "length in a row",
                "range" => "range in a row",
                "choice" => "choice in a row",
                _ => "type in a row",
            };
            let written = (check == "white space").then(|| Written { field: c.name.clone(), rows: Some(f.name.clone()), text: v.clone() });
            out.push(Form { check, label: format!("{}.{} {label}", f.name, c.name), seed: s, data: with(in_row(0, Some(&v))), written });
        }
    }
    out
}

/// The forms of each computation a column of the rows `f` may add up to: rows whose column comes
/// to each end of the computation's range and past it, where some count of rows reaches it within
/// the column's range. A computation of another shape is given the same rows, which the two must
/// answer the same all the same.
fn computed_forms(fields: &[Field], f: &Field, s: usize, seeds: &[Seed], with: &dyn Fn(Vec<Json>) -> Json) -> Vec<Form> {
    let mut out = Vec::new();
    let first = seeds[s].form.get(&f.name).and_then(Json::as_arr).and_then(|r| r.first()).cloned().unwrap_or(Json::Obj(Vec::new()));
    let (cmin, cmax) = (f.count_min.unwrap_or(0).max(1), f.count_max.unwrap_or(20));
    for h in fields.iter().filter(|h| h.computed && h.ty == Ty::Number) {
        let (Some(lo), Some(hi)) = (h.min.as_int(), h.max.as_int()) else { continue };
        for c in f.fields.iter().filter(|c| c.given() && c.ty == Ty::Number) {
            let (Some(a), Some(b)) = (c.min.as_int(), c.max.as_int()) else { continue };
            for (what, total) in [("the least", lo), ("one below the least", lo - 1), ("the most", hi), ("one past the most", hi + 1)] {
                let Some(n) = (cmin..=cmax).find(|n| n * a <= total && total <= n * b) else { continue };
                let mut left = total - n * a;
                let rows: Vec<Json> = (0..n)
                    .map(|_| {
                        let more = left.min(b - a);
                        left -= more;
                        let mut r = first.clone();
                        put(&mut r, &c.name, Json::Int(a + more));
                        r
                    })
                    .collect();
                out.push(Form { check: "computed range", label: format!("{} at {what} ({total}) by {n} rows of {}.{}", h.name, f.name, c.name), seed: s, data: with(rows), written: None });
            }
        }
    }
    out
}

/// The forms of a request made again, on the first the vectors make and on the ticket
/// [`filled_again`] makes (the last seed of a request made again): as it was sent back, each field
/// that holds a value emptied, each choice changed to each of its values (the fields its conditions
/// show and hide with it), each field given a new value, and each field shown on a condition given a
/// new value while a choice is changed to each value, hiding it or not (a field that holds a value
/// and is hidden the screen sends in `clear`).
fn again_forms(fields: &[Field], seeds: &[Seed], start: &Json) -> Vec<Form> {
    let mut out = Vec::new();
    let ours: Vec<usize> = (0..seeds.len()).filter(|i| seeds[*i].flow == Flow::Resubmit).collect();
    let (Some(&first), Some(&last)) = (ours.first(), ours.last()) else { return out };
    let choices: Vec<&Field> = fields.iter().filter(|f| f.given() && f.ty == Ty::Choice).collect();
    let can = Json::obj([("submit", Json::Bool(false)), ("resubmit", Json::Bool(true)), ("discard", Json::Bool(false))]);
    for s in if first == last { vec![first] } else { vec![first, last] } {
        let on = if s == first { String::new() } else { ", every field it takes holding a value".to_string() };
        let before = &seeds[s].form;
        let ticket = Json::obj([("id", Json::str(&seeds[s].id)), ("revision", Json::str("seen")), ("sent_back_by", Json::Null)]);
        let data = |form: Json| Json::obj([("form", form), ("before", before.clone()), ("ticket", ticket.clone()), ("can", can.clone()), ("requester", Json::Null), ("comment", Json::str(""))]);
        let holding = |api: &Json| held_form(fields, start, api);
        out.push(Form { check: "made again", label: format!("as it was sent back ({})", seeds[s].from), seed: s, data: data(holding(before)), written: None });
        for f in fields.iter().filter(|f| f.given()) {
            if before.get(&f.name).is_some_and(|v| *v != Json::Null) {
                let mut form = holding(before);
                put(&mut form, &f.name, empty(f));
                out.push(Form { check: "made again, emptied", label: format!("{} emptied{on}", f.name), seed: s, data: data(form), written: None });
            }
            if f.ty == Ty::Choice {
                for c in &f.choices {
                    let mut form = holding(before);
                    put(&mut form, &f.name, Json::arr([Json::str(c)]));
                    out.push(Form { check: "made again, a choice changed", label: format!("{} changed to `{c}`{on}", f.name), seed: s, data: data(form), written: None });
                }
            }
            if f.shown_when
                && let Some(v) = new_value(f, before)
            {
                for g in &choices {
                    for c in &g.choices {
                        let mut form = holding(before);
                        put(&mut form, &f.name, held(f, &v));
                        put(&mut form, &g.name, Json::arr([Json::str(c)]));
                        out.push(Form { check: "made again, hidden with a value", label: format!("{} given a new value, {} changed to `{c}`{on}", f.name, g.name), seed: s, data: data(form), written: None });
                    }
                }
            }
            if f.ty != Ty::Rows
                && let Some(v) = new_value(f, before)
            {
                let mut form = holding(before);
                put(&mut form, &f.name, held(f, &v));
                out.push(Form { check: "made again, a value changed", label: format!("{} changed{on}", f.name), seed: s, data: data(form), written: None });
            }
        }
    }
    out
}

/// A value of `f` the reference takes that is not the one `before` holds: a text of another letter,
/// the other end of a range, another file.
fn new_value(f: &Field, before: &Json) -> Option<Json> {
    let now = before.get(&f.name).cloned().unwrap_or(Json::Null);
    let v = match f.ty {
        Ty::Text | Ty::Long => Json::str("c".repeat(f.count_min.unwrap_or(1).max(1) as usize)),
        Ty::Number => {
            if now == f.max { f.min.clone() } else { f.max.clone() }
        }
        Ty::Date if !f.open_in => {
            if now == f.max { f.min.clone() } else { f.max.clone() }
        }
        Ty::Attachment => Json::str("file-3.pdf"),
        _ => return None,
    };
    (v != now && v != Json::Null).then_some(v)
}

/// The forms of an approver's edit, on the first the vectors make and on the ticket [`filled`] makes
/// (the last seed of an edit): as the server fills it (each field the approvers give or correct
/// holding the ticket's value), each such field emptied, given a new value and each value of
/// [`values_for`], and each one shown on a condition given a new value while a choice the approvers
/// edit is changed to each value.
fn edit_forms(fields: &[Field], edited: &[String], seeds: &[Seed]) -> Vec<Form> {
    let mut out = Vec::new();
    let ours: Vec<usize> = (0..seeds.len()).filter(|i| seeds[*i].flow == Flow::Edit).collect();
    let (Some(&first), Some(&last)) = (ours.first(), ours.last()) else { return out };
    let theirs: Vec<&Field> = fields.iter().filter(|f| !f.computed && (f.approvers || edited.contains(&f.name))).collect();
    let choices: Vec<&&Field> = theirs.iter().filter(|f| f.ty == Ty::Choice).collect();
    for s in if first == last { vec![first] } else { vec![first, last] } {
        let on = if s == first { String::new() } else { ", every field of the approvers' holding a value".to_string() };
        let before = &seeds[s].form;
        // `/edit` as the server fills it: the ticket's values of the fields, as the inputs hold them
        let filled = Json::obj(theirs.iter().map(|f| (f.name.clone(), before.get(&f.name).filter(|v| **v != Json::Null).map(|v| held(f, v)).unwrap_or_else(|| empty(f)))));
        let ticket = Json::obj([("id", Json::str(&seeds[s].id)), ("revision", Json::str("seen")), ("status", Json::str("in_progress")), ("stage", Json::Null)]);
        let data = |edit: Json| Json::obj([("form", before.clone()), ("edit", edit), ("ticket", ticket.clone()), ("can", Json::obj([("edit", Json::Bool(true))])), ("comment", Json::str(""))]);
        out.push(Form { check: "edited", label: format!("as the server fills it ({})", seeds[s].from), seed: s, data: data(filled.clone()), written: None });
        for f in &theirs {
            let with = |v: Json| {
                let mut e = filled.clone();
                put(&mut e, &f.name, v);
                e
            };
            out.push(Form { check: "edited, emptied", label: format!("{} emptied{on}", f.name), seed: s, data: data(with(empty(f))), written: None });
            if let Some(v) = new_value(f, before).or_else(|| sample(f, seeds)) {
                out.push(Form { check: "edited, a value given", label: format!("{} given a value{on}", f.name), seed: s, data: data(with(held(f, &v))), written: None });
                if f.shown_when {
                    for g in &choices {
                        for c in &g.choices {
                            let mut e = with(held(f, &v));
                            put(&mut e, &g.name, Json::arr([Json::str(c)]));
                            out.push(Form { check: "edited, hidden with a value", label: format!("{} given a value, {} changed to `{c}`{on}", f.name, g.name), seed: s, data: data(e), written: None });
                        }
                    }
                }
            }
            for (check, label, v) in values_for(f, before.get(&f.name).unwrap_or(&Json::Null)) {
                let check = match check {
                    "white space" => "white space",
                    "choice" => "edited, a choice changed",
                    _ => "edited, a value given",
                };
                let written = (check == "white space").then(|| Written { field: f.name.clone(), rows: None, text: v.clone() });
                out.push(Form { check, label: format!("{} {label}{on}", f.name), seed: s, data: data(with(held(f, &v))), written });
            }
        }
    }
    out
}

// ── The screens, read as a renderer reads them ──

/// Each form read on its screen, in Node: what the driver prints for each, one JSON a line.
fn read_screens(m: &Made, kind: &str, screens: &Json, forms: &[Form], seeds: &[Seed]) -> Vec<Json> {
    let t = ritsu_testkit::TempDir::new("ringi-a2ui-forms");
    t.write("functions.js", file(m, "a2ui/functions.js"));
    t.write("package.json", "{\"type\": \"module\"}\n");
    let list = Json::arr(forms.iter().map(|f| {
        let (screen, button) = seeds[f.seed].flow.screen();
        Json::obj([("screen", Json::str(screen)), ("button", Json::str(button)), ("data", f.data.clone())])
    }));
    t.write("forms.json", Json::obj([("kind", Json::str(kind)), ("screens", screens.clone()), ("forms", list)]).compact());
    t.write("driver.mjs", DRIVER);
    let r = ritsu_testkit::run::finish(Command::new("node").arg(t.path().join("driver.mjs")).arg(t.path()), Duration::from_secs(600), "node, reading the screens on the forms");
    assert!(r.ok, "{}", r.both());
    let out: Vec<Json> = r.stdout.lines().map(|l| json::parse(l).unwrap_or_else(|e| panic!("{e:?}: {l}"))).collect();
    assert_eq!(out.len(), forms.len(), "{kind}: {}", r.both());
    out
}

/// What reads a screen as a renderer does, for each form: the parts drawn from `root` (a `Show`
/// draws its child while `when` holds; a `LineItems` its row for each row of its value, a `List`
/// for each of its path's, each read in its row), every property of each part drawn read, the
/// checks of each part drawn and of the button pressed (not of the others), and what that button
/// sends; the form as the screen shows it (`given`), and each format of the basic catalog given a
/// null. The basic catalog's functions are as its catalog describes them.
const DRIVER: &str = r#"import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { join } from "node:path";

const dir = process.argv[2];
const { functions } = await import(pathToFileURL(join(dir, "functions.js")).href);
const { kind, screens, forms } = JSON.parse(readFileSync(join(dir, "forms.json"), "utf8"));
const read = {};
for (const [name, doc] of Object.entries(screens)) {
  const data = doc.messages.find((m) => m.updateDataModel).updateDataModel.value;
  const parts = doc.messages.find((m) => m.updateComponents).updateComponents.components;
  const surface = doc.messages.find((m) => m.createSurface).createSurface.surfaceId;
  read[name] = { data, surface, byId: Object.fromEntries(parts.map((c) => [c.id, c])) };
}

const empty = (v) => v === null || v === undefined || v === "" || (Array.isArray(v) && v.length === 0);
const holds = (v) => v !== false && !empty(v);

function at(model, path, scope) {
  if (!path.startsWith("/")) {
    if (!scope) throw new Error(`the path ${path} is read outside a row`);
    return path === "" || path === "." ? scope.item : at(scope.item, "/" + path);
  }
  let x = model;
  for (const seg of path.split("/").slice(1)) {
    if (x === null || x === undefined) return undefined;
    x = x[seg.replace(/~1/g, "/").replace(/~0/g, "~")];
  }
  return x;
}

function readForm(form) {
  const s = read[form.screen];
  const model = Object.assign(structuredClone(s.data), form.data);
  const nulls = [];
  const failed = [];
  const drawn = new Set();
  const formatted = (name) => (args) => {
    if (args.value === null || args.value === undefined) nulls.push(name);
    return `${name}(${JSON.stringify(args.value)})`;
  };
  const basic = {
    required: ({ value }) => !empty(value),
    or: ({ values }) => values.some((v) => v === true),
    and: ({ values }) => values.every((v) => v === true),
    not: ({ value }) => value !== true,
    formatCurrency: formatted("formatCurrency"),
    formatNumber: formatted("formatNumber"),
    formatDate: formatted("formatDate"),
    formatString: formatted("formatString"),
    openUrl: () => null,
  };
  const value = (x, scope) => {
    if (x === null || typeof x !== "object") return x;
    if (Array.isArray(x)) return x.map((y) => value(y, scope));
    if (typeof x["@path"] === "string") return at(model, x["@path"], scope);
    if (typeof x["@call"] === "string") {
      if (x["@call"] === "@index") {
        if (!scope) throw new Error("@index is read outside a row");
        return scope.index;
      }
      const f = functions[x["@call"]] || basic[x["@call"]];
      if (!f) throw new Error(`no function ${x["@call"]}`);
      return f(Object.fromEntries(Object.entries(x.args || {}).map(([k, v]) => [k, value(v, scope)])));
    }
    return Object.fromEntries(Object.entries(x).map(([k, v]) => [k, value(v, scope)]));
  };
  const structure = new Set(["id", "component", "child", "children", "action", "@checks"]);
  const walk = (id, scope) => {
    const c = s.byId[id];
    if (!c) throw new Error(`no part ${id}`);
    if (c.component === "Show") {
      if (holds(value(c.when, scope))) walk(c.child, scope);
      return;
    }
    drawn.add(id);
    for (const [k, v] of Object.entries(c)) if (!structure.has(k)) value(v, scope);
    if (c.component !== "Button" || id === form.button) {
      for (const ch of c["@checks"] || []) {
        if (value(ch.condition, scope) !== true) failed.push(`${id}${scope ? `[${scope.index}]` : ""}: ${ch.message}`);
      }
    }
    if (c.component === "LineItems") {
      const rows = value(c.value, scope);
      (Array.isArray(rows) ? rows : []).forEach((item, index) => walk(c.child, { item, index }));
      return;
    }
    if (c.children && !Array.isArray(c.children)) {
      const rows = at(model, c.children.path, scope);
      (Array.isArray(rows) ? rows : []).forEach((item, index) => walk(c.children.componentId, { item, index }));
      return;
    }
    if (typeof c.child === "string") walk(c.child, scope);
    if (Array.isArray(c.children)) c.children.forEach((x) => walk(x, scope));
  };
  walk("root", undefined);
  if (!drawn.has(form.button)) failed.push(`${form.button} is not drawn`);
  const sent = value(s.byId[form.button].action.event.context, undefined);
  const shown = form.screen === "card" ? functions[`${kind}.edited`]({ edit: model.edit, before: model.form }) : model.form;
  // what a renderer sends back for the button (A2UI's renderer_to_agent.json)
  const action = { name: s.byId[form.button].action.event.name, surfaceId: s.surface, sourceComponentId: form.button, timestamp: "2026-11-17T09:00:00Z", context: sent };
  return { failed, sent, action, given: functions[`${kind}.given`]({ form: shown }), nulls };
}

for (const form of forms) {
  let line;
  try {
    line = readForm(form);
  } catch (e) {
    line = { threw: String(e && e.stack ? e.stack : e) };
  }
  console.log(JSON.stringify(line));
}
"#;

// ── The two, held to each other ──

/// The forms of a kind of check: how many there are, how many both stop, how many the two answer
/// apart, and how many they would answer apart if the reference were given each text as it is
/// written, with the white space at its ends.
#[derive(Clone, Copy, Default)]
struct Count {
    forms: usize,
    stopped: usize,
    apart: usize,
    spaces: usize,
}

impl Count {
    fn add(self, o: Count) -> Count {
        Count { forms: self.forms + o.forms, stopped: self.stopped + o.stopped, apart: self.apart + o.apart, spaces: self.spaces + o.spaces }
    }

    fn cell(&self) -> String {
        format!("{}/{}/{}/{}", self.forms, self.stopped, self.apart, self.spaces)
    }
}

/// What is found of the forms of a kind: their counts by the kind of check, and what parts, said.
#[derive(Default)]
struct Found {
    counts: BTreeMap<&'static str, Count>,
    apart: Vec<String>,
    /// The messages the buttons made, each held to `renderer_to_agent.json`.
    messages: usize,
}

/// The forms `forms` of the kind `k` read on its screens `screens` and given to the reference: what
/// parts, by the kind of check.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn hold(m: &Made, k: &str, fields: &[Field], screens: &Json, forms: &[Form], seeds: &[Seed], mach: &Machine, org: &Org, schemas: &crate::schema::Schemas) -> Found {
    let outs = read_screens(m, k, screens, forms, seeds);
    let mut found = Found::default();
    let requester_field = |n: &str| fields.iter().any(|f| f.name == n && f.given());
    let kept_field = |n: &str| fields.iter().any(|f| f.name == n && !f.computed);
    for (f, out) in forms.iter().zip(&outs) {
        let seed = &seeds[f.seed];
        let at = format!("{k}: {} ({:?}, {})", f.label, seed.flow, f.check);
        let entry = found.counts.entry(f.check).or_default();
        entry.forms += 1;
        if let Some(t) = out.get("threw") {
            found.apart.push(format!("{at}: the screen cannot be read: {}", t.compact()));
            entry.apart += 1;
            continue;
        }
        if let Some(n) = out.get("nulls").and_then(Json::as_arr).filter(|n| !n.is_empty()) {
            found.apart.push(format!("{at}: a null is given to {}", Json::arr(n.to_vec()).compact()));
        }
        let failed: Vec<&str> = out.get("failed").and_then(Json::as_arr).unwrap_or_default().iter().filter_map(Json::as_str).collect();
        let sent = out.get("sent").cloned().unwrap_or(Json::Null);
        // the message the button makes, as a renderer sends it back
        let message = Json::obj([("version", Json::str("1.0")), ("action", out.get("action").cloned().unwrap_or(Json::Null))]);
        let wrong = schemas.errors(&format!("{}renderer_to_agent.json", crate::SPEC), &message);
        found.messages += 1;
        if !wrong.is_empty() {
            found.apart.push(format!("{at}: the message the button makes is not of renderer_to_agent.json: {}", wrong.join("; ")));
        }
        let screen = failed.is_empty();
        let reference = answer(mach, org, fields, seed, &sent);
        if !screen && reference.is_err() {
            entry.stopped += 1;
        }
        if screen != reference.is_ok() {
            entry.apart += 1;
            let why = match &reference {
                Ok(_) => format!("the screen stops it ({}), and the reference takes it", failed.join(" | ")),
                Err(e) => format!("the screen takes it, and the reference refuses it ({e})"),
            };
            found.apart.push(format!("{at}: {why}; it sends {}", canon(&sent)));
            continue;
        }
        // the text as it is written, which the reference counts with its white space
        if let Some(w) = &f.written {
            let key = if seed.flow == Flow::Edit { "fields" } else { "form" };
            let mut form = sent.get(key).cloned().unwrap_or(Json::Obj(Vec::new()));
            match &w.rows {
                None => put(&mut form, &w.field, w.text.clone()),
                Some(r) => {
                    if let Some(Json::Arr(mut rows)) = form.get(r).cloned() {
                        if let Some(first) = rows.first_mut() {
                            put(first, &w.field, w.text.clone());
                        }
                        put(&mut form, r, Json::Arr(rows));
                    }
                }
            }
            let mut raw = sent.clone();
            put(&mut raw, key, form);
            if answer(mach, org, fields, seed, &raw).is_ok() != screen {
                entry.spaces += 1;
            }
        }
        let Ok(t) = reference else { continue };
        // the ticket keeps what the screen sends, and what the screen shows the form to be
        let kept = form_json(&t.form);
        let shown = out.get("given").cloned().unwrap_or(Json::Null);
        let (expected, keep): (Json, &dyn Fn(&str) -> bool) = match seed.flow {
            Flow::Submit => (sent.get("form").cloned().unwrap_or(Json::Null), &requester_field),
            Flow::Resubmit | Flow::Edit => {
                let key = if seed.flow == Flow::Edit { "fields" } else { "form" };
                let mut merged = seed.form.clone();
                for (n, v) in sent.get(key).and_then(Json::as_obj).unwrap_or_default() {
                    put(&mut merged, n, v.clone());
                }
                for n in sent.get("clear").and_then(Json::as_arr).unwrap_or_default().iter().filter_map(Json::as_str) {
                    merged = without(&merged, n);
                }
                (merged, &kept_field)
            }
        };
        if canon(&only(&kept, keep)) != canon(&only(&expected, keep)) {
            found.apart.push(format!("{at}: the ticket keeps {}, and the screen sends {}", canon(&only(&kept, keep)), canon(&only(&expected, keep))));
        }
        if canon(&only(&kept, &requester_field)) != canon(&shown) {
            found.apart.push(format!("{at}: the ticket keeps {}, and the screen shows the form as {}", canon(&only(&kept, &requester_field)), canon(&shown)));
        }
    }
    found
}

/// A screen with every check of `code` taken out, of its parts and of its buttons.
fn without_check(screen: &mut Json, code: &str) -> usize {
    let mut taken = 0;
    if let Json::Obj(kv) = screen {
        for (k, v) in kv.iter_mut() {
            if k == "@checks"
                && let Json::Arr(rules) = v
            {
                let before = rules.len();
                rules.retain(|r| r.get("condition").and_then(|c| c.get("args")).and_then(|a| a.get("code")).and_then(Json::as_str) != Some(code));
                taken += before - rules.len();
            } else {
                taken += without_check(v, code);
            }
        }
    } else if let Json::Arr(xs) = screen {
        for x in xs {
            taken += without_check(x, code);
        }
    }
    taken
}

#[test]
fn the_screens_stop_what_the_reference_refuses() {
    if !node() {
        return;
    }
    let gear = "examples/gear.ringi";
    let mut laid = ritsu_base::fs::Overlay::new(ritsu_base::fs::current());
    laid.put(&ritsu_base::paths::absolute(Path::new(gear)), GEAR.to_string());
    ritsu_base::fs::with(Rc::new(laid), || {
        let mut paths: Vec<PathBuf> = crate::common::example_files().into_iter().filter(|f| !f.starts_with("examples/shared/")).map(PathBuf::from).collect();
        paths.push(PathBuf::from(gear));
        let m = made_of(&paths);
        let schemas = crate::schemas(&json::parse(file(&m, "a2ui/catalog.json")).unwrap());
        let mut messages = 0;
        let mut table: Vec<(String, BTreeMap<&'static str, Count>)> = Vec::new();
        let mut apart = Vec::new();
        let mut mutant = None;
        for k in &m.kinds {
            let name = k.named.name.clone();
            let types = json::parse(file(&m, &format!("types/{name}.json"))).unwrap();
            let fields = fields_of(types.get("form").unwrap());
            let edited: Vec<String> = types.get("approvers_edit").and_then(Json::as_arr).unwrap_or_default().iter().filter_map(|x| x.as_str().map(str::to_string)).collect();
            let read = |screen: &str| json::parse(file(&m, &format!("a2ui/{name}/{screen}.en.json"))).unwrap();
            let screens = Json::obj([("form", read("form")), ("card", read("card"))]);
            let start = screens.get("form").and_then(|s| s.get("messages")).and_then(Json::as_arr).and_then(|ms| ms.iter().find_map(|x| x.get("updateDataModel"))).and_then(|d| d.get("value")).and_then(|v| v.get("form")).cloned().unwrap();
            let run = Run::with(crate::common::joined(), Options { unlisted: true, unrun: true, ..Options::default() });
            let o = run.file(&k.file).unwrap_or_else(|e| panic!("{}: {e}", k.file));
            ringi::engine::with(&o, &run, |mach, org| {
                let mut seeds = seeds(mach, org);
                if let Some(s) = filled_again(mach, org, &fields, &seeds) {
                    seeds.push(s);
                }
                if let Some(s) = filled(mach, org, &fields, &edited, &seeds) {
                    seeds.push(s);
                }
                let takes = |s: usize, api: &Json| answer(mach, org, &fields, &seeds[s], &Json::obj([("form", api.clone())])).is_ok();
                let mut forms = request_forms(&fields, &seeds, &start, &takes);
                forms.extend(again_forms(&fields, &seeds, &start));
                forms.extend(edit_forms(&fields, &edited, &seeds));
                let found = hold(&m, &name, &fields, &screens, &forms, &seeds, mach, org, &schemas);
                messages += found.messages;
                apart.extend(found.apart);
                table.push((name.clone(), found.counts));
                // a template with the check of the destination taken out: the screen takes a trip
                // with no destination, which the reference refuses
                if name == "expense" {
                    let mut cut = screens.clone();
                    assert!(without_check(&mut cut, "trip_to.required") >= 2, "the form has no check of the destination to take out");
                    let found = hold(&m, &name, &fields, &cut, &forms, &seeds, mach, org, &schemas);
                    mutant = Some(found.apart);
                }
            })
            .unwrap_or_else(|t| panic!("{}: {}", k.file, t.en));
        }
        // the table: the forms of each kind of check, by the kind of request (the forms, those both
        // stop, those the two answer apart, those they would answer apart if the reference were
        // given the text as written)
        let checks: Vec<&'static str> = {
            let mut v: Vec<&'static str> = table.iter().flat_map(|(_, c)| c.keys().copied()).collect();
            v.sort();
            v.dedup();
            v
        };
        let mut text = format!("| check | {} | all |\n|---|{}---|\n", table.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(" | "), "---|".repeat(table.len()));
        let mut total = Count::default();
        for c in &checks {
            let cells: Vec<String> = table.iter().map(|(_, counts)| counts.get(c).map(Count::cell).unwrap_or_else(|| "-".into())).collect();
            let all = table.iter().filter_map(|(_, counts)| counts.get(c)).fold(Count::default(), |s, x| s.add(*x));
            total = total.add(all);
            text.push_str(&format!("| {c} | {} | {} |\n", cells.join(" | "), all.cell()));
        }
        println!("{text}\nall: {}\nthe messages the buttons make, held to renderer_to_agent.json: {messages}", total.cell());
        assert!(apart.is_empty(), "{} forms part:\n{}", apart.len(), apart.join("\n"));
        // the forms are of both answers, in every kind of request
        for (k, counts) in &table {
            let all = counts.values().fold(Count::default(), |s, x| s.add(*x));
            assert!(all.stopped > 0 && all.stopped < all.forms, "{k}: {} forms, {} stopped", all.forms, all.stopped);
        }
        let mutant = mutant.expect("no expense among the kinds");
        assert!(mutant.iter().any(|a| a.contains("trip_to left empty") && a.contains("the reference refuses it")), "the template without the check of the destination is not found:\n{}", mutant.join("\n"));
        // the white space at a text's ends parts them only on the forms of white space
        for (k, counts) in &table {
            for (c, n) in counts {
                assert!(n.spaces == 0 || *c == "white space", "{k}: {c}: {} forms part by white space", n.spaces);
            }
        }
    });
}
