//! The screens `ritsu gen --target requests` writes (DESIGN 13), for every kind of request of the
//! examples, in English and in Japanese. Each message reads as A2UI 1.0 (the schemas copied into
//! `tests/fixtures/a2ui-1.0/`, with ritsu's catalog beside the basic one, as A2UI's own validation
//! merges a catalog into `catalog.json`), and the catalog reads as a catalog. Each screen is one
//! tree of parts that names the catalog of each part and function it uses and reads its data from
//! places its data has; each input keeps the limits and the checks of its field; what each button
//! sends is what the screen shows (E701, W702), as an operation of the API. The changes that would
//! break one of these are found.
//! The functions (`functions.js`) run in Node against the vectors of the examples (DESIGN 13.7): a
//! form the reference takes passes every check of its screen, is sent as the reference took it and
//! works out what the reference does; a form the reference refuses as a form fails a check. So do
//! they for a request whose choice has values the lowered files write under other names. On forms
//! made from each kind's description, field by field and check by check, the screens stop what the
//! reference refuses and send what it keeps (`a2ui/forms.rs`). As a person fills a form in, value by
//! value, each condition and computation is worked out from the fields it reads alone, as the
//! reference works it out (`a2ui/steps.rs`).

mod common;
#[path = "a2ui/forms.rs"]
mod forms;
#[path = "a2ui/schema.rs"]
mod schema;
#[path = "a2ui/steps.rs"]
mod steps;
#[path = "a2ui/worked.rs"]
mod worked;

use ringi::ast::TypeExpr;
use ringi::ports::Engine;
use ringi::r#gen::{Fld, Kind};
use ringi::screens::{self, Given, bindings};
use ritsu_base::json::{self, Json};
use ritsu_ports::Requests;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

const CATALOG: &str = common::CATALOG;
const FIXTURES: &str = "tests/fixtures/a2ui-1.0";
const SPEC: &str = "https://a2ui.org/specification/v1_0/";

/// The kinds of request of the examples, what their screens are made with, and what `requests`
/// writes for them, by path.
struct Made {
    kinds: Vec<Kind>,
    given: Given,
    files: Vec<(String, String)>,
}

fn made() -> Made {
    // the kinds of request: the shared files are an organization, masters and a shared route
    let requests: Vec<PathBuf> = common::example_files().into_iter().filter(|f| !f.starts_with("examples/shared/")).map(PathBuf::from).collect();
    let m = made_of(&requests);
    let names: Vec<&str> = m.kinds.iter().map(|k| k.named.name.as_str()).collect();
    assert_eq!(names, ["account_ja", "account", "contract_ja", "contract", "expense_ja", "expense", "leave_ja", "leave", "purchase_ja", "purchase"]);
    m
}

/// The kinds of request at `requests`, what their screens are made with, and what `requests`
/// writes for them.
fn made_of(requests: &[PathBuf]) -> Made {
    let engine = Engine::new(common::joined());
    // as `ritsu gen` runs it (DESIGN 12.3)
    engine.keep_listing_apart();
    let root = PathBuf::from(".");
    let kinds = engine.kinds(requests, &root).unwrap_or_else(|t| panic!("{}", t.en));
    let given = Given { catalog: CATALOG.to_string(), rules: rules(&engine, &kinds) };
    let files = engine.requests_with(requests, &root, Some(&given)).unwrap_or_else(|t| panic!("{}", t.en)).into_iter().map(|(p, t, _)| (p, t)).collect();
    Made { kinds, given, files }
}

/// Each kind's `values.rule`, lowered, in the JavaScript rulec writes for a page: what `ritsu gen`
/// hands ringi, the lowered files laid over the disk as the run lays them and the rule and the
/// `choices.rule` it reads written by rulec's engine as the run writes them, in English.
fn rules(engine: &Engine, kinds: &[Kind]) -> Vec<(String, String)> {
    let rulec = rulec::ports::Engine::with_dates(Arc::new(koyomi::ports::Engine));
    let mut out = Vec::new();
    for k in kinds.iter().filter(|k| k.screen.worked.is_some()) {
        let lowered = engine.lowered(Path::new(&k.file)).unwrap_or_else(|_| panic!("{} is not lowered", k.file));
        let mut laid = ritsu_base::fs::Overlay::new(ritsu_base::fs::current());
        for l in &lowered {
            laid.put(&ritsu_base::paths::absolute(&l.path), l.text.clone());
        }
        let values = lowered.iter().find(|l| l.path.ends_with("values.rule")).unwrap_or_else(|| panic!("{} has no values.rule", k.file));
        let js = ritsu_base::fs::with(Rc::new(laid), || rulec.page_javascript(&values.path, ritsu_base::text::Lang::En)).unwrap_or_else(|_| panic!("{} does not pass rulec's check", values.path.display()));
        out.push((k.named.name.clone(), js));
    }
    out
}

fn file<'a>(m: &'a Made, path: &str) -> &'a str {
    m.files.iter().find(|(p, _)| p == path).map(|(_, t)| t.as_str()).unwrap_or_else(|| panic!("{path} is not written"))
}

/// Each screen written, with its kind, as its file holds it.
fn screens_of(m: &Made) -> Vec<(&Kind, String, Json)> {
    let mut out = Vec::new();
    for k in &m.kinds {
        for screen in screens::SCREENS {
            for lang in screens::LANGS {
                let path = format!("a2ui/{}/{screen}.{lang}.json", k.named.name);
                out.push((k, path.clone(), json::parse(file(m, &path)).unwrap_or_else(|e| panic!("{path}: {e:?}"))));
            }
        }
    }
    out
}

fn fixture(rel: &str) -> Json {
    let p = Path::new(FIXTURES).join(rel);
    json::parse(&std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).unwrap()
}

fn at<'a>(j: &'a mut Json, key: &str) -> &'a mut Json {
    match j {
        Json::Obj(pairs) => &mut pairs.iter_mut().find(|(k, _)| k == key).unwrap_or_else(|| panic!("no `{key}`")).1,
        Json::Arr(items) => items.get_mut(key.parse::<usize>().unwrap()).unwrap_or_else(|| panic!("no item {key}")),
        _ => panic!("nothing where `{key}` is looked for"),
    }
}

/// The property `key` of `j` made `v`, or taken away.
fn set(j: &mut Json, key: &str, v: Option<Json>) {
    let Json::Obj(pairs) = j else { panic!("no object where `{key}` is set") };
    pairs.retain(|(k, _)| k != key);
    if let Some(v) = v {
        pairs.push((key.to_string(), v));
    }
}

fn push_all(into: &mut Json, from: &Json) {
    match (into, from) {
        (Json::Obj(a), Json::Obj(b)) => a.extend(b.iter().cloned()),
        (Json::Arr(a), Json::Arr(b)) => a.extend(b.iter().cloned()),
        _ => panic!("two values that do not merge"),
    }
}

/// The schemas the messages and the catalog are validated with: the four of the protocol (what a
/// renderer sends back among them, which the forms' buttons are held to), and the
/// basic catalog with ritsu's beside it (its parts, its functions and the `oneOf` of each) at the
/// place the schema of the messages reads `catalog.json` from.
fn schemas(ours: &Json) -> schema::Schemas {
    let mut s = schema::Schemas::default();
    for f in ["common_types.json", "agent_to_renderer.json", "renderer_to_agent.json", "catalog_definition.json"] {
        s.add(fixture(&format!("specification/v1_0/json/{f}")));
    }
    let mut both = fixture("catalogs/basic/v1/catalog.json");
    for key in ["components", "functions"] {
        push_all(at(&mut both, key), ours.get(key).unwrap());
    }
    for key in ["anyComponent", "anyFunction"] {
        push_all(at(at(at(&mut both, "$defs"), key), "oneOf"), ours.get("$defs").and_then(|d| d.get(key)).and_then(|d| d.get("oneOf")).unwrap());
    }
    set(&mut both, "$id", Some(Json::str(format!("{SPEC}catalog.json"))));
    s.add(both);
    s
}

/// What is wrong with a screen's messages by the schemas.
fn invalid(s: &schema::Schemas, screen: &Json) -> Vec<String> {
    let ms = screen.get("messages").and_then(Json::as_arr).unwrap();
    ms.iter().enumerate().flat_map(|(i, msg)| s.errors(&format!("{SPEC}agent_to_renderer.json"), msg).into_iter().map(move |e| format!("message {i}: {e}"))).collect()
}

/// The parts of a screen, in order.
fn parts(screen: &Json) -> Vec<&Json> {
    screen.get("messages").and_then(Json::as_arr).unwrap().iter().filter_map(|m| m.get("updateComponents")).flat_map(|u| u.get("components").and_then(Json::as_arr).unwrap()).collect()
}

fn part<'a>(screen: &'a Json, id: &str) -> Option<&'a Json> {
    parts(screen).into_iter().find(|c| c.get("id").and_then(Json::as_str) == Some(id))
}

/// The value of the screen's data.
fn data(screen: &Json) -> &Json {
    screen.get("messages").and_then(Json::as_arr).unwrap().iter().find_map(|m| m.get("updateDataModel")).and_then(|u| u.get("value")).unwrap()
}

/// Every object under `j` that has `key`, with the value it gives it.
fn every<'a>(j: &'a Json, key: &str, out: &mut Vec<(&'a Json, &'a Json)>) {
    match j {
        Json::Obj(pairs) => {
            if let Some(v) = j.get(key) {
                out.push((j, v));
            }
            for (_, v) in pairs {
                every(v, key, out);
            }
        }
        Json::Arr(items) => items.iter().for_each(|v| every(v, key, out)),
        _ => {}
    }
}

#[test]
fn every_screen_and_the_catalog_read_as_a2ui_1_0() {
    let m = made();
    let catalog = json::parse(file(&m, "a2ui/catalog.json")).unwrap();
    let s = schemas(&catalog);
    let mut wrong = Vec::new();
    let mut messages = 0;
    let all = screens_of(&m);
    for (_, path, screen) in &all {
        let keys: Vec<&str> = screen.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["//", "messages"], "{path}");
        assert_eq!(screen.get("//").and_then(Json::as_str), Some(format!("Code generated by ritsu {}. DO NOT EDIT.", env!("CARGO_PKG_VERSION")).as_str()), "{path}");
        let ms = screen.get("messages").and_then(Json::as_arr).unwrap();
        let shapes: Vec<&str> = ms.iter().map(|m| m.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).find(|k| *k != "version").unwrap()).collect();
        assert_eq!(shapes, ["createSurface", "updateDataModel", "updateComponents"], "{path}");
        messages += ms.len();
        wrong.extend(invalid(&s, screen).into_iter().map(|e| format!("{path} {e}")));
    }
    wrong.extend(s.errors(&format!("{SPEC}catalog_definition.json"), &catalog).into_iter().map(|e| format!("catalog.json: {e}")));
    // where the schema of a catalog leaves a part or a function to JSON Schema's meta-schema, the
    // shape the parts and the calls of the screens are read in: each `oneOf` leads to its definition
    for (key, defs) in [("anyComponent", "components"), ("anyFunction", "functions")] {
        let refs: Vec<&str> = catalog.get("$defs").and_then(|d| d.get(key)).and_then(|d| d.get("oneOf")).and_then(Json::as_arr).unwrap().iter().map(|r| r.get("$ref").and_then(Json::as_str).unwrap()).collect();
        let names: Vec<String> = catalog.get(defs).and_then(Json::as_obj).unwrap().iter().map(|(n, _)| format!("#/{defs}/{n}")).collect();
        assert_eq!(refs, names, "{key}");
    }
    assert!(wrong.is_empty(), "{} problems:\n{}", wrong.len(), wrong[..wrong.len().min(40)].join("\n"));
    assert_eq!(all.len(), m.kinds.len() * screens::SCREENS.len() * screens::LANGS.len());
    println!("{messages} messages of {} screens of {} kinds of request, and the catalog, read as A2UI 1.0", all.len(), m.kinds.len());
}

/// The names the catalogs give: ritsu's parts and functions, and the basic catalog's functions.
struct Names {
    parts: BTreeSet<String>,
    calls: BTreeSet<String>,
    basic_calls: BTreeSet<String>,
}

fn names(catalog: &Json) -> Names {
    let keys = |j: &Json, key: &str| j.get(key).and_then(Json::as_obj).unwrap().iter().map(|(n, _)| n.clone()).collect();
    Names { parts: keys(catalog, "components"), calls: keys(catalog, "functions"), basic_calls: keys(&fixture("catalogs/basic/v1/catalog.json"), "functions") }
}

/// What is wrong with the screen of the surface `surface` (`expense.form`) as a tree: its surface,
/// its IDs and children, the catalog each part and call names, the places of the data it reads.
fn tree_wrong(surface: &str, screen: &Json, n: &Names) -> Vec<String> {
    let mut wrong = Vec::new();
    // the surface: one name in the three messages, on the basic catalog
    for msg in screen.get("messages").and_then(Json::as_arr).unwrap() {
        let body = &msg.as_obj().unwrap().iter().find(|(key, _)| key != "version").unwrap().1;
        if body.get("surfaceId").and_then(Json::as_str) != Some(surface) {
            wrong.push(format!("a message is not of the surface {surface}"));
        }
        if let Some(c) = body.get("catalogId")
            && c.as_str() != Some(screens::BASIC)
        {
            wrong.push("the surface is not on the basic catalog".to_string());
        }
    }
    // one tree: IDs once each, every child there, every part reached from `root` once
    let ps = parts(screen);
    let ids: Vec<&str> = ps.iter().map(|c| c.get("id").and_then(Json::as_str).unwrap()).collect();
    if ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
        wrong.push("an ID is given to two parts".to_string());
    }
    let children = |c: &Json| -> Vec<String> {
        let mut out: Vec<String> = c.get("child").and_then(Json::as_str).map(str::to_string).into_iter().collect();
        match c.get("children") {
            Some(Json::Arr(xs)) => out.extend(xs.iter().filter_map(Json::as_str).map(str::to_string)),
            Some(t) => out.extend(t.get("componentId").and_then(Json::as_str).map(str::to_string)),
            None => {}
        }
        out
    };
    let mut reached: Vec<String> = Vec::new();
    let mut todo = vec!["root".to_string()];
    while let Some(id) = todo.pop() {
        if reached.contains(&id) {
            wrong.push(format!("`{id}` is reached twice"));
            continue;
        }
        match ps.iter().find(|c| c.get("id").and_then(Json::as_str) == Some(&id)) {
            Some(c) => todo.extend(children(c)),
            None => wrong.push(format!("`{id}` is a child no part is")),
        }
        reached.push(id);
    }
    for id in ids.iter().filter(|id| !reached.iter().any(|r| r == *id)) {
        wrong.push(format!("`{id}` is reached from no part"));
    }
    // each part and each call names the catalog it is of: ritsu's by its ID, the basic one by none
    for c in &ps {
        let name = c.get("component").and_then(Json::as_str).unwrap();
        match (n.parts.contains(name), c.get("@catalogId").and_then(Json::as_str)) {
            (true, Some(CATALOG)) | (false, None) => {}
            _ => wrong.push(format!("the part `{}` ({name}) names its catalog otherwise", c.get("id").and_then(Json::as_str).unwrap())),
        }
    }
    let mut calls = Vec::new();
    every(screen, "@call", &mut calls);
    for (call, name) in calls {
        let name = name.as_str().unwrap();
        let ok = match call.get("catalogId").and_then(Json::as_str) {
            Some(CATALOG) => n.calls.contains(name),
            None => n.basic_calls.contains(name) || name == "@index",
            _ => false,
        };
        if !ok {
            wrong.push(format!("the call of `{name}` names its catalog otherwise, or no catalog has it"));
        }
    }
    // every place of the data read from the root is a place the data has
    let mut paths = Vec::new();
    every(screen, "@path", &mut paths);
    let mut places: Vec<&str> = paths.iter().filter_map(|(_, p)| p.as_str()).filter(|p| p.starts_with('/')).collect();
    places.extend(ps.iter().filter_map(|c| c.get("children").and_then(|t| t.get("path")).and_then(Json::as_str)));
    for p in places {
        if p.split('/').skip(1).try_fold(data(screen), |j, step| j.get(step)).is_none() {
            wrong.push(format!("`{p}` is no place of the screen's data"));
        }
    }
    wrong
}

/// Every part that finds its values at the server (`PersonPicker`, `MasterPicker`) finds them at
/// `/find` (DESIGN 13.2), which the screen's data holds as null for the server to fill (13.8); a
/// screen with no such part holds no `/find`. The catalog requires the address of both.
#[test]
fn every_picker_finds_at_the_address_the_server_fills() {
    let m = made();
    let catalog = json::parse(file(&m, "a2ui/catalog.json")).unwrap();
    for name in ["PersonPicker", "MasterPicker"] {
        let def = catalog.get("components").and_then(|c| c.get(name)).unwrap_or_else(|| panic!("no {name}"));
        let mut required = Vec::new();
        every(def, "required", &mut required);
        assert!(required.iter().any(|(_, r)| r.as_arr().is_some_and(|r| r.iter().any(|x| x.as_str() == Some("find")))), "{name} does not require `find`");
    }
    let mut wrong = Vec::new();
    let (mut pickers, mut screens) = (0, 0);
    for (_, path, screen) in screens_of(&m) {
        let found: Vec<&Json> = parts(&screen).into_iter().filter(|c| matches!(c.get("component").and_then(Json::as_str), Some("PersonPicker" | "MasterPicker"))).collect();
        for c in &found {
            if c.get("find").and_then(|f| f.get("@path")).and_then(Json::as_str) != Some("/find") {
                wrong.push(format!("{path}: `{}` does not find at `/find`", c.get("id").and_then(Json::as_str).unwrap()));
            }
        }
        pickers += found.len();
        screens += usize::from(!found.is_empty());
        match (found.is_empty(), data(&screen).get("find")) {
            (false, Some(Json::Null)) | (true, None) => {}
            (false, held) => wrong.push(format!("{path}: the data holds `/find` as {}, not null for the server", held.map(Json::compact).unwrap_or_else(|| "nothing".to_string()))),
            (true, Some(_)) => wrong.push(format!("{path}: the data holds `/find`, and no part finds at it")),
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert!(pickers > 0);
    println!("{pickers} pickers of {screens} screens find at `/find`");
}

fn surface_of(k: &Kind, path: &str) -> String {
    format!("{}.{}", k.named.name, path.rsplit('/').next().unwrap().split('.').next().unwrap())
}

#[test]
fn every_screen_is_one_tree_that_names_its_catalogs_and_reads_its_data() {
    let m = made();
    let catalog = json::parse(file(&m, "a2ui/catalog.json")).unwrap();
    assert_eq!(catalog.get("catalogId").and_then(Json::as_str), Some(CATALOG));
    let n = names(&catalog);
    let mut wrong = Vec::new();
    for (k, path, screen) in screens_of(&m) {
        wrong.extend(tree_wrong(&surface_of(k, &path), &screen, &n).into_iter().map(|w| format!("{path}: {w}")));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The codes of the checks a part carries.
fn codes(c: &Json) -> Vec<String> {
    c.get("@checks").and_then(Json::as_arr).unwrap_or(&[]).iter().filter_map(|r| r.get("condition").and_then(|c| c.get("args")).and_then(|a| a.get("code")).and_then(Json::as_str)).map(str::to_string).collect()
}

/// The codes of the checks of `f` (a field of the rows `rows`), as DESIGN 13.7 names them: whether
/// it is filled in where the machine asks for it (DESIGN 4.2, 4.3: a field the requester gives with
/// no `optional`, always or while it is shown, one with `required when` while it holds, and each
/// field of a row but an attachment; a check box and rows always hold a value), its value among a
/// choice's values, its length or count of rows, its range.
fn checks_of(f: &Fld, rows: Option<&Fld>) -> Vec<String> {
    let code = |what: &str| match rows {
        Some(r) => format!("{}.{}.{what}", r.named.name, f.named.name),
        None => format!("{}.{what}", f.named.name),
    };
    let mut out = Vec::new();
    let given = !f.computed && !f.approvers_only;
    let asked = match rows {
        Some(_) => given && !f.optional && !matches!(f.ty, TypeExpr::Attachment),
        None => given && (f.required_when.is_some() || !f.optional),
    };
    if asked && !matches!(f.ty, TypeExpr::Bool | TypeExpr::Rows) {
        out.push(code("required"));
    }
    if matches!(f.ty, TypeExpr::Choice | TypeExpr::ChoiceFrom(_)) && !f.choices.is_empty() {
        out.push(code("choice"));
    }
    let counted = f.count_min.is_some_and(|n| n > 0) || f.count_max.is_some();
    if counted && matches!(f.ty, TypeExpr::Rows | TypeExpr::Text | TypeExpr::LongText) {
        out.push(code("count"));
    }
    if matches!(f.ty, TypeExpr::Num(_) | TypeExpr::Date) && (f.min.is_some() || f.max.is_some()) {
        out.push(code("range"));
    }
    out
}

/// An end of a field's range as an input part holds it: a day as its text, a number as itself.
fn end_of(f: &Fld, n: Option<i128>) -> Option<Json> {
    n.map(|n| if matches!(f.ty, TypeExpr::Date) { Json::str(ringi::lex::date_text(i64::try_from(n).unwrap())) } else { Json::int(n) })
}

/// What is wrong with the part `c` a requester fills `f` in with (a field of the rows `rows`): its
/// part, its limits and its checks.
fn input_wrong(f: &Fld, rows: Option<&Fld>, c: &Json) -> Vec<String> {
    let mut out = Vec::new();
    let name = c.get("component").and_then(Json::as_str).unwrap_or_default();
    let want = match &f.ty {
        TypeExpr::Text | TypeExpr::LongText => "TextField",
        TypeExpr::Num(_) => "AmountField",
        TypeExpr::Date => "DateTimeInput",
        TypeExpr::Choice | TypeExpr::ChoiceFrom(_) if !f.choices.is_empty() => "ChoicePicker",
        TypeExpr::Choice | TypeExpr::ChoiceFrom(_) => "MasterPicker",
        TypeExpr::Attachment => "FileAttachment",
        TypeExpr::Person | TypeExpr::Team => "PersonPicker",
        TypeExpr::Bool => "CheckBox",
        TypeExpr::Rows => "LineItems",
    };
    if name != want {
        out.push(format!("{} is a {name}, not a {want}", f.named.name));
    }
    let (lo, hi) = match f.ty {
        TypeExpr::Rows => (f.count_min.map(Json::int), f.count_max.map(Json::int)),
        TypeExpr::Num(_) | TypeExpr::Date => (end_of(f, f.min), end_of(f, f.max)),
        _ => (None, None),
    };
    for (key, end) in [("min", lo), ("max", hi)] {
        if c.get(key).cloned() != end {
            let shown = |j: Option<Json>| j.map(|j| j.compact()).unwrap_or_else(|| "none".to_string());
            out.push(format!("{}'s {key} is {}, the field's {}", f.named.name, shown(c.get(key).cloned()), shown(end)));
        }
    }
    if let TypeExpr::Num(_) = f.ty {
        let step = c.get("step").and_then(Json::as_int).unwrap_or(1);
        if c.get("decimals").and_then(Json::as_int) != Some(i128::from(screens::decimals(&f.ty))) || step != screens::step(&f.ty) {
            out.push(format!("{} is read in other steps than its unit's", f.named.name));
        }
        let unit = c.get("currency").or_else(|| c.get("unit")).and_then(Json::as_str).unwrap_or_default();
        if unit != screens::unit_after(&f.ty, "en").trim() {
            out.push(format!("{} is read in `{unit}`, not in its unit", f.named.name));
        }
    }
    if name == "ChoicePicker" {
        let values: Vec<&str> = c.get("options").and_then(Json::as_arr).unwrap().iter().map(|o| o.get("value").and_then(Json::as_str).unwrap()).collect();
        let want: Vec<&str> = f.choices.iter().map(|c| c.name.as_str()).collect();
        if values != want {
            out.push(format!("{} offers {values:?}, not {want:?}", f.named.name));
        }
    }
    if codes(c) != checks_of(f, rows) {
        out.push(format!("{} checks {:?}, not {:?}", f.named.name, codes(c), checks_of(f, rows)));
    }
    out
}

#[test]
fn every_input_keeps_the_limits_and_the_checks_of_its_field() {
    let m = made();
    let mut wrong = Vec::new();
    let mut inputs = 0;
    for (k, path, screen) in screens_of(&m).into_iter().filter(|(_, p, _)| p.contains("/form.")) {
        let submit = part(&screen, "button.submit").unwrap_or_else(|| panic!("{path}: no button that makes the request"));
        let mut every_check = Vec::new();
        for f in k.given() {
            let c = part(&screen, &format!("field.{}", f.named.name)).unwrap_or_else(|| panic!("{path}: no part for {}", f.named.name));
            inputs += 1;
            wrong.extend(input_wrong(f, None, c).into_iter().map(|w| format!("{path}: {w}")));
            every_check.extend(checks_of(f, None));
            for col in f.fields.iter().filter(|c| c.given()) {
                let c = part(&screen, &format!("field.{}.{}", f.named.name, col.named.name)).unwrap_or_else(|| panic!("{path}: no part for {}.{}", f.named.name, col.named.name));
                inputs += 1;
                wrong.extend(input_wrong(col, Some(f), c).into_iter().map(|w| format!("{path}: {w}")));
                every_check.extend(checks_of(col, Some(f)));
            }
        }
        // the button that makes the request holds every check, each computation held to its range after
        every_check.extend(k.screen.worked.iter().flat_map(|w| &w.held).map(|h| format!("{}.worked", h.whole())));
        if codes(submit) != every_check {
            wrong.push(format!("{path}: the button checks {:?}, not {every_check:?}", codes(submit)));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    println!("{inputs} inputs keep the limits and the checks of their fields");
}

/// The numbers of `tests/machine/steps.ringi`, counted in steps of a part of their unit: what its
/// form reads each in and its cards show each with (DESIGN 13.1), and what each is written as, 125
/// of its steps in English and in Japanese, by `formatSteps` in Node and by the checks' words alike
/// (`screens::amount_text`), the framework's pages too (their DESIGN 5).
const STEPS: [(&str, i128, i128, &str, &str, &str); 10] = [
    // the field, the digits after the point, one step in the last of them, the unit after the
    // digits in English and in Japanese, and 125 of its steps as a screen writes them
    ("share", 2, 25, "%", "%", "31.25%"),
    ("double", 2, 25, "%", "%", "31.25%"),
    ("margin", 1, 25, "%", "%", "312.5%"),
    ("plain", 0, 1, "%", "%", "125%"),
    ("tenth", 1, 1, "%", "%", "12.5%"),
    ("cap", 2, 25, "%", "%", "31.25%"),
    ("cents", 2, 1, " USD", " USD", "1.25 USD"),
    ("sen", 2, 1, " JPY", "円", "1.25 JPY"),
    ("cut", 1, 5, "%", "%", "62.5%"),
    ("price", 2, 1, " USD", " USD", "1.25 USD"),
];

#[test]
fn a_number_is_read_and_shown_in_the_unit_people_count_in() {
    let m = made_of(&[PathBuf::from("tests/machine/steps.ringi")]);
    let k = &m.kinds[0];
    let fields: Vec<&Fld> = k.form.iter().flat_map(|f| std::iter::once(f).chain(&f.fields)).filter(|f| matches!(f.ty, TypeExpr::Num(_))).collect();
    let named = |n: &str| *fields.iter().find(|f| f.named.name == n).unwrap_or_else(|| panic!("no number `{n}`"));
    assert_eq!(fields.len(), STEPS.len());
    let mut wrong = Vec::new();
    let mut cases = Vec::new();
    for (n, decimals, step, en, ja, shown) in STEPS {
        let f = named(n);
        if (i128::from(screens::decimals(&f.ty)), screens::step(&f.ty)) != (decimals, step) {
            wrong.push(format!("{n} is counted in {} decimals, a step {} of the last", screens::decimals(&f.ty), screens::step(&f.ty)));
        }
        for (lang, unit) in [("en", en), ("ja", ja)] {
            let want = if lang == "ja" && unit == "円" { shown.replace(" JPY", "円") } else { shown.to_string() };
            if screens::amount_text(&f.ty, 125, lang) != want {
                wrong.push(format!("the checks say 125 of {n} as `{}`, not `{want}`", screens::amount_text(&f.ty, 125, lang)));
            }
            let mut args = vec![("value", Json::int(125)), ("decimals", Json::int(decimals))];
            if step != 1 {
                args.push(("step", Json::int(step)));
            }
            args.push(("unit", Json::str(unit)));
            cases.push(Json::obj([("args", Json::obj(args)), ("want", Json::str(&want))]));
        }
    }
    // the form reads each number a person gives in its unit's steps
    let form = screen_of(k, "form.en");
    for (n, decimals, step, en, ..) in STEPS.iter().filter(|s| named(s.0).given()) {
        let id = match k.form.iter().find(|r| r.fields.iter().any(|c| c.named.name == *n)) {
            Some(rows) => format!("field.{}.{n}", rows.named.name),
            None => format!("field.{n}"),
        };
        let c = part(&form, &id).unwrap_or_else(|| panic!("no part {id}"));
        let got = (c.get("decimals").and_then(Json::as_int), c.get("step").and_then(Json::as_int).unwrap_or(1), c.get("currency").or_else(|| c.get("unit")).and_then(Json::as_str).unwrap_or_default());
        if got != (Some(*decimals), *step, en.trim()) {
            wrong.push(format!("{id} reads {got:?}"));
        }
    }
    // the cards show each with `formatSteps`, in its unit in the card's language
    for lang in ["en", "ja"] {
        let card = screen_of(k, &format!("card.{lang}"));
        let mut calls = Vec::new();
        every(&card, "@call", &mut calls);
        let shown: Vec<(String, Json)> = calls
            .iter()
            .filter(|(_, c)| c.as_str() == Some("formatSteps"))
            .filter_map(|(call, _)| {
                let args = call.get("args")?;
                let p = args.get("value")?.get("@path")?.as_str()?;
                Some((p.rsplit('/').next()?.to_string(), args.clone()))
            })
            .collect();
        for s in STEPS {
            let unit = if lang == "en" { s.3 } else { s.4 };
            let mut want = vec![("value", Json::obj([("@path", Json::str(""))])), ("decimals", Json::int(s.1))];
            if s.2 != 1 {
                want.push(("step", Json::int(s.2)));
            }
            want.push(("unit", Json::str(unit)));
            match shown.iter().find(|(n, _)| n == s.0) {
                Some((_, args)) => {
                    let keys = |j: &Json| match j {
                        Json::Obj(o) => o.iter().filter(|(k, _)| k != "value").map(|(k, v)| (k.clone(), v.compact())).collect::<Vec<_>>(),
                        _ => Vec::new(),
                    };
                    if keys(args) != keys(&Json::obj(want)) {
                        wrong.push(format!("card.{lang} shows {} with {}", s.0, args.compact()));
                    }
                }
                None => wrong.push(format!("card.{lang} does not show {} with formatSteps", s.0)),
            }
        }
    }
    // the checks say a range in the unit too, with the zeros of its decimals, and a whole part of
    // five digits or more grouped
    for (lang, id, said) in [
        ("en", "field.share", ["-10.00%", "150.00%"]),
        ("ja", "field.share", ["-10.00%", "150.00%"]),
        ("en", "field.sen", ["0.01 JPY", "1000.00 JPY"]),
        ("ja", "field.sen", ["0.01円", "1000.00円"]),
        ("en", "field.cents", ["0.01 USD", "10,000.00 USD"]),
        ("ja", "field.cents", ["0.01 USD", "10,000.00 USD"]),
    ] {
        let form = screen_of(k, &format!("form.{lang}"));
        let mut messages = Vec::new();
        every(part(&form, id).unwrap(), "message", &mut messages);
        if !messages.iter().any(|(_, m)| said.iter().all(|w| m.as_str().unwrap_or_default().contains(w))) {
            wrong.push(format!("form.{lang}: no check of {id} says {said:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    // a rate read in hundredths of a percent where it is sent in quarters is found (W702)
    let found = |j: &Json| bindings::check(k, j).into_iter().map(|b| format!("{} {}", b.code, b.path)).collect::<Vec<_>>();
    let mut j = screen_of(k, "form.en");
    assert!(found(&j).is_empty());
    set(part_mut(&mut j, "field.share"), "step", None);
    assert_eq!(found(&j), ["W702 /form/share"]);
    // and `formatSteps` writes each as the checks do, with a few more: below zero, zeros after the
    // point, grouping from five digits, none
    for (args, want) in [
        (Json::obj([("value", Json::int(-2)), ("decimals", Json::int(2)), ("step", Json::int(25)), ("unit", Json::str("%"))]), "-0.50%"),
        (Json::obj([("value", Json::int(1000)), ("decimals", Json::int(1)), ("unit", Json::str("%"))]), "100.0%"),
        (Json::obj([("value", Json::int(1_234_567)), ("decimals", Json::int(2)), ("unit", Json::str(" USD"))]), "12,345.67 USD"),
        (Json::obj([("value", Json::int(12_000)), ("decimals", Json::int(0)), ("unit", Json::str("kg"))]), "12,000kg"),
        (Json::obj([("value", Json::int(9_007_199_254_740_991i64)), ("decimals", Json::int(2)), ("step", Json::int(25)), ("unit", Json::str("%"))]), "2,251,799,813,685,247.75%"),
        (Json::obj([("value", Json::Null), ("decimals", Json::int(2))]), ""),
    ] {
        cases.push(Json::obj([("args", args), ("want", Json::str(want))]));
    }
    if !node() {
        return;
    }
    let t = ritsu_testkit::TempDir::new("ringi-a2ui-steps");
    t.write("functions.js", file(&m, "a2ui/functions.js"));
    t.write("package.json", "{\"type\": \"module\"}\n");
    t.write("cases.json", Json::arr(cases).compact());
    t.write("driver.mjs", STEPS_DRIVER);
    let r = ritsu_testkit::run::finish(Command::new("node").arg(t.path().join("driver.mjs")).arg(t.path()), Duration::from_secs(120), "node, writing numbers in their units");
    assert!(r.ok && !r.stdout.contains("FAIL"), "{}", r.both());
    println!("Node {}: {}", ritsu_testkit::tools::version("node", "--version"), r.stdout.trim());
}

/// What runs `formatSteps` over each case and says those it writes otherwise.
const STEPS_DRIVER: &str = r#"import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { join } from "node:path";

const dir = process.argv[2];
const { functions } = await import(pathToFileURL(join(dir, "functions.js")).href);
const cases = JSON.parse(readFileSync(join(dir, "cases.json"), "utf8"));
let failed = 0;
for (const c of cases) {
  const got = functions.formatSteps(c.args);
  if (got !== c.want) {
    failed++;
    console.log(`FAIL formatSteps(${JSON.stringify(c.args)}) is ${JSON.stringify(got)}, not ${JSON.stringify(c.want)}`);
  }
}
console.log(`${cases.length - failed} of ${cases.length} numbers written in their units`);
"#;

#[test]
fn what_a_button_sends_is_what_its_screen_shows() {
    let m = made();
    let mut said = Vec::new();
    let mut buttons = 0;
    for k in &m.kinds {
        for (screen, lang, j) in screens::a2ui::screens(k, CATALOG) {
            let path = format!("a2ui/{}/{screen}.{lang}.json", k.named.name);
            assert_eq!(file(&m, &path), format!("{}\n", j.pretty()), "{path} is not the screen the check reads");
            buttons += parts(&j).iter().filter(|c| c.get("component").and_then(Json::as_str) == Some("Button")).count();
            said.extend(bindings::check(k, &j).into_iter().map(|b| format!("{path}: {} {} {}", b.code, b.path, b.why)));
        }
        // and nothing else is said of the examples' screens: no label is missing
        said.extend(screens::said(k, &m.given).into_iter().map(|s| format!("{}: {} {}", k.file, s.code, s.message.en)));
    }
    assert!(said.is_empty(), "{}", said.join("\n"));
    println!("{buttons} buttons send only what their screens show");
}

/// `j` itself, or what its `$ref` (`#/components/schemas/…`) leads to in the API `api`.
fn deref<'a>(api: &'a Json, mut j: &'a Json) -> &'a Json {
    while let Some(r) = j.get("$ref").and_then(Json::as_str) {
        let mut at = api;
        for key in r.trim_start_matches("#/").split('/') {
            at = at.get(key).unwrap_or_else(|| panic!("`{r}` leads nowhere"));
        }
        j = at;
    }
    j
}

/// An operation of the API: its `operationId`, whether its path holds a ticket (`{id}`), and the
/// keys its body and its query take and the ones they require.
struct Operation {
    id: String,
    ticket: bool,
    takes: BTreeSet<String>,
    requires: BTreeSet<String>,
}

/// Every operation of the API `api`.
fn operations(api: &Json) -> Vec<Operation> {
    let mut ops = Vec::new();
    let Some(Json::Obj(paths)) = api.get("paths") else { panic!("the API has no paths") };
    for (p, item) in paths {
        let Json::Obj(methods) = item else { continue };
        for (_, op) in methods {
            let Some(id) = op.get("operationId").and_then(Json::as_str) else { continue };
            let body = op.get("requestBody").and_then(|b| b.get("content")).and_then(|c| c.get("application/json")).and_then(|c| c.get("schema")).map(|s| deref(api, s));
            let keys = |key: &str| -> BTreeSet<String> {
                match body.and_then(|b| b.get(key)) {
                    Some(Json::Obj(pairs)) => pairs.iter().map(|(k, _)| k.clone()).collect(),
                    Some(Json::Arr(items)) => items.iter().filter_map(Json::as_str).map(str::to_string).collect(),
                    _ => BTreeSet::new(),
                }
            };
            let (mut takes, mut requires) = (keys("properties"), keys("required"));
            for q in op.get("parameters").and_then(Json::as_arr).unwrap_or_default().iter().map(|q| deref(api, q)).filter(|q| q.get("in").and_then(Json::as_str) == Some("query")) {
                let name = q.get("name").and_then(Json::as_str).unwrap_or_default().to_string();
                if q.get("required").and_then(Json::as_bool) == Some(true) {
                    requires.insert(name.clone());
                }
                takes.insert(name);
            }
            ops.push(Operation { id: id.to_string(), ticket: p.contains("{id}"), takes, requires });
        }
    }
    ops
}

/// What is wrong with the events the buttons of `screen` (at `path`) send, held to the operations
/// `ops`: each must be one, send `ticket` where its path holds one, and send as the rest of it what
/// its body or its query takes, all it requires among it. The names of the operations sent go into `used`; the
/// number of events comes back too.
fn events_wrong(ops: &[Operation], path: &str, screen: &Json, used: &mut BTreeSet<String>) -> (usize, Vec<String>) {
    let mut wrong = Vec::new();
    let mut events = 0;
    for c in parts(screen) {
        let Some(e) = c.get("action").and_then(|a| a.get("event")) else { continue };
        events += 1;
        let name = e.get("name").and_then(Json::as_str).unwrap_or_default();
        let sent: BTreeSet<String> = match e.get("context") {
            Some(Json::Obj(pairs)) => pairs.iter().map(|(k, _)| k.clone()).collect(),
            _ => BTreeSet::new(),
        };
        let Some(op) = ops.iter().find(|o| o.id == name) else {
            wrong.push(format!("{path}: `{name}` is no operation of the API"));
            continue;
        };
        used.insert(name.to_string());
        if sent.contains("ticket") != op.ticket {
            wrong.push(format!("{path}: `{name}` sends `ticket`: {}, its path holds a ticket: {}", sent.contains("ticket"), op.ticket));
        }
        let body: BTreeSet<String> = sent.iter().filter(|k| *k != "ticket").cloned().collect();
        let not_taken: Vec<&String> = body.difference(&op.takes).collect();
        let missing: Vec<&String> = op.requires.difference(&body).collect();
        if !not_taken.is_empty() || !missing.is_empty() {
            wrong.push(format!("{path}: `{name}` sends {not_taken:?}, which its body does not take, and not {missing:?}, which it requires"));
        }
    }
    (events, wrong)
}

/// Every event a button sends is an operation of the API (DESIGN 12.4, 13.1): its name is the
/// operation's `operationId`, `ticket` is the ticket of its path (`{id}`), and the rest of what it
/// sends is what the operation's body or query takes, everything they require among it. A server
/// makes each event into its operation, with nothing written for a kind.
#[test]
fn every_event_a_button_sends_is_an_operation_of_the_api() {
    let m = made();
    let ops = operations(&json::parse(file(&m, "openapi.json")).unwrap());
    let mut wrong = Vec::new();
    let mut events = 0;
    let mut used = BTreeSet::new();
    for (_, path, screen) in screens_of(&m) {
        let (n, w) = events_wrong(&ops, &path, &screen, &mut used);
        events += n;
        wrong.extend(w);
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    // the examples make every event of the API a person makes on a screen
    for op in ["get_ticket", "submit_expense", "resubmit_expense", "edit_expense", "approve", "reject", "send_back", "withdraw", "discard", "add_approver", "hand_over", "comment", "list_waiting", "list_mine"] {
        assert!(used.contains(op), "no screen sends `{op}`");
    }
    println!("{events} events, {} operations of the API", used.len());
}

/// The screen of `k` named `name` (`form.en`).
fn screen_of(k: &Kind, name: &str) -> Json {
    screens::a2ui::screens(k, CATALOG).into_iter().find(|(s, l, _)| format!("{s}.{l}") == name).map(|(_, _, j)| j).unwrap()
}

/// The part of a screen with the ID `id`, to change.
fn part_mut<'a>(screen: &'a mut Json, id: &str) -> &'a mut Json {
    let Json::Arr(ms) = at(screen, "messages") else { panic!("no messages") };
    let u = ms.iter_mut().find(|m| m.get("updateComponents").is_some()).map(|m| at(m, "updateComponents")).unwrap();
    let Json::Arr(cs) = at(u, "components") else { panic!("no parts") };
    cs.iter_mut().find(|c| c.get("id").and_then(Json::as_str) == Some(id)).unwrap_or_else(|| panic!("no part `{id}`"))
}

/// A part's children with `id` left out.
fn drop_child(screen: &mut Json, parent: &str, id: &str) {
    let Json::Arr(xs) = at(part_mut(screen, parent), "children") else { panic!("`{parent}` has no list of children") };
    let before = xs.len();
    xs.retain(|x| x.as_str() != Some(id));
    assert_eq!(xs.len() + 1, before, "`{id}` is no child of `{parent}`");
}

#[test]
fn the_changes_that_would_break_a_screen_are_found() {
    let m = made();
    let k = m.kinds.iter().find(|k| k.named.name == "expense").unwrap();
    let catalog = json::parse(file(&m, "a2ui/catalog.json")).unwrap();
    let s = schemas(&catalog);
    let n = names(&catalog);
    let found = |j: &Json| bindings::check(k, j).into_iter().map(|b| format!("{} {}", b.code, b.path)).collect::<Vec<_>>();
    let tree = |j: &Json| tree_wrong("expense.form", j, &n);
    let j = screen_of(k, "form.en");
    assert!(found(&j).is_empty() && invalid(&s, &j).is_empty() && tree(&j).is_empty());
    // an event that is no operation of the API, one that sends what its body does not take, and one
    // that does not send what its body requires
    let ops = operations(&json::parse(file(&m, "openapi.json")).unwrap());
    let events = |name: &str, j: &Json| events_wrong(&ops, name, j, &mut BTreeSet::new()).1;
    let mut j = screen_of(k, "card.en");
    set(at(at(part_mut(&mut j, "button.edit"), "action"), "event"), "name", Some(Json::str("edit")));
    assert_eq!(events("card.en", &j), ["card.en: `edit` is no operation of the API"]);
    let mut j = screen_of(k, "list.ja");
    set(at(at(part_mut(&mut j, "line.open"), "action"), "event"), "name", Some(Json::str("open")));
    assert_eq!(events("list.ja", &j), ["list.ja: `open` is no operation of the API"]);
    let mut j = screen_of(k, "form.en");
    set(at(at(at(part_mut(&mut j, "button.submit"), "action"), "event"), "context"), "requester_name", Some(Json::str("x")));
    assert_eq!(events("form.en", &j), ["form.en: `submit_expense` sends [\"requester_name\"], which its body does not take, and not [], which it requires"]);
    let mut j = screen_of(k, "card.en");
    set(at(at(at(part_mut(&mut j, "button.hand_over"), "action"), "event"), "context"), "to", None);
    assert_eq!(events("card.en", &j), ["card.en: `hand_over` sends [], which its body does not take, and not [\"to\"], which it requires"]);
    // a value the form sends that no part shows: the purpose taken out of the form; the part is
    // then reached from no other
    let mut j = screen_of(k, "form.en");
    drop_child(&mut j, "root", "field.purpose");
    assert_eq!(found(&j), ["E701 /form/purpose"]);
    assert_eq!(tree(&j), ["`field.purpose` is reached from no part"]);
    // a field an approver gives that the card does not show
    let mut j = screen_of(k, "card.ja");
    drop_child(&mut j, "edit", "edit.account");
    assert_eq!(found(&j), ["E701 /edit/account"]);
    // the next page of a list sends the ID of the page's last ticket, which each line shows: with
    // the number taken out of the line, it is sent and not shown
    for name in ["list.en", "mine.ja"] {
        let j = screen_of(k, name);
        assert!(found(&j).is_empty(), "{name}: {:?}", found(&j));
        let mut j = screen_of(k, name);
        drop_child(&mut j, "line", "line.id");
        assert_eq!(found(&j), ["E701 /tickets/*/id"], "{name}");
    }
    // a price shown with decimals yen are not sent in
    let mut j = screen_of(k, "form.ja");
    set(part_mut(&mut j, "field.items.price"), "decimals", Some(Json::int(2)));
    assert_eq!(found(&j), ["W702 /form/items/*/price"]);
    // the names A2UI 1.0 gives its own properties, written without their `@` as before 1.0
    for (id, from, to) in [("field.purpose", "@checks", "checks"), ("field.receipt", "@catalogId", "catalogId")] {
        let mut j = screen_of(k, "form.en");
        let p = part_mut(&mut j, id);
        let v = p.get(from).cloned();
        set(p, from, None);
        set(p, to, v);
        assert!(!invalid(&s, &j).is_empty(), "`{to}` in place of `{from}` reads as A2UI 1.0");
    }
    // a part of ritsu's that does not name ritsu's catalog would be looked for in the basic one:
    // the schema with the two catalogs merged takes it, the check of the tree does not
    let mut j = screen_of(k, "form.en");
    set(part_mut(&mut j, "field.receipt"), "@catalogId", None);
    assert!(invalid(&s, &j).is_empty());
    assert_eq!(tree(&j), ["the part `field.receipt` (FileAttachment) names its catalog otherwise"]);
    // and a call of ritsu's that names none
    let mut j = screen_of(k, "form.en");
    let condition = at(at(at(part_mut(&mut j, "field.purpose"), "@checks"), "0"), "condition");
    set(condition, "catalogId", None);
    assert_eq!(tree(&j), ["the call of `expense.check` names its catalog otherwise, or no catalog has it"]);
    // a picker that does not say where it finds, and a card whose data does not hold the address
    let mut j = screen_of(k, "card.en");
    set(part_mut(&mut j, "edit.account"), "find", None);
    assert!(!invalid(&s, &j).is_empty(), "a MasterPicker with no `find` reads as A2UI 1.0");
    let mut j = screen_of(k, "card.en");
    set(at(at(at(at(&mut j, "messages"), "1"), "updateDataModel"), "value"), "find", None);
    let said = tree_wrong("expense.card", &j, &n);
    assert!(!said.is_empty() && said.iter().all(|w| w == "`/find` is no place of the screen's data"), "{said:?}");
    // a place the data does not have
    let mut j = screen_of(k, "form.en");
    set(at(part_mut(&mut j, "field.purpose"), "value"), "@path", Some(Json::str("/form/purpose_")));
    assert_eq!(tree(&j), ["`/form/purpose_` is no place of the screen's data"]);
    // the rows' limit taken away, and the checks of the price: the inputs no longer keep their fields'
    let items = k.form.iter().find(|f| f.named.name == "items").unwrap();
    let price = items.fields.iter().find(|f| f.named.name == "price").unwrap();
    let mut j = screen_of(k, "form.en");
    set(part_mut(&mut j, "field.items"), "max", None);
    set(part_mut(&mut j, "field.items.price"), "@checks", None);
    assert_eq!(input_wrong(items, None, part(&j, "field.items").unwrap()), ["items's max is none, the field's 20"]);
    assert_eq!(input_wrong(price, Some(items), part(&j, "field.items.price").unwrap()), [r#"price checks [], not ["items.price.required", "items.price.range"]"#]);
}

/// A form as the API takes it, as a screen holds it: a choice a ChoicePicker writes as a list of one.
fn as_shown(k: &Kind, form: &Json) -> Json {
    let Json::Obj(pairs) = form else { return form.clone() };
    Json::obj(pairs.iter().map(|(name, v)| {
        let picked = k.form.iter().find(|f| f.named.name == *name).is_some_and(|f| matches!(f.ty, TypeExpr::Choice | TypeExpr::ChoiceFrom(_)) && !f.choices.is_empty());
        (name.clone(), if picked && v.as_str().is_some() { Json::arr([v.clone()]) } else { v.clone() })
    }))
}

/// The fields of the form of `k` its screens' functions work out (`<kind>.<field>`): a number the
/// form computes, which the run of the functions holds to the reference's value ([`cases`]).
fn worked_of(k: &Kind) -> Vec<&str> {
    k.form.iter().filter(|f| f.computed && matches!(f.ty, TypeExpr::Num(_)) && k.screen.worked.as_ref().is_some_and(|w| w.outputs.iter().any(|(n, _)| *n == f.named.name))).map(|f| f.named.name.as_str()).collect()
}

/// The fields a kind's description marks as worked out on the screen (`worked_on_screen`, DESIGN
/// 12.2) are the ones the screens' functions work out, each held to the reference's value: a
/// number of the form it computes; no computed day, and no field of a row.
#[test]
fn the_fields_marked_worked_out_are_the_ones_the_functions_work_out() {
    let m = made();
    let mut marked_in_all = 0;
    for k in &m.kinds {
        let ty = json::parse(file(&m, &format!("types/{}.json", k.named.name))).unwrap();
        let mut marked = Vec::new();
        for f in ty.get("form").and_then(Json::as_arr).unwrap() {
            let name = f.get("name").and_then(Json::as_str).unwrap();
            if f.get("worked_on_screen").and_then(Json::as_bool).unwrap_or_else(|| panic!("{}: `{name}` has no mark", k.named.name)) {
                marked.push(name);
            }
            for c in f.get("fields").and_then(Json::as_arr).unwrap_or_default() {
                assert_eq!(c.get("worked_on_screen").and_then(Json::as_bool), Some(false), "{}: a field of the rows of `{name}`", k.named.name);
            }
        }
        assert_eq!(marked, worked_of(k), "{}", k.named.name);
        let functions = file(&m, "a2ui/functions.js");
        assert!(functions.contains("if (f.worked) {"), "the functions work out the fields marked `worked`");
        marked_in_all += marked.len();
    }
    assert!(marked_in_all > 0);
}

/// What the module of the functions is run with, beside it: every form the vectors of the kinds
/// make a request with, as a screen holds it, with the codes of its screen's checks, what the
/// reference answers, the fields the reference took and what it worked out. With the count of each
/// kind's forms the reference takes and refuses, for the form or for the comment made with it
/// (whose length the screen holds as well: `commentFits`).
fn cases(m: &Made) -> (Vec<Json>, Vec<(String, usize, usize)>) {
    let mut cases = Vec::new();
    let mut counts = Vec::new();
    for k in &m.kinds {
        let before = cases.len();
        let mut taken = 0;
        let form = json::parse(file(m, &format!("a2ui/{}/form.en.json", k.named.name))).unwrap();
        let codes: Vec<Json> = codes(part(&form, "button.submit").unwrap()).into_iter().map(Json::str).collect();
        let given: Vec<&str> = k.given().map(|f| f.named.name.as_str()).collect();
        let worked = worked_of(k);
        for line in file(m, &format!("vectors/{}.jsonl", k.named.name)).lines() {
            let v = json::parse(line).unwrap();
            let Some(submit) = v.get("event").and_then(|e| e.get("submit")) else { continue };
            let answer = v.get("answer").unwrap();
            let (expect, why, took) = match (answer.get("ok"), answer.get("err")) {
                (Some(ok), _) => ("ok", String::new(), ok.get("ticket").and_then(|t| t.get("form")).cloned().unwrap_or(Json::Null)),
                (_, Some(err)) if matches!(err.get("refused").and_then(Json::as_str), Some("form" | "comment")) => {
                    ("refused", err.get("text").and_then(Json::as_str).unwrap_or_default().to_string(), Json::Null)
                }
                _ => continue,
            };
            taken += usize::from(expect == "ok");
            let pick = |names: &[&str]| Json::obj(names.iter().filter_map(|f| Some((*f, took.get(f)?.clone()))));
            cases.push(Json::obj([
                ("kind", Json::str(&k.named.name)),
                ("from", Json::str(format!("{} ({})", v.get("from").and_then(Json::as_str).unwrap_or_default(), k.file))),
                ("form", as_shown(k, submit.get("form").unwrap())),
                ("comment", submit.get("comment").cloned().unwrap_or(Json::Null)),
                ("codes", Json::arr(codes.clone())),
                ("expect", Json::str(expect)),
                ("why", Json::str(why)),
                ("took", pick(&given)),
                ("worked", pick(&worked)),
            ]));
        }
        counts.push((k.named.name.clone(), taken, cases.len() - before - taken));
    }
    (cases, counts)
}

/// Whether Node runs here, to run the functions with; the test says it is not run when it does not.
fn node() -> bool {
    ritsu_testkit::ready(ritsu_testkit::Need::Node, || ritsu_testkit::tools::runs("node", &["--version"]), "node is not on the PATH; the functions of the screens are not run against the vectors")
}

/// The comments the vectors of each kind that asks a reason (`comment required for`, DESIGN 7.2)
/// give with an event it asks one with, each with whether the reference takes the event (`taken`)
/// or refuses it for want of a reason; an event refused for anything else is left out.
fn reasons_of(m: &Made) -> Vec<Json> {
    let mut out = Vec::new();
    for k in m.kinds.iter().filter(|k| !k.reasoned.is_empty()) {
        for line in file(m, &format!("vectors/{}.jsonl", k.named.name)).lines() {
            let v = json::parse(line).unwrap();
            let Some((e, body)) = v.get("event").and_then(Json::as_obj).and_then(|o| o.first()) else { continue };
            if !k.reasoned.contains(&e.as_str()) {
                continue;
            }
            let answer = v.get("answer").unwrap();
            let taken = match (answer.get("ok"), answer.get("err")) {
                (Some(_), _) => true,
                (_, Some(err)) if err.get("text").and_then(Json::as_str).is_some_and(|t| t.ends_with("needs a reason: write it in the comment")) => false,
                _ => continue,
            };
            let from = format!("{} ({})", v.get("from").and_then(Json::as_str).unwrap_or_default(), k.file);
            out.push(Json::obj([("from", Json::str(from)), ("comment", body.get("comment").cloned().unwrap_or(Json::Null)), ("taken", Json::from(taken))]));
        }
    }
    out
}

/// The functions of `m` run in Node over `cases`, the forms with a field hidden, `hidden`, the
/// numbers written with white space at their ends, `numbers`, and the comments of the events its
/// kinds ask a reason with ([`reasons_of`]): what the driver prints.
fn run_functions(m: &Made, cases: Vec<Json>, hidden: Json, numbers: Json) -> String {
    let t = ritsu_testkit::TempDir::new("ringi-a2ui-functions");
    t.write("functions.js", file(m, "a2ui/functions.js"));
    t.write("catalog.json", file(m, "a2ui/catalog.json"));
    t.write("package.json", "{\"type\": \"module\"}\n");
    t.write("cases.json", Json::obj([("cases", Json::arr(cases)), ("hidden", hidden), ("reasons", Json::arr(reasons_of(m))), ("numbers", numbers)]).compact());
    t.write("driver.mjs", DRIVER);
    let r = ritsu_testkit::run::finish(Command::new("node").arg(t.path().join("driver.mjs")).arg(t.path()), Duration::from_secs(120), "node, running the functions of the screens");
    assert!(r.ok, "{}", r.both());
    format!("Node {}: {}", ritsu_testkit::tools::version("node", "--version"), r.stdout.trim())
}

/// What runs the functions: each case, and the functions every kind shares.
const DRIVER: &str = r#"import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { join } from "node:path";

const dir = process.argv[2];
const { functions, catalogId } = await import(pathToFileURL(join(dir, "functions.js")).href);
const catalog = JSON.parse(readFileSync(join(dir, "catalog.json"), "utf8"));
const { cases, hidden, reasons, numbers } = JSON.parse(readFileSync(join(dir, "cases.json"), "utf8"));
const out = [];
const fail = (s) => out.push("FAIL " + s);
const canon = (v) => JSON.stringify(v, (k, x) => (x && typeof x === "object" && !Array.isArray(x) ? Object.fromEntries(Object.entries(x).sort()) : x));

// the module defines the functions the catalog names, and no other
const named = Object.keys(catalog.functions).sort();
const defined = Object.keys(functions).sort();
for (const n of named.filter((n) => !defined.includes(n))) fail(`the catalog names ${n}, which the module does not define`);
for (const n of defined.filter((n) => !named.includes(n))) fail(`the module defines ${n}, which the catalog does not name`);
if (catalogId !== catalog.catalogId) fail(`the module's catalogId is ${catalogId}`);

let taken = 0, refused = 0, worked = 0;
for (const c of cases) {
  const k = c.kind;
  const failed = c.codes.filter((code) => functions[`${k}.check`]({ form: c.form, code }) !== true);
  if (!functions.commentFits({ text: c.comment ?? "" })) failed.push("the comment's length");
  if (c.expect === "ok") {
    taken++;
    if (failed.length) fail(`${c.from}: the reference takes the form, and ${failed.join(", ")} fails`);
    const sent = functions[`${k}.given`]({ form: c.form });
    if (canon(sent) !== canon(c.took)) fail(`${c.from}: the form is sent as ${canon(sent)}, and the reference took ${canon(c.took)}`);
    for (const [f, v] of Object.entries(c.worked)) {
      worked++;
      const got = functions[`${k}.${f}`]({ form: c.form });
      if (got !== v) fail(`${c.from}: ${f} is worked out as ${got}, and the reference's is ${v}`);
    }
  } else {
    refused++;
    if (!failed.length) fail(`${c.from}: the reference refuses the request (${c.why}), and every check passes`);
  }
}
for (const h of hidden) {
  const sent = functions[`${h.kind}.given`]({ form: h.form });
  if (h.field in sent) fail(`${h.kind}: the hidden ${h.field} is sent`);
  if (!functions[`${h.kind}.cleared`]({ form: h.form, before: h.before }).includes(h.field)) fail(`${h.kind}: the hidden ${h.field} is not made empty`);
  if (functions[`${h.kind}.shown_${h.field}`]({ form: h.form }) !== false) fail(`${h.kind}: ${h.field} is shown`);
}

// a number written in a text box, with white space at its ends: sent without Unicode's White_Space
// (U+0085 is, U+FEFF is not), and refused by the range where what is left is no number
for (const n of numbers) {
  const sent = functions[`${n.kind}.given`]({ form: n.form });
  const got = n.field in sent ? sent[n.field] : null;
  if (canon(got) !== canon(n.sent)) fail(`${n.kind}: ${JSON.stringify(n.form[n.field])} in ${n.field} is sent as ${canon(got)}, not ${canon(n.sent)}`);
  const passes = functions[`${n.kind}.check`]({ form: n.form, code: `${n.field}.range` });
  if (passes !== n.passes) fail(`${n.kind}: ${JSON.stringify(n.form[n.field])} in ${n.field} ${passes ? "passes" : "fails"} its range`);
}

// a comment given with an event the kind asks a reason with: a comment the reference refuses the
// event for want of is no comment to the card's button, and one it takes the event with is one
for (const r of reasons) {
  const given = functions.commentGiven({ text: r.comment ?? "" });
  if (given !== r.taken) fail(`${r.from}: commentGiven answers ${given} for ${JSON.stringify(r.comment)}, and the reference ${r.taken ? "takes the event" : "refuses it for want of a reason"}`);
}

// the functions every kind shares
const twoThousand = "\u{20BB7}".repeat(2000);
const shared = [
  ["comment, the white space at its ends", functions.comment({ text: "　 a comment \n　" }), "a comment"],
  ["comment, nothing but white space", functions.comment({ text: " 　\t" }), null],
  ["commentGiven, nothing but white space", functions.commentGiven({ text: "　" }), false],
  ["comment, U+0085 at its ends, which is white space", functions.comment({ text: "\u0085a\u0085" }), "a"],
  ["comment, U+FEFF, which is no white space", functions.comment({ text: "\uFEFF" }), "\uFEFF"],
  ["commentGiven, U+001C, which is no white space", functions.commentGiven({ text: "\u001C" }), true],
  ["commentFits, 2,000 characters outside the BMP", functions.commentFits({ text: twoThousand }), true],
  ["commentFits, 2,001 characters", functions.commentFits({ text: twoThousand + "a" }), false],
  ["commentFits, 2,000 characters within white space", functions.commentFits({ text: "　" + twoThousand + " " }), true],
  ["one, a list of one", functions.one({ values: ["travel"] }), "travel"],
  ["one, an empty list", functions.one({ values: [] }), null],
  ["lookup, a list of codes", functions.lookup({ value: ["a", "b"], table: { a: "A" }, separator: "、" }), "A、b"],
  ["lookup, a code with no words", functions.lookup({ value: "x", table: {} }), "x"],
  ["lookup, no value", functions.lookup({ value: null, table: { a: "A" } }), ""],
  ["numberFrom", functions.numberFrom({ text: " 12000 " }), 12000],
  ["numberFrom, no digits alone", functions.numberFrom({ text: "12,000" }), null],
  ["numberFrom, U+0085 at its ends, which is white space", functions.numberFrom({ text: "\u008512000\u0085" }), 12000],
  ["numberFrom, U+FEFF at its ends, which is no white space", functions.numberFrom({ text: "\uFEFF12000\uFEFF" }), null],
  ["numberFrom, U+3000 and a tab at its ends", functions.numberFrom({ text: "\u3000\t12000\u3000" }), 12000],
  ["equals", functions.equals({ a: [1, { b: 2 }], b: [1, { b: 2 }] }), true],
];
for (const [what, got, want] of shared) if (got !== want) fail(`${what}: ${JSON.stringify(got)}, not ${JSON.stringify(want)}`);

console.log(`${cases.length} forms of the vectors: ${taken} taken and ${refused} refused for the form or its comment; ${worked} values worked out; ${hidden.length} hidden field; ${reasons.length} comments of an event asked a reason with; ${numbers.length} numbers with white space at their ends; ${shared.length} answers of the shared functions`);
for (const line of out) console.log(line);
process.exit(out.length ? 1 : 0);
"#;

#[test]
fn the_functions_answer_the_vectors_as_the_reference_does() {
    if !node() {
        return;
    }
    let m = made();
    let (cases, counts) = cases(&m);
    // every kind's vectors make requests the reference takes, and some it refuses for the form or
    // its comment
    assert!(counts.iter().all(|(_, taken, refused)| *taken > 0 && *refused > 0), "{counts:?}");
    // the leave asks a reason: its vectors give the events it asks one with and none, and with one
    let reasons = reasons_of(&m);
    let taken = reasons.iter().filter(|r| r.get("taken") == Some(&Json::from(true))).count();
    assert!(taken > 0 && reasons.len() - taken >= 16, "{taken} of {}", reasons.len());
    // a field shown on a condition that does not hold is not sent, and is made empty when it was given
    let hidden = Json::arr([Json::obj([
        ("kind", Json::str("expense")),
        ("form", json::parse(r#"{"category": ["expense"], "items": [{"what": "Paper", "price": 1200}], "purpose": "Paper for the office", "used_on": "2026-11-16", "trip_to": "Osaka"}"#).unwrap()),
        ("before", json::parse(r#"{"category": "travel", "items": [{"what": "Train", "price": 1200}], "purpose": "A visit", "used_on": "2026-11-16", "trip_to": "Osaka"}"#).unwrap()),
        ("field", Json::str("trip_to")),
    ])]);
    // the days of a leave, written with white space at their ends: Unicode's White_Space, the 25 code
    // points the machine and a text read as white space, is taken off (U+0085 and U+3000 are, U+FEFF
    // is not); nothing left is nothing written; what is left that is no number fails the range
    let leave = |days: &str, sent: Json, passes: bool| {
        let form = json::parse(r#"{"kind": ["paid"], "first_day": "2026-11-16"}"#).unwrap();
        let mut form: Vec<(String, Json)> = form.as_obj().unwrap().to_vec();
        form.push(("days".to_string(), Json::str(days)));
        Json::obj([("kind", Json::str("leave")), ("form", Json::Obj(form)), ("field", Json::str("days")), ("sent", sent), ("passes", Json::from(passes))])
    };
    let numbers = Json::arr([
        leave("\u{85}5\u{85}", Json::int(5), true),
        leave("\u{3000}5\t", Json::int(5), true),
        leave("\u{85}", Json::Null, true),
        leave("\u{FEFF}5\u{FEFF}", Json::str("\u{FEFF}5\u{FEFF}"), false),
        leave("\u{FEFF}", Json::str("\u{FEFF}"), false),
    ]);
    println!("{}\n{counts:?}", run_functions(&m, cases, hidden, numbers));
}

/// A request whose choice has values rulec reads as something else (`after`, `by`), which the
/// lowered files write under other names (`after_value`, DESIGN 6.4): its form works out a total,
/// shows a field when the choice is `usual` and asks for another past a total; its scenarios, one on
/// each value of the choice, are what the vectors vary. Written beside the examples, it reads their
/// organization.
const WORDS: &str = "request words v1 \"Words the rules read otherwise\"
description \"A choice whose values rulec reads as something else (fictional rules)\"

use organization people from \"shared/company.ringi\"

form
  date  : choice \"Kind of day\"
    after \"After\"
    by    \"By\"
    usual \"Usual\"
  count : rows \"Items\"  count >=1 <=5
    max : money[JPY] \"Price\"  range >=1JPY <=1_000JPY
  total : money[JPY] \"Total\" = sum(count.max)  range >=1JPY <=5_000JPY
  note  : text \"Note\"  count <=100  shown when date is usual
  seats : number \"Seats\"  range >=0 <=9  required when total > 1_000JPY

route
  check   finance
  decide  boss  when total > 1_000JPY

who
  finance = team accounting
  boss    = manager of requester

every stage
  when empty escalate, then skip

ticket
  self approval  not allowed

scenario a_day_after
  requester e1201
  form date = after, count = [{max: 500JPY}]
| at               | event   | by    | -> stage | status      |
| 2026-11-16 09:00 | submit  | e1201 | check    | in progress |
| 2026-11-16 10:00 | approve | e1001 |          | approved    |

scenario a_day_by
  requester e1201
  form date = by, count = [{max: 1_000JPY}, {max: 200JPY}], seats = 1
| at               | event   | by    | -> stage | status      |
| 2026-11-16 09:00 | submit  | e1201 | check    | in progress |
| 2026-11-16 10:00 | approve | e1001 | decide   | in progress |
| 2026-11-16 11:00 | approve | e1101 |          | approved    |

scenario a_usual_day
  requester e1201
  form date = usual, count = [{max: 300JPY}], note = \"One item\"
| at               | event   | by    | -> stage | status      |
| 2026-11-16 09:00 | submit  | e1201 | check    | in progress |
| 2026-11-16 10:00 | approve | e1001 |          | approved    |
";

/// A choice's values the lowered files write under other names are given to `values.rule` under
/// those names (DESIGN 6.4, 13.7): on each value of the choice, the functions answer the vectors as
/// the reference does, what the form works out, the field shown and the field asked for.
#[test]
fn a_choice_the_rules_name_otherwise_is_given_to_them_so() {
    let path = "examples/words.ringi";
    let mut laid = ritsu_base::fs::Overlay::new(ritsu_base::fs::current());
    laid.put(&ritsu_base::paths::absolute(Path::new(path)), WORDS.to_string());
    ritsu_base::fs::with(Rc::new(laid), || {
        let m = made_of(&[PathBuf::from(path)]);
        let functions = file(&m, "a2ui/functions.js");
        // the choice is given to the rule with the names it writes `after` and `by` under
        for named in ["\"after\": \"after_value\"", "\"by\": \"by_value\""] {
            assert!(functions.contains(named), "{named} is not in the functions:\n{functions}");
        }
        if !node() {
            return;
        }
        let (cases, counts) = cases(&m);
        assert!(counts.iter().all(|(_, taken, refused)| *taken > 0 && *refused > 0), "{counts:?}");
        // the reference takes a form on each value of the choice
        for v in ["after", "by", "usual"] {
            let on = Json::arr([Json::str(v)]);
            assert!(cases.iter().any(|c| c.get("expect").and_then(Json::as_str) == Some("ok") && c.get("form").and_then(|f| f.get("date")) == Some(&on)), "no form the reference takes on `{v}`");
        }
        println!("{}\n{counts:?}", run_functions(&m, cases, Json::arr([]), Json::arr([])));
    });
}

/// A request written in English names with no `labels ja` (the tests of the machine write none) sets
/// its English labels apart in the Japanese sentences of its screens (DESIGN 13.4): a space after a
/// label that ends in a letter, a digit or a sign of ASCII, and before one that starts with one after
/// Japanese words. The examples write the labels of both languages, so their screens show none of it.
#[test]
fn an_english_label_in_a_japanese_sentence_is_set_apart() {
    let m = made_of(&[PathBuf::from("tests/machine/absent.ringi")]);
    let form = file(&m, "a2ui/absent/form.ja.json");
    for said in ["Kind を選んでください。", "Lines の Price は1円から1000円までにしてください。"] {
        assert!(form.contains(said), "`{said}` is not in the Japanese form:\n{form}");
    }
    let history = file(&m, "a2ui/absent/history.ja.json");
    assert!(history.contains("Supplies with values left empty の履歴"), "the title is not set apart in the Japanese history:\n{history}");
}

/// What draws a screen as a renderer does, for each case: a screen, data put over its own (key by
/// key), and a button. It prints the parts drawn (a `Show` draws its child while `when` holds, a
/// `List` a line for each of its path's) and what the button sends, null when it is not drawn.
const DRAW: &str = r#"import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { join } from "node:path";

const dir = process.argv[2];
const { functions } = await import(pathToFileURL(join(dir, "functions.js")).href);
const { cases } = JSON.parse(readFileSync(join(dir, "cases.json"), "utf8"));
const empty = (v) => v === null || v === undefined || v === "" || (Array.isArray(v) && v.length === 0);
const holds = (v) => v !== false && !empty(v);
const basic = {
  required: ({ value }) => !empty(value),
  or: ({ values }) => values.some((v) => v === true),
  and: ({ values }) => values.every((v) => v === true),
  not: ({ value }) => value !== true,
  formatCurrency: ({ value }) => String(value),
  formatNumber: ({ value }) => String(value),
  formatDate: ({ value }) => String(value),
  formatString: ({ value }) => String(value),
  openUrl: () => null,
};
for (const c of cases) {
  const doc = c.screen;
  const model = Object.assign(structuredClone(doc.messages.find((m) => m.updateDataModel).updateDataModel.value), c.data);
  const byId = Object.fromEntries(doc.messages.find((m) => m.updateComponents).updateComponents.components.map((x) => [x.id, x]));
  const at = (path, scope) => {
    if (!path.startsWith("/")) return path === "" || path === "." ? scope.item : at("/" + path, { item: undefined, root: scope.item });
    let x = scope && scope.root !== undefined ? scope.root : model;
    for (const seg of path.split("/").slice(1)) x = x === null || x === undefined ? undefined : x[seg];
    return x;
  };
  const value = (x, scope) => {
    if (x === null || typeof x !== "object") return x;
    if (Array.isArray(x)) return x.map((y) => value(y, scope));
    if (typeof x["@path"] === "string") return at(x["@path"], scope);
    if (typeof x["@call"] === "string") {
      if (x["@call"] === "@index") return scope.index;
      const f = functions[x["@call"]] || basic[x["@call"]];
      if (!f) throw new Error(`no function ${x["@call"]}`);
      return f(Object.fromEntries(Object.entries(x.args || {}).map(([k, v]) => [k, value(v, scope)])));
    }
    return Object.fromEntries(Object.entries(x).map(([k, v]) => [k, value(v, scope)]));
  };
  const drawn = [];
  const walk = (id, scope) => {
    const p = byId[id];
    if (p.component === "Show") {
      if (holds(value(p.when, scope))) walk(p.child, scope);
      return;
    }
    if (!scope) drawn.push(id);
    if (p.children && !Array.isArray(p.children)) {
      const rows = at(p.children.path, scope);
      (Array.isArray(rows) ? rows : []).forEach((item, index) => walk(p.children.componentId, { item, index }));
      return;
    }
    if (typeof p.child === "string") walk(p.child, scope);
    if (Array.isArray(p.children)) p.children.forEach((x) => walk(x, scope));
  };
  walk("root", undefined);
  const sent = c.button && drawn.includes(c.button) ? { name: byId[c.button].action.event.name, context: value(byId[c.button].action.event.context, undefined) } : null;
  console.log(JSON.stringify({ drawn, sent }));
}
"#;

/// What [`DRAW`] prints for `cases`, each `{screen, data, button}`.
fn drawn(m: &Made, cases: Vec<Json>) -> Vec<Json> {
    let t = ritsu_testkit::TempDir::new("ringi-a2ui-drawn");
    t.write("functions.js", file(m, "a2ui/functions.js"));
    t.write("package.json", "{\"type\": \"module\"}\n");
    let n = cases.len();
    t.write("cases.json", Json::obj([("cases", Json::arr(cases))]).compact());
    t.write("draw.mjs", DRAW);
    let r = ritsu_testkit::run::finish(Command::new("node").arg(t.path().join("draw.mjs")).arg(t.path()), Duration::from_secs(120), "node, drawing the screens");
    assert!(r.ok, "{}", r.both());
    let out: Vec<Json> = r.stdout.lines().map(|l| json::parse(l).unwrap_or_else(|e| panic!("{e:?}: {l}"))).collect();
    assert_eq!(out.len(), n, "{}", r.both());
    out
}

fn ids(j: &Json) -> Vec<&str> {
    j.get("drawn").and_then(Json::as_arr).unwrap_or_default().iter().filter_map(Json::as_str).collect()
}

/// A refusal is shown beside what it is of (DESIGN 13.8), on every form and card: after the field
/// its place names (`/refused/field`), after the comment when it is one of the comment
/// (`/refused/comment`, the kind `comment`), at the head of the screen otherwise; in one place, and
/// nowhere while there is none.
#[test]
fn a_refusal_is_shown_beside_what_it_is_of() {
    if !node() {
        return;
    }
    let m = made();
    let mut cases = Vec::new();
    let mut want: Vec<(String, Option<String>)> = Vec::new();
    for (k, path, screen) in screens_of(&m).into_iter().filter(|(_, p, _)| p.contains("/form.") || p.contains("/card.")) {
        let field = k.form.iter().find(|f| f.given()).map(|f| f.named.name.clone()).unwrap();
        for (text, of_field, of_comment, shown) in [
            (Some("Refused"), Some(field.as_str()), false, Some(format!("field.{field}.refused.text"))),
            (Some("Refused"), None, true, Some("comment.refused.text".to_string())),
            (Some("Refused"), None, false, Some("refused.text".to_string())),
            (None, None, false, None),
        ] {
            let refused = Json::obj([("text", text.map_or(Json::Null, Json::str)), ("field", of_field.map_or(Json::Null, Json::str)), ("comment", Json::Bool(of_comment))]);
            cases.push(Json::obj([("screen", screen.clone()), ("data", Json::obj([("refused", refused)])), ("button", Json::Null)]));
            want.push((path.clone(), shown));
        }
    }
    let out = drawn(&m, cases);
    for ((path, shown), got) in want.iter().zip(&out) {
        let refusals: Vec<&str> = ids(got).into_iter().filter(|id| id.ends_with("refused.text")).collect();
        assert_eq!(refusals, shown.as_deref().into_iter().collect::<Vec<_>>(), "{path}");
    }
    println!("{} screens drawn with a refusal of each place", want.len());
}

/// Where each screen shows a refusal of each field of the form (DESIGN 13.8, the table of where a
/// refusal is shown): the form after the field it places (the requester's fields, and the
/// computations the screens work out), and at its head for a field it does not place (the
/// approvers' fields, and a computation the machine alone works out, a counted date); the card
/// places every field, and shows each one's refusal after it. No field's refusal is shown nowhere,
/// on any form or card of the examples, in English and in Japanese.
#[test]
fn a_refusal_of_every_field_is_shown_where_the_table_says() {
    if !node() {
        return;
    }
    let m = made();
    let mut cases = Vec::new();
    let mut want: Vec<(String, String, &str)> = Vec::new();
    let mut by_place: std::collections::BTreeMap<(&str, &str), usize> = std::collections::BTreeMap::new();
    for (k, path, screen) in screens_of(&m).into_iter().filter(|(_, p, _)| p.contains("/form.") || p.contains("/card.")) {
        let form = path.contains("/form.");
        for f in &k.form {
            let name = &f.named.name;
            let (place, shown) = match (form, f.given(), f.worked_on_screen) {
                (true, true, _) => ("the form: a field the requester gives", format!("field.{name}.refused.text")),
                (true, false, true) => ("the form: a computation the screens work out", format!("field.{name}.refused.text")),
                (true, false, false) => ("the form: a field it does not place", "refused.text".to_string()),
                (false, ..) => ("the card: every field", format!("field.{name}.refused.text")),
            };
            *by_place.entry((place, if f.computed { "computed" } else if f.approvers_only { "approvers'" } else { "given" })).or_default() += 1;
            let refused = Json::obj([("text", Json::str("Refused")), ("field", Json::str(name)), ("comment", Json::Bool(false))]);
            cases.push(Json::obj([("screen", screen.clone()), ("data", Json::obj([("refused", refused)])), ("button", Json::Null)]));
            want.push((format!("{path}, {name}"), shown, place));
        }
    }
    let out = drawn(&m, cases);
    for ((what, shown, place), got) in want.iter().zip(&out) {
        let refusals: Vec<&str> = ids(got).into_iter().filter(|id| id.ends_with("refused.text")).collect();
        assert_eq!(refusals, [shown.as_str()], "{what} ({place})");
    }
    // the examples hold every row of the table: the computations of the expense and the purchase
    // the screens work out, the leave's counted day, and approvers' fields
    for place in ["the form: a field the requester gives", "the form: a computation the screens work out", "the form: a field it does not place", "the card: every field"] {
        assert!(by_place.keys().any(|(p, _)| *p == place), "no field is drawn on {place}: {by_place:?}");
    }
    assert!(by_place.contains_key(&("the form: a field it does not place", "approvers'")) && by_place.contains_key(&("the form: a field it does not place", "computed")), "{by_place:?}");
    println!("{} refusals of a field drawn: {by_place:?}", want.len());
}

/// Below a full page of a list, a button reads the next (`/more`, the list's `more`): the same list
/// with the ID of the page's last ticket as `before`, and the person's own requests of the same kind
/// (`kind`). It is not drawn below the last page.
#[test]
fn a_full_page_reads_the_next() {
    if !node() {
        return;
    }
    let m = made();
    let mut cases = Vec::new();
    let mut want: Vec<(String, Option<Json>)> = Vec::new();
    let ticket = |id: &str| Json::obj([("id", Json::str(id)), ("status", Json::str("in_progress")), ("stage", Json::Null), ("requester", Json::str("e1201")), ("made_by", Json::Null), ("form", Json::Obj(Vec::new()))]);
    for (k, path, screen) in screens_of(&m).into_iter().filter(|(_, p, _)| p.contains("/list.") || p.contains("/mine.")) {
        let (op, kind) = if path.contains("/list.") { ("list_waiting", None) } else { ("list_mine", Some(k.named.name.clone())) };
        for more in [true, false] {
            let data = Json::obj([("tickets", Json::arr([ticket("T-7"), ticket("T-3")])), ("more", Json::Bool(more))]);
            cases.push(Json::obj([("screen", screen.clone()), ("data", data), ("button", Json::str("more"))]));
            let mut context = vec![("before", Json::str("T-3"))];
            if let Some(kind) = &kind {
                context.push(("kind", Json::str(kind)));
            }
            want.push((path.clone(), more.then(|| Json::obj([("name", Json::str(op)), ("context", Json::obj(context))]))));
        }
    }
    let out = drawn(&m, cases);
    for ((path, sent), got) in want.iter().zip(&out) {
        assert_eq!(got.get("sent").filter(|s| !s.is_null()), sent.as_ref(), "{path}");
    }
    println!("{} lists drawn with a full page and the last", want.len());
}

/// The checks of each button of `screen`, by the event it sends: the function each calls, with
/// its message.
fn button_checks(screen: &Json) -> Vec<(String, Vec<(String, String)>)> {
    parts(screen)
        .into_iter()
        .filter(|p| p.get("component").and_then(Json::as_str) == Some("Button"))
        .filter_map(|p| {
            let event = p.get("action")?.get("event")?.get("name")?.as_str()?.to_string();
            let checks = p.get("@checks").and_then(Json::as_arr).unwrap_or_default();
            let checks = checks.iter().map(|c| (c.get("condition").and_then(|c| c.get("@call")).and_then(Json::as_str).unwrap_or_default().to_string(), c.get("message").and_then(Json::as_str).unwrap_or_default().to_string())).collect();
            Some((event, checks))
        })
        .collect()
}

/// A kind that asks a reason with a rejection and a send-back (`comment required for`, DESIGN 7.2,
/// 13.1) has the two buttons of its card hold that a comment is given (`commentGiven`), said in the
/// card's language; no other button holds it but the comment's own, and no button of a kind that
/// asks none. What the function answers for the comments of the vectors is held in
/// [`the_functions_answer_the_vectors_as_the_reference_does`].
#[test]
fn a_reason_asked_is_held_by_the_buttons_of_the_card() {
    let m = made();
    let mut held = Vec::new();
    for (k, path, screen) in screens_of(&m) {
        let words = if path.ends_with(".ja.json") { "理由をコメントに書いてください。" } else { "Write the reason in the comment." };
        for (event, checks) in button_checks(&screen) {
            let given: Vec<&str> = checks.iter().filter(|(call, _)| call == "commentGiven").map(|(_, w)| w.as_str()).collect();
            if event == "comment" {
                assert_eq!(given.len(), 1, "{path}: {event}");
            } else if k.reasoned.contains(&event.as_str()) {
                assert_eq!(given, [words], "{path}: {event}");
                held.push(format!("{path}: {event}"));
            } else {
                assert!(given.is_empty(), "{path}: {event}: {given:?}");
            }
        }
    }
    let want: Vec<String> = ["leave_ja", "leave"].iter().flat_map(|k| ["en", "ja"].map(move |l| format!("a2ui/{k}/card.{l}.json"))).flat_map(|p| ["reject", "send_back"].map(|e| format!("{p}: {e}"))).collect();
    assert_eq!(held, want);
}

/// What a screen says when a computed value is outside its range (DESIGN 4.4, 13.7): the range,
/// then the fields to change, by their titles in the screen's language, as the server's pages say
/// the machine's refusal; the fields of one rows with the rows once. The purchase's subtotal and
/// total read its lines' quantity and unit price, the expense's total its items' price.
#[test]
fn a_computed_value_outside_its_range_names_the_fields_to_change() {
    let m = made();
    let said = |kind: &str, lang: &str, code: &str| -> Vec<String> {
        let screen = json::parse(file(&m, &format!("a2ui/{kind}/form.{lang}.json"))).unwrap();
        let mut found = Vec::new();
        every(&screen, "condition", &mut found);
        let mut out: Vec<String> = found
            .into_iter()
            .filter(|(_, c)| c.get("args").and_then(|a| a.get("code")).and_then(Json::as_str) == Some(code))
            .filter_map(|(rule, _)| Some(rule.get("message")?.as_str()?.to_string()))
            .collect();
        out.dedup();
        out
    };
    assert_eq!(said("purchase", "en", "lines.subtotal.worked"), ["The Subtotal (Lines) the form works out is outside its range (from 1 JPY to 100,000,000 JPY). Change Quantity or Unit price (Lines)."]);
    assert_eq!(said("purchase", "ja", "lines.subtotal.worked"), ["計算した明細の小計が、範囲（1円から100,000,000円まで）の外です。明細の数量か単価を直してください。"]);
    assert_eq!(said("purchase_ja", "ja", "amount.worked"), ["計算した合計が、範囲（1円から100,000,000円まで）の外です。明細の数量か単価を直してください。"]);
    assert_eq!(said("expense", "en", "amount.worked"), ["The Amount the form works out is outside its range (from 1 JPY to 10,000,000 JPY). Change Price (Items)."]);
    assert_eq!(said("expense_ja", "ja", "amount.worked"), ["計算した合計が、範囲（1円から10,000,000円まで）の外です。明細の金額を直してください。"]);
}
