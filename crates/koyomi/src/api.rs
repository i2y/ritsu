//! `koyomi api` (DESIGN 8): what calling the generated code needs, as JSON, without reading
//! it. The shape follows rulec's `api`, with an entry per target, so that a tool that reads
//! rulec's (dandori's `src/rulec.rs`) can read this one the same way. The names and the
//! signatures come from `naming.rs`, which stage C's generators use too. The Wasm entry has
//! the shape of rulec's: the files, the build line, the exports a host needs and the shapes of
//! the answer (DESIGN 13), with the keys of `out` in the place of the signatures.

use crate::ast::{At, Ty};
use crate::calendar::{Calendar, Info};
use crate::naming::{self, TARGETS, Target};
use crate::resolve::Model;
use crate::sources::{Law, Table};
use serde_json::{Value, json};
use std::path::{Component, Path, PathBuf};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// `to` as seen from the directory `from` (both relative to where koyomi runs, cleaned). In a module, a
/// file of a dependency (`@pricing/…`) is written from the module's root, as a file writes it there
/// (ritsu's DESIGN 17.6).
pub fn relative(from: &Path, to: &Path) -> String {
    let a: Vec<Component> = from.components().collect();
    let b: Vec<Component> = to.components().collect();
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    if common == 0 && b.first().is_some_and(|c| c.as_os_str().as_encoded_bytes().first() == Some(&b'@')) && ritsu_base::modules::module_of(from).is_some() {
        return to.to_string_lossy().to_string();
    }
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

/// What `call` answers (DESIGN 13): the record, the line `koyomi vectors` writes, and the refusal
/// in its place, with the values `refused` takes: `input` for an object that does not carry the
/// inputs, and the kinds of `wire.errors`.
fn wasm_answers() -> Value {
    json!([
        {"kind": "record", "keys": ["in", "out"]},
        {"kind": "refusal", "keys": ["refused", "error"], "refused": ["input", "range", "data", "reject", "date"]},
    ])
}

/// The Wasm entry: the files and the build, as rulec's `api` gives its Wasm entry, then the keys
/// of `out` for each date (`functions`), the question the calendar takes (`is_open`), and the
/// answers. The paths but `file` are in `wasm/`, where `build` and `component` run.
fn wasm_json(alias: &str, version: &str, functions: Option<Vec<Value>>, calendar: bool) -> Value {
    use crate::codegen::wasm;
    let t = Target::Wasm;
    let mut v = json!({
        "file": t.file(alias),
        "module": t.module(alias),
        "build": wasm::build_line(alias),
        "wit": format!("{alias}.wit"),
        "package": wasm::package(alias, version),
        "world": wasm::world(alias),
        "call": "call",
        "call_signature": naming::CALL_SIGNATURE,
        "post_return": "cabi_post_call",
        "realloc": "cabi_realloc",
        "memory": "memory",
        "runner": format!("{alias}_runner.mjs"),
        "component": wasm::component_line(alias),
        "date_type": t.date_type(),
    });
    let o = v.as_object_mut().unwrap();
    if let Some(f) = functions {
        o.insert("functions".into(), Value::Array(f));
    }
    if calendar {
        o.insert("is_open".into(), json!({"in": "date", "out": "open"}));
    }
    o.insert("answers".into(), wasm_answers());
    v
}

/// The Wasm entry of a dates file: each date by the key of `out` it comes under, and its time's.
fn wasm_dates_json(m: &Model) -> Value {
    let alias = m.file.name.ascii().unwrap_or("");
    let t = Target::Wasm;
    let functions = m
        .dates
        .iter()
        .map(|d| {
            let mut f = json!({"name": d.name, "key": d.name});
            let o = f.as_object_mut().unwrap();
            if d.at.is_some() {
                o.insert("at_key".into(), json!(format!("{}.at", d.name)));
            }
            o.insert(
                "params".into(),
                Value::Array(d.params.iter().map(|k| json!({"name": m.inputs[*k].name, "alias": m.inputs[*k].alias, "type": t.ty(m.inputs[*k].ty)})).collect()),
            );
            f
        })
        .collect();
    wasm_json(alias, &m.file.version, Some(functions), m.cal.is_some())
}

/// A target's entry for a dates file.
fn target_json(t: Target, m: &Model) -> Value {
    if t == Target::Wasm {
        return wasm_dates_json(m);
    }
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
        if t == Target::Wasm {
            o.insert(t.key().into(), wasm_json(&alias, &c.info.version, None, true));
            continue;
        }
        let module = t.module(&alias);
        o.insert(
            t.key().into(),
            json!({"file": t.file(&alias), "module": module, "date_type": t.date_type(), "is_open": t.is_open(&module), "errors": [t.errors()]}),
        );
    }
    v
}
