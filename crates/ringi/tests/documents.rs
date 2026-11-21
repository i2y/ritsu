//! What the documents of `--target requests` say to a server and to an agent (DESIGN 12.2, 12.4),
//! for the examples: the tools of `mcp/tools.json` (which operations an agent has, and which cannot
//! be taken back), what a form works out, read alone, the key of an operation, the pages of the
//! lists and the limit on operations, every answer of each operation and the one shape of the body
//! of an error, the ranges of a revision, the refusal's sentences and place, what a submit and its
//! fields say, who sees a ticket and what each 404 says, the finds of people, teams and rows of
//! masters, the flows of a kind's description, the language and the titles of a kind in the list
//! of kinds, the events a kind asks a reason with, and the language a person reads in.

mod common;

use ringi::ports::Engine;
use ritsu_base::json::{self, Json};
use std::path::PathBuf;

/// The documents written for the examples, as `ritsu gen` writes them (the screens aside): the
/// API, the tools, each kind's description, by its name, and the schema and the columns of the
/// organization's data.
struct Docs {
    api: Json,
    tools: Json,
    types: Vec<(String, Json)>,
    org_schema: Json,
    org_columns: String,
}

fn docs() -> Docs {
    let requests: Vec<PathBuf> = common::example_files().into_iter().filter(|f| !f.starts_with("examples/shared/")).map(PathBuf::from).collect();
    let engine = Engine::new(common::joined());
    engine.keep_listing_apart();
    let files = engine.requests_with(&requests, &PathBuf::from("."), None).unwrap_or_else(|t| panic!("{}", t.en));
    let get = |p: &str| json::parse(&files.iter().find(|(f, _, _)| f == p).unwrap_or_else(|| panic!("{p} is not written")).1).unwrap();
    let types = files
        .iter()
        .filter_map(|(f, t, _)| Some((f.strip_prefix("types/")?.strip_suffix(".json")?.to_string(), json::parse(t).unwrap())))
        .collect();
    let columns = files.iter().find(|(f, _, _)| f == "organization/columns.md").map(|(_, t, _)| t.clone()).expect("organization/columns.md is not written");
    Docs { api: get("openapi.json"), tools: get("mcp/tools.json"), types, org_schema: get("organization/schema.json"), org_columns: columns }
}

fn str_at<'j>(j: &'j Json, path: &[&str]) -> &'j str {
    path.iter().try_fold(j, |j, k| j.get(k)).and_then(Json::as_str).unwrap_or_else(|| panic!("no {path:?}"))
}

fn tool<'j>(d: &'j Docs, name: &str) -> &'j Json {
    d.tools.get("tools").and_then(Json::as_arr).unwrap().iter().find(|t| t.get("name").and_then(Json::as_str) == Some(name)).unwrap_or_else(|| panic!("no tool {name}"))
}

/// The operation of the API named `id`.
fn operation<'j>(d: &'j Docs, id: &str) -> &'j Json {
    for item in d.api.get("paths").and_then(Json::as_obj).unwrap().iter().map(|(_, i)| i) {
        for (_, op) in item.as_obj().unwrap() {
            if op.get("operationId").and_then(Json::as_str) == Some(id) {
                return op;
            }
        }
    }
    panic!("no operation {id}")
}

/// The tools are the operations a person asks their assistant for: reading the lists and a ticket,
/// a request of each kind, what an approver answers, and the requester's withdrawal, discard and
/// retraction, marked as what cannot be taken back. An administrator's operations are no tools
/// (DESIGN 12.2).
#[test]
fn the_tools_are_what_a_person_asks_an_assistant_for() {
    let d = docs();
    let names: Vec<&str> = d.tools.get("tools").and_then(Json::as_arr).unwrap().iter().map(|t| t.get("name").and_then(Json::as_str).unwrap()).collect();
    for n in ["list_requests", "list_waiting", "list_mine", "get_ticket", "approve", "send_back", "reject", "withdraw", "discard", "retract", "comment", "submit_purchase", "edit_contract", "find_people", "find_teams"] {
        assert!(names.contains(&n), "no tool {n}: {names:?}");
    }
    for n in ["pause", "resume", "add_approver", "hand_over"] {
        assert!(!names.contains(&n), "{n} is an administrator's, and no tool");
    }
    // a tool that reads says so; one that moves a ticket says whether it can be taken back
    for n in &names {
        let hint = |h: &str| tool(&d, n).get("annotations").and_then(|a| a.get(h)).and_then(Json::as_bool);
        let reads = n.starts_with("list_") || n.starts_with("get_") || n.starts_with("find_") || n.starts_with("worked_") || n.starts_with("ticket_worked_");
        assert_eq!(hint("readOnlyHint"), Some(reads), "{n}");
        assert_eq!(hint("destructiveHint"), (!reads).then_some(matches!(*n, "withdraw" | "discard" | "retract")), "{n}");
    }
}

/// What a form works out is an operation and a tool of each kind (DESIGN 12.4, 13.7), read alone:
/// no idempotency key and no revision, nothing it changes; its body what the entry `worked` takes,
/// its answer what the entry answers, and 400 for a body not of its shape, in the words of the
/// entry's error. The kind's answers every person, and says why: it reads no ticket and no
/// organization, and the screens' functions give every browser the same rules; while no version of
/// the kind is in effect, it answers 404 as a submit does. A kind that sends back or whose approvers
/// edit has one on a ticket too, by the rules of the version the ticket was made on, which answers
/// 404 to one who may not see the ticket; a kind with neither has none.
#[test]
fn what_a_form_works_out_is_read_alone() {
    let d = docs();
    let paths = d.api.get("paths").and_then(Json::as_obj).unwrap();
    let post = |p: &str| paths.iter().find(|(q, _)| q == p).and_then(|(_, i)| i.get("post"));
    let codes = |op: &Json| -> Vec<String> { op.get("responses").and_then(Json::as_obj).unwrap().iter().map(|(c, _)| c.clone()).collect() };
    let keys = |j: &Json, at: &str| -> Vec<String> {
        match j.get("inputSchema").and_then(|s| s.get(at)) {
            Some(Json::Obj(o)) => o.iter().map(|(k, _)| k.clone()).collect(),
            Some(Json::Arr(a)) => a.iter().filter_map(Json::as_str).map(str::to_string).collect(),
            _ => Vec::new(),
        }
    };
    let mut on_tickets = 0;
    for (n, _) in &d.types {
        let op = post(&format!("/requests/{n}/worked")).unwrap_or_else(|| panic!("no POST /requests/{n}/worked"));
        let again = post(&format!("/requests/{n}/tickets/{{id}}/resubmit")).is_some() || post(&format!("/requests/{n}/tickets/{{id}}/edit")).is_some();
        let on = post(&format!("/requests/{n}/tickets/{{id}}/worked"));
        assert_eq!(on.is_some(), again, "{n}: a form worked out on a ticket where the kind makes it again or edits it, and only there");
        for (id, op, ticket) in [(format!("worked_{n}"), op, false)].into_iter().chain(on.map(|o| (format!("ticket_worked_{n}"), o, true))) {
            assert_eq!(str_at(op, &["operationId"]), id);
            assert_eq!(str_at(op, &["x-ritsu-request", "name"]), n.as_str(), "{id}");
            let params: Vec<&str> = op.get("parameters").and_then(Json::as_arr).unwrap_or_default().iter().filter_map(|p| p.get("$ref").and_then(Json::as_str)).collect();
            assert_eq!(params, if ticket { vec!["#/components/parameters/id"] } else { vec![] }, "{id} takes no idempotency key");
            assert_eq!(str_at(op, &["requestBody", "content", "application/json", "schema", "$ref"]), format!("#/components/schemas/{n}.worked.in"), "{id}");
            assert_eq!(str_at(op, &["responses", "200", "content", "application/json", "schema", "$ref"]), format!("#/components/schemas/{n}.worked.out"), "{id}");
            assert_eq!(str_at(op, &["responses", "400", "$ref"]), "#/components/responses/unworked", "{id}");
            assert_eq!(codes(op), ["200", "400", "401", "404", "413", "415", "500"], "{id}");
            assert_eq!(str_at(op, &["responses", "404", "$ref"]), if ticket { "#/components/responses/unseen" } else { "#/components/responses/unoffered" }, "{id}");
            let said = str_at(op, &["summary"]);
            let words: &[&str] = if ticket {
                &["the version of the kind the ticket was made on", "nothing of the ticket is answered", "It changes nothing", "404 to the others, and for a ticket of another kind"]
            } else {
                &["null while it is not decided", "It changes nothing", "reads no ticket and no organization", "`functions.js` already gives the same rules to every person's browser", "it answers every person the server authenticates", "by the version of the kind a new request takes now, and 404 while none is in effect"]
            };
            for w in words {
                assert!(said.contains(w), "{id}: {said}");
            }
            // the tool: the body's keys, the ticket's ID before them on a ticket
            let t = tool(&d, &id);
            let (props, required) = if ticket { (vec!["id", "form", "edited"], vec!["id", "form"]) } else { (vec!["form", "edited"], vec!["form"]) };
            assert_eq!((keys(t, "properties"), keys(t, "required")), (props.iter().map(|s| s.to_string()).collect(), required.iter().map(|s| s.to_string()).collect()), "{id}");
            assert!(str_at(t, &["description"]).contains("null where it is not decided yet"), "{id}");
            on_tickets += usize::from(ticket);
        }
    }
    assert!(on_tickets > 0, "no kind of the examples makes a request again or edits it");
    let said = str_at(&d.api, &["components", "responses", "unworked", "description"]);
    for w in ["the generated entry `worked` stops at", "in the language of the list of kinds", "are no error: what reads them is answered as not decided"] {
        assert!(said.contains(w), "{said}");
    }
}

/// Each 404 says what is not there (DESIGN 12.4, 14.2). On a ticket, no such ticket or one the
/// person may not see; on an operation of one kind on a ticket (`/requests/<name>/tickets/{id}/…`:
/// the request made again, an approver's edit, what the form works out on it), a ticket of another
/// kind too, as for no such ticket: one answer (`unseen`), whose text says both, for outside the
/// server they are the same. A
/// submit and what a kind's form works out, no version of the kind in effect now (`unoffered`):
/// to one the gate lets submit the kind, with the day the nearest later version starts (`from`, a
/// date it need not give), and to the others as for a kind there is not. The organization — no
/// version in effect, a person not of its people, or the finds of its people and teams
/// (`no_organization`); a find of a master's rows, no kind the server holds choosing from it
/// (`unchosen`). No other operation answers 404, and the tools of them say it too.
#[test]
fn each_404_says_what_is_not_there() {
    let d = docs();
    let mut said = Vec::new();
    for (path, item) in d.api.get("paths").and_then(Json::as_obj).unwrap() {
        for (_, op) in item.as_obj().unwrap() {
            let id = str_at(op, &["operationId"]);
            let want = if path.contains("{id}") {
                Some("unseen")
            } else if id.starts_with("submit_") || id.starts_with("worked_") {
                Some("unoffered")
            } else if id == "get_organization" || id == "find_people" || id == "find_teams" {
                Some("no_organization")
            } else if id.starts_with("find_master_") {
                Some("unchosen")
            } else {
                None
            };
            let got = op.get("responses").and_then(|r| r.get("404")).map(|r| str_at(r, &["$ref"]).trim_start_matches("#/components/responses/"));
            assert_eq!(got, want, "{id} ({path})");
            if let Some(w) = want {
                said.push((id, w));
            }
        }
    }
    for (head, w) in [
        ("resubmit_", "unseen"),
        ("edit_", "unseen"),
        ("ticket_worked_", "unseen"),
        ("get_ticket", "unseen"),
        ("approve", "unseen"),
        ("submit_", "unoffered"),
        ("worked_", "unoffered"),
        ("get_organization", "no_organization"),
        ("find_people", "no_organization"),
        ("find_teams", "no_organization"),
        ("find_master_", "unchosen"),
    ] {
        assert!(said.iter().any(|(id, x)| id.starts_with(head) && *x == w), "no {head}… answers 404 `{w}`: {said:?}");
    }
    let responses = d.api.get("components").and_then(|c| c.get("responses")).unwrap();
    // one answer on a ticket: what every operation on one says first, then what one of a kind says
    let of_tickets = str_at(responses, &["unseen", "description"]);
    let (unseen, of_kind) = of_tickets.split_once(". An operation of one kind of request on a ticket (`/requests/<name>/tickets/{id}/…`").unwrap_or_else(|| panic!("the 404 on a ticket says what an operation of one kind answers too: {of_tickets}"));
    assert!(unseen.starts_with("No such ticket, or one the person may not see.") && !unseen.contains("of the kind"), "{of_tickets}");
    assert!(of_kind.contains("answers 404 too for a ticket that is not of the kind `<name>`, whoever asks, as for no such ticket"), "{of_tickets}");
    assert!(responses.as_obj().unwrap().iter().all(|(k, _)| k == "unseen" || !k.starts_with("unseen")), "one answer for a ticket that is not there");
    // no version in effect: the gate is asked first, and `from` is the one key the server must mean
    let unoffered = str_at(responses, &["unoffered", "description"]);
    for w in ["No version of the kind is in effect now", "The server asks the gate first whether the person may submit the kind (`submit`)", "it answers as for a kind there is not, saying nothing of its versions (no `from`)", "`from` says the day the nearest later version starts"] {
        assert!(unoffered.contains(w), "{unoffered}");
    }
    // the tools say it as the API does
    for (id, w) in &said {
        let t = d.tools.get("tools").and_then(Json::as_arr).unwrap().iter().find(|t| t.get("name").and_then(Json::as_str) == Some(*id));
        let Some(t) = t else { continue };
        let text = str_at(t, &["description"]);
        match *w {
            "unoffered" => assert!(text.contains(&format!("Answered 404: {unoffered}")), "{id}: {text}"),
            "unseen" if id.starts_with("ticket_worked_") => {
                let n = id.trim_start_matches("ticket_worked_");
                assert!(text.contains(&format!("Answered 404 for a ticket that is not of `{n}`, whoever asks, as for no such ticket, and: {unseen}")), "{id}: {text}");
            }
            "unseen" if *id == "get_ticket" => assert!(text.contains(&format!("Answered 404: {unseen}")), "{id}: {text}"),
            "no_organization" => assert!(text.contains(&format!("Answered 404: {}", str_at(responses, &["no_organization", "description"]))), "{id}: {text}"),
            _ => {}
        }
    }
}

/// Each operation lists every answer the server gives it (DESIGN 12.4), and every answer of an
/// error holds its text in one shape (`error`, `{"error": "…"}`, closed, as the server writes it),
/// but a refusal (409), which is the machine's, and the 404 of no version in effect, which may hold
/// the day the next version starts besides (`from`). Every operation may answer 401 and 500, and
/// one with a body 413 and 415; a list, a master's rows and a find answer 400 for a query not as
/// they take it.
#[test]
fn an_error_answers_its_text_in_one_shape() {
    let d = docs();
    let components = d.api.get("components").unwrap();
    let keys = |s: &Json| -> Vec<String> { s.get("properties").and_then(Json::as_obj).unwrap().iter().map(|(k, _)| k.clone()).collect() };
    let required = |s: &Json| -> Vec<String> { s.get("required").and_then(Json::as_arr).unwrap_or_default().iter().filter_map(Json::as_str).map(str::to_string).collect() };
    let error = components.get("schemas").and_then(|s| s.get("error")).expect("no schema `error`");
    assert_eq!((keys(error), required(error), error.get("additionalProperties")), (vec!["error".to_string()], vec!["error".to_string()], Some(&Json::Bool(false))));
    assert_eq!(str_at(error, &["properties", "error", "type"]), "string");
    let responses = components.get("responses").unwrap();
    let mut asked = Vec::new();
    for (path, item) in d.api.get("paths").and_then(Json::as_obj).unwrap() {
        for (_, op) in item.as_obj().unwrap() {
            let id = str_at(op, &["operationId"]);
            let answers = op.get("responses").and_then(Json::as_obj).unwrap();
            let codes: Vec<&str> = answers.iter().map(|(c, _)| c.as_str()).collect();
            let mut sorted = codes.clone();
            sorted.sort();
            assert_eq!(codes, sorted, "{id}: the answers in the order of their codes");
            let body = op.get("requestBody").is_some();
            for code in ["401", "500"].into_iter().chain(if body { vec!["413", "415"] } else { vec![] }) {
                assert!(codes.contains(&code), "{id} ({path}) does not say it may answer {code}");
            }
            if !body && (id.starts_with("list_waiting") || id.starts_with("list_mine") || id.starts_with("get_master_") || id.starts_with("find_")) {
                assert_eq!(str_at(op, &["responses", "400", "$ref"]), "#/components/responses/unasked", "{id}");
                asked.push(id);
            }
            for (code, answer) in answers {
                if code == "200" {
                    continue;
                }
                let name = str_at(answer, &["$ref"]).trim_start_matches("#/components/responses/");
                let schema = responses.get(name).and_then(|r| r.get("content")).and_then(|c| c.get("application/json")).and_then(|j| j.get("schema")).unwrap_or_else(|| panic!("{id} {code}: `{name}` says no body"));
                match (code.as_str(), name) {
                    ("409", _) => assert_eq!(str_at(schema, &["$ref"]), "#/components/schemas/refusal", "{id}"),
                    (_, "unoffered") => {
                        assert_eq!((keys(schema), required(schema), schema.get("additionalProperties")), (vec!["error".to_string(), "from".to_string()], vec!["error".to_string()], Some(&Json::Bool(false))), "{id}");
                        assert_eq!((str_at(schema, &["properties", "from", "type"]), str_at(schema, &["properties", "from", "format"])), ("string", "date"));
                    }
                    _ => assert_eq!(str_at(schema, &["$ref"]), "#/components/schemas/error", "{id} {code} (`{name}`)"),
                }
            }
        }
    }
    assert!(asked.len() >= 4, "{asked:?}");
    let unasked = str_at(responses, &["unasked", "description"]);
    for w in ["of a list, a `limit` that is not a whole number from 1 to 100", "a `kind` that is no kind of request", "of a find, words (`q`) not written in UTF-8 or of more than 200 characters", "an `on` not written `YYYY-MM-DD`"] {
        assert!(unasked.contains(w), "{unasked}");
    }
}

/// An operation's key says when a new one is made and when the same is sent again, and its form:
/// in the API's header and in each tool that moves a ticket.
#[test]
fn the_key_says_when_it_is_new_and_its_form() {
    let d = docs();
    let header = d.api.get("components").and_then(|c| c.get("parameters")).and_then(|p| p.get("idempotency_key")).unwrap_or_else(|| panic!("no header"));
    let key = tool(&d, "approve").get("inputSchema").and_then(|s| s.get("properties")).and_then(|p| p.get("idempotency_key")).unwrap();
    for (what, said, schema) in [("the header", str_at(header, &["description"]), header.get("schema").unwrap()), ("the tool", str_at(key, &["description"]), key)] {
        for words in ["A new operation takes a new key", "the same call sent again", "takes the same key", "255 bytes"] {
            assert!(said.contains(words), "{what}: {said}");
        }
        assert_eq!(schema.get("maxLength").and_then(Json::as_int), Some(255), "{what}");
        assert_eq!(schema.get("pattern").and_then(Json::as_str), Some("^[!-~]+$"), "{what}");
    }
}

/// The lists come in pages (`limit`, at most 100 and 50 when it is left out, and `before`), the
/// person's own requests by kind too, the larger ID first where the time is the same, and each page
/// says whether there is a next (`more`); a comment alone may be limited (429), with `Retry-After`
/// when it came too soon and none past the most a ticket holds, and an operation on a ticket the
/// person may not see is answered 404, which says who sees one.
#[test]
fn the_lists_come_in_pages_and_the_answers_are_said() {
    let d = docs();
    let limit = d.api.get("components").and_then(|c| c.get("parameters")).and_then(|p| p.get("limit")).and_then(|l| l.get("schema")).unwrap();
    assert_eq!((limit.get("minimum").and_then(Json::as_int), limit.get("maximum").and_then(Json::as_int), limit.get("default").and_then(Json::as_int)), (Some(1), Some(100), Some(50)));
    for (id, more) in [("list_waiting", 0), ("list_mine", 1)] {
        let params: Vec<String> = operation(&d, id).get("parameters").and_then(Json::as_arr).unwrap().iter().map(|p| p.get("$ref").and_then(Json::as_str).map(String::from).unwrap_or_else(|| str_at(p, &["name"]).to_string())).collect();
        assert_eq!(params.len(), 2 + more, "{id}: {params:?}");
        assert!(params.iter().any(|p| p.ends_with("/limit")) && params.iter().any(|p| p.ends_with("/before")), "{id}: {params:?}");
        let input = tool(&d, id).get("inputSchema").and_then(|s| s.get("properties")).unwrap();
        assert!(input.get("limit").is_some() && input.get("before").is_some(), "{id}");
        let answer = operation(&d, id).get("responses").and_then(|r| r.get("200")).and_then(|r| r.get("content")).and_then(|c| c.get("application/json")).and_then(|c| c.get("schema")).unwrap();
        let required: Vec<&str> = answer.get("required").and_then(Json::as_arr).unwrap().iter().filter_map(Json::as_str).collect();
        assert_eq!(required, ["tickets", "more"], "{id}");
        assert_eq!(str_at(answer, &["properties", "more", "type"]), "boolean", "{id}");
        assert!(str_at(operation(&d, id), &["summary"]).contains("the larger ID first (the later one) where that is the same"), "{id}");
        assert!(str_at(tool(&d, id), &["description"]).contains("`more` says whether there is a next page"), "{id}");
    }
    let kinds = operation(&d, "list_mine").get("parameters").and_then(Json::as_arr).unwrap().iter().find(|p| p.get("name").and_then(Json::as_str) == Some("kind")).and_then(|p| p.get("schema")).and_then(|s| s.get("enum")).and_then(Json::as_arr).unwrap().len();
    assert_eq!(kinds, d.types.len());
    for (id, codes) in [
        ("list_waiting", &["200", "400", "401", "500"][..]),
        ("get_ticket", &["200", "401", "404", "500"]),
        ("approve", &["200", "400", "401", "404", "409", "413", "415", "500"]),
        ("comment", &["200", "400", "401", "404", "409", "413", "415", "429", "500"]),
    ] {
        let got: Vec<&str> = operation(&d, id).get("responses").and_then(Json::as_obj).unwrap().iter().map(|(c, _)| c.as_str()).collect();
        assert_eq!(got, codes, "{id}");
    }
    // no operation but the comment answers 429
    let mut limited = Vec::new();
    for item in d.api.get("paths").and_then(Json::as_obj).unwrap().iter().map(|(_, i)| i) {
        for (_, op) in item.as_obj().unwrap() {
            if op.get("responses").and_then(|r| r.get("429")).is_some() {
                limited.push(str_at(op, &["operationId"]));
            }
        }
    }
    assert_eq!(limited, ["comment"]);
    let said = str_at(&d.api, &["components", "responses", "limited", "description"]);
    for words in ["how soon a person comments on a ticket again", "how many comments a ticket holds", "the ticket is as it was", "with the same idempotency key", "there is no `Retry-After`"] {
        assert!(said.contains(words), "{said}");
    }
    let retry = d.api.get("components").and_then(|c| c.get("responses")).and_then(|r| r.get("limited")).and_then(|l| l.get("headers")).and_then(|h| h.get("Retry-After")).unwrap();
    assert_eq!((str_at(retry, &["schema", "type"]), retry.get("schema").and_then(|s| s.get("minimum")).and_then(Json::as_int)), ("integer", Some(1)));
    assert!(str_at(retry, &["description"]).contains("only when it came too soon"));
    assert!(str_at(tool(&d, "comment"), &["description"]).contains("comes too soon after one's last on the ticket"));
    let unseen = str_at(&d.api, &["components", "responses", "unseen", "description"]);
    for words in ["the requester's side", "the approvers' side", "the gate lets `see` it", "whatever the gate lets them do"] {
        assert!(unseen.contains(words), "{unseen}");
        assert!(str_at(tool(&d, "get_ticket"), &["description"]).contains(words), "get_ticket");
    }
}

/// The ranges of a revision in the API are the ones the machine reads it in (`record Revision` of
/// the machine's code): a revision outside them, or with a key left out, is answered 400, not taken
/// to the machine; what the person writes is the machine's to refuse, 409.
#[test]
fn the_ranges_of_a_revision_are_the_machines() {
    let d = docs();
    let kn = std::fs::read_to_string("src/lower/ticket.kn").unwrap();
    let record: Vec<&str> = kn.lines().skip_while(|l| *l != "record Revision").skip(1).take_while(|l| !l.trim().is_empty()).collect();
    assert_eq!(record.len(), 3, "{record:?}");
    let revision = d.api.get("components").and_then(|c| c.get("schemas")).and_then(|s| s.get("revision")).and_then(|r| r.get("properties")).unwrap();
    for line in record {
        let (name, rest) = line.trim().split_once(':').unwrap();
        let ends: Vec<i128> = rest.split_whitespace().filter_map(|w| w.trim_start_matches(">=").trim_start_matches("<=").replace('_', "").parse().ok()).collect();
        let p = revision.get(name.trim()).unwrap_or_else(|| panic!("no `{name}`"));
        assert_eq!(ends, [p.get("minimum").and_then(Json::as_int).unwrap(), p.get("maximum").and_then(Json::as_int).unwrap()], "{name}");
    }
    let unread = str_at(&d.api, &["components", "responses", "unread", "description"]);
    for words in ["a revision with a key left out or outside its ranges", "What the person writes is the machine's to refuse (409)", "as `form`, a value of the form", "a field it asks for left empty, null or left out", "as `comment`, a comment's text left empty or longer than 2,000 characters"] {
        assert!(unread.contains(words), "{unread}");
    }
}

/// A refusal says why in English and in Japanese and names the place of the form it is of; a
/// submit says what its kind is for, and a field the conditions it is shown on.
#[test]
fn a_refusal_and_a_form_say_what_an_agent_reads() {
    let d = docs();
    let refusal = d.api.get("components").and_then(|c| c.get("schemas")).and_then(|s| s.get("refusal")).unwrap();
    let keys: Vec<&str> = refusal.get("required").and_then(Json::as_arr).unwrap().iter().filter_map(Json::as_str).collect();
    assert_eq!(keys, ["refused", "text", "text_ja", "at"]);
    // a refusal of what the person wrote is a kind of its own, named apart from a field of the form
    let kinds: Vec<&str> = refusal.get("properties").and_then(|p| p.get("refused")).and_then(|r| r.get("enum")).and_then(Json::as_arr).unwrap().iter().filter_map(Json::as_str).collect();
    assert!(kinds.contains(&"comment") && kinds.contains(&"form"), "{kinds:?}");
    assert!(str_at(refusal, &["properties", "refused", "description"]).contains("`comment` refuses what the person wrote with the operation"));
    for (kind, t) in &d.types {
        let said = str_at(tool(&d, &format!("submit_{kind}")), &["description"]);
        if let Some(about) = t.get("description").and_then(Json::as_str) {
            assert!(said.contains(about), "submit_{kind}: {said}");
        }
    }
    let risks = tool(&d, "edit_contract").get("inputSchema").and_then(|s| s.get("$defs")).and_then(Json::as_obj).unwrap().iter().find_map(|(_, s)| s.get("properties")?.get("risks")).unwrap();
    assert!(str_at(risks, &["description"]).contains("Shown only when `kind is not nda`"), "{}", risks.compact());
}

/// A kind's description lists the flows it reads, each with its alias, its role, the workflow and
/// its version, its type and task queue, and its outputs in the order the flow declares them; in
/// each of the five examples, and in its Japanese version the same.
#[test]
fn a_kind_says_the_flows_it_reads() {
    let d = docs();
    let flows = |k: &str| d.types.iter().find(|(n, _)| n == k).and_then(|(_, t)| t.get("flows")).and_then(Json::as_arr).unwrap_or_else(|| panic!("{k}: no flows")).to_vec();
    let mut seen = 0;
    for k in ["account", "contract", "expense", "leave", "purchase"] {
        let (en, ja) = (flows(k), flows(&format!("{k}_ja")));
        assert_eq!(en.len(), ja.len(), "{k}");
        for (e, j) in en.iter().zip(&ja) {
            for key in ["role", "alias", "workflow", "version", "workflow_type", "task_queue", "outputs"] {
                assert_eq!(e.get(key), j.get(key), "{k}: {key}");
            }
            // the alias as the request declares it, and its role as the request uses it
            for t in [e, j] {
                let src = std::fs::read_to_string(d.types.iter().find(|(_, x)| x.get("flows").and_then(Json::as_arr).is_some_and(|fs| fs.contains(t))).and_then(|(_, x)| x.get("file")).and_then(Json::as_str).unwrap()).unwrap();
                let written = str_at(t, &["written"]);
                let from = format!("from \"{}\"", str_at(t, &["file"]).trim_start_matches("examples/"));
                let alias = str_at(t, &["alias"]);
                assert!(src.contains(&format!("use flow {written} {from}")) || src.contains(&format!("use flow {written}({alias}) {from}")), "{k}: `use flow {written}`");
                let used = match str_at(t, &["role"]) {
                    "launch" => format!("launch {written}("),
                    "agent" => format!("agent {written},"),
                    "summary" => format!("note from {written}"),
                    r => panic!("{k}: the role `{r}`"),
                };
                assert!(src.contains(&used), "{k}: `{used}`");
            }
            let workflow = str_at(e, &["workflow"]);
            let version = e.get("version").and_then(Json::as_int).unwrap();
            assert_eq!(str_at(e, &["workflow_type"]), format!("{workflow}_v{version}"), "{k}");
            // the outputs in the order the flow's file declares them
            let file = std::fs::read_to_string(str_at(e, &["file"])).unwrap();
            let declared: Vec<String> = file.lines().skip_while(|l| l.trim() != "outputs").skip(1).take_while(|l| l.starts_with("  ")).filter_map(|l| l.split(':').next().map(|n| n.trim().to_string())).collect();
            let outputs: Vec<String> = e.get("outputs").and_then(Json::as_arr).unwrap().iter().map(|o| str_at(o, &["name"]).to_string()).collect();
            assert_eq!(outputs, declared, "{k}: {workflow}");
            seen += 1;
        }
    }
    assert!(seen >= 5, "{seen}");
}

/// The people, the teams and the rows of each master a kind reads are found by words (DESIGN 13.2):
/// an operation of the API and a tool each, which read, ask the words (`q`, at most 200
/// characters), how many (`limit`, 1 to 100, 20 when it is left out) and the language (`lang`), a
/// master the day too (`on`), and answer the values found with their names and whether more are.
#[test]
fn people_teams_and_rows_are_found_by_words() {
    let d = docs();
    let masters: Vec<String> = d.api.get("paths").and_then(Json::as_obj).unwrap().iter().filter_map(|(p, _)| p.strip_prefix("/masters/").map(|m| m.replace('/', "_"))).collect();
    assert!(!masters.is_empty());
    let finds: Vec<String> = ["find_people".to_string(), "find_teams".to_string()].into_iter().chain(masters.iter().map(|m| format!("find_master_{m}"))).collect();
    let params = d.api.get("components").and_then(|c| c.get("parameters")).unwrap();
    let limit = params.get("find_limit").and_then(|l| l.get("schema")).unwrap();
    assert_eq!((limit.get("minimum").and_then(Json::as_int), limit.get("maximum").and_then(Json::as_int), limit.get("default").and_then(Json::as_int)), (Some(1), Some(100), Some(20)));
    assert_eq!(params.get("q").and_then(|q| q.get("schema")).and_then(|s| s.get("maxLength")).and_then(Json::as_int), Some(200));
    let found = d.api.get("components").and_then(|c| c.get("schemas")).and_then(|s| s.get("found")).unwrap();
    let required = |j: &Json| j.get("required").and_then(Json::as_arr).unwrap().iter().filter_map(Json::as_str).map(String::from).collect::<Vec<_>>();
    assert_eq!(required(found), ["found", "more"]);
    assert_eq!(required(found.get("properties").and_then(|p| p.get("found")).and_then(|f| f.get("items")).unwrap()), ["value", "name", "label"]);
    for id in &finds {
        let op = operation(&d, id);
        let named: Vec<String> = op.get("parameters").and_then(Json::as_arr).unwrap().iter().map(|p| p.get("$ref").and_then(Json::as_str).map(|r| r.rsplit('/').next().unwrap().to_string()).unwrap_or_else(|| str_at(p, &["name"]).to_string())).collect();
        let on = id.starts_with("find_master_");
        assert_eq!(named, if on { vec!["q", "find_limit", "lang", "on"] } else { vec!["q", "find_limit", "lang"] }, "{id}");
        let codes: Vec<&str> = op.get("responses").and_then(Json::as_obj).unwrap().iter().map(|(c, _)| c.as_str()).collect();
        assert_eq!(codes, vec!["200", "400", "401", "404", "500"], "{id}");
        assert!(str_at(op, &["summary"]).contains("organization/find.jsonl"), "{id}");
        let input = tool(&d, id).get("inputSchema").and_then(|s| s.get("properties")).and_then(Json::as_obj).unwrap();
        let keys: Vec<&str> = input.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, if on { vec!["q", "limit", "lang", "on"] } else { vec!["q", "limit", "lang"] }, "{id}");
        assert_eq!(tool(&d, id).get("annotations").and_then(|a| a.get("readOnlyHint")).and_then(Json::as_bool), Some(true), "{id}");
    }
}

/// What the vectors of finding (`organization/find.jsonl`, DESIGN 13.2) answer, case by case, as
/// the comparison and the order say: each step of the folding, the white space, the ranks and their
/// order, `limit` and `more`, and the arguments a find answers 400 for. A server is held to the
/// vectors; this holds the vectors to the rule.
#[test]
fn the_vectors_of_finding_answer_as_the_comparison_says() {
    let lines: Vec<Json> = ringi::find::vectors().lines().map(|l| json::parse(l).unwrap()).collect();
    let answer = |case: &str| -> &Json {
        let line = lines.iter().find(|l| str_at(l, &["case"]) == case).unwrap_or_else(|| panic!("no case {case:?}"));
        line.get("answer").unwrap()
    };
    let found = |case: &str| -> (Vec<String>, bool) {
        let a = answer(case);
        let values = a.get("found").and_then(Json::as_arr).unwrap_or_else(|| panic!("{case}: {}", a.compact())).iter().map(|f| str_at(f, &["value"]).to_string()).collect();
        (values, a.get("more").and_then(Json::as_bool).unwrap())
    };
    let just = |case: &str, want: &[&str]| assert_eq!(found(case), (want.iter().map(|s| s.to_string()).collect::<Vec<_>>(), false), "{case}");
    let people: Vec<&str> = vec!["e1101", "e1201", "e1202", "e1203", "e2001", "e2002", "e2003", "e2004", "e2005", "e4001", "e4002", "e4003", "e4004", "e9001"];
    just("no words: every value, in the order of the values", &people);
    just("only white space is no words", &people);
    just("a word in a name", &["e1201", "e1202", "e1203", "e1101"]);
    just("every word is found: in the name and in the ID", &["e1201"]);
    just("words found in the name of the other language", &["e1203"]);
    just("the names in Japanese", &["e1101", "e1201", "e1202", "e1203"]);
    just("a Japanese name with no space, found with one", &["e2001"]);
    just("an ideographic space between the words", &["e2002"]);
    just("hiragana found as katakana", &["e2004"]);
    just("hiragana in a name, found by katakana", &["e2003"]);
    just("half-width katakana and its voiced mark, found by full-width", &["e2004"]);
    just("full-width letters, found in lower case", &["e2005"]);
    just("words in full-width letters", &["e2001"]);
    just("a final sigma, found by a sigma in capitals", &["e4001"]);
    just("a capital I with a dot above, found by i", &["e4002"]);
    just("a capital sharp s, found by ß", &["e4003"]);
    just("ß is not ss", &[]);
    just("U+0085 is white space between the words", &["e1201"]);
    just("U+FEFF is no white space: one word, which no value holds", &[]);
    just("U+001C is no white space: one word, which no value holds", &[]);
    just("a circled digit as a digit", &["t-sales1"]);
    just("a parenthesized ideograph", &["t-midori"]);
    just("the ideograph alone", &["t-midori"]);
    just("half-width katakana found by hiragana", &["t-gas"]);
    just("half-width katakana in a row's name", &["V-002"]);
    // the ranks: the ID as typed, the ID folded, a name, a name that begins with the first word, the
    // rest; values as close in the order of their code points
    just("the ranks: the ID as typed, the ID folded, a name, a name that begins with the word, the rest", &["x9", "X9", "a1", "a2", "a0", "a3"]);
    just("the ranks, the ID typed in capitals", &["X9", "x9", "a1", "a2", "a0", "a3"]);
    just("the ranks, the ID typed after U+0085, which is white space at an end", &["x9", "X9", "a1", "a2", "a0", "a3"]);
    // values as close come by code points: `ｚ` (U+FF5A) before `𠮷` (U+20BB7), which UTF-16 units
    // put the other way, and not in the order the data holds them in
    let case = "values as close in the order of their code points, not in the order of the data nor of UTF-16 units";
    just(case, &["z-01", "あ-01", "ｚ-01", "𠮷-01"]);
    let line = lines.iter().find(|l| str_at(l, &["case"]) == case).unwrap();
    let held: Vec<&str> = line.get("values").and_then(Json::as_arr).unwrap().iter().map(|v| str_at(v, &["value"])).collect();
    assert_eq!(held, ["𠮷-01", "ｚ-01", "あ-01", "z-01"]);
    let mut by_units = held.clone();
    by_units.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    assert_eq!(by_units, ["z-01", "あ-01", "𠮷-01", "ｚ-01"]);
    // the rows of a master: those the day `on` holds, both ends counted
    just("the rows of the day: a row on its last day", &["A-100", "A-102"]);
    just("the rows of the day: a row on its first day", &["A-101", "A-102"]);
    // the labels: the name and the value, in the language's parentheses, or the value alone
    let labels = |case: &str| answer(case).get("found").and_then(Json::as_arr).unwrap().iter().map(|f| str_at(f, &["label"]).to_string()).collect::<Vec<_>>();
    assert_eq!(labels("words found in the name of the other language"), ["Sales staff 3 (e1203)"]);
    assert_eq!(labels("a Japanese name with no space, found with one"), ["佐藤 一郎（e2001）"]);
    assert_eq!(labels("a person with no name: the label is the ID"), ["e9001"]);
    assert_eq!(labels("a row with no name: the label is the key"), ["V-011"]);
    assert_eq!(labels("a name that is the ID: the label is the ID"), ["e5001", "Ito Mei (e5002)"]);
    assert_eq!(labels("a Japanese name that is the ID: the label is the ID"), ["e5001", "e5002"]);
    // how many: 20 when `limit` says nothing, and whether more are found
    let count = |case: &str| {
        let (values, more) = found(case);
        (values.len(), more)
    };
    assert_eq!(count("no limit: 20, and more"), (20, true));
    assert_eq!(found("the least limit"), (vec!["p01".to_string()], true));
    assert_eq!(count("as many as there are: no more"), (25, false));
    assert_eq!(count("one fewer than there are: more"), (24, true));
    assert_eq!(count("the most limit"), (25, false));
    assert_eq!(count("words of 200 characters"), (0, false));
    // the arguments a find answers 400 for, by their names; of two wrong, the first of `q`, `limit`,
    // `lang`, `on`
    for (case, bad) in [
        ("a limit of 0", "limit"),
        ("a limit past the most", "limit"),
        ("words of 201 characters", "q"),
        ("a language of neither", "lang"),
        ("a day that is no date", "on"),
        ("a day not written YYYY-MM-DD", "on"),
        ("words of 201 characters and a limit of 0: the words are named", "q"),
        ("a limit of 0 and a language of neither: the limit is named", "limit"),
        ("a language of neither and a day that is no date: the language is named", "lang"),
    ] {
        assert_eq!(answer(case).compact(), format!(r#"{{"bad":"{bad}"}}"#), "{case}");
    }
    // every line says the day it asks (null but for the rows of a master), and a row of a master
    // the days it may be chosen on
    for l in &lines {
        let on = l.get("on").unwrap_or_else(|| panic!("{}", l.compact()));
        assert!(*on == Json::Null || str_at(l, &["among"]) == "master", "{}", l.compact());
        if str_at(l, &["among"]) == "master" {
            for v in l.get("values").and_then(Json::as_arr).unwrap() {
                assert!(v.get("valid_from").and_then(Json::as_str).is_some() && v.get("valid_to").and_then(Json::as_str).is_some(), "{}", l.compact());
            }
        }
    }
}

/// A kind in the list of the kinds a person may submit (`list_requests`, the schema
/// `request_kind`) says the language its file is written in and its title in the others, as its
/// description does (DESIGN 12.4): every key the schema asks for is one the description gives, and
/// the language and the titles are of the languages the schema names. The English leave has its
/// Japanese title, and its Japanese version its English one.
#[test]
fn a_kind_in_the_list_says_its_language_and_its_titles() {
    let d = docs();
    let kind = d.api.get("components").and_then(|c| c.get("schemas")).and_then(|s| s.get("request_kind")).unwrap();
    let props: Vec<&str> = kind.get("properties").and_then(Json::as_obj).unwrap().iter().map(|(k, _)| k.as_str()).collect();
    let required: Vec<&str> = kind.get("required").and_then(Json::as_arr).unwrap().iter().filter_map(Json::as_str).collect();
    assert_eq!(required, ["name", "written", "version", "language", "title", "labels", "description", "file"]);
    assert_eq!(props, required);
    let langs = |s: &Json| -> Vec<String> { s.get("enum").and_then(Json::as_arr).unwrap().iter().filter_map(Json::as_str).map(str::to_string).collect() };
    let at = |path: &[&str]| path.iter().try_fold(kind, |j, k| j.get(k)).unwrap_or_else(|| panic!("no {path:?}"));
    assert_eq!(langs(at(&["properties", "language"])), ["en", "ja"]);
    assert_eq!(langs(at(&["properties", "labels", "items", "properties", "lang"])), ["en", "ja"]);
    assert_eq!(at(&["properties", "labels", "items", "required"]).compact(), r#"["lang","text"]"#);
    // the answer of `list_requests` is a list of them
    let answer = operation(&d, "list_requests").get("responses").and_then(|r| r.get("200")).unwrap().compact();
    assert!(answer.contains(r##""items":{"$ref":"#/components/schemas/request_kind"}"##), "{answer}");
    for (name, t) in &d.types {
        for k in &required {
            assert!(t.get(k).is_some(), "{name}: {k}");
        }
        let lang = str_at(t, &["language"]);
        assert!(["en", "ja"].contains(&lang), "{name}: {lang}");
        for l in t.get("labels").and_then(Json::as_arr).unwrap() {
            let other = str_at(l, &["lang"]);
            assert!(["en", "ja"].contains(&other) && other != lang, "{name}: {other}");
            assert!(!str_at(l, &["text"]).is_empty(), "{name}");
        }
    }
    let of = |k: &str| d.types.iter().find(|(n, _)| n == k).map(|(_, t)| t).unwrap();
    assert_eq!((str_at(of("leave"), &["language"]), of("leave").get("labels").unwrap().compact()), ("en", r#"[{"lang":"ja","text":"休暇の申請"}]"#.to_string()));
    assert_eq!((str_at(of("leave_ja"), &["language"]), of("leave_ja").get("labels").unwrap().compact()), ("ja", r#"[{"lang":"en","text":"Leave request"}]"#.to_string()));
}

/// A kind that asks a reason with a rejection and a send-back (`comment required for`, DESIGN 7.2)
/// says so at each of its stages (`comment_required`, the events by their names in the contract);
/// a kind that asks none says an empty list at each. The comment of the API and the tools of the two
/// events say that the machine refuses one with no reason; the tool of an approval does not.
#[test]
fn a_kind_says_the_events_it_asks_a_reason_with() {
    let d = docs();
    let mut asking = Vec::new();
    for (name, t) in &d.types {
        let stages = t.get("stages").and_then(Json::as_arr).unwrap();
        assert!(!stages.is_empty(), "{name}");
        let each: Vec<String> = stages.iter().map(|s| s.get("comment_required").unwrap_or_else(|| panic!("{name}: no `comment_required`")).compact()).collect();
        assert!(each.iter().all(|e| *e == each[0]), "{name}: {each:?}");
        if each[0] != "[]" {
            assert_eq!(each[0], r#"["reject","send_back"]"#, "{name}");
            asking.push(name.as_str());
        }
    }
    asking.sort_unstable();
    assert_eq!(asking, ["leave", "leave_ja"]);
    assert!(d.api.compact().contains("A kind may ask a reason with a `reject` or a `send_back` (each stage's `comment_required` in `types/<name>.json`): the machine refuses one with none, or with white space alone (409, `comment`)"));
    let reason = "A kind whose stages list it in `comment_required` takes it only with a reason in `comment`";
    for e in ["reject", "send_back"] {
        assert!(str_at(tool(&d, e), &["description"]).ends_with(reason), "{e}");
    }
    assert!(!str_at(tool(&d, "approve"), &["description"]).contains(reason));
}

/// A person of the organization says the language they read their notices and pages in (DESIGN
/// 8.2): a column of `people.csv` and a key of `organization.json` that may be left out, `en`,
/// `ja`, empty or null, as the organization's description writes it for a server to read; and the
/// person `get_organization` answers with has it too, `en`, `ja` or null.
#[test]
fn a_person_says_the_language_they_read_in() {
    let d = docs();
    let langs = |s: &Json| s.get("enum").map(Json::compact).unwrap_or_default();
    let found: Vec<String> = {
        let mut out = Vec::new();
        let mut stack = vec![&d.org_schema];
        while let Some(j) = stack.pop() {
            match j {
                Json::Obj(o) => {
                    if let Some(p) = j.get("properties").and_then(|p| p.get("language")) {
                        let required = j.get("required").map(Json::compact).unwrap_or_default();
                        assert!(!required.contains("\"language\""), "{required}");
                        out.push(langs(p));
                    }
                    stack.extend(o.iter().map(|(_, v)| v));
                }
                Json::Arr(a) => stack.extend(a.iter()),
                _ => {}
            }
        }
        out
    };
    assert_eq!(found, [r#"["en","ja","",null]"#], "{}", d.org_schema.compact());
    assert!(d.org_columns.contains("| `language` | no | The language they read their notices and pages in, `en` or `ja`; empty for the language of each kind |"), "{}", d.org_columns);
    let person = d.api.get("components").and_then(|c| c.get("schemas")).and_then(|s| s.get("person")).unwrap();
    assert_eq!(person.get("properties").and_then(|p| p.get("language")).map(langs).as_deref(), Some(r#"["en","ja",null]"#), "{}", person.compact());
    assert_eq!(person.get("required").map(Json::compact).as_deref(), Some(r#"["id","name"]"#));
}
