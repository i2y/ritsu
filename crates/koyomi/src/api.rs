//! `koyomi api` (DESIGN 8): what calling the generated code needs, as JSON, without reading
//! it. The shape follows rulec's `api`, with an entry per target, so that a tool that reads
//! rulec's (dandori's `src/rulec.rs`) can read this one the same way. The names and the
//! signatures come from `naming.rs`, which stage C's generators use too.

use crate::ast::{At, Ty};
use crate::calendar::{Calendar, Info};
use crate::naming::{self, TARGETS, Target};
use crate::resolve::Model;
use crate::sources::{Law, Table};
use serde_json::{Value, json};
use std::path::{Component, Path, PathBuf};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// `to` as seen from the directory `from` (both relative to where koyomi runs, cleaned).
pub fn relative(from: &Path, to: &Path) -> String {
    let a: Vec<Component> = from.components().collect();
    let b: Vec<Component> = to.components().collect();
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut out = PathBuf::new();
    for _ in common..a.len() {
        out.push("..");
    }
    for c in &b[common..] {
        out.push(c.as_os_str());
    }
    out.to_string_lossy().to_string()
}

fn info_json(i: &Info, base: &Path) -> Value {
    json!({
        "name": i.name,
        "alias": i.alias,
        "version": i.version,
        "file": relative(base, &i.path),
        "source_sha256": i.sha256,
    })
}

fn table_json(t: &Table, base: &Path) -> Value {
    let (format, encoding, division) = match &t.format {
        crate::ast::Format::Csv { shift_jis } => ("csv", Some(if *shift_jis { "shift_jis" } else { "utf8" }), None),
        crate::ast::Format::GovUk { division } => ("govuk", None, Some(division.clone())),
    };
    let mut v = json!({
        "name": t.name,
        "kind": "file",
        "file": relative(base, &crate::calendar::clean(&t.path)),
        "url": t.url,
        "sha256": t.pin,
        "format": format,
    });
    let o = v.as_object_mut().unwrap();
    if let Some(e) = encoding {
        o.insert("encoding".into(), json!(e));
    }
    if let Some(d) = division {
        o.insert("division".into(), json!(d));
    }
    o.insert(
        "covers".into(),
        if t.listed_years {
            json!({"listed_years": true, "from": t.covers.0.to_string(), "to": t.covers.1.to_string()})
        } else {
            json!({"from": t.covers.0.to_string(), "to": t.covers.1.to_string()})
        },
    );
    o.insert("rows".into(), json!(t.rows.len()));
    v
}

fn law_json(l: &Law) -> Value {
    json!({
        "name": l.name,
        "kind": "law",
        "db": "egov",
        "id": l.id,
        "asof": l.asof.to_string(),
        "revision": l.revision,
        "pins": l.pins.iter().map(|p| json!({"fragment": p.fragment, "sha256": p.pin})).collect::<Vec<_>>(),
    })
}

/// The calendar: its files, its closing rules (merged with the calendars it reads), its
/// sources and its data range. `base` is the directory paths are given from.
pub fn calendar_json(c: &Calendar, base: &Path) -> Value {
    let mut closed = Vec::new();
    let weekly: Vec<&str> = (0..7).filter(|d| c.weekly[*d]).map(|d| crate::kw::WEEKDAYS[d]).collect();
    if !weekly.is_empty() {
        closed.push(json!({"weekly": weekly}));
    }
    for t in &c.tables {
        closed.push(json!({"table": t.name}));
    }
    for e in &c.every {
        closed.push(json!({"every": e.text(), "name": e.name}));
    }
    for s in &c.days {
        closed.push(json!({"days": s.text(), "name": s.name}));
    }
    let open: Vec<Value> = c.opens.iter().map(|s| json!({"days": s.text(), "name": s.name})).collect();
    let mut sources: Vec<Value> = c.tables.iter().map(|t| table_json(t, base)).collect();
    sources.extend(c.laws.iter().map(law_json));
    let mut v = info_json(&c.info, base);
    let o = v.as_object_mut().unwrap();
    o.insert("offset".into(), json!(c.offset_text()));
    o.insert("uses".into(), Value::Array(c.used.iter().map(|i| info_json(i, base)).collect()));
    o.insert("closed".into(), Value::Array(closed));
    o.insert("open".into(), Value::Array(open));
    o.insert("sources".into(), Value::Array(sources));
    o.insert("data_range".into(), json!({"from": c.data.0.to_string(), "to": c.data.1.to_string()}));
    v
}

fn wire() -> Value {
    json!({
        "date": "YYYY-MM-DD",
        "int": "a JSON number",
        "at": "RFC 3339 in UTC, ending in Z, as dandori's timestamp",
        "errors": ["range", "data", "reject", "date"],
    })
}

/// A target's entry for a dates file.
fn target_json(t: Target, m: &Model) -> Value {
    let alias = m.file.name.ascii().unwrap_or("");
    let module = t.module(alias);
    let functions: Vec<Value> = m
        .dates
        .iter()
        .map(|d| {
            let params: Vec<(String, Ty)> = d.params.iter().map(|k| (m.inputs[*k].alias.clone(), m.inputs[*k].ty)).collect();
            let mut f = json!({
                "name": d.name,
                "function": t.function(&d.alias),
                "signature": t.signature(&module, &d.alias, &params, false),
            });
            let o = f.as_object_mut().unwrap();
            if d.at.is_some() {
                let at = naming::at_alias(&d.alias);
                o.insert("at_function".into(), json!(t.function(&at)));
                o.insert("at_signature".into(), json!(t.signature(&module, &at, &params, true)));
            }
            o.insert(
                "params".into(),
                Value::Array(d.params.iter().map(|k| json!({"name": m.inputs[*k].name, "alias": m.inputs[*k].alias, "type": t.ty(m.inputs[*k].ty)})).collect()),
            );
            f
        })
        .collect();
    let mut v = json!({
        "file": t.file(alias),
        "module": module,
        "date_type": t.date_type(),
        "functions": functions,
    });
    let o = v.as_object_mut().unwrap();
    if m.cal.is_some() {
        o.insert("is_open".into(), json!(t.is_open(&module)));
    }
    o.insert("errors".into(), json!([t.errors()]));
    v
}

/// `koyomi api` of a dates file.
pub fn dates_json(m: &Model) -> Value {
    let base = crate::calendar::clean(m.path.parent().unwrap_or(Path::new("")));
    let mut v = json!({
        "koyomi": VERSION,
        "kind": "dates",
        "name": m.file.name.text,
        "alias": m.file.name.ascii(),
        "version": m.file.version,
        "source_sha256": m.sha256,
        "description": m.file.description,
    });
    let o = v.as_object_mut().unwrap();
    o.insert("calendar".into(), m.cal.as_ref().map(|c| calendar_json(c, &base)).unwrap_or(Value::Null));
    o.insert("sources".into(), Value::Array(m.laws.iter().map(law_json).collect()));
    o.insert(
        "inputs".into(),
        Value::Array(
            m.inputs
                .iter()
                .map(|i| {
                    let (lo, hi) = match i.ty {
                        Ty::Date => (json!(i.show(i.lo)), json!(i.show(i.hi))),
                        Ty::Int => (json!(i.lo), json!(i.hi)),
                    };
                    json!({"name": i.name, "alias": i.alias, "type": if i.ty == Ty::Date { "date" } else { "int" }, "range": {"min": lo, "max": hi}})
                })
                .collect(),
        ),
    );
    let offset = m.cal.as_ref().and_then(|c| c.offset_text());
    o.insert(
        "dates".into(),
        Value::Array(
            m.dates
                .iter()
                .map(|d| {
                    let mut x = json!({"name": d.name, "alias": d.alias, "params": d.params.iter().map(|k| m.inputs[*k].name.clone()).collect::<Vec<_>>()});
                    if let Some((at, _)) = d.at {
                        let time = match at {
                            At::Time(t) => format!("{:02}:{:02}", t / 60, t % 60),
                            At::EndOfDay => "end of day".into(),
                        };
                        x.as_object_mut().unwrap().insert("at".into(), json!({"time": time, "offset": offset}));
                    }
                    x
                })
                .collect(),
        ),
    );
    o.insert("claims".into(), Value::Array(m.claims.iter().map(|c| json!({"name": c.name, "text": c.text})).collect()));
    o.insert("wire".into(), wire());
    for t in TARGETS {
        o.insert(t.key().into(), target_json(t, m));
    }
    v
}

/// `koyomi api` of a calendar file: the calendar, and `is_open` in each target.
pub fn calendar_file_json(c: &Calendar) -> Value {
    let base = crate::calendar::clean(c.info.path.parent().unwrap_or(Path::new("")));
    let alias = c.info.alias.clone().unwrap_or_default();
    let mut v = json!({
        "koyomi": VERSION,
        "kind": "calendar",
        "name": c.info.name,
        "alias": c.info.alias,
        "version": c.info.version,
        "source_sha256": c.info.sha256,
    });
    let o = v.as_object_mut().unwrap();
    o.insert("calendar".into(), calendar_json(c, &base));
    o.insert("wire".into(), wire());
    for t in TARGETS {
        let module = t.module(&alias);
        o.insert(
            t.key().into(),
            json!({"file": t.file(&alias), "module": module, "date_type": t.date_type(), "is_open": t.is_open(&module), "errors": [t.errors()]}),
        );
    }
    v
}
