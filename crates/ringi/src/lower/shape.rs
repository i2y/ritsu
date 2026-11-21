//! The JSON of the ticket machine (DESIGN 10.4, 12.4): the one shape the vectors, `ringi run
//! --format json`, the code kaname writes from a request's glue ([`super::glue`]) and a server take
//! and answer. The reference interpreter's types write what they hold in the words of chapter 7
//! ([`crate::engine`]'s `to_json`); this is the one place those words become the contract's, and
//! nothing else writes the JSON. The two differ in these places, and nowhere else:
//!
//! - A value written with a space (`in progress`, `cancel timer`, `went up`) has `_` for it; the
//!   state `open` of a stage and of a part is `active` (kaname takes no `open` among an enum's
//!   values); a part's `with` is `of`, a `hand over`'s `from` is `of`, and `pass` of a part's
//!   `sole` is `passing`, with `by` a variant of its own (`{"by": {"person": …}}`): words the
//!   languages kaname writes keep.
//! - A thing to do is a variant named by what it does, with its key as three fields (`{"key":
//!   {"ticket", "round", "serial"}, "what": {"hold": {…}}}`); `record` is `history`, `void` is
//!   `release`.
//! - The arguments of a thing to do, the outputs of a note and the values of a row of the
//!   organization are lists of names and values, each value a variant of its kind (`str`, `num`,
//!   `truth`, `rows`, or `nothing`): kaname has no map of values of several types.
//! - An event is a variant named by the event (`{"approve": {"by": …, "revision": …, "comment":
//!   …}}`, `{"comment": {"by": …, "comment": …}}`), and a clock's, which carries nothing, is the
//!   name alone (`"late"`); a `submit` is `{"submit": …}`, what `start` is given. An answer is `ok`
//!   with the ticket and the things to do, or `err` with the refusal.

use ritsu_base::json::Json;

/// The version of the contract (DESIGN 12.4): the shape of the JSON of a ticket, an event, the part
/// of the organization and an answer, what `start` and `next` take and answer, and the files of
/// `--target requests`. One number for all of them, written in each language's list of the kinds of
/// request and in what `--target requests` writes; raised when a server written for the one before
/// would read a package of this one wrongly (a key or a word changed, gone or of another meaning),
/// and kept when what changes leaves such a server right (a new kind of request, a new key it need
/// not read). A server refuses a package whose number it does not know.
pub const CONTRACT: u32 = 3;

/// A transition (a line of the vectors, a row of `ringi run --format json`) as the contract writes
/// it: the ticket before, the event, the part of the organization and the answer in its words, the
/// other keys as they are.
pub fn stepped(v: &Json) -> Json {
    map_fields(v, |k, x| match k {
        "ticket" => Some((k.into(), ticket(x))),
        "event" => Some((k.into(), event(x))),
        "organization" => Some((k.into(), organization(x))),
        "answer" => Some((k.into(), answer(x))),
        _ => Some((k.into(), x.clone())),
    })
}

/// A word of chapter 7 as the contract writes it: `_` for each space (`in progress`,
/// `resubmit late`).
pub fn word(w: &str) -> String {
    w.replace(' ', "_")
}

/// The function a host calls for a line of the vectors (`start` for a `submit`, `next` for any
/// other event), what it is given (`submit`, `organization`, `now`; or `ticket`, `event`,
/// `organization`, `now`), and what it answers. None for a line that is no vector.
pub fn call_of(v: &Json) -> Option<(&'static str, Json, Json)> {
    let ev = v.get("event")?;
    let org = v.get("organization")?.clone();
    let now = v.get("now")?.clone();
    let answer = v.get("answer")?.clone();
    if let Some(sub) = ev.get("submit") {
        return Some(("start", Json::Obj(vec![("submit".into(), sub.clone()), ("organization".into(), org), ("now".into(), now)]), answer));
    }
    let ticket = v.get("ticket").cloned().unwrap_or(Json::Null);
    Some(("next", Json::Obj(vec![("ticket".into(), ticket), ("event".into(), ev.clone()), ("organization".into(), org), ("now".into(), now)]), answer))
}

/// A word with `_` for each space.
fn spaced(j: &Json) -> Json {
    match j {
        Json::Str(s) => Json::Str(word(s)),
        x => x.clone(),
    }
}

/// The object with each field `f` changes, the others as they are.
fn map_fields(o: &Json, f: impl Fn(&str, &Json) -> Option<(String, Json)>) -> Json {
    Json::Obj(o.as_obj().unwrap_or_default().iter().filter_map(|(k, v)| f(k, v)).collect())
}

/// A value of an argument, an output or a row: a variant of its kind.
fn value(v: &Json) -> Json {
    let one = |kind: &str, x: Json| Json::Obj(vec![(kind.into(), Json::Obj(vec![("v".into(), x)]))]);
    match v {
        Json::Null => Json::str("nothing"),
        Json::Bool(b) => one("truth", Json::Bool(*b)),
        Json::Int(_) | Json::Frac(_) => one("num", v.clone()),
        Json::Str(_) => one("str", v.clone()),
        Json::Arr(rows) => one("rows", Json::Arr(rows.iter().map(args).collect())),
        Json::Obj(_) => one("str", Json::str(v.compact())),
    }
}

/// An object of names and values, as a list of each name with its value.
fn args(o: &Json) -> Json {
    Json::Arr(o.as_obj().unwrap_or_default().iter().map(|(k, v)| Json::Obj(vec![("name".into(), Json::str(k.clone())), ("value".into(), value(v))])).collect())
}

/// The part of the organization: each row of a master with its values as a list of cells.
fn organization(o: &Json) -> Json {
    map_fields(o, |k, v| {
        if k != "rows" {
            return Some((k.into(), v.clone()));
        }
        let rows = v.as_arr().unwrap_or_default().iter().map(|r| {
            map_fields(r, |k, v| {
                if k != "values" {
                    return Some((k.into(), v.clone()));
                }
                let cells = v.as_obj().unwrap_or_default().iter().map(|(n, x)| Json::Obj(vec![("name".into(), Json::str(n.clone())), ("value".into(), x.clone())]));
                Some((k.into(), Json::Arr(cells.collect())))
            })
        });
        Some((k.into(), Json::Arr(rows.collect())))
    })
}

/// `open` as kaname names it, `active`.
fn state(j: &Json) -> Json {
    match j.as_str() {
        Some("open") => Json::str("active"),
        _ => j.clone(),
    }
}

fn part(p: &Json) -> Json {
    map_fields(p, |k, v| match k {
        "state" => Some((k.into(), state(v))),
        "with" => Some(("of".into(), v.clone())),
        "sole" => Some((
            k.into(),
            match v {
                Json::Str(s) if s == "pass" => Json::str("passing"),
                Json::Obj(_) => Json::Obj(vec![("by".into(), Json::Obj(vec![("person".into(), v.get("by").cloned().unwrap_or(Json::Null))]))]),
                x => x.clone(),
            },
        )),
        _ => Some((k.into(), v.clone())),
    })
}

fn stage(s: &Json) -> Json {
    map_fields(s, |k, v| match k {
        "phase" => Some((k.into(), state(v))),
        "parts" => Some((k.into(), Json::Arr(v.as_arr().unwrap_or_default().iter().map(part).collect()))),
        _ => Some((k.into(), v.clone())),
    })
}

fn ticket(t: &Json) -> Json {
    if t.is_null() {
        return Json::Null;
    }
    map_fields(t, |k, v| match k {
        "status" => Some((k.into(), spaced(v))),
        "stages" => Some((k.into(), Json::Arr(v.as_arr().unwrap_or_default().iter().map(stage).collect()))),
        "notes" => Some((k.into(), Json::Arr(v.as_arr().unwrap_or_default().iter().map(|n| map_fields(n, |k, v| Some((k.into(), if k == "outputs" { args(v) } else { v.clone() })))).collect()))),
        _ => Some((k.into(), v.clone())),
    })
}

/// A thing to do: its key as three fields, and what it does as a variant.
fn effect(e: &Json) -> Json {
    let key = e.get("key").and_then(Json::as_arr).unwrap_or_default();
    let at = |i: usize| key.get(i).cloned().unwrap_or(Json::Null);
    let key = Json::Obj(vec![("ticket".into(), at(0)), ("round".into(), at(1)), ("serial".into(), at(2))]);
    let does = e.get("do").and_then(Json::as_str).unwrap_or_default();
    let name = match does {
        "record" => "history".to_string(),
        "void" => "release".to_string(),
        other => word(other),
    };
    let rest = map_fields(e, |k, v| match k {
        "key" | "do" => None,
        "args" => Some((k.into(), args(v))),
        "timer" | "notice" => Some((k.into(), spaced(v))),
        _ => Some((k.into(), v.clone())),
    });
    Json::Obj(vec![("key".into(), key), ("what".into(), Json::Obj(vec![(name, rest)]))])
}

/// An event: a variant named by it, with what it carries (a person's event and a flow's answer,
/// the revision of the ticket it was made on; a person's, what they wrote too); a clock's, which
/// carries nothing, the name alone.
fn event(e: &Json) -> Json {
    let name = word(e.get("event").and_then(Json::as_str).unwrap_or_default());
    if matches!(name.as_str(), "remind" | "late" | "resubmit_late" | "pause_ends") {
        return Json::str(name);
    }
    let rest = map_fields(e, |k, v| match (name.as_str(), k) {
        (_, "event") => None,
        ("hand_over", "from") => Some(("of".into(), v.clone())),
        ("note_ready", "outputs") => Some((k.into(), args(v))),
        ("note_ready" | "agent_answered" | "flow_failed", "by") => None,
        _ => Some((k.into(), v.clone())),
    });
    Json::Obj(vec![(name, rest)])
}

/// What the machine answers: `ok` with the ticket and the things to do, or `err` with the refusal
/// (its kind, its sentence in English and in Japanese, and the place of the form it names when it
/// names one).
fn answer(a: &Json) -> Json {
    if a.get("refused").is_some() {
        let refusal: Vec<(String, Json)> = ["refused", "text", "text_ja", "at"].iter().map(|k| (k.to_string(), a.get(k).cloned().unwrap_or(Json::Null))).collect();
        return Json::Obj(vec![("err".into(), Json::Obj(refusal))]);
    }
    let effects = a.get("effects").and_then(Json::as_arr).unwrap_or_default().iter().map(effect).collect();
    let moved = Json::Obj(vec![("ticket".into(), a.get("ticket").map(ticket).unwrap_or(Json::Null)), ("effects".into(), Json::Arr(effects))]);
    Json::Obj(vec![("ok".into(), moved)])
}
