//! `mcp/tools.json`: the tools an agent is given (DESIGN 12.2), as a server that answers MCP lists
//! them (`name`, `title`, `description`, `inputSchema`, `annotations`). A tool is an operation of
//! `openapi.json`, named by its `operationId`: the waiting list and the person's own requests (each
//! in pages), a ticket, the kinds one may submit, a request of each kind, made again when it is sent
//! back and edited by an approver where the kind has them, what a form of each kind works out for the
//! values given (read alone, and on a ticket made again or edited by the version it was made on, as
//! the operations `worked_<name>` and `ticket_worked_<name>` answer), the approval, the send-back and the
//! rejection, the withdrawal, the discard of a request sent back and the retraction of an approval
//! (marked `destructiveHint`: none of them can be taken back), a comment, the organization and the
//! masters, and the finds of people, teams and the rows of each master by words. The operations of an administrator (`pause`, `resume`, `add_approver`, `hand_over`) are
//! no tools: they change who decides a ticket, or stop its clocks, on a person's own judgment of the
//! ticket and the organization (DESIGN 12.2). Each that moves a ticket takes what the person writes
//! with it (`comment`), which a reject and a send-back say they need where a kind asks a reason with
//! them (DESIGN 7.2); a resubmit and an edit say that a value of null keeps the one the ticket
//! holds, and that `clear` makes a field empty (DESIGN 7.3). A form's fields carry their labels, the
//! conditions they are shown and asked for on, and the labels of their choices, and a submit says
//! what its kind is for: an agent fills a form from the tools alone. What a tool is given goes where
//! the operation takes it: `idempotency_key` in the header `Idempotency-Key`, `id` in the path, `on`,
//! `limit`, `before`, `kind`, `q` and `lang` in the query, the rest in the body. Each input schema holds every
//! schema it refers to (`$defs`), so that it reads alone.

use super::openapi::{FINDS, KEY, NO_ORGANIZATION, PAGE, PAGE_MOST, UNOFFERED, UNSEEN, common_named, edit_body, event_body, key_schema, kind_schemas, resubmit_body, submit_body, worked_schemas};
use super::{Kind, head};
use ritsu_base::json::Json;

/// `j` with each reference to a schema of the API's components made one to the `$defs` beside it.
fn rebased(j: &Json) -> Json {
    match j {
        Json::Obj(o) => Json::Obj(
            o.iter()
                .map(|(k, v)| match (k.as_str(), v.as_str()) {
                    ("$ref", Some(s)) => (k.clone(), Json::str(s.replace("#/components/schemas/", "#/$defs/"))),
                    _ => (k.clone(), rebased(v)),
                })
                .collect(),
        ),
        Json::Arr(a) => Json::Arr(a.iter().map(rebased).collect()),
        x => x.clone(),
    }
}

/// The input of a tool: `props` with the ones `required` names required, and the schemas they
/// refer to.
fn input(props: Vec<(String, Json)>, required: &[String], defs: Vec<(String, Json)>) -> Json {
    let mut o = vec![
        ("type".to_string(), Json::str("object")),
        ("required".into(), Json::arr(required.iter().map(Json::str))),
        ("properties".into(), rebased(&Json::Obj(props))),
        ("additionalProperties".into(), Json::Bool(false)),
    ];
    if !defs.is_empty() {
        o.push(("$defs".into(), rebased(&Json::Obj(defs))));
    }
    Json::Obj(o)
}

/// What a tool does to the tickets: reads them, moves one, or moves one in a way that cannot be taken
/// back (a withdrawal, a discard, the retraction of an approval).
#[derive(Clone, Copy, PartialEq)]
enum Does {
    Reads,
    Moves,
    Ends,
}

fn tool(name: &str, title: &str, description: &str, input: Json, does: Does) -> Json {
    let annotations = match does {
        Does::Reads => Json::obj([("readOnlyHint", Json::Bool(true))]),
        _ => Json::obj([("readOnlyHint", Json::Bool(false)), ("destructiveHint", Json::Bool(does == Does::Ends)), ("idempotentHint", Json::Bool(true))]),
    };
    Json::obj([("name", Json::str(name)), ("title", Json::str(title)), ("description", Json::str(description)), ("inputSchema", input), ("annotations", annotations)])
}

fn text(t: &str) -> Json {
    Json::obj([("type", Json::str("string")), ("description", Json::str(t))])
}

fn key() -> (String, Json) {
    let mut o = key_schema().as_obj().map(|o| o.to_vec()).unwrap_or_default();
    o.push(("description".into(), Json::str(KEY)));
    ("idempotency_key".into(), Json::Obj(o))
}

/// What a find is asked (`q`, `limit`, `lang`).
fn asked() -> Vec<(String, Json)> {
    vec![
        ("q".into(), Json::obj([("type", Json::str("string")), ("maxLength", Json::int(crate::find::LONGEST as i128)), ("description", Json::str("The words to find by, apart with white space: up to 200 characters. None find every one"))])),
        ("limit".into(), Json::obj([("type", Json::str("integer")), ("minimum", Json::int(1)), ("maximum", Json::int(crate::find::MOST as i128)), ("description", Json::str(format!("The most it answers; {} when it is left out", crate::find::SHOWN)))])),
        ("lang".into(), Json::obj([("type", Json::str("string")), ("enum", Json::arr([Json::str("en"), Json::str("ja")])), ("description", Json::str("The language of the names; the server's when it is left out"))])),
    ]
}

/// The page of a list (`limit`, `before`).
fn page() -> Vec<(String, Json)> {
    vec![
        ("limit".into(), Json::obj([("type", Json::str("integer")), ("minimum", Json::int(1)), ("maximum", Json::int(PAGE_MOST)), ("description", Json::str(format!("The most tickets the page holds; {PAGE} when it is left out")))])),
        ("before".into(), text("The ID of the last ticket of the page before: the page holds the tickets after it. Left out for the first page")),
    ]
}

fn ticket_id() -> (String, Json) {
    ("id".into(), text("The ticket's ID"))
}

/// The properties of a body, with what it requires.
fn body_props(body: &Json) -> (Vec<(String, Json)>, Vec<String>) {
    let props = body.get("properties").and_then(Json::as_obj).map(|o| o.to_vec()).unwrap_or_default();
    let required = body.get("required").and_then(Json::as_arr).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
    (props, required)
}

pub(super) fn json(kinds: &[Kind]) -> Json {
    let mut tools = Vec::new();
    let none = || input(Vec::new(), &[], Vec::new());
    tools.push(tool("list_requests", "Kinds of request", "The kinds of request the person may submit, each by its name, version and title, with what it is for", none(), Does::Reads));
    tools.push(tool("list_waiting", "Waiting tickets", "The tickets that wait for the person's answer, or for the answer of one they stand in for as a proxy that day, the newest first, in pages: `more` says whether there is a next page, read with the last ticket's ID as `before`", input(page(), &[], Vec::new()), Does::Reads));
    let mut mine = page();
    mine.push(("kind".into(), Json::obj([("type", Json::str("string")), ("enum", Json::arr(kinds.iter().map(|k| Json::str(&k.named.name)))), ("description", Json::str("Keeps the tickets of one kind of request; all kinds when it is left out"))])));
    tools.push(tool("list_mine", "My requests", "The person's own requests, as they are now: the tickets they are the requester of, or made for someone else as their proxy, the newest first, in pages: `more` says whether there is a next page, read with the last ticket's ID as `before`", input(mine, &[], Vec::new()), Does::Reads));
    let seen = format!("A ticket and its history: its state, the stages of its route with the approvers and the answers of each, its form. Answered 404: {UNSEEN}");
    tools.push(tool("get_ticket", "A ticket", &seen, input(vec![ticket_id()], &["id".to_string()], Vec::new()), Does::Reads));
    for k in kinds {
        let n = &k.named.name;
        let (mut props, mut required) = body_props(&submit_body(k));
        props.insert(0, key());
        required.insert(0, "idempotency_key".into());
        let what = k.description.as_ref().map(|d| format!(". What it is for: {d}")).unwrap_or_default();
        let description = format!("Make a request of `{n}` ({}), with the values of its form{what}. Answered 404: {UNOFFERED}", k.named.label);
        let defs: Vec<(String, Json)> = kind_schemas(k).into_iter().filter(|(d, _)| !d.ends_with(".changes") && !d.ends_with(".edited")).collect();
        tools.push(tool(&format!("submit_{n}"), &k.named.label, &description, input(props, &required, defs), Does::Moves));
        // made again when it is sent back, and edited by an approver, where the kind has them
        let again = [
            ("resubmit", "Make again", format!("Make the request of `{n}` ({}) sent back again, with the values that change", k.named.label), resubmit_body(k), ".changes"),
            ("edit", "Edit", format!("Fill in, correct or make empty the fields an approver gives on a ticket of `{n}` ({}) that waits for the person's answer", k.named.label), edit_body(k), ".edited"),
        ];
        for (e, verb, what, body, values) in again.into_iter().filter(|(e, ..)| k.has(e)) {
            let (mut props, mut required) = body_props(&body);
            props.insert(0, ticket_id());
            props.insert(0, key());
            required.splice(0..0, ["idempotency_key".to_string(), "id".to_string()]);
            let mut defs = vec![("revision".to_string(), common_named("revision"))];
            defs.extend(kind_schemas(k).into_iter().filter(|(d, _)| d.ends_with(values) || d.ends_with(".row")));
            let description = format!(
                "{what}, on the revision of the ticket one saw (`get_ticket`). A field left out, or given null, keeps the value the ticket holds: to make a field empty, name it in `clear`. A field the values given hide loses its value without being named, and the line of the ticket's history says each field that lost its value (`cleared`)"
            );
            tools.push(tool(&format!("{e}_{n}"), &format!("{verb}: {}", k.named.label), &description, input(props, &required, defs), Does::Moves));
        }
        // what the form works out, read alone (DESIGN 13.7): of the kind, and on a ticket made again
        // or edited, by the rules of the version it was made on
        let worked_in = worked_schemas(k).into_iter().find(|(d, _)| d.ends_with(".worked.in")).map(|(_, s)| s).unwrap_or(Json::Null);
        let (props, required) = body_props(&worked_in);
        let undecided = "null where it is not decided yet: a field it reads that the form asks for is empty, or a value it reads is outside its field's range or none of its choices";
        let description = format!(
            "What the form of `{n}` ({}) works out for the values given, as its screens work it out: whether each field shown or asked for on a condition is, and the value of each field it works out; {undecided}. It reads no ticket and no organization, changes nothing, and answers every person, by the version of the kind a new request takes now. Answered 404: {UNOFFERED}",
            k.named.label
        );
        tools.push(tool(&format!("worked_{n}"), &format!("Worked out: {}", k.named.label), &description, input(props.clone(), &required, Vec::new()), Does::Reads));
        if k.has("resubmit") || k.has("edit") {
            let (mut props, mut required) = (props, required);
            props.insert(0, ticket_id());
            required.insert(0, "id".into());
            let description = format!(
                "What the form of `{n}` ({}) works out for the values given, by the rules of the version of the kind the ticket was made on, which decide the request made again and the approvers' edits; {undecided}. Of the ticket it reads the version alone, and answers nothing of it. Answered 404 for a ticket that is not of `{n}`, whoever asks, as for no such ticket, and: {UNSEEN}",
                k.named.label
            );
            tools.push(tool(&format!("ticket_worked_{n}"), &format!("Worked out on a ticket: {}", k.named.label), &description, input(props, &required, Vec::new()), Does::Reads));
        }
    }
    // the requester's side ends a request (`withdraw`, `discard`), and an approver takes back their
    // approval (`retract`): none of them is taken back
    let events = [("approve", "Approve", Does::Moves), ("send_back", "Send back", Does::Moves), ("reject", "Reject", Does::Moves), ("withdraw", "Withdraw", Does::Ends), ("discard", "Discard", Does::Ends), ("retract", "Retract approval", Does::Ends), ("comment", "Comment", Does::Moves)];
    for (e, title, does) in events {
        if !kinds.iter().any(|k| k.has(e)) {
            continue;
        }
        let (summary, body) = event_body(e);
        let (mut props, mut required) = body_props(&body);
        props.insert(0, ticket_id());
        props.insert(0, key());
        required.splice(0..0, ["idempotency_key".to_string(), "id".to_string()]);
        // a comment says no revision: it moves nothing
        let (defs, description) = match e {
            "comment" => (Vec::new(), format!("{summary}. It says no revision of the ticket. The server may refuse a comment that comes too soon after one's last on the ticket (to be sent again later, with the same key), or one past the most comments a ticket holds")),
            _ => {
                let ends = if does == Does::Ends { ". It cannot be undone" } else { "" };
                // a kind that asks a reason with it (`comment required for`, DESIGN 7.2)
                let reason = if kinds.iter().any(|k| k.reasoned.contains(&e)) { ". A kind whose stages list it in `comment_required` takes it only with a reason in `comment`" } else { "" };
                (vec![("revision".to_string(), common_named("revision"))], format!("{summary}, on the revision of the ticket one saw (`get_ticket`){ends}{reason}"))
            }
        };
        tools.push(tool(e, title, &description, input(props, &required, defs), does));
    }
    tools.push(tool("get_organization", "The organization", &format!("The version of the organization in effect today: its teams, people and members. Answered 404: {NO_ORGANIZATION}"), none(), Does::Reads));
    tools.push(tool("find_people", "Find people", &format!("The people of the version of the organization in effect today, {FINDS}. Answered 404: {NO_ORGANIZATION}"), input(asked(), &[], Vec::new()), Does::Reads));
    tools.push(tool("find_teams", "Find teams", &format!("The teams of the version of the organization in effect today, {FINDS}. Answered 404: {NO_ORGANIZATION}"), input(asked(), &[], Vec::new()), Does::Reads));
    let mut seen: Vec<(&str, &str)> = Vec::new();
    for m in kinds.iter().flat_map(|k| &k.masters) {
        if seen.contains(&(m.masters.as_str(), m.master.as_str())) {
            continue;
        }
        seen.push((&m.masters, &m.master));
        let on = ("on".to_string(), Json::obj([("type", Json::str("string")), ("format", Json::str("date")), ("description", Json::str("The day the rows may be chosen on; today when it is left out"))]));
        tools.push(tool(&format!("get_master_{}_{}", m.masters, m.master), &m.named.label, &format!("The rows of the master `{}` one may choose on a day", m.named.label), input(vec![on.clone()], &[], Vec::new()), Does::Reads));
        let mut find = asked();
        find.push(on);
        let title = format!("Find: {}", m.named.label);
        let text = format!("The rows of the master `{}` one may choose on a day (`on`, today when it is left out), {FINDS}", m.named.label);
        tools.push(tool(&format!("find_master_{}_{}", m.masters, m.master), &title, &text, input(find, &[], Vec::new()), Does::Reads));
    }
    Json::obj([("//", Json::str(head())), ("contract", Json::int(crate::lower::shape::CONTRACT)), ("tools", Json::Arr(tools))])
}
