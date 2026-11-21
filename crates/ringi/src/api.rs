//! `ringi api`: what a `.ringi` declares, as JSON (DESIGN 16.1, 16.2). The things it holds are the
//! ones the port `Items` hands over ([`crate::items`]), and what it says of a request's ticket is
//! what the port `Requests` hands the checks across the languages ([`crate::ticket`]): the command
//! and the ports answer from the same places, so they say the same of a file.
//!
//! A name is its ASCII name (`name`), with the name as written beside it (`written`), as the JSON
//! of `run`, `check` and `simulate` have it.

use crate::ast::{Field, File, FileKind, Name, TypeExpr};
use crate::check::Outcome;
use crate::engine::Names;
use ritsu_base::json::Json;
use ritsu_base::text::Lang;
use ritsu_ports::{Carried, Found, Lined, RequestTicket};
use std::path::Path;

fn line(n: usize) -> Json {
    if n == 0 { Json::Null } else { Json::int(n as i128) }
}

/// Words that may name rows of the route (DESIGN 16.2): the words without the rows (`text`) and the
/// lines of the rows (`lines`), so that the words do not change when a line is added above them.
fn lined_json(l: &Lined, lang: Lang) -> Json {
    Json::obj([("text", Json::str(l.text.get(lang))), ("lines", Json::arr(l.lines.iter().map(|n| line(*n))))])
}

fn opt<T: Into<i128>>(v: Option<T>) -> Json {
    v.map(|v| Json::int(v.into())).unwrap_or(Json::Null)
}

fn both(n: &Name) -> [(&'static str, Json); 2] {
    [("name", Json::str(n.ascii())), ("written", Json::str(&n.text))]
}

/// A type with the names in it in ASCII: `choice from マスタ.systems` is `choice from m.systems`, a
/// rule's enum is named by the rule's alias in ASCII. A unit stays as written (`money[円]`): it is
/// the unit table's spelling, not a name of the file. A rate is said with its step ([`stepped`]).
pub(crate) fn type_in_ascii(written: &str, names: &Names) -> String {
    match written.strip_prefix("choice from ") {
        Some(from) => match names.master(from) {
            m if m != from => format!("choice from {m}"),
            _ => format!("choice from {}", names.spec(from)),
        },
        None => stepped(written),
    }
}

/// A type as a reader of the description counts its integer: a rate written with no step counts
/// whole percents, as `rate[step 1%]` (DESIGN 13.1), and is said so; any other type as written.
pub(crate) fn stepped(written: &str) -> String {
    if written == "rate" { "rate[step 1%]".to_string() } else { written.to_string() }
}

/// A field of the form, as the port `Requests` hands it over ([`crate::ports::field`]): its type,
/// the range of its value and the limits on its length or its rows in the keys and the form of a
/// field of `types/<name>.json` (DESIGN 12.2, 16.2; a number's range as the integers on the wire, a
/// date's as dates, a count in `count_min` and `count_max`), and the fields of its rows.
fn field_json(f: &Field, names: &Names) -> Json {
    let rf = crate::ports::field(f);
    let day = |d: Option<i128>| d.and_then(|d| i64::try_from(d).ok()).map(|d| Json::str(ritsu_ports::day_text(d))).unwrap_or(Json::Null);
    let (min, max, count_min, count_max) = match &f.ty {
        TypeExpr::Num(_) => (opt(rf.lo), opt(rf.hi), Json::Null, Json::Null),
        TypeExpr::Date => (day(rf.lo), day(rf.hi), Json::Null, Json::Null),
        TypeExpr::Text | TypeExpr::LongText | TypeExpr::Rows => (Json::Null, Json::Null, opt(rf.lo), opt(rf.hi)),
        _ => (Json::Null, Json::Null, Json::Null, Json::Null),
    };
    Json::obj([
        ("name", Json::str(&rf.code_name)),
        ("written", Json::str(&rf.name)),
        ("label", Json::str(&rf.label)),
        ("type", Json::str(type_in_ascii(&rf.ty, names))),
        ("type_written", Json::str(&rf.ty)),
        ("min", min),
        ("max", max),
        ("count_min", count_min),
        ("count_max", count_max),
        ("values", Json::arr(rf.values.iter().map(Json::str))),
        ("optional", Json::from(rf.optional)),
        ("computed", Json::from(rf.computed)),
        ("approvers_only", Json::from(rf.approvers_only)),
        ("line", line(rf.line)),
        ("fields", Json::arr(f.children.iter().map(|x| field_json(x, names)))),
    ])
}

/// What a value carries: its type, whether it may be none, and its ends or its values.
fn carried_json(c: &Carried) -> Json {
    let (c, optional) = c.present();
    let day = |d: Option<i64>| d.map(|d| Json::str(ritsu_ports::day_text(d))).unwrap_or(Json::Null);
    let (ty, min, max, values) = match c {
        Carried::Bool => ("bool".to_string(), Json::Null, Json::Null, Json::Null),
        Carried::Str => ("string".to_string(), Json::Null, Json::Null, Json::Null),
        Carried::Date { min, max } => ("date".to_string(), day(*min), day(*max), Json::Null),
        Carried::Days(_) => ("date".to_string(), Json::Null, Json::Null, Json::Null),
        Carried::Enum { name, values } => (name.clone(), Json::Null, Json::Null, Json::arr(values.iter().map(|v| Json::str(&v.public)))),
        Carried::Num { written, min, max, .. } => (written.clone(), opt(*min), opt(*max), Json::Null),
        Carried::Opt(_) | Carried::Other(_) => (
            match c {
                Carried::Other(w) => w.clone(),
                _ => String::new(),
            },
            Json::Null,
            Json::Null,
            Json::Null,
        ),
    };
    Json::obj([("type", Json::str(ty)), ("optional", Json::from(optional)), ("min", min), ("max", max), ("values", values)])
}

/// What ringi knows of a request's ticket (the port's [`RequestTicket`]), the files written from
/// `root`.
fn ticket_json(t: &RequestTicket, names: &Names, root: &Path, lang: Lang) -> Json {
    let rel = |p: &Path| ritsu_base::paths::from_root(root, p).unwrap_or_else(|| p.display().to_string());
    let found = |f: &Found<Json>| match f {
        Found::Value(v) => v.clone(),
        Found::Undecided(why) => Json::obj([("undecided", Json::str(why.get(lang)))]),
    };
    let time = &t.time;
    Json::obj([
        ("self_approval", Json::from(t.self_approval)),
        ("self_approval_line", line(t.self_approval_line)),
        ("gates", Json::arr(t.gates.iter().map(|g| Json::obj([("file", Json::str(rel(&g.file))), ("line", line(g.line))])))),
        (
            "operations",
            Json::arr(t.operations.iter().map(|o| Json::obj([("name", Json::str(&o.name)), ("line", line(o.line)), ("by_approver", Json::from(o.by_approver)), ("decides", Json::from(o.decides))]))),
        ),
        ("endings", Json::arr(t.endings.iter().map(Json::str))),
        (
            "time",
            Json::obj([
                ("round", opt(time.round)),
                ("send_backs", Json::int(i128::from(time.backs))),
                ("resubmit", opt(time.resubmit)),
                ("pause", opt(time.pause)),
                ("total", opt(time.total)),
                ("held", opt(time.held)),
                ("no_end", Json::arr(time.no_end.iter().map(|n| Json::obj([("text", Json::str(n.text.get(lang))), ("fix", Json::str(n.fix.get(lang))), ("in_round", Json::from(n.in_round))])))),
                ("calendars", Json::arr(time.calendars.iter().map(|c| Json::obj([("alias", Json::str(&c.code_name)), ("written", Json::str(&c.alias)), ("file", Json::str(rel(&c.file)))])))),
            ]),
        ),
        (
            "holds",
            Json::arr(t.holds.iter().map(|h| {
                Json::obj([
                    ("alias", Json::str(names.used(&h.alias))),
                    ("written", Json::str(&h.alias)),
                    ("book", Json::str(rel(&h.book))),
                    ("transfer", Json::str(&h.transfer)),
                    ("line", line(h.line)),
                    (
                        "requester_arguments",
                        Json::arr(h.args.iter().map(|a| {
                            let without = match &a.without {
                                Found::Value(vs) => Found::Value(Json::arr(vs.iter().map(|v| {
                                    Json::obj([("from", Json::str(ritsu_ports::day_text(v.from))), ("requesters", Json::int(v.requesters as i128)), ("without", Json::arr(v.without.iter().map(Json::str)))])
                                }))),
                                Found::Undecided(why) => Found::Undecided(why.clone()),
                            };
                            Json::obj([("name", Json::str(&a.name)), ("written", Json::str(&a.written)), ("line", line(a.line)), ("without", found(&without))])
                        })),
                    ),
                ])
            })),
        ),
        (
            "flows",
            Json::arr(t.flows.iter().map(|f| {
                // where it is asked: the ways of an agent, the stage of a summary, the endings of a
                // `launch`
                let at = |x: &str| match f.kind.as_str() {
                    "agent" => names.way(x).to_string(),
                    "summary" => names.stage(x).to_string(),
                    _ => x.to_string(),
                };
                Json::obj([
                    ("kind", Json::str(&f.kind)),
                    ("alias", Json::str(names.used(&f.alias))),
                    ("written", Json::str(&f.alias)),
                    ("flow", Json::str(rel(&f.flow))),
                    ("line", line(f.line)),
                    ("at", Json::arr(f.at.iter().map(|x| Json::str(at(x))))),
                    (
                        "given",
                        Json::arr(f.given.iter().map(|g| {
                            let carried = match &g.carried {
                                Found::Value(c) => Found::Value(carried_json(c)),
                                Found::Undecided(why) => Found::Undecided(why.clone()),
                            };
                            // null for a value that is always there, the words of how it can be
                            // missing otherwise
                            let none = match &g.none {
                                Found::Value(n) => Found::Value(n.as_ref().map(|l| lined_json(l, lang)).unwrap_or(Json::Null)),
                                Found::Undecided(why) => Found::Undecided(why.clone()),
                            };
                            Json::obj([
                                ("name", Json::str(&g.name)),
                                ("written", Json::str(&g.written)),
                                ("carries", found(&carried)),
                                ("none", found(&none)),
                                ("where", g.narrowed.as_ref().map(|l| lined_json(l, lang)).unwrap_or(Json::Null)),
                                ("may_be_less", g.may_be_less.as_ref().map(|t| Json::str(t.get(lang))).unwrap_or(Json::Null)),
                                ("line", line(g.line)),
                            ])
                        })),
                    ),
                ])
            })),
        ),
    ])
}

/// The JSON of a file that passes its check: `ringi` (the version), `file`, `kind`, `name`,
/// `written`, `version`, `title`, `description`, `owner`, `effective_from`, `uses`, `form`, `items`,
/// `usable`, `utc_offset_minutes` (a request's, DESIGN 7.5; as the description of the kind says
/// it), `ticket`, in that order.
pub fn to_json(o: &Outcome, f: &File, src: &str, names: &Names, root: &Path, lang: Lang) -> Json {
    let file = ritsu_base::paths::from_root(root, Path::new(&o.path)).unwrap_or_else(|| o.path.clone());
    let [name, written] = both(&f.name);
    let uses = f.uses.iter().map(|u| {
        Json::obj([
            ("kind", Json::str(u.kind.word())),
            ("alias", u.alias.as_ref().map(|a| Json::str(a.ascii())).unwrap_or(Json::Null)),
            ("written", u.alias.as_ref().map(|a| Json::str(&a.text)).unwrap_or(Json::Null)),
            ("file", Json::str(&u.path)),
            ("line", line(u.line)),
        ])
    });
    let form = if f.kind == FileKind::Request { f.form.iter().map(|x| field_json(x, names)).collect() } else { Vec::new() };
    let items = crate::items::things(f, src).into_iter().map(|t| {
        Json::obj([
            ("kind", Json::str(t.kind)),
            ("name", Json::str(&t.ascii)),
            ("written", Json::str(&t.name)),
            ("lines", Json::arr([Json::int(t.lines.0 as i128), Json::int(t.lines.1 as i128)])),
            ("text", Json::str(&t.text)),
        ])
    });
    Json::obj([
        ("ringi", Json::str(env!("CARGO_PKG_VERSION"))),
        ("file", Json::str(file)),
        ("kind", Json::str(f.kind.word())),
        name,
        written,
        ("version", Json::int(i128::from(f.version))),
        ("title", Json::opt_str(f.title.clone())),
        ("description", Json::opt_str(f.description.as_ref().map(|d| d.0.clone()))),
        ("owner", Json::opt_str(f.owner.as_ref().map(|w| w.text.clone()))),
        ("effective_from", Json::opt_str(f.effective_from.map(|d| crate::lex::date_text(d.0)))),
        ("uses", Json::arr(uses)),
        ("form", Json::arr(form)),
        ("items", Json::arr(items)),
        ("usable", o.usable.as_ref().map(|u| u.to_json(names, root, lang)).unwrap_or(Json::Null)),
        ("utc_offset_minutes", o.offset.map(|m| Json::int(i128::from(m))).unwrap_or(Json::Null)),
        ("ticket", o.ticket.as_ref().map(|t| ticket_json(t, names, root, lang)).unwrap_or(Json::Null)),
    ])
}
