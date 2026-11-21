//! `organization/schema.json` and `organization/columns.md`: the data of the organization the kinds
//! read and of their masters (DESIGN 4.6, 5.8, 8.2), for whoever keeps it (an administrator, a
//! server that takes it in) to write and check it before ritsu reads it.
//!
//! The schema is of a version's `organization.json`: its teams, people and members, each object with
//! the columns of the version's CSV files as keys, a team's kind and a member's position one of the
//! names the organization's file declares (as written, or in ASCII). Its `$defs` hold a row of
//! `proxies.csv` (`proxy`) and the part of the organization `start` and `next` are given (`part`:
//! the vectors' `organization`, DESIGN 7.5, 10.4). Kinds that read more than one organization have a
//! schema of each in `$defs`, the version one of them (`oneOf`). The columns are each CSV's: of a
//! version, of the proxies, and of each master's rows.

use super::{Fld, Kind, MasterOf, OrgOf, head};
use crate::ast::TypeExpr;
use ritsu_base::json::Json;

fn t(ty: &str) -> Json {
    Json::obj([("type", Json::str(ty))])
}

fn or_null(ty: &str) -> Json {
    Json::obj([("type", Json::arr([Json::str(ty), Json::str("null")]))])
}

fn with(j: Json, k: &str, v: Json) -> Json {
    let mut o = j.as_obj().map(|o| o.to_vec()).unwrap_or_default();
    o.push((k.to_string(), v));
    Json::Obj(o)
}

fn object(props: &[(&str, Json)], required: &[&str]) -> Json {
    Json::obj([
        ("type", Json::str("object")),
        ("required", Json::arr(required.iter().map(|x| Json::str(*x)))),
        ("properties", Json::Obj(props.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())),
    ])
}

fn array(items: Json) -> Json {
    Json::obj([("type", Json::str("array")), ("items", items)])
}

/// The language a person reads in, as a version's `organization.json` holds it (`data`: an empty
/// string or null for none, as an empty cell of `people.csv`) or as the API answers it (null for
/// none). A server sends a person their notices, and shows them its pages, in it; where it is none,
/// in the language of the kind.
pub(super) fn person_language(data: bool) -> Json {
    let mut vs: Vec<Json> = crate::screens::LANGS.iter().map(|l| Json::str(*l)).collect();
    if data {
        vs.push(Json::str(""));
    }
    vs.push(Json::Null);
    let langs = crate::screens::LANGS.map(|l| format!("`{l}`")).join(" or ");
    let none = if data { "empty or null" } else { "null" };
    Json::obj([
        ("type", Json::arr([Json::str("string"), Json::str("null")])),
        ("enum", Json::Arr(vs)),
        ("description", Json::str(format!("The language the person reads their notices and pages in, {langs}; {none} for the language of each kind"))),
    ])
}

/// The names declared, each as written and in ASCII.
fn spellings(ns: &[super::Named]) -> Json {
    let mut vs: Vec<String> = Vec::new();
    for n in ns {
        for s in [&n.written, &n.name] {
            if !vs.contains(s) {
                vs.push(s.clone());
            }
        }
    }
    Json::obj([("enum", Json::arr(vs.iter().map(Json::str)))])
}

/// The organizations the kinds read, each once, in the order of the kinds.
fn organizations(kinds: &[Kind]) -> Vec<&OrgOf> {
    let mut out: Vec<&OrgOf> = Vec::new();
    for o in kinds.iter().filter_map(|k| k.organization.as_ref()) {
        if !out.iter().any(|x| x.file == o.file) {
            out.push(o);
        }
    }
    out
}

/// The masters the kinds read, each once.
fn masters(kinds: &[Kind]) -> Vec<&MasterOf> {
    let mut out: Vec<&MasterOf> = Vec::new();
    for m in kinds.iter().flat_map(|k| &k.masters) {
        if !out.iter().any(|x| x.masters == m.masters && x.master == m.master) {
            out.push(m);
        }
    }
    out
}

/// A version's `organization.json`, its kinds and positions those `o` declares.
fn version(o: &OrgOf) -> Json {
    let id = || with(t("string"), "minLength", Json::int(1));
    let team = object(
        &[
            ("id", id()),
            ("name", t("string")),
            ("name_ja", or_null("string")),
            ("kind", spellings(&o.team_kinds)),
            ("parent", with(or_null("string"), "description", Json::str("The team above it; null or empty for the team at the top"))),
            ("head", with(or_null("string"), "description", Json::str("The ID of its head; null or empty while there is none"))),
        ],
        &["id", "name", "kind", "parent", "head"],
    );
    let person = object(&[("id", id()), ("name", t("string")), ("name_ja", or_null("string")), ("email", or_null("string")), (crate::org::LANGUAGE, person_language(true))], &["id", "name"]);
    let primary = Json::obj([("enum", Json::arr([Json::Bool(true), Json::Bool(false), Json::str("yes"), Json::str("no"), Json::str(""), Json::Null])), ("description", Json::str("Whether it is the person's primary team: true or `yes`; false, `no`, empty or null for a team they also belong to"))]);
    let member = object(&[("person", id()), ("team", id()), ("position", spellings(&o.positions)), ("primary", primary)], &["person", "team", "position", "primary"]);
    Json::obj([
        ("title", Json::str(format!("A version of the organization `{}` ({})", o.name, o.title))),
        ("type", Json::str("object")),
        ("required", Json::arr(["teams", "people", "members"].map(Json::str))),
        ("properties", Json::obj([("teams", array(team)), ("people", array(person)), ("members", array(member))])),
    ])
}

/// A row of `proxies.csv` (DESIGN 5.8), and the part of the organization `start` and `next` are
/// given (DESIGN 7.5).
fn defs() -> Vec<(String, Json)> {
    let day = || with(t("string"), "format", Json::str("date"));
    let proxy = object(
        &[
            ("person", t("string")),
            ("proxy", t("string")),
            ("kind", Json::obj([("enum", Json::arr([Json::str("approve"), Json::str("submit")]))])),
            ("from", day()),
            ("to", with(or_null("string"), "description", Json::str("The last day, which it includes; empty or null for no end"))),
            ("requests", with(or_null("string"), "description", Json::str("The kinds of request it is for, their ASCII names apart with spaces; empty or null for every kind"))),
        ],
        &["person", "proxy", "kind", "from", "to"],
    );
    let part_team = object(&[("id", t("string")), ("kind", t("string")), ("parent", or_null("string")), ("head", or_null("string"))], &["id", "kind", "parent", "head"]);
    let part_person = object(&[("id", t("string")), ("name", t("string")), ("name_ja", t("string"))], &["id", "name", "name_ja"]);
    let part_member = object(&[("person", t("string")), ("team", t("string")), ("position", t("string")), ("primary", t("boolean"))], &["person", "team", "position", "primary"]);
    let part_proxy = object(&[("person", t("string")), ("proxy", t("string")), ("kind", Json::obj([("enum", Json::arr([Json::str("approve"), Json::str("submit")]))]))], &["person", "proxy", "kind"]);
    let cell = object(&[("name", t("string")), ("value", t("string"))], &["name", "value"]);
    let row = object(&[("master", t("string")), ("key", t("string")), ("values", array(cell))], &["master", "key", "values"]);
    let part = with(
        object(
            &[
                ("day", day()),
                ("version", day()),
                ("positions", with(array(t("string")), "description", Json::str("The positions in ASCII, the lowest first"))),
                ("teams", array(part_team)),
                ("people", with(array(part_person), "description", Json::str("The people the ticket needs, each with their names as the data writes them (an empty string where it has none), which a refusal says after the ID"))),
                ("members", array(part_member)),
                ("proxies", array(part_proxy)),
                ("rows", array(row)),
            ],
            &["day", "version", "positions", "teams", "people", "members", "proxies", "rows"],
        ),
        "description",
        Json::str("The part of the organization `start` and `next` are given: of the version in effect on the day of the event, what the ticket needs, as the vectors' `organization` holds it"),
    );
    vec![("proxy".to_string(), proxy), ("part".to_string(), part)]
}

pub(super) fn schema(kinds: &[Kind]) -> Json {
    let orgs = organizations(kinds);
    let mut o: Vec<(String, Json)> = vec![("$schema".into(), Json::str("https://json-schema.org/draft/2020-12/schema")), ("$comment".into(), Json::str(head()))];
    let mut defs = defs();
    match orgs.as_slice() {
        [one] => {
            for (k, v) in version(one).as_obj().unwrap_or_default() {
                o.push((k.clone(), v.clone()));
            }
        }
        many => {
            o.push(("title".into(), Json::str("A version of one of the organizations")));
            o.push(("oneOf".into(), Json::arr(many.iter().map(|x| Json::obj([("$ref", Json::str(format!("#/$defs/{}", x.name)))])))));
            defs.extend(many.iter().map(|x| (x.name.clone(), version(x))));
        }
    }
    o.push(("$defs".into(), Json::Obj(defs)));
    Json::Obj(o)
}

/// What a cell of a field holds, in words.
fn cell(f: &Fld) -> String {
    let range = |lo: &Json, hi: &Json| match (lo.as_str().map(str::to_string).or(lo.as_int().map(|n| n.to_string())), hi.as_str().map(str::to_string).or(hi.as_int().map(|n| n.to_string()))) {
        (Some(a), Some(b)) => format!(", from {a} to {b}"),
        (Some(a), None) => format!(", from {a}"),
        (None, Some(b)) => format!(", up to {b}"),
        (None, None) => String::new(),
    };
    match &f.ty {
        TypeExpr::Text | TypeExpr::LongText => match f.count_max {
            Some(n) => format!("text of {n} characters at the most"),
            None => "text".to_string(),
        },
        TypeExpr::Num(_) => format!("{}, a whole number of its unit's step{}", f.type_ascii, range(&f.end(f.min), &f.end(f.max))),
        TypeExpr::Date => format!("a day, `YYYY-MM-DD`{}", range(&f.end(f.min), &f.end(f.max))),
        TypeExpr::Person => "a person's ID".to_string(),
        TypeExpr::Team => "a team's ID".to_string(),
        TypeExpr::Bool => "`yes` or `no`".to_string(),
        TypeExpr::Choice => format!("one of {}", f.choices.iter().map(|c| format!("`{}`", c.name)).collect::<Vec<_>>().join(", ")),
        other => other.word(),
    }
}

/// A table of Markdown: its head and its rows.
fn table(head: &[&str], rows: &[Vec<String>]) -> String {
    let mut s = format!("| {} |\n|{}\n", head.join(" | "), "---|".repeat(head.len()));
    for r in rows {
        s.push_str(&format!("| {} |\n", r.join(" | ")));
    }
    s
}

pub(super) fn columns(kinds: &[Kind]) -> String {
    let mut s = format!("<!-- {} -->\n\n# The organization's data\n\n", head());
    s.push_str("The files ritsu reads the organization and its masters from. A version of the organization is a directory named by the day it takes effect, with `teams.csv`, `people.csv` and `members.csv`, or one `organization.json` of the same columns (`schema.json`). `proxies.csv` is beside the versions' directories. A cell left empty is none.\n");
    let names = |ns: &[super::Named]| ns.iter().map(|n| if n.name == n.written { format!("`{}`", n.name) } else { format!("`{}` (`{}`)", n.written, n.name) }).collect::<Vec<_>>().join(", ");
    for o in organizations(kinds) {
        s.push_str(&format!("\n## The organization `{}` ({})\n\nIts file is `{}`", o.name, o.title, o.file));
        match &o.data {
            Some(d) => s.push_str(&format!(", and its data is in `{d}`.\n\n")),
            None => s.push_str(", and it names no place for its data.\n\n"),
        }
        s.push_str("### `teams.csv`\n\n");
        s.push_str(&table(
            &["Column", "Needed", "What it holds"],
            &[
                vec!["`id`".into(), "yes".into(), "The team's ID".into()],
                vec!["`name`".into(), "yes".into(), "Its name".into()],
                vec!["`name_ja`".into(), "no".into(), "Its name in Japanese".into()],
                vec!["`kind`".into(), "yes".into(), format!("One of {}", names(&o.team_kinds))],
                vec!["`parent`".into(), "yes".into(), "The ID of the team above it; empty for the team at the top".into()],
                vec!["`head`".into(), "yes".into(), "The ID of its head; empty while there is none".into()],
            ],
        ));
        s.push_str("\n### `people.csv`\n\n");
        s.push_str(&table(
            &["Column", "Needed", "What it holds"],
            &[
                vec!["`id`".into(), "yes".into(), "The person's ID".into()],
                vec!["`name`".into(), "yes".into(), "Their name".into()],
                vec!["`name_ja`".into(), "no".into(), "Their name in Japanese".into()],
                vec!["`email`".into(), "no".into(), "Their address".into()],
                vec![
                    format!("`{}`", crate::org::LANGUAGE),
                    "no".into(),
                    format!("The language they read their notices and pages in, {}; empty for the language of each kind", crate::screens::LANGS.map(|l| format!("`{l}`")).join(" or ")),
                ],
            ],
        ));
        s.push_str("\n### `members.csv`\n\n");
        s.push_str(&table(
            &["Column", "Needed", "What it holds"],
            &[
                vec!["`person`".into(), "yes".into(), "The person's ID".into()],
                vec!["`team`".into(), "yes".into(), "The team's ID".into()],
                vec!["`position`".into(), "yes".into(), format!("One of {}, the lowest first", names(&o.positions))],
                vec!["`primary`".into(), "yes".into(), "`yes` for the person's primary team; `no` or empty for a team they also belong to".into()],
            ],
        ));
        s.push_str("\n### `proxies.csv`\n\n");
        s.push_str(&table(
            &["Column", "Needed", "What it holds"],
            &[
                vec!["`person`".into(), "yes".into(), "The ID of the person a proxy stands in for".into()],
                vec!["`proxy`".into(), "yes".into(), "The ID of the proxy".into()],
                vec!["`kind`".into(), "yes".into(), "`approve` or `submit`".into()],
                vec!["`from`".into(), "yes".into(), "The first day, `YYYY-MM-DD`".into()],
                vec!["`to`".into(), "yes".into(), "The last day; empty for no end".into()],
                vec!["`requests`".into(), "no".into(), "The kinds of request it is for, their ASCII names apart with spaces; empty for every kind".into()],
            ],
        ));
    }
    for m in masters(kinds) {
        s.push_str(&format!("\n## The master `{}.{}` ({})\n\nIts file is `{}`", m.masters, m.master, m.named.label, m.file));
        match &m.data {
            Some(d) => s.push_str(&format!(", and its rows are in `{d}`")),
            None => s.push_str(", and it names no file for its rows"),
        }
        match &m.key {
            Some(k) => s.push_str(&format!("; its key is `{k}`.\n\n")),
            None => s.push_str(".\n\n"),
        }
        let mut rows: Vec<Vec<String>> = m.fields.iter().map(|f| vec![format!("`{}`", f.named.name), if f.optional { "no".into() } else { "yes".into() }, format!("{}: {}", f.named.label, cell(f))]).collect();
        rows.push(vec!["`valid_from`".into(), "yes".into(), "The first day the row may be chosen on; empty for any day before".into()]);
        rows.push(vec!["`valid_to`".into(), "yes".into(), "The last day the row may be chosen on; empty for no end".into()]);
        s.push_str(&table(&["Column", "Needed", "What it holds"], &rows));
    }
    s
}
