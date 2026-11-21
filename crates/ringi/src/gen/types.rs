//! `types/<name>.json`: a kind of request as the files describe it (DESIGN 12.2), for a server to
//! list and serve it, a screen to show its form and an agent to read it. Its keys, in this order,
//! whatever the kind:
//!
//! - `//` (the head), `contract` (DESIGN 12.4), `file`, `name`, `written`, `version`, `language`,
//!   `title`, `labels`, `description`, `owner`, `effective_from`. `language` is the language the
//!   kind is written in (`en` or `ja`): its `title` and every `label` below are in it, and `labels`
//!   holds what is added in the others. A server marks a name shown in another language than its
//!   page's (one with no label in the page's language) as of `language`.
//! - `usable`: the days it can be used on and the data that decide them (DESIGN 9.6), as `ringi api`
//!   writes them: a server tells an administrator before the last of them.
//! - `utc_offset_minutes`: the UTC offset its days are read in, in minutes east of UTC (DESIGN 7.5),
//!   as `ringi api` writes it: a server finds the day of an event's time in it, and gives the
//!   version of the organization in force that day.
//! - `submitted_by`: the groups of `submitted by` and the ones left out (`except`), each group with
//!   the same keys; null when everyone in the organization may submit it. A server lists to a
//!   person the kinds they may submit.
//! - `self_approval`: whether its requester may approve their own request (`self approval`, DESIGN
//!   5.5), as `ringi api` writes it in `ticket`: a server gives it to the gate as each stage's
//!   `self_approval` (DESIGN 12.4).
//! - `organization`: the organization it reads (its file, name, title, positions from the lowest,
//!   kinds of team from the widest, and where its data is).
//! - `form`: each field with its name, type, range (a number's as the integers on the wire, a
//!   date's as its day), the limits on its length or its rows, its choices, whether it is written
//!   `optional`, is computed (with the fields its computation reads, `reads`), is worked out by the
//!   screens as the form is filled in (`worked_on_screen`: a computed number of the form; the
//!   machine alone works out a computed date and a computed field of a row) or is the approvers', the
//!   calendar of its open days, the conditions it is shown and asked for on, and the fields of its
//!   rows. As the form says (DESIGN 4.2), a field with `required_when` is asked for only when its
//!   condition holds, and one with `shown_when` is neither asked for nor sent while it is hidden; each
//!   condition is written as in the file, with the function of the lowered rules that works it out
//!   (`worked`) and the fields it reads (`reads`). The fields a condition or a computation reads are
//!   fields of the form (a field of a row as `items.price`), through the computations it reads, as
//!   the lowered rule reads them: what its answer changes with, and nothing else (DESIGN 4.9). A
//!   condition reads no field its answer cannot change with: `over is none`, where `over` is worked
//!   out from a field that may be empty and one the requester always fills in, reads the first. A
//!   field the form hides is read as empty, so a condition also changes with what the condition a
//!   field it reads is shown on reads.
//! - `stages` (each with the events the kind asks a reason with at it, `comment_required`: `reject`,
//!   `send_back`, empty where it asks none), `ways`, `endings`, `events` (the events a person makes,
//!   by their names in the contract), `approvers_edit`, `masters` (the masters it reads, each with
//!   its key, the field that names a row as a screen shows it (`name_field`: the first text field
//!   that is not the key), and its fields).
//! - `flows`: each flow it uses, with its `role` (`launch`, `agent` or `summary`), the alias the
//!   request reads it by, its file, where it is asked (`at`: the endings a launch starts on, the way
//!   to choose an agent is, the stage of a summary), and what dandori declares of it: the
//!   workflow's name and version, its type on Temporal and its task queue (the same name), and its
//!   outputs in the order written, each with its type as dandori writes it (a summary's note holds
//!   them in that order). A server starts a flow by its type on that queue, and sees before it
//!   starts that it has a worker for every flow a kind uses.

use super::{Cond, FlowOf, Fld, Kind, MasterOf, Named, OrgOf, head, labels_json};
use ritsu_base::json::Json;

pub(super) fn json(k: &Kind) -> Json {
    let opt = |s: &Option<String>| s.as_ref().map(Json::str).unwrap_or(Json::Null);
    let submitted_by = k.submitted_by.as_ref().map(|(g, x)| Json::obj([("groups", Json::arr(g.iter().cloned())), ("except", Json::arr(x.iter().cloned()))])).unwrap_or(Json::Null);
    Json::obj([
        ("//", Json::str(head())),
        ("contract", Json::int(crate::lower::shape::CONTRACT)),
        ("file", Json::str(&k.file)),
        ("name", Json::str(&k.named.name)),
        ("written", Json::str(&k.named.written)),
        ("version", Json::int(k.version)),
        ("language", Json::str(k.language)),
        ("title", Json::str(&k.named.label)),
        ("labels", labels_json(&k.named.labels)),
        ("description", opt(&k.description)),
        ("owner", opt(&k.owner)),
        ("effective_from", opt(&k.effective_from)),
        ("usable", k.usable.clone()),
        ("utc_offset_minutes", Json::int(i128::from(k.utc_offset))),
        ("submitted_by", submitted_by),
        ("self_approval", Json::from(k.self_approval)),
        ("organization", k.organization.as_ref().map(org_json).unwrap_or(Json::Null)),
        ("form", Json::arr(k.form.iter().map(field_json))),
        ("stages", Json::arr(k.stages.iter().map(|n| stage_json(n, &k.reasoned)))),
        ("ways", Json::arr(k.ways.iter().map(Named::json))),
        ("endings", Json::arr(k.endings.iter().map(Json::str))),
        ("events", Json::arr(k.events.iter().map(|e| Json::str(*e)))),
        ("approvers_edit", Json::arr(k.approvers_edit.iter().map(Json::str))),
        ("masters", Json::arr(k.masters.iter().map(master_json))),
        ("flows", Json::arr(k.flows.iter().map(flow_json))),
    ])
}

/// A stage: its names and titles, and the events the kind asks a reason with at it
/// (`comment_required`, DESIGN 7.2), which a screen and a tool show the comment as required for.
fn stage_json(n: &Named, reasoned: &[&str]) -> Json {
    let mut j = n.json();
    if let Json::Obj(o) = &mut j {
        o.push(("comment_required".to_string(), Json::arr(reasoned.iter().map(|e| Json::str(*e)))));
    }
    j
}

/// A flow the kind uses: null for what dandori declares when the run does not read flows.
fn flow_json(f: &FlowOf) -> Json {
    let d = f.declared.as_ref();
    Json::obj([
        ("role", Json::str(&f.role)),
        ("alias", Json::str(&f.alias)),
        ("written", Json::str(&f.written)),
        ("file", Json::str(&f.file)),
        ("at", Json::arr(f.at.iter().map(Json::str))),
        ("workflow", d.map(|d| Json::str(&d.workflow)).unwrap_or(Json::Null)),
        ("version", d.map(|d| Json::int(i128::from(d.version))).unwrap_or(Json::Null)),
        ("workflow_type", d.map(|d| Json::str(&d.workflow_type)).unwrap_or(Json::Null)),
        // the workers dandori writes listen on the queue named as the workflow's type
        ("task_queue", d.map(|d| Json::str(&d.workflow_type)).unwrap_or(Json::Null)),
        ("outputs", d.map(|d| Json::arr(d.outputs.iter().map(|(n, t, o)| Json::obj([("name", Json::str(n)), ("type", Json::str(t)), ("optional", Json::from(*o))])))).unwrap_or(Json::Null)),
    ])
}

/// A field, with the same keys whatever its type.
pub(super) fn field_json(f: &Fld) -> Json {
    let int = |n: Option<i128>| n.map(Json::int).unwrap_or(Json::Null);
    Json::obj([
        ("name", Json::str(&f.named.name)),
        ("written", Json::str(&f.named.written)),
        ("label", Json::str(&f.named.label)),
        ("labels", labels_json(&f.named.labels)),
        ("type", Json::str(&f.type_ascii)),
        ("type_written", Json::str(&f.type_written)),
        ("min", f.end(f.min)),
        ("max", f.end(f.max)),
        ("count_min", int(f.count_min)),
        ("count_max", int(f.count_max)),
        ("choices", Json::arr(f.choices.iter().map(Named::json))),
        ("optional", Json::from(f.optional)),
        ("computed", Json::from(f.computed)),
        ("reads", Json::arr(f.reads.iter().map(Json::str))),
        ("worked_on_screen", Json::from(f.worked_on_screen)),
        ("approvers_only", Json::from(f.approvers_only)),
        ("open_in", f.open_in.as_ref().map(Json::str).unwrap_or(Json::Null)),
        ("shown_when", Cond::json(&f.shown_when)),
        ("required_when", Cond::json(&f.required_when)),
        ("fields", Json::arr(f.fields.iter().map(field_json))),
    ])
}

fn org_json(o: &OrgOf) -> Json {
    Json::obj([
        ("file", Json::str(&o.file)),
        ("name", Json::str(&o.name)),
        ("title", Json::str(&o.title)),
        ("positions", Json::arr(o.positions.iter().map(Named::json))),
        ("team_kinds", Json::arr(o.team_kinds.iter().map(Named::json))),
        ("data", o.data.as_ref().map(Json::str).unwrap_or(Json::Null)),
    ])
}

fn master_json(m: &MasterOf) -> Json {
    Json::obj([
        ("name", Json::str(&m.named.name)),
        ("written", Json::str(&m.named.written)),
        ("label", Json::str(&m.named.label)),
        ("masters", Json::str(&m.masters)),
        ("master", Json::str(&m.master)),
        ("file", Json::str(&m.file)),
        ("key", m.key.as_ref().map(Json::str).unwrap_or(Json::Null)),
        ("name_field", m.name_field.as_ref().map(Json::str).unwrap_or(Json::Null)),
        ("fields", Json::arr(m.fields.iter().map(field_json))),
        ("data", m.data.as_ref().map(Json::str).unwrap_or(Json::Null)),
    ])
}
