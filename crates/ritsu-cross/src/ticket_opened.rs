//! What a workflow gives a ticket through the server's API, held to the request as it is now
//! (ringi's DESIGN 9.3, X20 on the side where a flow opens a ticket). A flow calls the operations of
//! the server `ritsu gen --target requests` describes (`use openapi … from "…/openapi.json"`), and
//! dandori holds what the flow sends to the document (its E016): the types, the ranges, the values
//! of an enum, the fields the operation requires. The document is written from the requests: an
//! operation that takes a form names its request (`x-ritsu-request`: the file, from the root of the
//! run that wrote it; the request's name; its version). Each call of such an operation is a border:
//!
//! - the operation is what ringi writes of the request now: its method and path, its body (the
//!   references followed) and the request it names. Shown to hold, as dandori holds what the flow
//!   sends to it. The rest of an operation (its parameters, its answers) is the same for every kind
//!   of request, and is not compared.
//! - it is not (E210): the document was written from the request as it was, so what dandori held
//!   the flow to is not what the request takes now. Said once at the call, with the places the two
//!   differ at; an operation the request no longer has is this error too.
//! - the file the operation names is no request of the project (W210): there is nothing to hold the
//!   document to, and the flow is held to the document alone.
//!
//! A flow that does not pass dandori's check is dandori's to say (what it sends against the
//! document, said once), and a request that does not pass is ringi's. A document `ritsu gen` writes
//! in the same run is not held to the request: the run writes it again from the request as it is.

use crate::{Border, Borders, Came, Check, Side};
use ritsu_base::diag::Diag;
use ritsu_base::json::Json;
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_ports::{Finding, Flows, Found, Ports, Requests};
use ritsu_project::{Joined, Project};
use ritsu_units::written;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// How many places a difference names before it says how many more.
const SHOWN: usize = 3;

/// X20 on the side where a flow opens a ticket, through the ports `joined` holds.
pub(crate) fn check(project: &Project, joined: &Joined, lang: Lang, borders: &mut Borders) -> Vec<Finding> {
    let writes = joined.ringi.writes_api();
    check_with(project, &*joined.requests(), &*joined.dandori, &joined.ports(), writes.as_deref(), lang, borders)
}

/// [`check`], with each port given, and the document `ritsu gen` writes in the run (as the system
/// opens it), which is not held to its requests.
pub fn check_with(project: &Project, requests: &dyn Requests, flows: &dyn Flows, ports: &Ports, writes: Option<&Path>, lang: Lang, borders: &mut Borders) -> Vec<Finding> {
    let mut out = Vec::new();
    // each document read once, and each request's API as ringi writes it now asked once
    let mut docs: BTreeMap<String, Option<Json>> = BTreeMap::new();
    let mut now: BTreeMap<PathBuf, Option<Found<Json>>> = BTreeMap::new();
    for f in project.of(Tool::Dandori) {
        let src = ritsu_base::fs::read_to_string(ritsu_base::paths::on_disk(&project.root, &f.rel)).unwrap_or_default();
        // a flow that reads no API calls no operation of one
        if !src.contains("openapi") {
            continue;
        }
        let Ok((_, calls)) = flows.operation_calls(&project.root, &f.rel, ports) else { continue };
        for c in calls.iter().filter(|c| c.operation.tool == Tool::Openapi) {
            let Some((_, id)) = c.operation.items.iter().find(|(k, _)| k == "operation") else { continue };
            let doc_rel = c.operation.path.clone();
            let doc_disk = ritsu_base::paths::on_disk(&project.root, &doc_rel);
            if writes.is_some_and(|w| ritsu_base::paths::opened(&doc_disk) == w) {
                continue;
            }
            let doc = docs.entry(doc_rel.clone()).or_insert_with(|| ritsu_base::fs::read_to_string(&doc_disk).ok().and_then(|t| ritsu_base::json::parse(&t).ok()));
            let Some(doc) = doc.as_ref() else { continue };
            let Some(op) = operation(doc, id) else { continue };
            // an operation that takes no form names no request
            let Some(named) = op.json.get("x-ritsu-request") else { continue };
            let one = One { project, flow: f, src: &src, line: c.line, id, doc_rel: &doc_rel, named: Named::of(named) };
            let (border, said) = match one.request() {
                Err(why) => one.no_request(why),
                Ok(disk) => {
                    let api = now.entry(disk.clone()).or_insert_with(|| match requests.openapi(std::slice::from_ref(&disk), &project.root) {
                        Ok(Found::Value(t)) => Some(ritsu_base::json::parse(&t).map_or_else(|_| Found::Undecided(tr!("ringi の書く API を読めません", "the API ringi writes does not read")), Found::Value)),
                        Ok(Found::Undecided(why)) => Some(Found::Undecided(why)),
                        // a request that does not pass is ringi's to say
                        Err(_) => None,
                    });
                    match api {
                        None => continue,
                        Some(Found::Undecided(why)) => one.undecided(&disk, why.clone()),
                        Some(Found::Value(api)) => one.held_to(&disk, doc, &op, api),
                    }
                }
            };
            borders.add(border);
            out.extend(said.map(|d| Finding::of(&d, Some(f.rel.clone()), lang)));
        }
    }
    out
}

/// An operation of a document, with its method and path.
struct Operation<'a> {
    method: String,
    path: String,
    json: &'a Json,
}

/// The operation of `doc` named `id`: its `operationId`, else its method and path (`POST /x`), as
/// ritsu-base's reader of documents names it.
fn operation<'a>(doc: &'a Json, id: &str) -> Option<Operation<'a>> {
    for (path, item) in doc.get("paths")?.as_obj()? {
        for (method, op) in item.as_obj().unwrap_or_default() {
            let by_id = op.get("operationId").and_then(Json::as_str) == Some(id);
            let by_place = op.get("operationId").is_none() && format!("{} {path}", method.to_ascii_uppercase()) == id;
            if by_id || by_place {
                return Some(Operation { method: method.clone(), path: path.clone(), json: op });
            }
        }
    }
    None
}

/// What an operation's `x-ritsu-request` names.
struct Named {
    file: Option<String>,
    name: Option<String>,
    version: Option<i128>,
}

impl Named {
    fn of(j: &Json) -> Named {
        Named { file: j.get("file").and_then(Json::as_str).map(str::to_string), name: j.get("name").and_then(Json::as_str).map(str::to_string), version: j.get("version").and_then(Json::as_int) }
    }

    /// The request as a message names it: `leave` (version 1).
    fn text(&self) -> Text {
        match (&self.name, self.version) {
            (Some(n), Some(v)) => tr!("`{n}` の版 {v}", "`{n}` (version {v})"),
            (Some(n), None) => Text::same(format!("`{n}`")),
            _ => tr!("名前の無い申請", "a request with no name"),
        }
    }
}

/// One call of an operation that names a request.
struct One<'a> {
    project: &'a Project,
    flow: &'a ritsu_project::File,
    src: &'a str,
    line: usize,
    id: &'a str,
    doc_rel: &'a str,
    named: Named,
}

impl One<'_> {
    fn at(&self, code: &'static str, message: Text) -> Diag {
        Diag::at(code, &self.flow.shown, self.line, 0, message).source(self.src).rel(&self.flow.rel)
    }

    fn border(&self, to: Option<Side>, came: Came, code: Option<&str>) -> Border {
        Border::new(Check::TicketFlow, self.id.to_string(), Side::at(Tool::Dandori, &self.flow.rel, self.line), to, came, code)
    }

    /// The request's file, where the project has it; else why it has none.
    fn request(&self) -> Result<PathBuf, Text> {
        let Some(file) = self.named.file.as_deref() else {
            return Err(tr!("`x-ritsu-request` にファイルがありません", "its `x-ritsu-request` names no file"));
        };
        if file.starts_with('/') || file.split('/').any(|p| p == "..") {
            return Err(tr!("`{file}` は、プロジェクトの根の外です", "`{file}` is outside the project's root"));
        }
        let disk = ritsu_base::paths::on_disk(&self.project.root, file);
        if !file.ends_with(".ringi") || !ritsu_base::fs::is_file(&disk) {
            return Err(tr!("プロジェクトに `{file}` という申請のファイルはありません", "the project has no request's file `{file}`"));
        }
        Ok(disk)
    }

    /// The side of the request at `disk`.
    fn request_side(&self, disk: &Path) -> Side {
        Side::reached(self.project, Tool::Ringi, disk, None)
    }

    /// The operation of the document, against what ringi writes of the request now (`api`).
    fn held_to(&self, disk: &Path, doc: &Json, op: &Operation, api: &Json) -> (Border, Option<Diag>) {
        let to = Some(self.request_side(disk));
        let file = self.named.file.clone().unwrap_or_default();
        let (id, doc_rel) = (self.id, self.doc_rel);
        let fix = tr!(
            "`ritsu gen --target requests` で \"{doc_rel}\" を書き直してから、flow をもう一度検査してください。",
            "Write \"{doc_rel}\" again (`ritsu gen --target requests`), then check the flow again."
        );
        let dandori = tr!(
            "dandori は、flow が送る値をこの文書と比べます。今の申請が受け取る値とは比べていません。",
            "dandori holds what the flow sends to this document, not to what the request takes now."
        );
        let id_now = op.json.get("operationId").and_then(Json::as_str);
        let Some(now) = id_now.and_then(|i| operation(api, i)).or_else(|| operation(api, &format!("{} {}", op.method.to_ascii_uppercase(), op.path))) else {
            let ops: Vec<Text> = moving_ids(api).into_iter().map(|i| Text::same(format!("`{i}`"))).collect();
            let has = if ops.is_empty() {
                tr!("今の `{file}` から書く API に、フォームを受け取る操作はありません。", "The API `{file}` makes now has no operation that takes its form.")
            } else {
                let l = Text::list(&ops);
                tr!("今の `{file}` から書く API の、フォームを受け取る操作: {}。", "The operations that take the form in the API `{file}` makes now: {}.", l.ja; l.en)
            };
            let named = self.named.text();
            let d = self
                .at("E210", tr!(
                    "flow は \"{doc_rel}\" の `{id}` を呼びますが、今の `{file}` から書く API にその操作はありません",
                    "The flow calls `{id}` of \"{doc_rel}\", and the API `{file}` makes now has no such operation"
                ))
                .note(tr!("文書は、この操作を申請 {} から書いたと言います。", "The document says the operation is written from the request {}.", named.ja; named.en))
                .note(has)
                .note(dandori)
                .note(fix);
            return (self.border(to, Came::Failed, Some("E210")), Some(d));
        };
        // what the operation is written from the request: where it is, what its body takes, and
        // the request it names (the rest is the same for every kind of request)
        let mut found = Vec::new();
        if (op.method.as_str(), op.path.as_str()) != (now.method.as_str(), now.path.as_str()) {
            found.push(Difference { place: "operation".into(), was: Some(format!("{} {}", op.method.to_ascii_uppercase(), op.path)), is: Some(format!("{} {}", now.method.to_ascii_uppercase(), now.path)) });
        }
        for key in ["requestBody", "x-ritsu-request"] {
            let (was, is) = (op.json.get(key).map(|j| resolved(doc, j, &mut Vec::new())), now.json.get(key).map(|j| resolved(api, j, &mut Vec::new())));
            differences(&mut vec![key.to_string()], was.as_ref(), is.as_ref(), &mut found);
        }
        if found.is_empty() {
            return (self.border(to, Came::Held, None), None);
        }
        let named = self.named.text();
        let mut d = self
            .at("E210", tr!(
                "flow が呼ぶ \"{doc_rel}\" の `{id}` は、今の `{file}` から書いたものではありません",
                "The flow calls `{id}` of \"{doc_rel}\", which is not written from `{file}` as it is now"
            ))
            .note(tr!("文書は、この操作を申請 {} から書いたと言います。", "The document says the operation is written from the request {}.", named.ja; named.en));
        for x in found.iter().take(SHOWN) {
            d = d.note(x.text());
        }
        let more = found.len().saturating_sub(SHOWN);
        if more > 0 {
            d = d.note(tr!("ほかに {more} か所違います。", "{more} more place(s) differ."));
        }
        let d = d.note(dandori).note(fix);
        (self.border(to, Came::Failed, Some("E210")), Some(d))
    }

    /// The file the operation names is no request of the project.
    fn no_request(&self, why: Text) -> (Border, Option<Diag>) {
        let (id, doc_rel) = (self.id, self.doc_rel);
        let named = self.named.text();
        let d = self
            .at("W210", tr!(
                "flow が呼ぶ \"{doc_rel}\" の `{id}` が、今の申請に合うかを決められません",
                "Whether `{id}` of \"{doc_rel}\", which the flow calls, is what the request is now cannot be decided"
            ))
            .note(tr!("文書は、この操作を申請 {} から書いたと言いますが、{}。", "The document says the operation is written from the request {}, and {}.", named.ja, why.ja; named.en, why.en))
            .note(tr!("dandori は、flow が送る値をこの文書とだけ比べます。", "dandori holds what the flow sends to the document alone."))
            .note(tr!(
                "申請がこのプロジェクトにあるなら、プロジェクトの根から `ritsu gen --target requests` で \"{doc_rel}\" を書き直してください。ほかのプロジェクトの申請なら、そのプロジェクトで書き直した文書に置き換えてください。",
                "If the request is in this project, write \"{doc_rel}\" again with `ritsu gen --target requests` from the project's root; if it is another project's, replace the document with the one that project writes now."
            ));
        (self.border(None, Came::Undecided, Some("W210")), Some(d))
    }

    /// ringi cannot write the API of the request now.
    fn undecided(&self, disk: &Path, why: Text) -> (Border, Option<Diag>) {
        let (id, doc_rel) = (self.id, self.doc_rel);
        let file = self.named.file.clone().unwrap_or_default();
        let d = self
            .at("W210", tr!(
                "flow が呼ぶ \"{doc_rel}\" の `{id}` が、今の `{file}` に合うかを決められません",
                "Whether `{id}` of \"{doc_rel}\", which the flow calls, is what `{file}` is now cannot be decided"
            ))
            .note(tr!("決められない理由: ringi は今の `{file}` から API を書けません（{}）。", "Why it cannot be decided: ringi cannot write the API of `{file}` now ({}).", why.ja; why.en))
            .note(tr!("dandori は、flow が送る値をこの文書とだけ比べます。", "dandori holds what the flow sends to the document alone."));
        (self.border(Some(self.request_side(disk)), Came::Undecided, Some("W210")), Some(d))
    }
}

/// The operations of a document that take a form (those with `x-ritsu-request`), by their names.
fn moving_ids(doc: &Json) -> Vec<String> {
    let mut out = Vec::new();
    for (_, item) in doc.get("paths").and_then(Json::as_obj).unwrap_or_default() {
        for (_, op) in item.as_obj().unwrap_or_default() {
            if op.get("x-ritsu-request").is_some()
                && let Some(i) = op.get("operationId").and_then(Json::as_str)
            {
                out.push(i.to_string());
            }
        }
    }
    out
}

/// `node`, with each reference into `doc` (`{"$ref": "#/…"}`) replaced by what it points at; a
/// reference met again inside itself is left as it is.
fn resolved(doc: &Json, node: &Json, seen: &mut Vec<String>) -> Json {
    match node {
        Json::Obj(pairs) => {
            if let [(k, Json::Str(r))] = pairs.as_slice()
                && k == "$ref"
                && let Some(p) = r.strip_prefix("#/")
                && !seen.contains(r)
                && let Some(target) = p.split('/').try_fold(doc, |j, part| j.get(&part.replace("~1", "/").replace("~0", "~")))
            {
                seen.push(r.clone());
                let out = resolved(doc, target, seen);
                seen.pop();
                return out;
            }
            Json::Obj(pairs.iter().map(|(k, v)| (k.clone(), resolved(doc, v, seen))).collect())
        }
        Json::Arr(items) => Json::Arr(items.iter().map(|v| resolved(doc, v, seen)).collect()),
        other => other.clone(),
    }
}

/// A place the document's operation and ringi's now differ at, with what each has there.
struct Difference {
    place: String,
    was: Option<String>,
    is: Option<String>,
}

impl Difference {
    fn text(&self) -> Text {
        let p = &self.place;
        match (&self.was, &self.is) {
            (Some(a), Some(b)) => tr!("`{p}` は、文書では {a}、今は {b} です。", "`{p}` is {a} in the document, and {b} now."),
            (None, Some(b)) => tr!("`{p}` は、文書に無く、今は {b} です。", "`{p}` is not in the document, and is {b} now."),
            (Some(a), None) => tr!("`{p}` は、文書では {a} で、今はありません。", "`{p}` is {a} in the document, and is not there now."),
            (None, None) => Text::same(format!("`{p}`")),
        }
    }
}

/// A value of a difference, cut short when it is long. A whole number is said with its digits
/// grouped (`1_000_000`), as the sentences of the checks say the integers of a contract (ritsu's
/// DESIGN 6.2 item 8); the rest as the document's JSON.
fn short(j: &Json) -> String {
    if let Json::Int(n) = j {
        return written::digits(*n);
    }
    let s = j.compact();
    if s.chars().count() <= 60 { s } else { format!("{}…", s.chars().take(59).collect::<String>()) }
}

/// The places `was` and `is` differ at, under `path` (the keys from the operation): a key one has
/// and the other not, two values that differ (a list of plain values, or two lists of different
/// lengths, compared whole). The body's schema is named from the body (`body.form.days.maximum`),
/// and a schema's `properties` are left out of the place.
fn differences(path: &mut Vec<String>, was: Option<&Json>, is: Option<&Json>, out: &mut Vec<Difference>) {
    let place = |path: &[String]| {
        let mut parts: Vec<&str> = path.iter().map(String::as_str).collect();
        if parts.starts_with(&["requestBody", "content", "application/json", "schema"]) {
            parts.splice(0..4, ["body"]);
        }
        parts.retain(|p| *p != "properties");
        parts.join(".")
    };
    match (was, is) {
        (Some(a), Some(b)) if a == b => {}
        (Some(Json::Obj(a)), Some(Json::Obj(b))) => {
            let mut keys: Vec<&String> = a.iter().map(|(k, _)| k).collect();
            keys.extend(b.iter().map(|(k, _)| k).filter(|k| !a.iter().any(|(x, _)| x == *k)));
            for k in keys {
                let (x, y) = (a.iter().find(|(x, _)| x == k).map(|(_, v)| v), b.iter().find(|(x, _)| x == k).map(|(_, v)| v));
                path.push(k.clone());
                differences(path, x, y, out);
                path.pop();
            }
        }
        (Some(Json::Arr(a)), Some(Json::Arr(b))) if a.len() == b.len() && a.iter().chain(b).any(|x| matches!(x, Json::Obj(_) | Json::Arr(_))) => {
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                path.push(i.to_string());
                differences(path, Some(x), Some(y), out);
                path.pop();
            }
        }
        (a, b) => out.push(Difference { place: place(path), was: a.map(short), is: b.map(short) }),
    }
}
