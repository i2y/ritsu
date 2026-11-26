//! `a2ui/catalog.json` (DESIGN 13.2): the parts and the functions the screens use beside A2UI's
//! basic catalog, as an A2UI 1.0 catalog. Its ID (`ritsu gen --catalog`) is a name for the
//! project's catalog, written as a URL the project owns: it need not be where the server serves
//! the file, and it stays the same in every environment. The parts are the same whatever the
//! kinds; the functions are the ones every kind shares, then each kind's own (`expense.check`),
//! which `a2ui/functions.js` defines. The file is a catalog, whose keys the catalog's schema fixes,
//! so the head of what ritsu generates is its `description`.

use crate::r#gen::Kind;
use ritsu_base::json::Json;

fn obj(pairs: Vec<(&str, Json)>) -> Json {
    Json::obj(pairs)
}

/// A type of the common types of A2UI (`DynamicString`).
fn common(name: &str) -> Json {
    obj(vec![("$ref", Json::str(format!("common_types.json#/$defs/{name}")))])
}

fn typed(t: &str) -> Json {
    obj(vec![("type", Json::str(t))])
}

fn described(mut j: Json, words: &str) -> Json {
    if let Json::Obj(pairs) = &mut j {
        pairs.push(("description".to_string(), Json::str(words)));
    }
    j
}

/// A part: its properties beside `component`, which it requires, and whether it takes `@checks`.
fn component(name: &str, words: &str, props: Vec<(&str, Json)>, required: &[&str], checkable: bool) -> Json {
    let mut all = vec![("component", obj(vec![("const", Json::str(name))]))];
    all.extend(props);
    let mut req = vec![Json::str("component")];
    req.extend(required.iter().map(|r| Json::str(*r)));
    let body = obj(vec![("type", Json::str("object")), ("properties", obj(all)), ("required", Json::arr(req))]);
    let parts = if checkable { vec![common("Checkable"), body] } else { vec![body] };
    obj(vec![("type", Json::str("object")), ("description", Json::str(words)), ("allOf", Json::arr(parts))])
}

/// A function: its arguments, the ones it requires, and what it returns.
fn function(name: &str, words: &str, args: Vec<(&str, Json)>, required: &[&str], returns: &str) -> Json {
    let a = obj(vec![("type", Json::str("object")), ("properties", obj(args)), ("required", Json::arr(required.iter().map(|r| Json::str(*r)))), ("unevaluatedProperties", Json::from(false))]);
    obj(vec![
        ("type", Json::str("object")),
        ("description", Json::str(words)),
        ("returnType", Json::str(returns)),
        ("properties", obj(vec![("@call", obj(vec![("const", Json::str(name))])), ("args", a)])),
        ("required", Json::arr([Json::str("@call"), Json::str("args")])),
    ])
}

/// Where a picker finds its values, and how the server answers it (DESIGN 13.2).
const FIND: &str = "The address the server finds values at: the server's, which fills it in the screen's data (`/find`). A picker of people asks `<find>/people`, of teams `<find>/teams`, and of a master's rows `<find>/masters/<master>`, each a `GET` with `q` (the words typed, up to 200 characters; none find every value), `limit` (1 to 100; 20 when it is left out) and `lang` (`en` or `ja`, the language of the names; when it is left out, the first of `Accept-Language` that is one, else `en`). It is answered `{\"found\": [{\"value\": …, \"name\": …, \"label\": …}], \"more\": …}`: in `found`, each value whose ID or key, or one of whose names, holds every word, with `value` (a person's or a team's ID, or a row's key), `name` (its name in `lang`; empty where it has none) and `label` (the name with the value, as a list shows it); `more`, whether more values hold the words than were answered.";

/// The parts of the catalog, by name, in the order DESIGN 13.2 lists them.
pub fn components() -> Vec<(&'static str, Json)> {
    let label = || described(common("DynamicString"), "The label shown with the part.");
    vec![
        (
            "Show",
            component(
                "Show",
                "Shows its child while `when` holds: a value that is neither false, null, an empty string nor an empty list. A field it hides is not sent.",
                vec![("when", common("DynamicValue")), ("child", common("ComponentId"))],
                &["when", "child"],
                false,
            ),
        ),
        (
            "LineItems",
            component(
                "LineItems",
                "The rows of a field: `child` is the template of a row, whose paths are read in the row, with a button that adds a row and one on each row that removes it. It keeps the count of rows from `min` to `max`.",
                vec![
                    ("label", label()),
                    ("value", common("DataBinding")),
                    ("child", common("ComponentId")),
                    ("min", typed("integer")),
                    ("max", typed("integer")),
                    ("addLabel", common("DynamicString")),
                    ("removeLabel", common("DynamicString")),
                ],
                &["label", "value", "child"],
                true,
            ),
        ),
        (
            "AmountField",
            component(
                "AmountField",
                "An amount typed in the unit people count in, shown with its currency (an ISO 4217 code) or its unit (`%`, `kg`), with up to `decimals` digits after the point; written to `value` as the integer of the unit's steps, the amount times 10^decimals divided by `step` (`decimals` 2 and `step` 25 for a rate in steps of 0.25%: 31.25 is 125). An amount that is no whole number of steps is refused, not rounded. The value is kept from `min` to `max`, integers of steps.",
                vec![
                    ("label", label()),
                    ("value", common("DynamicNumber")),
                    ("currency", typed("string")),
                    ("unit", typed("string")),
                    ("decimals", typed("integer")),
                    ("step", described(typed("integer"), "One step of the unit, counted in the last of the `decimals` digits: 1 when not given.")),
                    ("min", typed("integer")),
                    ("max", typed("integer")),
                ],
                &["label", "value", "decimals"],
                true,
            ),
        ),
        (
            "FileAttachment",
            component(
                "FileAttachment",
                "Picks a file, hands it to the server at `upload` (a `POST` of `multipart/form-data` with one part, `file`, answered with the ID the server keeps the file by: `{\"id\": …}`), and writes that ID to `value`. A person who holds as many files on no ticket as the server takes is answered 429, with no `Retry-After`: waiting does not change it.",
                vec![
                    ("label", label()),
                    ("value", common("DynamicString")),
                    ("name", described(common("DynamicString"), "The name of the file `value` holds, shown in place of its ID: the screen looks it up in the names the server fills in its data (`/file_names`), which have the files the person may open; a file it does not have is shown by its ID.")),
                    ("upload", described(common("DynamicString"), "The address the file is handed to: the server's, which fills it in the screen's data (`/upload`).")),
                ],
                &["label", "value", "upload"],
                true,
            ),
        ),
        (
            "PersonPicker",
            component(
                "PersonPicker",
                "Finds a person (or a team, with `of` as `team`) of the organization at the server (`find`) and writes their ID to `value`: with `among`, one of the IDs it lists.",
                vec![
                    ("label", label()),
                    ("value", common("DynamicString")),
                    ("of", obj(vec![("type", Json::str("string")), ("enum", Json::arr([Json::str("person"), Json::str("team")]))])),
                    ("among", described(common("DynamicStringList"), "The IDs of the only people (or teams) it offers, where it is given.")),
                    ("find", described(common("DynamicString"), FIND)),
                ],
                &["label", "value", "find"],
                true,
            ),
        ),
        (
            "MasterPicker",
            component(
                "MasterPicker",
                "Finds a row of a master the server serves (`master` is its path under `/masters/`) at the server (`find`) and writes its key to `value`.",
                vec![("label", label()), ("value", common("DynamicString")), ("master", typed("string")), ("find", described(common("DynamicString"), FIND))],
                &["label", "value", "master", "find"],
                true,
            ),
        ),
        (
            "ApprovalRoute",
            component(
                "ApprovalRoute",
                "The stages of a ticket in their order, each with its approvers, their answers and its phase, and the stage it is at. `labels` names the stages and `ways` the ways approvers are chosen, in the screen's language; `people` names the people by their IDs.",
                vec![
                    ("stages", common("DynamicValue")),
                    ("current", common("DynamicString")),
                    ("labels", obj(vec![("type", Json::str("object")), ("additionalProperties", typed("string"))])),
                    ("ways", obj(vec![("type", Json::str("object")), ("additionalProperties", typed("string"))])),
                    ("people", common("DynamicValue")),
                ],
                &["stages"],
                false,
            ),
        ),
    ]
}

/// Every function of the catalog, by name: the ones every kind shares, then each kind's.
pub fn functions(kinds: &[Kind]) -> Vec<(String, Json)> {
    let v = || common("DynamicValue");
    let s = || common("DynamicString");
    let mut out: Vec<(String, Json)> = vec![
        (
            "lookup".into(),
            function(
                "lookup",
                "The words of a code: `table[value]`, the value itself when the table has no words for it. A list of codes is the words of each, joined by `separator`.",
                vec![("value", v()), ("table", v()), ("separator", s())],
                &["value", "table"],
                "string",
            ),
        ),
        ("equals".into(), function("equals", "Whether `a` and `b` are the same value.", vec![("a", v()), ("b", v())], &["a", "b"], "boolean")),
        (
            "formatSteps".into(),
            function(
                "formatSteps",
                "The text of `value`, an integer of a unit's steps, in the unit people count in: `value` times `step` written with `decimals` digits after the point (zeros too), a whole part of five digits or more grouped by commas, a minus before a value below zero, and `unit` after it as it is written (`31.25%`, `1.25 USD`, `12,000 kg`). Worked out with whole numbers, so every value a screen holds is written exactly. An empty string for null.",
                vec![("value", common("DynamicNumber")), ("decimals", typed("integer")), ("step", described(typed("integer"), "One step, counted in the last of the `decimals` digits: 1 when not given.")), ("unit", s())],
                &["value", "decimals"],
                "string",
            ),
        ),
        ("numberFrom".into(), function("numberFrom", "The number the digits of `text` write, null when they write none.", vec![("text", s())], &["text"], "any")),
        ("one".into(), function("one", "The value a choice holds (a ChoicePicker writes a list), null when none, or an empty one, is chosen.", vec![("values", common("DynamicStringList"))], &["values"], "any")),
        (
            "comment".into(),
            function(
                "comment",
                "The comment as it is sent: `text` without the white space at its ends (the 25 code points of Unicode's White_Space property, the ideographic space U+3000 among them, as the ticket machine reads white space), null when nothing is left.",
                vec![("text", s())],
                &["text"],
                "any",
            ),
        ),
        ("commentFits".into(), function("commentFits", "Whether the comment as it is sent is 2,000 characters long or shorter, counted in Unicode code points.", vec![("text", s())], &["text"], "boolean")),
        ("commentGiven".into(), function("commentGiven", "Whether there is a comment to send: one left after the white space at its ends is taken off, as `comment` takes it off. A rejection and a send-back of a kind that asks a reason with them are sent only with one.", vec![("text", s())], &["text"], "boolean")),
        (
            "lastOf".into(),
            function("lastOf", "The value of `key` of the last of `values`, a list of objects: null when the list is empty, or its last has no such key.", vec![("values", v()), ("key", typed("string"))], &["values", "key"], "any"),
        ),
    ];
    for k in kinds {
        let n = &k.named.name;
        let form = || vec![("form", common("DynamicValue"))];
        out.push((
            format!("{n}.check"),
            function(
                &format!("{n}.check"),
                &format!("Whether the form of {n} passes the check `code` (`<field>.required`, `.count`, `.range`, `.choice`, `<rows>.<field>.…`, `<field>.worked`): for the row `row` of a field of the rows, or every row when it is not given. A text is read without the white space at its ends. A computation that reads rows is held to its range once the rows are as many as their field asks for; with `filling`, the check a field of rows shows below itself while the form is filled in, fewer rows than it asks for pass."),
                vec![("form", v()), ("code", typed("string")), ("row", common("DynamicNumber")), ("filling", typed("boolean"))],
                &["form", "code"],
                "boolean",
            ),
        ));
        out.push((format!("{n}.given"), function(&format!("{n}.given"), &format!("The form of {n} as it is sent to make the request: the fields given and shown, each as the API takes it, a text without the white space at its ends."), form(), &["form"], "object")));
        if k.has("resubmit") {
            let fb = || vec![("form", v()), ("before", v())];
            out.push((format!("{n}.changes"), function(&format!("{n}.changes"), &format!("The fields of {n} the requester changed from `before`, as the API takes them, when the request is made again."), fb(), &["form", "before"], "object")));
            out.push((format!("{n}.cleared"), function(&format!("{n}.cleared"), &format!("The fields of {n} the requester made empty (or that are hidden now) that `before` held, by their names."), fb(), &["form", "before"], "array")));
        }
        if k.has("edit") && k.edited().next().is_some() {
            let eb = || vec![("edit", v()), ("before", v())];
            out.push((
                format!("{n}.edited"),
                function(&format!("{n}.edited"), &format!("The form of {n} as an approver's edit leaves it: the fields an approver gives or corrects as `edit` holds them, the others as `before` does."), eb(), &["edit", "before"], "object"),
            ));
            out.push((format!("{n}.edits"), function(&format!("{n}.edits"), &format!("The fields of {n} an approver gave or changed from `before`, as the API takes them."), eb(), &["edit", "before"], "object")));
            out.push((format!("{n}.edits_cleared"), function(&format!("{n}.edits_cleared"), &format!("The fields of {n} an approver made empty (or that are hidden after the edit) that `before` held, by their names."), eb(), &["edit", "before"], "array")));
        }
        for f in &k.form {
            if f.shown_when.is_some() {
                out.push((format!("{n}.shown_{}", f.named.name), function(&format!("{n}.shown_{}", f.named.name), &format!("Whether the field `{}` of {n} is shown.", f.named.name), form(), &["form"], "boolean")));
            }
            if f.required_when.is_some() {
                out.push((format!("{n}.required_{}", f.named.name), function(&format!("{n}.required_{}", f.named.name), &format!("Whether the field `{}` of {n} is asked for.", f.named.name), form(), &["form"], "boolean")));
            }
            if f.worked_on_screen {
                out.push((format!("{n}.{}", f.named.name), function(&format!("{n}.{}", f.named.name), &format!("The `{}` the form of {n} works out, null while the form cannot work it out.", f.named.name), form(), &["form"], "any")));
            }
        }
    }
    out
}

/// The catalog, whose ID is `id`.
pub fn json(kinds: &[Kind], id: &str) -> Json {
    let comps = components();
    let funcs = functions(kinds);
    let any = |what: &str, names: Vec<String>| obj(vec![("oneOf", Json::arr(names.into_iter().map(|n| obj(vec![("$ref", Json::str(format!("#/{what}/{n}")))]))))]);
    let any_component = any("components", comps.iter().map(|(n, _)| n.to_string()).collect());
    let any_function = any("functions", funcs.iter().map(|(n, _)| n.clone()).collect());
    Json::obj([
        ("description", Json::str(crate::r#gen::head())),
        ("$schema", Json::str("https://json-schema.org/draft/2020-12/schema")),
        ("catalogId", Json::str(id)),
        ("protocolVersion", Json::str("1.0")),
        ("title", Json::str("The parts and functions of the screens of the kinds of request")),
        (
            "instructions",
            Json::str(
                "Use these beside the basic catalog, whose parts the surfaces default to: each part and each call of a function here names this catalog (`@catalogId`, `catalogId`). The functions are in `functions.js` beside this file, an ES module whose `functions` maps each name to the function, given the call's arguments.",
            ),
        ),
        ("components", Json::obj(comps)),
        ("functions", Json::obj(funcs)),
        ("$defs", Json::obj([("anyComponent", any_component), ("anyFunction", any_function)])),
    ])
}
