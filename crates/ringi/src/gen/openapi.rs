//! `openapi.json`: the server's API, as OpenAPI 3.1 (DESIGN 12.2, 12.4). Each event a person makes
//! on a ticket is an operation, and so is reading what a screen and an agent read:
//!
//! - `GET /requests`: the kinds of request the person may submit. `GET /waiting`: the tickets that
//!   wait for the person's answer. `GET /mine`: the person's own requests. Both lists come in
//!   pages (`limit`, `before`), the newest first, and say whether there is a next page (`more`).
//!   `GET /tickets/{id}`: a ticket and its history. `GET /organization`: the version of the
//!   organization in effect. `GET /masters/<masters>/<master>`: the rows of a master one may choose
//!   on a day. `GET /find/people`, `/find/teams` and `/find/masters/<masters>/<master>`: the people,
//!   the teams and the rows of a master found by words, as a screen's pickers find them (DESIGN
//!   13.2): their IDs or keys and names alone.
//! - `POST /requests/<name>/tickets`: a request of that kind made (`submit_<name>`), its body the
//!   values of the form as the kind takes them; `…/tickets/{id}/resubmit` and `…/tickets/{id}/edit`
//!   for a kind that sends back and one whose approvers edit, each with the values that change and
//!   the fields it makes empty (`clear`: a value of null keeps the one the ticket holds, DESIGN 7.3).
//!   Each of them carries `x-ritsu-request`: the kind's file, name and version.
//! - `POST /requests/<name>/worked` (`worked_<name>`): what the kind's form works out for a form
//!   filled in so far, as the generated entry `worked` answers it (DESIGN 13.7); and, for a kind that
//!   sends back or whose approvers edit, `…/tickets/{id}/worked` (`ticket_worked_<name>`): the same,
//!   by the rules of the version of the kind the ticket was made on, which decide its resubmit and
//!   its edits. They read alone: they change nothing, and take no idempotency key and no revision.
//!   The kind's answers every person the server authenticates, for it reads no ticket and no
//!   organization, and the screens' `functions.js` gives every browser the same rules; the ticket's,
//!   those who may see the ticket. They carry `x-ritsu-request` too.
//! - `POST /tickets/{id}/<event>`: every other event a kind's tickets take (`approve`, `reject`,
//!   `send_back`, `retract`, `pause`, `resume`, `withdraw`, `discard`, `add_approver`, `hand_over`),
//!   and `comment`, which every kind's tickets take.
//!
//! Each kind's schemas hold, besides, what its entry `worked` takes and answers (`<name>.worked.in`,
//! `<name>.worked.out`, DESIGN 13.7): a form filled in so far, each value of its type alone (a value
//! outside its field's range or none of its choices, and rows past the most their field takes, it
//! answers as not decided; rows fewer than the field asks for it works out as they are there), and
//! whether each field shown or asked for on a condition is, and the values the form works out.
//!
//! Every POST but the two that work a form out takes the operation's idempotency key in the header
//! `Idempotency-Key`: a new key for a new operation, the same key for the same call sent again, which
//! answers what it answered the first time (up to 255 bytes of visible ASCII). Every one of them but
//! a submit and a comment takes the revision of the ticket it was made on (`revision`, DESIGN 7.2,
//! with the ranges the machine takes).
//! Each may carry what the person writes with it (`comment`, up to 2,000 characters), and a comment
//! is nothing else. A key is required where the entry stops without it; the values of a form, of
//! its rows and a comment's text are the machine's to refuse (409), left out or null as much as
//! empty, and none of them is required. Who makes it is the person the server authenticates: no body names them. An
//! event taken answers 200 with the ticket; one refused answers 409 with the refusal (`refused`, one
//! of the kinds of DESIGN 7.2, its text in English and in Japanese, and the value of the form it is
//! of, `at`; a refusal of what the person wrote with it is `comment`, of no value of the form), the
//! ticket as it was. A gate that does not let the person do it refuses it as
//! `person`, and a book that refuses a hold as `book`. A body not of its shape is 400 (a value the
//! form does not take is the machine's, 409), a ticket the person may not see is 404 whatever the
//! gate lets them do, as is a ticket of another kind on an operation of one kind on a ticket
//! (`/requests/<name>/tickets/{id}/…`; one answer, `unseen`, says both), a submit while no version of its kind is
//! in effect is 404 (`unoffered`, which says the day the next version starts, `from`, to those the
//! gate lets submit the kind, DESIGN 14.2), and a comment past the server's limits is 429 (DESIGN
//! 12.4). No `servers` is written: where the server is, is the reader's (dandori's `use openapi`
//! takes it from the `url` under it).
//!
//! Each operation lists every answer it may give, as the server gives them. A read answers 400 for a
//! query not as it takes (`unasked`: a list's `limit` and `kind`, a find's words, `limit` and
//! `lang`, a master's `on`), the organization 404 to one not of its people or while no version is in
//! effect (`no_organization`), and a find of a master's rows 404 when no kind the server holds
//! chooses from it (`unchosen`). Every operation may answer 401 when no one is signed in
//! (`unsigned`) and 500 when the server fails (`failed`), and one with a body 413 for a body too
//! large (`oversized`) and 415 for one not sent as JSON (`not_json`). Every answer of an error but a
//! refusal holds its text alone (`error`, `{"error": "…"}`), but the 404 of no version in effect,
//! which may hold `from` besides.
//!
//! The two that work a form out answer 200 with what it works out (`<name>.worked.out`), and 400 for
//! a body not of its shape, where the entry `worked` stops, its error in the language of the list of
//! kinds: a value outside its field's range, none of its choices, or rows past the most their field
//! takes are no error there, but not decided (null). The kind's answers by the version a new
//! request takes now, and 404 while none is in effect, as a submit does (`unoffered`). The ticket's
//! answers 404 as the other operations of one kind on a ticket do: for a ticket the person may not
//! see, and for one of another kind.

use super::{EVENTS, Fld, Kind, MasterOf, head};
use crate::ast::TypeExpr;
use ritsu_base::json::Json;

/// The kinds of refusal (DESIGN 7.2), as a refusal's `refused` writes them.
pub(super) const REFUSALS: [&str; 12] = ["state", "person", "submit", "form", "comment", "limit", "clock", "stale", "event", "data", "book", "unread"];

fn r(name: &str) -> Json {
    Json::obj([("$ref", Json::str(format!("#/components/schemas/{name}")))])
}

fn string() -> Json {
    Json::obj([("type", Json::str("string"))])
}

fn integer() -> Json {
    Json::obj([("type", Json::str("integer"))])
}

fn nullable(t: &str) -> Json {
    Json::obj([("type", Json::arr([Json::str(t), Json::str("null")]))])
}

fn described(t: Json, text: &str) -> Json {
    let mut o = t.as_obj().map(|o| o.to_vec()).unwrap_or_default();
    o.push(("description".to_string(), Json::str(text)));
    Json::Obj(o)
}

/// An object with the properties given, the ones `required` names required, and no other.
fn object(props: Vec<(String, Json)>, required: &[&str]) -> Json {
    Json::obj([
        ("type", Json::str("object")),
        ("required", Json::arr(required.iter().map(|x| Json::str(*x)))),
        ("properties", Json::Obj(props)),
        ("additionalProperties", Json::Bool(false)),
    ])
}

fn props(ps: &[(&str, Json)]) -> Vec<(String, Json)> {
    ps.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

/// What a person writes with an event (DESIGN 7.2): a text of up to 2,000 characters, null or left
/// out for none. A longer one is the machine's to refuse (409, `comment`), as the entry reads it, and
/// so is none with an event the kind asks a reason with.
fn comment() -> Json {
    let most = Json::int(crate::engine::COMMENT_MOST as i128);
    Json::obj([("type", Json::arr([Json::str("string"), Json::str("null")])), ("maxLength", most), ("description", Json::str("What the person writes with it, kept on the line of the ticket's history: up to 2,000 characters, a longer one refused by the machine (409, `comment`); null or left out for none. A kind may ask a reason with a `reject` or a `send_back` (each stage's `comment_required` in `types/<name>.json`): the machine refuses one with none, or with white space alone (409, `comment`)"))])
}

/// What a reader of a field learns of it beside its name, its title and its type (DESIGN 12.2):
/// its label in another language, when the form shows it and asks for it, with the fields each
/// condition is worked out from, and the labels of its choices. An agent fills a form from these
/// alone.
fn about(f: &Fld) -> Vec<String> {
    let mut out = Vec::new();
    for (lang, text) in &f.named.labels {
        out.push(format!("Its label in `{lang}`: {text}"));
    }
    if let Some(c) = &f.shown_when {
        out.push(format!("Shown only when `{}`{}; while it is hidden it is not given, and loses its value", c.written, from(&c.reads)));
    }
    if let Some(c) = &f.required_when {
        out.push(format!("Required when `{}`{}", c.written, from(&c.reads)));
    } else if f.shown_when.is_some() && !f.optional && f.given() {
        out.push("Required while it is shown".to_string());
    } else if f.optional {
        out.push("May be left empty".to_string());
    }
    if f.choices.iter().any(|c| c.label != c.name || !c.labels.is_empty()) {
        let each: Vec<String> = f
            .choices
            .iter()
            .map(|c| {
                let other: Vec<String> = c.labels.iter().map(|(lang, t)| format!("{lang}: {t}")).collect();
                if other.is_empty() { format!("`{}` ({})", c.name, c.label) } else { format!("`{}` ({}; {})", c.name, c.label, other.join("; ")) }
            })
            .collect();
        out.push(format!("Its choices: {}", each.join(", ")));
    }
    out
}

/// The fields a condition is worked out from, as a reader is told them: ` (worked out from `kind`)`,
/// ` (worked out from `kind` and `items.price`)`; nothing where it reads no field.
fn from(reads: &[String]) -> String {
    let named: Vec<String> = reads.iter().map(|r| format!("`{r}`")).collect();
    match named.split_last() {
        None => String::new(),
        Some((last, [])) => format!(" (worked out from {last})"),
        Some((last, rest)) => format!(" (worked out from {} and {last})", rest.join(", ")),
    }
}

/// The schema of a field's value, as the JSON of the contract carries it: `null` among its types
/// where it may be left empty, `rows` the name of its rows' schema. Its `description` says what its
/// type carries, what [`about`] says and, where the form asks for it every time (`required`), that
/// it is required.
fn value(f: &Fld, null: bool, rows: &str, required: bool) -> Json {
    let mut o: Vec<(String, Json)> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let ty = |t: &str| if null { Json::arr([Json::str(t), Json::str("null")]) } else { Json::str(t) };
    let int = |n: Option<i128>| n.map(Json::int);
    match &f.ty {
        TypeExpr::Text | TypeExpr::LongText => {
            o.push(("type".into(), ty("string")));
            o.extend(int(f.count_min).map(|n| ("minLength".to_string(), n)));
            o.extend(int(f.count_max).map(|n| ("maxLength".to_string(), n)));
        }
        TypeExpr::Num(_) => {
            o.push(("type".into(), ty("integer")));
            o.extend(int(f.min).map(|n| ("minimum".to_string(), n)));
            o.extend(int(f.max).map(|n| ("maximum".to_string(), n)));
            notes.push(format!("{}, as a whole number of its unit's step", f.type_ascii));
        }
        TypeExpr::Date => {
            o.push(("type".into(), ty("string")));
            o.push(("format".into(), Json::str("date")));
            let (lo, hi) = (f.end(f.min), f.end(f.max));
            notes.push(match (lo.as_str(), hi.as_str()) {
                (Some(a), Some(b)) => format!("A day from {a} to {b}"),
                (Some(a), None) => format!("A day from {a}"),
                (None, Some(b)) => format!("A day up to {b}"),
                (None, None) => "A day".to_string(),
            });
        }
        TypeExpr::Choice | TypeExpr::ChoiceFrom(_) if !f.choices.is_empty() => {
            o.push(("type".into(), ty("string")));
            let mut vs: Vec<Json> = f.choices.iter().map(|c| Json::str(&c.name)).collect();
            if null {
                vs.push(Json::Null);
            }
            o.push(("enum".into(), Json::Arr(vs)));
        }
        TypeExpr::ChoiceFrom(_) => {
            o.push(("type".into(), ty("string")));
            notes.push(format!("The key of a row of {}", f.type_ascii.trim_start_matches("choice from ")));
        }
        TypeExpr::Choice => o.push(("type".into(), ty("string"))),
        TypeExpr::Attachment => {
            o.push(("type".into(), ty("string")));
            notes.push("The ID of a file the server took".to_string());
        }
        TypeExpr::Person => {
            o.push(("type".into(), ty("string")));
            notes.push("A person's ID".to_string());
        }
        TypeExpr::Team => {
            o.push(("type".into(), ty("string")));
            notes.push("A team's ID".to_string());
        }
        TypeExpr::Bool => o.push(("type".into(), ty("boolean"))),
        TypeExpr::Rows => {
            o.push(("type".into(), ty("array")));
            o.push(("items".into(), r(rows)));
            o.extend(int(f.count_min).map(|n| ("minItems".to_string(), n)));
            o.extend(int(f.count_max).map(|n| ("maxItems".to_string(), n)));
        }
    }
    notes.extend(about(f));
    if required {
        notes.push("Required".to_string());
    }
    if !notes.is_empty() {
        o.push(("description".into(), Json::str(notes.join(". "))));
    }
    o.push(("title".into(), Json::str(&f.named.label)));
    Json::Obj(o)
}

/// The values of the fields `fields` of the kind `k` as an event gives them, each null or left out
/// where it is empty: the entry reads a field left out as one given null, and the machine refuses a
/// field the form asks for left empty (409, `form`, DESIGN 12.4), so the schema requires none of
/// them. `required` says which the form asks for every time, which their descriptions say.
fn values(k: &Kind, fields: &[&Fld], required: impl Fn(&Fld) -> bool) -> Json {
    let ps = fields.iter().map(|f| (f.named.name.clone(), value(f, true, &format!("{}.{}.row", k.named.name, f.named.name), required(f)))).collect();
    object(ps, &[])
}

/// A row of a `rows` field: each field the requester gives, null or left out where it is empty, as
/// the form's own fields ([`values`]); the description of each a row asks for (one that may not be
/// empty, but an attachment, which a row may leave empty: DESIGN 4.3) says it is required.
fn row(f: &Fld) -> Json {
    let given: Vec<&Fld> = f.fields.iter().filter(|c| !c.computed).collect();
    let ps = given.iter().map(|c| (c.named.name.clone(), value(c, true, "", !c.optional && c.ty != TypeExpr::Attachment))).collect();
    described(object(ps, &[]), ROW)
}

/// What the values of a submit say of a field left out, or left empty (DESIGN 12.4).
const GIVEN: &str = "The values of the form. A field left out is as one given null, and a field the form asks for left empty is the machine's to refuse (409, `form`), as any value the form does not take";

/// What a row says of a field left out, or left empty (DESIGN 12.4).
const ROW: &str = "A row. A field left out is as one given null, and a field the row asks for left empty is the machine's to refuse (409, `form`)";

/// A master's row, as the server answers it: its fields, and the days it may be chosen on.
fn master_row(m: &MasterOf) -> Json {
    let mut ps: Vec<(String, Json)> = m.fields.iter().map(|f| (f.named.name.clone(), value(f, f.optional, "", false))).collect();
    ps.push(("valid_from".into(), described(nullable("string"), "The first day the row may be chosen on; null for any day before")));
    ps.push(("valid_to".into(), described(nullable("string"), "The last day the row may be chosen on; null for no end")));
    let mut req: Vec<&str> = m.fields.iter().filter(|f| !f.optional).map(|f| f.named.name.as_str()).collect();
    req.extend(["valid_from", "valid_to"]);
    object(ps, &req)
}

/// The schemas every API has: the ticket in the contract's shape (DESIGN 12.4) and what it holds,
/// a refusal, a revision, an entry of the history, a kind of request, the organization.
fn common() -> Vec<(String, Json)> {
    let arr = |x: Json| Json::obj([("type", Json::str("array")), ("items", x)]);
    let en = |vs: &[&str]| Json::obj([("type", Json::str("string")), ("enum", Json::arr(vs.iter().map(|v| Json::str(*v))))]);
    let time = || described(nullable("integer"), "A time, in seconds since 1970-01-01T00:00:00Z");
    let mut out: Vec<(String, Json)> = Vec::new();
    let mut put = |n: &str, j: Json| out.push((n.to_string(), j));
    let ranged = |t: Json, name: &str| {
        let (lo, hi) = revision_range(name);
        let mut o = t.as_obj().map(|o| o.to_vec()).unwrap_or_default();
        o.push(("minimum".into(), Json::int(lo)));
        o.push(("maximum".into(), Json::int(hi)));
        Json::Obj(o)
    };
    put(
        "revision",
        described(
            object(
                props(&[
                    ("round", ranged(integer(), "round")),
                    ("stage", described(ranged(nullable("integer"), "stage"), "The place in the ticket's stages of the stage it is at, from 0; null while it is with the requester and once it has ended")),
                    ("edits", ranged(integer(), "edits")),
                ]),
                &["round", "stage", "edits"],
            ),
            "The revision of the ticket an operation is made on, as the ticket's `revision` holds it, with its three keys. A key left out, or a value outside these ranges, is no revision a ticket has: the server answers 400",
        ),
    );
    put(
        "refusal",
        object(
            props(&[
                ("refused", described(en(&REFUSALS), "The kind of refusal. `form` refuses a value of the form, and names its place (`at`); `comment` refuses what the person wrote with the operation (its `comment`), and names no place of the form, which may have a field named `comment` of its own")),
                ("text", described(string(), "Why, in English")),
                ("text_ja", described(string(), "Why, in Japanese")),
                ("at", described(nullable("string"), "The place of the form the refusal is of, as a path: a field's name (`amount`, a computed one too), or a field of a row after the row's place from 0 in brackets (`items[1].price`; for a computation of a row, the first row it is outside its range on, `lines[0].subtotal`); a `rows` field by its name for the count of its rows. Null for a refusal that names no field of the form, a refusal of a comment among them")),
            ]),
            &["refused", "text", "text_ja", "at"],
        ),
    );
    put("answer", object(props(&[("by", string()), ("seat", described(string(), "The approver whose answer it counts as: the one a proxy answered for")), ("yes", Json::obj([("type", Json::str("boolean"))])), ("at", integer())]), &["by", "seat", "yes", "at"]));
    let sole = Json::obj([(
        "oneOf",
        Json::arr([en(&["passing", "other"]), object(props(&[("by", object(props(&[("person", string())]), &["person"]))]), &["by"])]),
    )]);
    put(
        "part",
        object(
            props(&[
                ("way", string()),
                ("key", string()),
                ("approvers", arr(string())),
                ("team", nullable("string")),
                ("needed", integer()),
                ("answers", arr(r("answer"))),
                ("state", en(&["active", "passed", "skipped", "stopped", "agent", "checked", "once", "deferred"])),
                ("of", described(nullable("string"), "The flow of an agent's part, or the stage a part is done with or waits for")),
                ("sole", sole),
            ]),
            &["way", "key", "approvers", "team", "needed", "answers", "state", "of", "sole"],
        ),
    );
    put(
        "stage",
        object(
            props(&[
                ("name", string()),
                ("value", string()),
                ("phase", en(&["ahead", "active", "answered", "skipped", "circulated", "noted", "once", "deferred"])),
                ("started", time()),
                ("held", integer()),
                ("remind", time()),
                ("reminded", described(Json::obj([("type", Json::str("boolean"))]), "Whether the stage's reminder has rung, since the stage started or last went up: a reminder that has not is set again when the stage is resumed or come back to, where it comes before the deadline")),
                ("due", time()),
                ("up", integer()),
                ("org", described(nullable("string"), "The day of the version of the organization its approvers were chosen in")),
                ("again", Json::obj([("type", Json::str("boolean"))])),
                ("parts", arr(r("part"))),
            ]),
            &["name", "value", "phase", "started", "held", "remind", "reminded", "due", "up", "org", "again", "parts"],
        ),
    );
    let one = |kind: &str, v: Json| object(vec![(kind.to_string(), object(props(&[("v", v)]), &["v"]))], &[kind]);
    put(
        "value",
        Json::obj([(
            "oneOf",
            Json::arr([
                en(&["nothing"]),
                one("str", string()),
                one("num", integer()),
                one("truth", Json::obj([("type", Json::str("boolean"))])),
                one("rows", arr(arr(r("named_value")))),
            ]),
        )]),
    );
    put("named_value", object(props(&[("name", string()), ("value", r("value"))]), &["name", "value"]));
    put("note", object(props(&[("stage", string()), ("outputs", arr(r("named_value")))]), &["stage", "outputs"]));
    put("hold", object(props(&[("book", string()), ("transfer", string()), ("round", integer())]), &["book", "transfer", "round"]));
    put("ended", object(props(&[("at", integer()), ("stage", nullable("string")), ("by", nullable("string"))]), &["at", "stage", "by"]));
    let ended = Json::obj([("oneOf", Json::arr([r("ended"), Json::obj([("type", Json::str("null"))])]))]);
    put(
        "ticket",
        object(
            props(&[
                ("id", string()),
                ("request", string()),
                ("version", integer()),
                ("requester", string()),
                ("made_by", nullable("string")),
                ("sent_back_by", described(nullable("string"), "The person who sent the ticket back to the requester (a proxy's own ID where a proxy did), while it is with the requester; null at any other time, and where the requester's side withdrew it")),
                ("status", en(&["in_progress", "paused", "with_requester", "approved", "rejected", "withdrawn", "expired"])),
                ("round", integer()),
                ("attempt", described(string(), "The try that started the round: the `attempt` the server gave the `submit` or the `resubmit` (DESIGN 12.4), which the round's holds are keyed by")),
                ("serial", integer()),
                ("revision", r("revision")),
                ("form", described(Json::obj([("type", Json::str("object"))]), "The values of the form, as the kind's `types/<name>.json` describes them")),
                ("stage", nullable("string")),
                ("stages", arr(r("stage"))),
                ("backs", integer()),
                ("edits", integer()),
                ("pause_left", integer()),
                ("paused_at", time()),
                ("pause_ends", time()),
                ("resubmit_due", time()),
                ("holds", arr(r("hold"))),
                ("notes", arr(r("note"))),
                ("ended", ended),
            ]),
            &["id", "request", "version", "requester", "made_by", "sent_back_by", "status", "round", "attempt", "serial", "revision", "form", "stage", "stages", "backs", "edits", "pause_left", "paused_at", "pause_ends", "resubmit_due", "holds", "notes", "ended"],
        ),
    );
    put(
        "history",
        object(
            props(&[
                ("at", described(integer(), "When the event was, in seconds since 1970-01-01T00:00:00Z")),
                ("did", string()),
                ("stage", nullable("string")),
                ("by", nullable("string")),
                ("proxy_of", nullable("string")),
                ("organization", nullable("string")),
                ("once_with", nullable("string")),
                ("to", arr(string())),
                ("note", nullable("string")),
                ("comment", described(nullable("string"), "What the person wrote with what they did, or the reason an agent gave")),
                ("cleared", described(arr(string()), "The fields that lost their value with a resubmit or an edit, by their names in the order the form declares them: those it made empty, and those the form no longer shows for the values it gave; empty on every other line")),
            ]),
            &["at", "did", "stage", "by", "proxy_of", "organization", "once_with", "to", "note", "comment", "cleared"],
        ),
    );
    // a kind's language and its titles in the others, as `types/<name>.json` writes them
    let langs = || en(&crate::screens::LANGS);
    let labels = arr(object(props(&[("lang", langs()), ("text", string())]), &["lang", "text"]));
    put(
        "request_kind",
        object(
            props(&[
                ("name", string()),
                ("written", string()),
                ("version", integer()),
                ("language", described(langs(), "The language the kind's file is written in, which its `title` and the names its screens show are in where no `labels` give them in another")),
                ("title", string()),
                ("labels", described(labels, "The kind's title in each other language its file declares `labels` in, as `types/<name>.json` writes it (`labels`)")),
                ("description", described(nullable("string"), "What the kind is for, as its file says it (`description`); null where it says nothing")),
                ("file", string()),
            ]),
            &["name", "written", "version", "language", "title", "labels", "description", "file"],
        ),
    );
    put("team", object(props(&[("id", string()), ("name", string()), ("name_ja", nullable("string")), ("kind", string()), ("parent", nullable("string")), ("head", nullable("string"))]), &["id", "name", "kind", "parent", "head"]));
    put("person", object(props(&[("id", string()), ("name", string()), ("name_ja", nullable("string")), ("email", nullable("string")), (crate::org::LANGUAGE, super::orgschema::person_language(false))]), &["id", "name"]));
    put("member", object(props(&[("person", string()), ("team", string()), ("position", string()), ("primary", Json::obj([("type", Json::str("boolean"))]))]), &["person", "team", "position", "primary"]));
    put(
        "organization",
        object(props(&[("version", described(string(), "The day the version takes effect")), ("teams", arr(r("team"))), ("people", arr(r("person"))), ("members", arr(r("member")))]), &["version", "teams", "people", "members"]),
    );
    out
}

/// The ends of a value of the revision (`round`, `stage`, `edits`) as the machine takes it
/// (`record Revision` of `lower/ticket.kn`): what the generated entries stop at, and the schema
/// holds a body to.
fn revision_range(name: &str) -> (i128, i128) {
    let src = crate::lower::glue::TICKET;
    let at = src.find("\nrecord Revision\n").expect("the machine declares its revision");
    let line = src[at..].lines().skip(2).take_while(|l| l.starts_with("  ")).find(|l| l.trim_start().starts_with(&format!("{name} "))).expect("the revision has the value");
    let num = |key: &str| -> i128 {
        let rest = &line[line.find(key).expect("the value has its range") + key.len()..];
        rest.split_whitespace().next().unwrap_or_default().replace('_', "").parse().expect("an end of the range is a whole number")
    };
    (num(">="), num("<="))
}

/// What a POST takes beside its body: the operation's idempotency key, and the ticket's ID where it
/// names one.
fn params(id: bool) -> Json {
    let key = Json::obj([("$ref", Json::str("#/components/parameters/idempotency_key"))]);
    let tid = Json::obj([("$ref", Json::str("#/components/parameters/id"))]);
    Json::arr(if id { vec![key, tid] } else { vec![key] })
}

fn body(schema: Json) -> Json {
    Json::obj([("required", Json::Bool(true)), ("content", Json::obj([("application/json", Json::obj([("schema", schema)]))]))])
}

/// The answers of an operation (DESIGN 12.4), each it may answer, by their codes in order: `ok`;
/// the refusal of an event (`refused`); what does not read or is not asked as it takes (`unread`,
/// the name of the 400 it answers); what is not there (`unseen`, the name of the 404: no such ticket,
/// on an operation of one kind on a ticket a ticket of another kind too, no version of a kind in
/// effect, …); of a comment alone (`limited`), the server's limits on comments; of an operation
/// with a body (`body`), one too large or not JSON; and of every operation, no one signed in and the
/// server failing. All but `ok` and a refusal hold their text in `error`.
fn answered(ok: Json, refused: bool, unread: Option<&str>, unseen: Option<&str>, limited: bool, body: bool, runs: bool) -> Json {
    let at = |name: &str| Json::obj([("$ref", Json::str(format!("#/components/responses/{name}")))]);
    let mut o = vec![("200".to_string(), ok), ("401".into(), at("unsigned")), ("500".into(), at("failed"))];
    if refused {
        o.push(("409".into(), at("refused")));
    }
    if let Some(unread) = unread {
        o.push(("400".into(), at(unread)));
    }
    if let Some(unseen) = unseen {
        o.push(("404".into(), at(unseen)));
    }
    if limited {
        o.push(("429".into(), at("limited")));
    }
    if body {
        o.push(("413".into(), at("oversized")));
        o.push(("415".into(), at("not_json")));
    }
    if runs {
        o.push(("503".into(), at("unavailable")));
    }
    o.sort_by(|a, b| a.0.cmp(&b.0));
    Json::Obj(o)
}

/// What an operation's idempotency key is, and what it does (DESIGN 12.4): an agent makes its keys
/// from this alone.
pub(super) const KEY: &str = "The operation's key. A new operation takes a new key, and the same call sent again (after a network failure, or when no answer came back) takes the same key: the same key again answers what the first answered, and changes nothing. A submit or a resubmit made again with other values, or after a refusal, is a new operation, and takes a new key. Up to 255 bytes of visible ASCII (`!` to `~`, no space); another key is answered 400";

/// The schema of an idempotency key.
pub(super) fn key_schema() -> Json {
    Json::obj([("type", Json::str("string")), ("minLength", Json::int(1)), ("maxLength", Json::int(255)), ("pattern", Json::str("^[!-~]+$"))])
}

/// How many tickets a page of `list_waiting` and `list_mine` holds when `limit` says nothing, and
/// the most it holds.
pub(super) const PAGE: i128 = 50;
pub(super) const PAGE_MOST: i128 = 100;

/// What the 400 of an operation that moves a ticket says (DESIGN 12.4): what a body of the wrong
/// shape is, and what goes to the machine instead.
const UNREAD: &str = "The request does not read, and the ticket is as it was: a body not of its shape (a value of another JSON type, a key it requires missing, a key it does not take, a day not written `YYYY-MM-DD`, a whole number past 64 bits, a revision with a key left out or outside its ranges), no idempotency key, one not of its form or one used before for another operation, a file it names that is neither one the ticket holds nor one the person uploaded (`no such file`), or a value the generated entry stops at as no input of the contract. What the person writes is the machine's to refuse (409) whatever the schemas say of it: as `form`, a value of the form or of a row it does not take (a choice not among its choices, a number outside its range, a text too long, too many rows, a field it asks for left empty, null or left out); as `comment`, a comment's text left empty or longer than 2,000 characters";

/// What the 400 of a read says (DESIGN 12.4, 13.2): a query not as it takes.
const UNASKED: &str = "The read is not asked as it takes: of a list, a `limit` that is not a whole number from 1 to 100, or (of the person's own requests) a `kind` that is no kind of request; of a find, words (`q`) not written in UTF-8 or of more than 200 characters, a `limit` outside 1 to 100, or a `lang` other than `en` and `ja`; of the rows of a master and a find of them, an `on` not written `YYYY-MM-DD`";

/// What the 404 of the organization says (DESIGN 12.4, 9.1).
pub(super) const NO_ORGANIZATION: &str = "No version of the organization is in effect today, or the person is not one of the people of the version in effect: the organization holds everyone's addresses, and is read by its people alone";

/// What the 404 of a find of a master's rows says (DESIGN 13.2).
const UNCHOSEN: &str = "No kind of request the server holds chooses from the master";

/// What the answers every operation may give besides say (DESIGN 12.4): no one signed in, a body
/// too large or not JSON, the server failing.
const UNSIGNED: &str = "No one is signed in: the server authenticates no person for the call";
const OVERSIZED: &str = "The body is larger than the server takes";
const NOT_JSON: &str = "The body is not sent as `application/json`: the call's `Content-Type` says another type, or none";
const FAILED: &str = "The server failed, and says no more here (its log says how). Whether an operation that moves a ticket was done is not said: the same call is sent again with the same idempotency key, which answers what the first did if it was done";

/// What the text of an error says (DESIGN 12.4): the body of every answer of an error but a refusal.
const ERROR_TEXT: &str = "What is wrong, in a sentence for a person: the server's own, or, where the generated entry stops a value, the entry's, in the language of the list of kinds (`lang`)";

/// What a find answers, and how it compares (DESIGN 13.2): the operation's text, the same for the
/// three.
pub(super) const FINDS: &str = "found by words, the closest first: the ones whose ID (a row's key) or name, in either language, holds every word of `q`, the words and the values compared in NFKC, without white space, the hiragana as katakana and in lower case; no words find every one. The ID or key and the name alone are answered, at most `limit` of them, and `more` says whether more are found. The vectors `organization/find.jsonl` say each step of the comparison and of the order";

/// What the 404 of an operation on a ticket says (DESIGN 12.4): who may see a ticket.
pub(super) const UNSEEN: &str = "No such ticket, or one the person may not see. A ticket is seen by the requester's side (its requester, the one who made the request for them, and the proxies who may submit it for them that day), by the approvers' side (the approvers of its stages now and before, those who answered and the approvers they answered for, the one who sent it back, and the proxies who may approve for any of them that day), and by those the gate lets `see` it. Every operation on a ticket the person may not see answers 404, whatever the gate lets them do";

/// What the 404 of an operation on a ticket says besides (DESIGN 12.4): on an operation of one kind,
/// a ticket of another kind is answered as one there is not. The one answer `unseen` says it after
/// `UNSEEN`, for outside the server the two are the same: no ticket the person may see is there.
const OF_KIND: &str = "An operation of one kind of request on a ticket (`/requests/<name>/tickets/{id}/…`: the request made again, an approver's edit, what the form works out on the ticket) answers 404 too for a ticket that is not of the kind `<name>`, whoever asks, as for no such ticket: what its body holds is of that kind";

/// What the 404 of a submit and of what a kind's form works out says (DESIGN 12.4, 14.2): no
/// version of the kind is in effect, and what the server says of it to whom.
pub(super) const UNOFFERED: &str = "No version of the kind is in effect now: each starts later, or an administrator stopped it, and the kind is not among the kinds of request (`list_requests`) meanwhile. The server asks the gate first whether the person may submit the kind (`submit`): to a person it does not let, it answers as for a kind there is not, saying nothing of its versions (no `from`); to a person it lets, `from` says the day the nearest later version starts, from which the request may be made, when that is decided (none when the kind is stopped and no later version is to start)";

/// What the `from` of a 404 `unoffered` says.
const UNOFFERED_FROM: &str = "The day the nearest later version of the kind starts (`YYYY-MM-DD`, the kind's day), from which the request may be made. Left out when no later version is to start, and to a person the gate does not let submit the kind";

/// Whom the operation that works a kind's form out answers, and why (DESIGN 12.4).
const ANYONE: &str = "It reads no ticket and no organization: what it answers comes from the kind's rules and the values given alone, and the screens' `functions.js` already gives the same rules to every person's browser. So it answers every person the server authenticates";

/// What the 400 of an operation that works a form out says (DESIGN 12.4, 13.7).
const UNWORKED: &str = "The request does not read: a body not of its shape (a value of another JSON type, `form` left out or not an object, a key it does not take, a day not written `YYYY-MM-DD`, a whole number past 64 bits), or a value the generated entry `worked` stops at as no input of the contract, its error in the language of the list of kinds (`lang`). A value outside its field's range or none of its choices, and rows past the most their field takes, are no error: what reads them is answered as not decided (null); rows fewer than the field asks for are worked out as they are there";

/// What the 429 of a comment says (DESIGN 12.4), and its `Retry-After`.
const LIMITED: &str = "Too many comments: the server limits how soon a person comments on a ticket again, and how many comments a ticket holds, and this one is not taken (no refusal of the machine, and the ticket is as it was). When it came too soon, `Retry-After` says in how many seconds it may be sent again, with the same idempotency key; when the ticket holds as many comments as the server takes, there is no `Retry-After`, for waiting does not change that";
const RETRY_AFTER: &str = "In how many seconds the comment may be sent again; only when it came too soon after the person's last comment on the ticket";

/// What the 503 of an operation that runs the code of a kind says (DESIGN 12.4): the moves, the
/// working out of a form, and the list of the kinds a person may submit.
const UNAVAILABLE: &str = "The server could not run the code of the kind of request just now: no instance of it was free, or the kind failed to answer the same event of the ticket less than a minute ago. Nothing was done, and the ticket is as it was: the same call is sent again after the seconds `Retry-After` says, an operation that moves a ticket with the same idempotency key";

/// When a call answered 503 is sent again.
const RETRY_UNAVAILABLE: &str = "In how many seconds the call may be sent again";

fn json_response(text: &str, schema: Json) -> Json {
    Json::obj([("description", Json::str(text)), ("content", Json::obj([("application/json", Json::obj([("schema", schema)]))]))])
}

/// An operation that moves a ticket: its id, its tag, what it says, the body's schema, and the
/// request it is of. Its 404: no such ticket, and on an operation of one kind on a ticket, a ticket
/// of another kind too; of a submit, no version of the kind in effect.
fn moving(id: &str, tag: &str, summary: &str, body_schema: Json, ticket: bool, request: Option<&Kind>) -> Json {
    let unseen = match (ticket, request.is_some()) {
        (true, _) => Some("unseen"),
        (false, true) => Some("unoffered"),
        (false, false) => None,
    };
    let mut o = vec![
        ("operationId".to_string(), Json::str(id)),
        ("tags".into(), Json::arr([Json::str(tag)])),
        ("summary".into(), Json::str(summary)),
        ("parameters".into(), params(ticket)),
        ("requestBody".into(), body(body_schema)),
        ("responses".into(), answered(Json::obj([("$ref", Json::str("#/components/responses/moved"))]), true, Some("unread"), unseen, id == "comment", true, true)),
    ];
    if let Some(k) = request {
        o.push(("x-ritsu-request".into(), Json::obj([("file", Json::str(&k.file)), ("name", Json::str(&k.named.name)), ("version", Json::int(k.version))])));
    }
    Json::Obj(o)
}

/// An operation that reads: 400 for a query not as it takes (`unasked`), and the 404 it answers
/// (`unseen`). Of the reads, only `list_requests` runs the kinds' code (it asks each kind whether
/// the person may submit it), so only it may answer 503.
fn reading(id: &str, tag: &str, summary: &str, parameters: Vec<Json>, ok: Json, unasked: bool, unseen: Option<&str>) -> Json {
    let mut o = vec![("operationId".to_string(), Json::str(id)), ("tags".into(), Json::arr([Json::str(tag)])), ("summary".into(), Json::str(summary))];
    if !parameters.is_empty() {
        o.push(("parameters".into(), Json::Arr(parameters)));
    }
    o.push(("responses".into(), answered(ok, false, unasked.then_some("unasked"), unseen, false, false, id == "list_requests")));
    Json::Obj(o)
}

/// An operation that works the form of `k` out (DESIGN 13.7): read alone, so no idempotency key and
/// no revision; its body what the entry `worked` takes, its answer what the entry answers. On a ticket
/// (`ticket`), the ticket's ID, and 404 for one the person may not see or of another kind; of the
/// kind, 404 while no version of it is in effect, as a submit.
fn working(id: &str, summary: &str, k: &Kind, ticket: bool) -> Json {
    let n = &k.named.name;
    let mut o = vec![("operationId".to_string(), Json::str(id)), ("tags".into(), Json::arr([Json::str(n)])), ("summary".into(), Json::str(summary))];
    if ticket {
        o.push(("parameters".into(), Json::arr([Json::obj([("$ref", Json::str("#/components/parameters/id"))])])));
    }
    o.push(("requestBody".into(), body(r(&format!("{n}.worked.in")))));
    let unseen = if ticket { "unseen" } else { "unoffered" };
    o.push(("responses".into(), answered(json_response("What the form works out", r(&format!("{n}.worked.out"))), false, Some("unworked"), Some(unseen), false, true, true)));
    o.push(("x-ritsu-request".into(), Json::obj([("file", Json::str(&k.file)), ("name", Json::str(n)), ("version", Json::int(k.version))])));
    Json::Obj(o)
}

/// What each event a person makes on a ticket of any kind says, and what its body carries beside the
/// revision.
fn event_of(e: &str) -> (&'static str, Vec<(&'static str, Json, bool)>) {
    let to_stage = described(nullable("string"), "The stage it goes back to, by its ASCII name; null for the requester");
    match e {
        "approve" => ("Approve the stage the ticket is at", vec![]),
        "reject" => ("Reject the request", vec![]),
        "send_back" => ("Send the ticket back to the requester, or to an earlier stage", vec![("to", to_stage, false)]),
        "retract" => ("Take back one's approval of the stage before, while the stage after it has no answer", vec![]),
        "pause" => ("Stop the ticket's clocks for a while", vec![]),
        "resume" => ("Start the paused ticket's clocks again", vec![]),
        "withdraw" => ("Withdraw the request", vec![]),
        "discard" => ("Discard the request sent back, instead of making it again", vec![]),
        "add_approver" => (
            "Add an approver to the stage the ticket is at",
            vec![("approver", described(string(), "The person's ID"), true), ("part", described(nullable("string"), "The way of the stage the approver answers for, by its ASCII name, where the stage has several; null for the first that waits"), false)],
        ),
        "hand_over" => (
            "Hand one approver's place at the stage to another person",
            vec![("of", described(string(), "The ID of the approver whose place it is"), true), ("to", described(string(), "The ID of the person who takes it"), true)],
        ),
        "comment" => ("Write on the ticket: a line of its history, and a notice to the requester, to the ones its stage waits for and, while it is with the requester, to the one who sent it back; nothing else of it changes", vec![]),
        _ => ("", vec![]),
    }
}

/// What a resubmit and an edit say of a value of null, or of a field left out (DESIGN 7.3, 12.4).
const KEPT: &str = "A field left out, or given null, keeps the value the ticket holds: naming it in `clear` is what makes it empty";

/// The schemas of a kind (named `<name>.…`): the values of its form a submit gives (`given`) and a
/// resubmit changes (`changes`), the values an approver edits (`edited`, where they do), and a row
/// of each `rows` field among them.
pub(super) fn kind_schemas(k: &Kind) -> Vec<(String, Json)> {
    let n = &k.named.name;
    let mut out = Vec::new();
    let given: Vec<&Fld> = k.given().collect();
    out.push((format!("{n}.given"), described(values(k, &given, |f| f.required()), GIVEN)));
    out.push((format!("{n}.changes"), described(values(k, &given, |_| false), &format!("The values that change. {KEPT}"))));
    let edited: Vec<&Fld> = k.edited().collect();
    if k.has("edit") {
        out.push((format!("{n}.edited"), described(values(k, &edited, |_| false), &format!("The values the approver gives or corrects. {KEPT}"))));
    }
    for f in k.form.iter().filter(|f| f.ty == TypeExpr::Rows && (f.given() || edited.iter().any(|x| x.named.name == f.named.name))) {
        out.push((format!("{n}.{}.row", f.named.name), row(f)));
    }
    out
}

/// The schema of a field's value as the entry `worked` takes it, from a form filled in so far: of its
/// type alone, null among its types. A value outside its range or none of its choices, and rows past
/// the most the field takes, are what `worked` answers as not decided, no error; rows fewer than the
/// field asks for it works out as they are there.
fn loose(f: &Fld) -> Json {
    let ty = |t: &str| Json::arr([Json::str(t), Json::str("null")]);
    let mut o: Vec<(String, Json)> = match &f.ty {
        TypeExpr::Num(_) => vec![("type".into(), ty("integer")), ("description".into(), Json::str(format!("{}, as a whole number of its unit's step", f.type_ascii)))],
        TypeExpr::Date => vec![("type".into(), ty("string")), ("format".into(), Json::str("date"))],
        TypeExpr::Bool => vec![("type".into(), ty("boolean")), ("description".into(), Json::str("Read as no where it is null or left out, as a box that is not ticked"))],
        TypeExpr::Rows => {
            let cells = f.fields.iter().filter(|c| !c.computed).map(|c| (c.named.name.clone(), loose(c))).collect();
            vec![("type".into(), ty("array")), ("items".into(), object(cells, &[]))]
        }
        _ => vec![("type".into(), ty("string"))],
    };
    o.push(("title".into(), Json::str(&f.named.label)));
    Json::Obj(o)
}

/// The schema of what the form works out of a value, as `worked` answers it: of the field's type,
/// null while it is not decided.
fn worked_value(f: &Fld) -> Json {
    match &f.ty {
        TypeExpr::Date => described(Json::obj([("type", Json::arr([Json::str("string"), Json::str("null")])), ("format", Json::str("date"))]), "a day; null while it is not decided"),
        _ => described(nullable("integer"), &format!("{}, as a whole number of its unit's step; null while it is not decided", f.type_ascii)),
    }
}

/// What the entry `worked` of the kind takes (`<name>.worked.in`) and answers (`<name>.worked.out`),
/// DESIGN 13.7.
pub(super) fn worked_schemas(k: &Kind) -> Vec<(String, Json)> {
    let n = &k.named.name;
    let given: Vec<(String, Json)> = k.form.iter().filter(|f| !f.computed).map(|f| (f.named.name.clone(), loose(f))).collect();
    let edited: Vec<(String, Json)> = k.edited().map(|f| (f.named.name.clone(), loose(f))).collect();
    let mut edit = object(edited, &[]);
    if let Some(o) = edit.as_obj().map(|o| o.to_vec()) {
        let mut o = o;
        o.retain(|(k, _)| k != "type");
        o.insert(0, ("type".to_string(), Json::arr([Json::str("object"), Json::str("null")])));
        edit = Json::Obj(o);
    }
    let input = object(
        props(&[
            ("form", described(object(given, &[]), "The form filled in so far, in the shape a submit gives it: a field left out or null is empty")),
            ("edited", described(edit, "The values an approver gives or corrects in it, in the shape an edit gives them (`fields`): each in place of the form's, a field left out or null keeping the form's; null or left out for none")),
        ]),
        &["form"],
    );
    let undecided = |what: &str| described(nullable("boolean"), &format!("{what}; null while it is not decided"));
    let mut fields: Vec<(String, Json)> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for f in &k.form {
        let mut ps: Vec<(String, Json)> = Vec::new();
        if let Some(c) = &f.shown_when {
            ps.push(("shown".into(), undecided(&format!("Whether the form shows the field: whether `{}` holds", c.written))));
        }
        if let Some(c) = &f.required_when {
            ps.push(("required".into(), undecided(&format!("Whether the form asks for the field: whether `{}` holds", c.written))));
        }
        if f.computed {
            ps.push(("value".into(), worked_value(f)));
        }
        if f.ty == TypeExpr::Rows && f.fields.iter().any(|c| c.computed) {
            let cells: Vec<(String, Json)> = f.fields.iter().filter(|c| c.computed).map(|c| (c.named.name.clone(), object(props(&[("value", worked_value(c))]), &["value"]))).collect();
            let req: Vec<String> = cells.iter().map(|(c, _)| c.clone()).collect();
            let row = object(cells, &req.iter().map(String::as_str).collect::<Vec<_>>());
            ps.push(("rows".into(), described(Json::obj([("type", Json::str("array")), ("items", row)]), "For each row of the form, in its order, what the form works out of it")));
        }
        if ps.is_empty() {
            continue;
        }
        let req: Vec<String> = ps.iter().map(|(p, _)| p.clone()).collect();
        fields.push((f.named.name.clone(), object(ps, &req.iter().map(String::as_str).collect::<Vec<_>>())));
        names.push(f.named.name.clone());
    }
    let all = object(fields, &names.iter().map(String::as_str).collect::<Vec<_>>());
    let output = object(props(&[("fields", described(all, "Each field shown or asked for on a condition, and each the form works out, in the form's order"))]), &["fields"]);
    vec![
        (format!("{n}.worked.in"), described(input, "What the entry `worked` takes: a form filled in so far, and what an approver gives or corrects in it. A value of its type alone: one outside its field's range or none of its choices, and rows past the most their field takes, are answered as not decided; rows fewer than the field asks for are worked out as they are there")),
        (format!("{n}.worked.out"), described(output, "What the form works out, as its screens work it out (DESIGN 13.7): whether each field shown or asked for on a condition is, and the value of each field it works out; null where it is not decided")),
    ]
}

/// What a submit of the kind carries: the requester when a proxy makes it, the form, and what the
/// person writes with it.
pub(super) fn submit_body(k: &Kind) -> Json {
    object(
        props(&[
            ("requester", described(nullable("string"), "The ID of the person the request is made for, when the person who makes it is their proxy; null or left out for oneself")),
            ("form", r(&format!("{}.given", k.named.name))),
            ("comment", comment()),
        ]),
        &["form"],
    )
}

/// The fields an event makes empty (DESIGN 7.3), by their names: those of `fields` the form does not
/// ask for on every request (a field asked for while it is shown may be named, and is refused empty
/// while it is shown).
fn clear(fields: &[&Fld]) -> Json {
    let names: Vec<Json> = fields.iter().filter(|f| !f.required()).map(|f| Json::str(&f.named.name)).collect();
    let text = "The fields to make empty, by their names; null or left out for none. A field the form asks for while it shows it is refused empty, and one the values given hide loses its value without being named";
    let mut o = vec![("type".to_string(), Json::arr([Json::str("array"), Json::str("null")]))];
    if names.is_empty() {
        o.push(("maxItems".into(), Json::int(0)));
    } else {
        o.push(("items".into(), Json::obj([("type", Json::str("string")), ("enum", Json::Arr(names))])));
    }
    o.push(("description".into(), Json::str(text)));
    Json::Obj(o)
}

/// What a resubmit of the kind carries: the revision of the ticket, the values that change, the
/// fields made empty, and what the person writes with it.
pub(super) fn resubmit_body(k: &Kind) -> Json {
    let given: Vec<&Fld> = k.given().collect();
    object(props(&[("revision", r("revision")), ("form", r(&format!("{}.changes", k.named.name))), ("clear", clear(&given)), ("comment", comment())]), &["revision", "form"])
}

/// What an approver's edit of the kind carries: the revision of the ticket, the values the approver
/// gives or corrects, the fields made empty, and what the person writes with it.
pub(super) fn edit_body(k: &Kind) -> Json {
    let edited: Vec<&Fld> = k.edited().collect();
    object(props(&[("revision", r("revision")), ("fields", r(&format!("{}.edited", k.named.name))), ("clear", clear(&edited)), ("comment", comment())]), &["revision", "fields"])
}

/// The schema of a revision, and of the refusal.
pub(super) fn common_named(name: &str) -> Json {
    common().into_iter().find(|(n, _)| n == name).map(|(_, j)| j).unwrap_or(Json::Null)
}

/// What the body of an event a person makes on a ticket of any kind carries: its revision, what
/// the event names besides (the stage of `send_back`, the people of `add_approver` and
/// `hand_over`), and what the person writes with it; with what the event says. A comment carries
/// its text alone, and no revision: the machine refuses one with no text (409, `comment`), so the
/// schema does not require it.
pub(super) fn event_body(e: &str) -> (&'static str, Json) {
    let (summary, extra) = event_of(e);
    if e == "comment" {
        let most = Json::int(crate::engine::COMMENT_MOST as i128);
        let text = Json::obj([
            ("type", Json::arr([Json::str("string"), Json::str("null")])),
            ("minLength", Json::int(1)),
            ("maxLength", most),
            ("description", Json::str("What the person writes: up to 2,000 characters. One left out, null, empty or longer is the machine's to refuse (409, `comment`), as the entry reads it")),
        ]);
        return (summary, object(props(&[("comment", text)]), &[]));
    }
    let mut ps = vec![("revision", r("revision"), true)];
    ps.extend(extra);
    ps.push(("comment", comment(), false));
    let req: Vec<&str> = ps.iter().filter(|p| p.2).map(|p| p.0).collect();
    (summary, object(ps.iter().map(|(k, v, _)| (k.to_string(), v.clone())).collect(), &req))
}

pub(super) fn json(kinds: &[Kind]) -> Json {
    let mut schemas = common();
    let mut paths: Vec<(String, Json)> = Vec::new();
    paths.push((
        "/requests".into(),
        Json::obj([("get", reading("list_requests", "requests", "The kinds of request the person may submit", vec![], json_response("The kinds", object(props(&[("requests", Json::obj([("type", Json::str("array")), ("items", r("request_kind"))]))]), &["requests"])), false, None))]),
    ));
    for k in kinds {
        let n = &k.named.name;
        schemas.extend(kind_schemas(k));
        schemas.extend(worked_schemas(k));
        let submit = submit_body(k);
        paths.push((format!("/requests/{n}/tickets"), Json::obj([("post", moving(&format!("submit_{n}"), n, &format!("Make a request of `{n}`"), submit, false, Some(k)))])));
        if k.has("resubmit") {
            paths.push((format!("/requests/{n}/tickets/{{id}}/resubmit"), Json::obj([("post", moving(&format!("resubmit_{n}"), n, "Make the request sent back again, with the values that change and the fields made empty", resubmit_body(k), true, Some(k)))])));
        }
        if k.has("edit") {
            paths.push((format!("/requests/{n}/tickets/{{id}}/edit"), Json::obj([("post", moving(&format!("edit_{n}"), n, "Fill in, correct or make empty the fields an approver gives", edit_body(k), true, Some(k)))])));
        }
        // what the form works out (DESIGN 13.7): of the kind, and on a ticket made again or edited,
        // by the rules of the version it was made on
        let worked = format!("What the form of `{n}` works out for the values given, as its screens work it out: whether each field shown or asked for on a condition is, and the value of each field it works out, null while it is not decided. It changes nothing. {ANYONE}. It answers by the version of the kind a new request takes now, and 404 while none is in effect, as a submit does");
        paths.push((format!("/requests/{n}/worked"), Json::obj([("post", working(&format!("worked_{n}"), &worked, k, false))])));
        if k.has("resubmit") || k.has("edit") {
            let on = format!("What the form of `{n}` works out for the values given, by the rules of the version of the kind the ticket was made on, which decide the request made again and the approvers' edits: the answer `worked_{n}` gives where that version is the one in effect. Of the ticket it reads the version alone: the values are the body's, and nothing of the ticket is answered. It changes nothing, and answers those who may see the ticket: 404 to the others, and for a ticket of another kind than `{n}`");
            paths.push((format!("/requests/{n}/tickets/{{id}}/worked"), Json::obj([("post", working(&format!("ticket_worked_{n}"), &on, k, true))])));
        }
    }
    let more = described(Json::obj([("type", Json::str("boolean"))]), "Whether the list holds tickets after the last of the page: the next page is read with that ticket's ID as `before`");
    let tickets = || json_response("The tickets, in that order", object(props(&[("tickets", Json::obj([("type", Json::str("array")), ("items", r("ticket"))])), ("more", more.clone())]), &["tickets", "more"]));
    let page = || vec![Json::obj([("$ref", Json::str("#/components/parameters/limit"))]), Json::obj([("$ref", Json::str("#/components/parameters/before"))])];
    paths.push((
        "/waiting".into(),
        Json::obj([("get", reading("list_waiting", "tickets", "The tickets that wait for the person's answer, or for the answer of one they stand in for as a proxy that day: the newest first, by when the stage that waits for it started, and the larger ID first (the later one) where that is the same", page(), tickets(), true, None))]),
    ));
    let mut mine = page();
    mine.push(Json::obj([
        ("name", Json::str("kind")),
        ("in", Json::str("query")),
        ("required", Json::Bool(false)),
        ("description", Json::str("Keeps the tickets of one kind of request, by its name; all kinds when it is left out")),
        ("schema", Json::obj([("type", Json::str("string")), ("enum", Json::arr(kinds.iter().map(|k| Json::str(&k.named.name))))])),
    ]));
    paths.push((
        "/mine".into(),
        Json::obj([("get", reading("list_mine", "tickets", "The person's own requests: the tickets they are the requester of, or made for someone else as their proxy, the newest first, by when they were made, and the larger ID first (the later one) where that is the same", mine, tickets(), true, None))]),
    ));
    let tid = Json::obj([("$ref", Json::str("#/components/parameters/id"))]);
    paths.push((
        "/tickets/{id}".into(),
        Json::obj([("get", reading("get_ticket", "tickets", "A ticket and its history", vec![tid], json_response("The ticket", object(props(&[("ticket", r("ticket")), ("history", Json::obj([("type", Json::str("array")), ("items", r("history"))]))]), &["ticket", "history"])), false, Some("unseen")))]),
    ));
    for e in EVENTS.into_iter().filter(|e| !matches!(*e, "resubmit" | "edit")).filter(|e| kinds.iter().any(|k| k.has(e))) {
        let (summary, b) = event_body(e);
        paths.push((format!("/tickets/{{id}}/{e}"), Json::obj([("post", moving(e, "tickets", summary, b, true, None))])));
    }
    paths.push(("/organization".into(), Json::obj([("get", reading("get_organization", "organization", "The version of the organization in effect today", vec![], json_response("The organization", r("organization")), false, Some("no_organization")))])));
    // the finds: the people and the teams of the version of the organization in effect today, and
    // the rows of each master a kind reads
    let mut finds: Vec<(String, String, String, Option<Json>)> = vec![
        ("/find/people".into(), "find_people".into(), format!("The people of the version of the organization in effect today, {FINDS}"), None),
        ("/find/teams".into(), "find_teams".into(), format!("The teams of the version of the organization in effect today, {FINDS}"), None),
    ];
    let mut seen: Vec<(&str, &str)> = Vec::new();
    for m in kinds.iter().flat_map(|k| &k.masters) {
        if seen.contains(&(m.masters.as_str(), m.master.as_str())) {
            continue;
        }
        seen.push((&m.masters, &m.master));
        let name = format!("masters.{}.{}", m.masters, m.master);
        schemas.push((name.clone(), master_row(m)));
        let on = Json::obj([
            ("name", Json::str("on")),
            ("in", Json::str("query")),
            ("required", Json::Bool(false)),
            ("description", Json::str("The day the rows may be chosen on; today when it is left out")),
            ("schema", Json::obj([("type", Json::str("string")), ("format", Json::str("date"))])),
        ]);
        let ok = json_response("The rows", object(props(&[("rows", Json::obj([("type", Json::str("array")), ("items", r(&name))]))]), &["rows"]));
        paths.push((format!("/masters/{}/{}", m.masters, m.master), Json::obj([("get", reading(&format!("get_master_{}_{}", m.masters, m.master), "organization", &format!("The rows of the master `{}`", m.named.label), vec![on.clone()], ok, true, None))])));
        finds.push((format!("/find/masters/{}/{}", m.masters, m.master), format!("find_master_{}_{}", m.masters, m.master), format!("The rows of the master `{}` one may choose on a day (`on`, today when it is left out), {FINDS}", m.named.label), Some(on)));
    }
    for (path, id, summary, on) in finds {
        let mut ps: Vec<Json> = ["q", "find_limit", "lang"].iter().map(|p| Json::obj([("$ref", Json::str(format!("#/components/parameters/{p}")))])).collect();
        ps.extend(on);
        let mut o = vec![("operationId".to_string(), Json::str(&id)), ("tags".into(), Json::arr([Json::str("organization")])), ("summary".into(), Json::str(&summary)), ("parameters".into(), Json::Arr(ps))];
        let master = id.starts_with("find_master_");
        let unseen = if master { Some("unchosen") } else { Some("no_organization") };
        o.push(("responses".into(), answered(json_response("The values found", r("found")), false, Some("unasked"), unseen, false, false, false)));
        paths.push((path, Json::obj([("get", Json::Obj(o))])));
    }
    let one_found = object(
        props(&[
            ("value", described(string(), "The person's or the team's ID, or the row's key")),
            ("name", described(string(), "Its name in the language of `lang`; empty for none")),
            ("label", described(string(), "The name and the value as a list shows them (`Sales staff 3 (e1203)`, `営業担当 3（e1203）`); the value alone where the name is empty or the value itself")),
        ]),
        &["value", "name", "label"],
    );
    let found = object(
        props(&[("found", Json::obj([("type", Json::str("array")), ("items", one_found)])), ("more", described(Json::obj([("type", Json::str("boolean"))]), "Whether more are found than `limit` answers"))]),
        &["found", "more"],
    );
    schemas.push(("found".to_string(), found));
    let parameters = Json::obj([
        ("idempotency_key", Json::obj([("name", Json::str("Idempotency-Key")), ("in", Json::str("header")), ("required", Json::Bool(true)), ("description", Json::str(KEY)), ("schema", key_schema())])),
        ("id", Json::obj([("name", Json::str("id")), ("in", Json::str("path")), ("required", Json::Bool(true)), ("description", Json::str("The ticket's ID")), ("schema", string())])),
        (
            "limit",
            Json::obj([
                ("name", Json::str("limit")),
                ("in", Json::str("query")),
                ("required", Json::Bool(false)),
                ("description", Json::str(format!("The most tickets the page holds; {PAGE} when it is left out"))),
                ("schema", Json::obj([("type", Json::str("integer")), ("minimum", Json::int(1)), ("maximum", Json::int(PAGE_MOST)), ("default", Json::int(PAGE))])),
            ]),
        ),
        (
            "q",
            Json::obj([
                ("name", Json::str("q")),
                ("in", Json::str("query")),
                ("required", Json::Bool(false)),
                ("description", Json::str("The words to find by, apart with white space: up to 200 characters. None find every one")),
                ("schema", Json::obj([("type", Json::str("string")), ("maxLength", Json::int(crate::find::LONGEST as i128))])),
            ]),
        ),
        (
            "find_limit",
            Json::obj([
                ("name", Json::str("limit")),
                ("in", Json::str("query")),
                ("required", Json::Bool(false)),
                ("description", Json::str(format!("The most it answers; {} when it is left out", crate::find::SHOWN))),
                ("schema", Json::obj([("type", Json::str("integer")), ("minimum", Json::int(1)), ("maximum", Json::int(crate::find::MOST as i128)), ("default", Json::int(crate::find::SHOWN as i128))])),
            ]),
        ),
        (
            "lang",
            Json::obj([
                ("name", Json::str("lang")),
                ("in", Json::str("query")),
                ("required", Json::Bool(false)),
                ("description", Json::str("The language of the names: `en` or `ja`; when it is left out, the first of them `Accept-Language` names, else `en`")),
                ("schema", Json::obj([("type", Json::str("string")), ("enum", Json::arr([Json::str("en"), Json::str("ja")]))])),
            ]),
        ),
        (
            "before",
            Json::obj([
                ("name", Json::str("before")),
                ("in", Json::str("query")),
                ("required", Json::Bool(false)),
                ("description", Json::str("The ID of the last ticket of the page before: the page holds the tickets after it in the list's order. Left out for the first page; a ticket the list does not hold gives an empty page")),
                ("schema", string()),
            ]),
        ),
    ]);
    // the body of every answer of an error but a refusal: its text alone (DESIGN 12.4)
    schemas.push(("error".to_string(), described(object(props(&[("error", described(string(), ERROR_TEXT))]), &["error"]), "The body of every answer of an error but a refusal (409, the machine's): its text alone. The 404 of no version of a kind in effect (`unoffered`) holds it with the day the next version starts besides (`from`), when that is said")));
    let error = |text: &str| json_response(text, r("error"));
    let responses = Json::obj([
        ("moved", json_response("The event is taken: the ticket as it is now", object(props(&[("ticket", r("ticket"))]), &["ticket"]))),
        ("refused", json_response("The event is refused, and the ticket is as it was", r("refusal"))),
        ("unread", error(UNREAD)),
        ("unworked", error(UNWORKED)),
        ("unasked", error(UNASKED)),
        ("unseen", error(&format!("{UNSEEN}. {OF_KIND}"))),
        // the text of the error, and the day the next version starts where the person may know it
        ("unoffered", json_response(UNOFFERED, object(props(&[("error", described(string(), ERROR_TEXT)), ("from", described(Json::obj([("type", Json::str("string")), ("format", Json::str("date"))]), UNOFFERED_FROM))]), &["error"]))),
        ("no_organization", error(NO_ORGANIZATION)),
        ("unchosen", error(UNCHOSEN)),
        (
            "limited",
            Json::obj([
                ("description", Json::str(LIMITED)),
                ("headers", Json::obj([("Retry-After", Json::obj([("description", Json::str(RETRY_AFTER)), ("schema", Json::obj([("type", Json::str("integer")), ("minimum", Json::int(1))]))]))])),
                ("content", Json::obj([("application/json", Json::obj([("schema", r("error"))]))])),
            ]),
        ),
        (
            "unavailable",
            Json::obj([
                ("description", Json::str(UNAVAILABLE)),
                ("headers", Json::obj([("Retry-After", Json::obj([("description", Json::str(RETRY_UNAVAILABLE)), ("schema", Json::obj([("type", Json::str("integer")), ("minimum", Json::int(1))]))]))])),
                ("content", Json::obj([("application/json", Json::obj([("schema", r("error"))]))])),
            ]),
        ),
        ("unsigned", error(UNSIGNED)),
        ("oversized", error(OVERSIZED)),
        ("not_json", error(NOT_JSON)),
        ("failed", error(FAILED)),
    ]);
    Json::obj([
        ("openapi", Json::str("3.1.0")),
        ("x-ritsu-generated", Json::str(head())),
        ("info", Json::obj([("title", Json::str("Requests")), ("version", Json::str(crate::lower::shape::CONTRACT.to_string()))])),
        ("x-ritsu-contract", Json::int(crate::lower::shape::CONTRACT)),
        ("paths", Json::Obj(paths)),
        ("components", Json::obj([("schemas", Json::Obj(schemas)), ("parameters", parameters), ("responses", responses)])),
    ])
}
