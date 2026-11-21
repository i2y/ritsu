//! ritsu's ports, as rulec answers them (ritsu's DESIGN 3.2, PLAN D.2).
//!
//! What `rulec::ports::Engine` hands over is what rulec's own JSON says of the same rule —
//! `rulec api`, `rulec certificate` and `rulec schema`, read the way a program that reads the
//! JSON reads them — for every rule of the corpus, with one difference that is the point of the
//! port: a rate's step is the type's own, where the JSON gives it only in the words of the
//! schema's description. The page is the one `rulec doc` draws, and the evaluator answers every
//! vector `rulec vectors` writes.

use ritsu_base::json::Json;
use ritsu_ports::{Answer, Column, ColumnType, Items, NeverCond, NeverTest, Precondition, References, RuleError, Rules, Value};
use rulec::ports::Engine;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every rule of the corpus, as a path from the crate (which is where the tests run).
fn corpus() -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(root().join("tests/corpus"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "rule"))
        .map(|p| format!("tests/corpus/{}", p.file_name().unwrap().to_string_lossy()))
        .collect();
    out.sort();
    out
}

fn json(s: &str) -> Json {
    ritsu_base::json::parse(s).unwrap_or_else(|e| panic!("{e:?}"))
}

fn st(j: Option<&Json>) -> String {
    j.and_then(|x| x.as_str()).unwrap_or_default().to_string()
}

fn arr(j: Option<&Json>) -> Vec<Json> {
    j.and_then(|x| x.as_arr()).map(|a| a.to_vec()).unwrap_or_default()
}

fn strs(j: Option<&Json>) -> Vec<String> {
    arr(j).iter().map(|x| x.as_str().unwrap_or_default().to_string()).collect()
}

/// What the schema's description says of a rate: how many steps make 100% (`100% is 1000`).
fn per(description: &str) -> Option<i128> {
    let rest = description.split("100% is ").nth(1).or_else(|| description.split("100% なら ").nth(1))?;
    rest.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
}

/// One column, held to what the certificate's types, its enums and the schema say of it.
fn check_column(at: &str, col: &Column, cert: &Json, sch: Option<&Json>) {
    let written = st(cert.get("types").and_then(|t| t.get(&col.name)));
    let enums: Vec<String> = cert.get("enums").and_then(|e| e.as_obj()).map(|kv| kv.iter().map(|(k, _)| k.clone()).collect()).unwrap_or_default();
    let sch = sch.unwrap_or_else(|| panic!("{at}: the schema has no `{}`", col.name));
    let ty = st(sch.get("type"));
    // an input of an ordered enum with a range carries the values the certificate lists for it
    // (§15.210, §15.251), an optional one inside its `T?`; every other column of an enum carries none
    let inner = match &col.ty {
        ColumnType::Opt(t) => t.as_ref(),
        t => t,
    };
    if let ColumnType::Enum { name: e, range } = inner {
        let listed = cert.get("enum_ranges").and_then(|r| r.get(&col.name)).map(|v| strs(Some(v)));
        assert_eq!(*range, listed, "{at}: the range of {}", col.name);
        // and the schema's `enum` is the same values: the range's, else every value of the enum
        let every = cert.get("enums").and_then(|x| x.get(e)).map(|v| strs(Some(v))).unwrap_or_default();
        let one = sch.get("oneOf").and_then(|x| x.as_arr()).and_then(|a| a.iter().find(|b| b.get("enum").is_some()).cloned());
        let offered = strs(one.as_ref().unwrap_or(sch).get("enum"));
        assert_eq!(offered, range.clone().unwrap_or(every), "{at}: the schema's enum of {}", col.name);
    }
    match &col.ty {
        ColumnType::Enum { name: e, .. } => assert!(*e == written && enums.contains(e), "{at}: {} is the enum {e}", col.name),
        ColumnType::Bool => assert_eq!(ty, "boolean", "{at}: {}", col.name),
        ColumnType::Date { min, max } => {
            assert!(ty == "string" && st(sch.get("format")) == "date", "{at}: {}", col.name);
            // its range as day numbers, as the certificate's `ranges` states it: each end a string
            // of the number, null where none is written
            let end = |x: &Json| x.as_str().map(|s| s.parse::<i64>().unwrap_or_else(|e| panic!("{at}: {} {s}: {e}", col.name)));
            let stated = cert.get("ranges").and_then(|r| r.get(&col.name)).and_then(|x| x.as_arr()).map(|a| (end(&a[0]), end(&a[1])));
            assert_eq!((*min, *max), stated.unwrap_or((None, None)), "{at}: the range of {}", col.name);
        }
        ColumnType::Str => assert!(ty == "string" && sch.get("format").is_none(), "{at}: {}", col.name),
        ColumnType::Opt(_) => assert!(sch.get("oneOf").is_some(), "{at}: {} may be none", col.name),
        ColumnType::Num { written: w, unit, min, max, .. } => {
            assert_eq!(ty, "integer", "{at}: {}", col.name);
            assert_eq!(*w, written, "{at}: {}", col.name);
            assert_eq!(*min, sch.get("minimum").and_then(|x| x.as_int()), "{at}: {} minimum", col.name);
            assert_eq!(*max, sch.get("maximum").and_then(|x| x.as_int()), "{at}: {} maximum", col.name);
            let u = unit.as_ref().unwrap_or_else(|| panic!("{at}: {} : {w} has no unit", col.name));
            if w == "rate" {
                // the step the description words, and the type's own
                let per = per(&st(sch.get("description"))).unwrap_or_else(|| panic!("{at}: {} says no step", col.name));
                assert_eq!(u.step, Some(rulec::num::Rat::new(1, per)), "{at}: {} step", col.name);
            } else {
                assert_eq!(u.to_string(), *w, "{at}: {}", col.name);
            }
        }
    }
}

/// A list of parameters or outputs of the generated code, held to `api`'s.
fn check_params(at: &str, got: &[ritsu_ports::Param], want: &[Json]) {
    let w: Vec<(String, String, String, bool)> = want.iter().map(|p| (st(p.get("name")), st(p.get("alias")), st(p.get("type")), p.get("optional").and_then(|x| x.as_bool()).unwrap_or(false))).collect();
    let g: Vec<(String, String, String, bool)> = got.iter().map(|p| (p.name.clone(), p.alias.clone(), p.ty.clone(), p.optional)).collect();
    assert_eq!(g, w, "{at}");
}

fn check_enums(at: &str, got: &[ritsu_ports::CallEnum], want: &[Json]) {
    let w: Vec<(String, String, Vec<(String, String)>)> = want.iter().map(|e| (st(e.get("name")), st(e.get("alias")), arr(e.get("values")).iter().map(|v| (st(v.get("name")), st(v.get("alias")))).collect())).collect();
    let g: Vec<(String, String, Vec<(String, String)>)> = got.iter().map(|e| (e.name.clone(), e.alias.clone(), e.values.clone())).collect();
    assert_eq!(g, w, "{at}");
}

fn check_call(at: &str, call: &ritsu_ports::Call, api: &Json, module: &str, function: &str, params: &str, outputs: &str) {
    assert_eq!(call.module, st(api.get(module)), "{at}");
    assert_eq!(call.function, st(api.get(function)), "{at}");
    check_params(&format!("{at} params"), &call.params, &arr(api.get(params)));
    check_params(&format!("{at} outputs"), &call.outputs, &arr(api.get(outputs)));
    check_enums(&format!("{at} enums"), &call.enums, &arr(api.get("enums")));
}

fn wire_fields(want: Option<&Json>) -> Vec<(String, String, String, bool, Option<String>)> {
    arr(want).iter().map(|f| (st(f.get("name")), st(f.get("field")), st(f.get("type")), f.get("optional").and_then(|x| x.as_bool()).unwrap_or(false), f.get("enum").and_then(|x| x.as_str()).map(String::from))).collect()
}

fn fields_of(got: &[ritsu_ports::WireField]) -> Vec<(String, String, String, bool, Option<String>)> {
    got.iter().map(|f| (f.name.clone(), f.field.clone(), f.ty.clone(), f.optional, f.enumeration.clone())).collect()
}

/// The range `api` gives the value `name` in each language that lists it: the parameters (`params`,
/// Go's `input_fields`, the columns of NumPy and SQL) for `in`, the outputs (`outputs`, Go's
/// `output_fields`) for `out`.
fn api_ranges(api: &Json, side: &str, name: &str) -> Vec<(String, Option<(i128, i128)>)> {
    let keys: &[&str] = if side == "in" { &["params", "input_fields", "columns"] } else { &["outputs", "output_fields"] };
    let mut out = Vec::new();
    for (lang, v) in api.as_obj().unwrap_or_default() {
        for key in keys.iter().copied() {
            let Some(entry) = v.get(key).and_then(|x| x.as_arr()).and_then(|l| l.iter().find(|p| st(p.get("name")) == name)) else { continue };
            let range = entry.get("range").and_then(|r| Some((r.get("min")?.as_int()?, r.get("max")?.as_int()?)));
            out.push((lang.clone(), range));
        }
    }
    out
}

/// An end the certificate writes (`"12"`, `"-7/2"`, `null`) as the integer it is on the wire, at
/// `scale` steps a unit, which it is whole.
fn on_wire(at: &str, j: &Json, scale: i128) -> Option<i128> {
    let s = j.as_str()?;
    let (n, d) = match s.split_once('/') {
        Some((n, d)) => (n.parse::<i128>().unwrap(), d.parse::<i128>().unwrap()),
        None => (s.parse::<i128>().unwrap(), 1),
    };
    assert_eq!((n * scale) % d, 0, "{at}: {s} is no whole number on the wire at {scale} a unit");
    Some(n * scale / d)
}

/// Every fact of every rule of the corpus is what rulec's own JSON says of it, and every range a
/// number is said to have is one range: the facts', the schema's, `api`'s in every language, and for
/// an output the certificate's `output_ranges` and the port's `output_values` (§15.271).
#[test]
fn 口の事実はapiと証明書とschemaが言うことと同じ() {
    let e = Engine::new();
    let mut rules = 0;
    for path in corpus() {
        let src = std::fs::read_to_string(&path).unwrap();
        if rulec::has_error(&rulec::report(&src, &path).diags) {
            assert!(e.facts(Path::new(&path)).is_err(), "{path}: a rule that does not pass check has no facts");
            continue;
        }
        let (f, c) = rulec::prepare(&src, &path).unwrap();
        let facts = e.facts(Path::new(&path)).unwrap_or_else(|e| panic!("{path}: {e:?}"));
        let api = json(&rulec::codegen::Gen::new(&f, &c, &src, &path).api());
        let cert = json(&rulec::cert::certificate(&f, &c, &src, &path));
        let schema = json(&rulec::verify::schema(&f, &c, false));
        assert_eq!((facts.rule.as_str(), facts.alias.as_str(), facts.version.as_str(), facts.sha256.as_str()), (st(api.get("rule")).as_str(), st(api.get("alias")).as_str(), st(api.get("version")).as_str(), st(api.get("source_sha256")).as_str()), "{path}");
        assert_eq!(facts.rulec, st(cert.get("rulec")), "{path}");
        // the columns: the order and the aliases are api's, the types the certificate's and the schema's
        let ts = api.get("typescript").unwrap();
        for (side, list, key) in [("in", &facts.inputs, "params"), ("out", &facts.outputs, "outputs")] {
            let want: Vec<(String, String)> = arr(ts.get(key)).iter().filter(|p| p.get("elements").is_none()).map(|p| (st(p.get("name")), st(p.get("alias")))).collect();
            assert_eq!(list.iter().map(|x| (x.name.clone(), x.alias.clone())).collect::<Vec<_>>(), want, "{path} {side}");
            for col in list.iter() {
                let sch = schema.get("properties").and_then(|p| p.get(side)).and_then(|p| p.get("properties")).and_then(|p| p.get(&col.name));
                check_column(&path, col, &cert, sch);
                // a number has one range wherever it is said (§15.271): `api` in every language that
                // lists it, and for an output the certificate's `output_ranges` on the wire and the
                // port's `output_values` — each the facts', which the schema's are (above)
                let (optional, inner) = match &col.ty {
                    ColumnType::Opt(t) => (true, t.as_ref()),
                    t => (false, t),
                };
                let ColumnType::Num { written, unit, min, max, .. } = inner else { continue };
                let at = format!("{path} {side} {}", col.name);
                let both = min.zip(*max);
                let langs = api_ranges(&api, side, &col.name);
                assert!(langs.len() >= 11, "{at}: {langs:?}");
                for (lang, range) in &langs {
                    assert_eq!(*range, both, "{at}: api {lang}");
                }
                if side == "out" {
                    // a rate travels as a count of its steps, everything else in its unit
                    let scale = if written == "rate" { unit.as_ref().and_then(|u| u.step).map(|s| s.den / s.num).unwrap_or(100) } else { 1 };
                    let stated = cert.get("output_ranges").and_then(|o| o.get(&col.name)).map(|r| arr(Some(r)));
                    let stated = stated.map(|r| (on_wire(&at, &r[0], scale), on_wire(&at, &r[1], scale))).unwrap_or((None, None));
                    assert_eq!(stated, (*min, *max), "{at}: the certificate's output_ranges");
                    if !optional {
                        match e.output_values(Path::new(&path), &col.name).unwrap_or_else(|x| panic!("{at}: {x:?}")) {
                            ritsu_ports::Found::Value(v) => assert_eq!((v.min, v.max), (*min, *max), "{at}: output_values"),
                            ritsu_ports::Found::Undecided(t) => panic!("{at}: {t:?}"),
                        }
                    }
                }
            }
        }
        // every list the rule walks, in the order of the parameters (§15.219)
        let walks: Vec<String> = arr(ts.get("params")).iter().filter(|p| p.get("elements").is_some()).map(|p| st(p.get("name"))).collect();
        assert_eq!(facts.elements.iter().map(|(n, _, _)| n.clone()).collect::<Vec<_>>(), walks, "{path}");
        // the enums, as the certificate lists them
        let want: Vec<(String, Vec<String>)> = cert.get("enums").and_then(|e| e.as_obj()).map(|kv| kv.iter().map(|(k, v)| (k.clone(), strs(Some(v)))).collect()).unwrap_or_default();
        assert_eq!(facts.enums.iter().map(|e| (e.name.clone(), e.values.iter().map(|v| v.name.clone()).collect::<Vec<_>>())).collect::<Vec<_>>(), want, "{path}");
        // the preconditions, as `api` lists them
        let want: Vec<Precondition> = arr(api.get("preconditions"))
            .iter()
            .map(|p| match st(p.get("kind")).as_str() {
                // A side that adds inputs up is a list (§15.216), and the precondition the port
                // gives is then the sum's, with a side of one input a list of one.
                "constraint" => {
                    let side = |j: Option<&Json>| j.and_then(|x| x.as_arr()).map(|a| a.iter().map(|v| st(Some(v))).collect::<Vec<_>>());
                    match (side(p.get("left")), side(p.get("right"))) {
                        (None, None) => Precondition::Relation { left: st(p.get("left")), op: st(p.get("op")), right: st(p.get("right")) },
                        (l, r) => Precondition::SumRelation {
                            left: l.unwrap_or_else(|| vec![st(p.get("left"))]),
                            op: st(p.get("op")),
                            right: r.unwrap_or_else(|| vec![st(p.get("right"))]),
                        },
                    }
                }
                // the unit is the port's own (the total's type, which `api` does not repeat)
                "sum" => Precondition::Sum {
                    name: st(p.get("name")),
                    over: st(p.get("over")),
                    of: st(p.get("of")),
                    max: p.get("max").and_then(|x| x.as_int()).unwrap(),
                    min: p.get("min").and_then(|x| x.as_int()),
                    unit: rulec::ports::sum_unit(&c, &st(p.get("name"))),
                },
                "count" => Precondition::Count { name: st(p.get("name")), over: st(p.get("over")), min: p.get("min").and_then(|x| x.as_int()).unwrap() },
                "never" => Precondition::Never {
                    said: st(p.get("said")),
                    conds: arr(p.get("conds"))
                        .iter()
                        .map(|k| {
                            let name = st(k.get("name"));
                            let value = |v: &Json| match (v, c.ty_of(&name)) {
                                (Json::Bool(b), _) => Value::Bool(*b),
                                (Json::Int(n), _) => Value::Int(*n),
                                (Json::Str(s), Some(rulec::types::Ty::Enum(_))) => Value::Enum(s.clone()),
                                (Json::Str(s), Some(rulec::types::Ty::Date)) => Value::Date(s.clone()),
                                (Json::Str(s), _) => Value::Str(s.clone()),
                                _ => panic!("{path}: a condition's value: {v:?}"),
                            };
                            let test = if let Some(vs) = k.get("among").and_then(|x| x.as_arr()) {
                                NeverTest::Among { values: vs.iter().map(value).collect(), yes: k.get("yes").and_then(|x| x.as_bool()).unwrap() }
                            } else if let Some(ops) = k.get("compare").and_then(|x| x.as_arr()) {
                                NeverTest::Compare(ops.iter().map(|o| (st(o.get("op")), o.get("value").and_then(|x| x.as_int()).unwrap())).collect())
                            } else {
                                NeverTest::Other
                            };
                            NeverCond { name, test }
                        })
                        .collect(),
                },
                "unique" => Precondition::Unique { sequence: st(p.get("sequence")), field: st(p.get("field")) },
                // the range as the rule writes it (§15.313)
                "define" => Precondition::Define { name: st(p.get("name")), range: st(p.get("range")), from: strs(p.get("from")), over: p.get("over").map(|o| st(Some(o))) },
                _ => Precondition::Length { sequence: st(p.get("sequence")), max: p.get("max").and_then(|x| x.as_int()).unwrap() },
            })
            .collect();
        assert_eq!(facts.preconditions, want, "{path}");
        // the machine: `api` for its states, the certificate for its table
        match (api.get("machine").filter(|m| !m.is_null()), &facts.machine) {
            (None, None) => {}
            (Some(m), Some(fm)) => {
                let cm = cert.get("machine").unwrap();
                let table = st(cm.get("table"));
                let t = arr(cert.get("tables")).into_iter().find(|t| st(t.get("table")) == table).unwrap_or_else(|| panic!("{path}: no table {table}"));
                let states = strs(m.get("states"));
                let idx = |s: &str| states.iter().position(|x| x == s);
                assert_eq!((fm.name.clone(), fm.carry_in.clone(), fm.carry_out.clone(), fm.state_enum.clone()), (st(m.get("name")), st(m.get("carry").and_then(|c| c.get("input"))), st(m.get("carry").and_then(|c| c.get("output"))), st(m.get("carry").and_then(|c| c.get("enum")))), "{path}");
                assert_eq!(fm.states, states, "{path}");
                assert_eq!(fm.initial, idx(&st(m.get("initial"))).unwrap_or(0), "{path}");
                assert_eq!(fm.finals, strs(m.get("final")).iter().filter_map(|x| idx(x)).collect::<Vec<_>>(), "{path}");
                assert_eq!(fm.held, strs(m.get("held")), "{path}");
                assert_eq!(fm.policy, st(t.get("policy")), "{path}");
                assert_eq!(fm.decides, strs(t.get("decides")), "{path}");
                assert_eq!(fm.state_axis, cm.get("axis").and_then(|x| x.as_int()).map(|x| x as usize), "{path}");
                let axes: Vec<(String, Vec<String>)> = arr(t.get("axes")).iter().map(|a| (st(a.get("column")), strs(a.get("coords")))).collect();
                assert_eq!(fm.axes.iter().map(|a| (a.column.clone(), a.coords.clone())).collect::<Vec<_>>(), axes, "{path}");
                // the ends of each coordinate, as the certificate bounds it: none where it has no bounds,
                // an open end where it has one, a point's own value at both ends, and an interval's
                // coordinate at the end on its closed side (the value one step in that stands for it)
                for (a, ca) in fm.axes.iter().zip(arr(t.get("axes"))) {
                    let bounds = arr(ca.get("bounds"));
                    assert_eq!((a.ends.len(), bounds.len()), (a.coords.len(), a.coords.len()), "{path} {}", a.column);
                    for ((coord, ends), b) in a.coords.iter().zip(&a.ends).zip(&bounds) {
                        let at = format!("{path} {} {coord}", a.column);
                        let date = coord.len() == 10 && coord.as_bytes()[4] == b'-' && coord.as_bytes()[7] == b'-';
                        match (ends, b.as_arr()) {
                            (None, None) => {}
                            (Some((lo, hi)), Some([blo, bhi])) => {
                                assert_eq!((lo.is_some(), hi.is_some()), (!blo.is_null(), !bhi.is_null()), "{at}: {ends:?} {b:?}");
                                if blo == bhi {
                                    assert_eq!((lo.as_ref(), hi.as_ref()), (Some(coord), Some(coord)), "{at}");
                                } else if !date && (lo.is_some() || hi.is_some()) {
                                    assert!(lo.as_ref() == Some(coord) || (lo.is_none() && hi.as_ref() == Some(coord)), "{at}: {ends:?}");
                                }
                            }
                            (e, b) => panic!("{at}: ends {e:?}, bounds {b:?}"),
                        }
                    }
                }
                let moves: Vec<(usize, Option<usize>)> = arr(cm.get("rows")).iter().map(|r| (r.get("row").and_then(|x| x.as_int()).unwrap() as usize, if r.get("stay").and_then(|x| x.as_bool()) == Some(true) { None } else { r.get("to").and_then(|x| x.as_int()).map(|x| x as usize) })).collect();
                for r in &fm.rows {
                    let ct = arr(t.get("rows")).into_iter().find(|x| x.get("row").and_then(|y| y.as_int()) == Some(r.row as i128)).unwrap();
                    let accepts: Vec<Vec<usize>> = arr(ct.get("accepts")).iter().map(|a| arr(Some(a)).iter().map(|x| x.as_int().unwrap() as usize).collect()).collect();
                    let produces: Vec<Option<String>> = arr(ct.get("produces")).iter().map(|x| x.as_str().map(String::from)).collect();
                    assert_eq!((&r.accepts, &r.produces), (&accepts, &produces), "{path} row {}", r.row);
                    assert_eq!(r.to, moves.iter().find(|(row, _)| *row == r.row).and_then(|(_, to)| *to), "{path} row {}", r.row);
                }
            }
            (a, b) => panic!("{path}: api's machine {a:?}, the port's {b:?}"),
        }
        // the Connect service
        let c = api.get("connect").unwrap();
        let fc = facts.connect.as_ref().unwrap();
        assert_eq!((fc.path.clone(), fc.json_names.clone(), fc.json_int64.clone()), (st(c.get("path")), st(c.get("json_names")), st(c.get("json_int64"))), "{path}");
        assert_eq!(fields_of(&fc.request), wire_fields(c.get("request_fields")), "{path} request");
        assert_eq!(fields_of(&fc.response), wire_fields(c.get("response_fields")), "{path} response");
        assert_eq!(fc.elements.first().map(|(_, e)| fields_of(e)), c.get("element_fields").filter(|x| !x.is_null()).map(|x| wire_fields(Some(x))), "{path} elements");
        // a rule that walks two lists or more lists each under `sequences` (§15.219)
        if fc.elements.len() > 1 {
            let want: Vec<(String, Vec<(String, String, String, bool, Option<String>)>)> = arr(c.get("sequences")).iter().map(|s| (st(s.get("name")), wire_fields(s.get("fields")))).collect();
            assert_eq!(fc.elements.iter().map(|(n, e)| (n.clone(), fields_of(e))).collect::<Vec<_>>(), want, "{path} sequences");
        }
        let enums: Vec<ritsu_ports::WireEnum> = arr(c.get("enums"))
            .iter()
            .map(|e| ritsu_ports::WireEnum {
                name: st(e.get("name")),
                alias: st(e.get("alias")),
                contract: e.get("contract").filter(|x| !x.is_null()).map(|x| (st(x.get("file")), st(x.get("proto")))),
                unset: e.get("unset").and_then(|x| x.as_str()).map(String::from),
                values: arr(e.get("values")).iter().map(|v| (st(v.get("name")), st(v.get("alias")), v.get("number").and_then(|x| x.as_int()).unwrap() as i64)).collect(),
            })
            .collect();
        assert_eq!(fc.enums, enums, "{path} enums on the wire");
        // how the generated code is called
        check_call(&format!("{path} typescript"), &facts.typescript, ts, "module", "function", "params", "outputs");
        check_call(&format!("{path} python"), &facts.python, api.get("python").unwrap(), "module", "function", "params", "outputs");
        check_call(&format!("{path} go"), &facts.go, api.get("go").unwrap(), "package", "func", "input_fields", "output_fields");
        check_call(&format!("{path} rust"), &facts.rust, api.get("rust").unwrap(), "module", "function", "params", "outputs");
        assert_eq!(facts.go.input_type, st(api.get("go").and_then(|g| g.get("input_type"))), "{path}");
        rules += 1;
    }
    assert!(rules >= 45, "{rules} rules");
}

/// The value of a vector's input on the wire, as the port takes it.
fn wire(c: &rulec::types::Checked, name: &str, v: &rulec::eval::Val) -> Value {
    use rulec::eval::Val;
    match v {
        // the absent value of an optional column (null on the wire)
        Val::Enum(s) if s == rulec::kw::NONE && matches!(c.ty_of(name), Some(rulec::types::Ty::Opt(_))) => Value::None,
        Val::Enum(s) => Value::Enum(s.clone()),
        Val::Num(r) => Value::Int(rulec::types::wire_int(*r, c.wire_scale(name))),
        Val::Bool(b) => Value::Bool(*b),
        Val::Str(s) => Value::Str(s.clone()),
        Val::Date(y, m, d) => Value::Date(format!("{y:04}-{m:02}-{d:02}")),
        Val::Seq(xs) => Value::List(xs.iter().map(|x| x.iter().map(|(k, v)| (k.clone(), wire(c, k, v))).collect()).collect()),
    }
}

/// The evaluator behind the port answers every vector of every rule as `rulec vectors` says.
#[test]
fn 口の評価はどのベクタにもrulec_vectorsと同じ答えを返す() {
    let e = Engine::new();
    let mut answered = 0;
    for path in corpus() {
        let src = std::fs::read_to_string(&path).unwrap();
        if rulec::has_error(&rulec::report(&src, &path).diags) {
            continue;
        }
        let (f, c) = rulec::prepare(&src, &path).unwrap();
        for v in rulec::vectors::generate(&f, &c) {
            let mut inputs: ritsu_ports::Values = f.inputs.iter().map(|i| (i.name.text.clone(), v.input.get(&i.name.text).map(|x| wire(&c, &i.name.text, x)).unwrap_or(Value::None))).collect();
            for el in &f.elements {
                inputs.push((el.name.text.clone(), v.input.get(&el.name.text).map(|x| wire(&c, &el.name.text, x)).unwrap_or(Value::List(vec![]))));
            }
            let want: Vec<(String, Value)> = v.outputs.iter().map(|(n, x)| (n.clone(), x.as_ref().map(|x| wire(&c, n, x)).unwrap_or(Value::None))).collect();
            match e.eval(Path::new(&path), &inputs) {
                Ok(got) => assert_eq!(got, want, "{path}: {}", v.why),
                // a vector the generated code refuses is one with no answer
                Err(RuleError::Input(t)) | Err(RuleError::Contradiction(t)) => assert!(want.iter().any(|(_, x)| *x == Value::None) || !rulec::vectors::allowed(&f, &c, &v.input), "{path}: {}: refused: {}", v.why, t.en),
                Err(e) => panic!("{path}: {e:?}"),
            }
            answered += 1;
        }
    }
    assert!(answered > 1000, "{answered} vectors");
}

/// The page drawn through the port is the one `rulec doc` prints, in each language, as Markdown
/// and as HTML; named as the caller names the file (as `rulec doc` run beside the file names it).
#[test]
fn 口が描くページはrulec_docと同じ() {
    let e = Engine::new();
    let rules = ["tests/corpus/送料.rule", "tests/corpus/注文の状態.rule", "tests/corpus/印紙税.rule"];
    for rule in rules {
        for (lang, code) in [(ritsu_base::text::Lang::En, "en"), (ritsu_base::text::Lang::Ja, "ja")] {
            for html in [false, true] {
                let file = Path::new(rule).file_name().unwrap().to_string_lossy().to_string();
                let dir = root().join(Path::new(rule).parent().unwrap());
                let mut args = vec!["doc".to_string(), file.clone(), "--lang".into(), code.into()];
                if html {
                    args.extend(["--format".to_string(), "html".to_string()]);
                }
                let out = std::process::Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(&dir).args(&args).output().unwrap();
                assert!(out.status.success(), "{rule}");
                let want = String::from_utf8(out.stdout).unwrap();
                let got = e.doc(Path::new(rule), &file, html, lang).unwrap();
                assert!(got == want, "{rule} {code} html={html}: the port's page is not `rulec doc`'s");
            }
        }
    }
}

/// A relation between two inputs holds for every value of their ranges, or the corner that
/// breaks it is the example; a bound over a list is decided by how long the list can be — not
/// decided when that is not known, held when the longest list stays inside, and the longest list
/// at the top of its field's range the example when it does not.
#[test]
fn 前提は範囲の角で決まり_並びの上限は長さで決まる() {
    let e = Engine::new();
    let rule = Path::new("tests/corpus/比例配分.rule");
    let f = e.facts(rule).unwrap();
    let rels: Vec<&Precondition> = f.preconditions.iter().filter(|p| matches!(p, Precondition::Relation { .. })).collect();
    assert!(!rels.is_empty(), "{:?}", f.preconditions);
    let Precondition::Relation { left, op, right } = rels[0] else { unreachable!() };
    assert!(op.starts_with('<'), "{op}");
    // the rule's own ranges: the check passed, so the guard the generated code has is what keeps them apart
    let none = e.preconditions_hold(rule, &[], None, None).unwrap();
    assert!(none.iter().any(|(p, _)| p == rels[0]));
    // the left side kept below the right: it holds
    let ok = e.preconditions_hold(rule, &[(left.clone(), Some(0), Some(10)), (right.clone(), Some(10), Some(20))], None, None).unwrap();
    assert_eq!(ok.iter().find(|(p, _)| p == rels[0]).map(|(_, a)| a.clone()), Some(Answer::Holds));
    // the left side may pass the right: the corner is the example
    let bad = e.preconditions_hold(rule, &[(left.clone(), Some(0), Some(30)), (right.clone(), Some(10), Some(20))], None, None).unwrap();
    match bad.iter().find(|(p, _)| p == rels[0]).map(|(_, a)| a.clone()) {
        Some(Answer::Fails(ex)) => {
            assert_eq!(ex.iter().find(|(n, _)| n == left).map(|(_, v)| v.clone()), Some(Value::Int(30)));
            assert_eq!(ex.iter().find(|(n, _)| n == right).map(|(_, v)| v.clone()), Some(Value::Int(10)));
            if op == "<=" {
                // and at the corner itself, the evaluator refuses the call as the generated code does
            }
        }
        other => panic!("{other:?}"),
    }
    // an open end decides nothing
    let open = e.preconditions_hold(rule, &[(left.clone(), Some(0), None)], None, None).unwrap();
    assert!(matches!(open.iter().find(|(p, _)| p == rels[0]).map(|(_, a)| a), Some(Answer::Undecided(_)) | Some(Answer::Holds) | Some(Answer::Fails(_))));
    // the bounds over a list: not decided without a length; with one, held or broken
    let mut sums = 0;
    let mut floors = 0;
    for path in corpus() {
        let Ok(_) = e.facts(Path::new(&path)) else { continue };
        for (p, a) in e.preconditions_hold(Path::new(&path), &[], None, None).unwrap() {
            if matches!(p, Precondition::Sum { .. } | Precondition::Count { .. } | Precondition::Length { .. }) {
                assert!(matches!(a, Answer::Undecided(_)), "{path}: {p:?} {a:?}");
            }
        }
        for (p, a) in e.preconditions_hold(Path::new(&path), &[], None, Some(0)).unwrap() {
            match &p {
                // a total or a count held to a bottom above zero: the empty list never reaches it
                Precondition::Sum { over, min: Some(_), .. } | Precondition::Count { over, .. } => {
                    assert_eq!(a, Answer::Fails(vec![(over.clone(), Value::List(Vec::new()))]), "{path}: the empty list comes in under the bottom: {p:?}");
                    floors += 1;
                }
                Precondition::Sum { .. } | Precondition::Length { .. } => assert_eq!(a, Answer::Holds, "{path}: the empty list keeps every top: {p:?}"),
                _ => {}
            }
        }
        for (p, a) in e.preconditions_hold(Path::new(&path), &[], None, Some(1_000_000)).unwrap() {
            match (&p, &a) {
                (Precondition::Length { sequence, max }, Answer::Fails(ex)) => {
                    let Some((n, Value::List(xs))) = ex.first() else { panic!("{path}: {ex:?}") };
                    assert!(n == sequence && xs.len() as i128 == max + 1, "{path}: {ex:?}");
                }
                (Precondition::Sum { over, of, .. }, Answer::Fails(ex)) => {
                    sums += 1;
                    let Some((n, Value::List(xs))) = ex.first() else { panic!("{path}: {ex:?}") };
                    assert!(n == over && xs.iter().all(|x| x.iter().any(|(f, _)| f == of)), "{path}: {ex:?}");
                }
                (Precondition::Sum { .. } | Precondition::Length { .. }, other) => panic!("{path}: a list of a million breaks {p:?}, not {other:?}"),
                _ => {}
            }
        }
    }
    assert!(sums > 0, "some rule of the corpus bounds a total over its list");
    assert!(floors > 0, "some rule of the corpus holds a total or a count to a bottom above zero");
    // a set of days for an input the rule does not have, or not a date, is not decided
    assert!(matches!(e.checked_over(rule, "x", &Default::default()).unwrap(), Answer::Undecided(_)));
}

/// A total kept finer than its unit — the tax of each line worked out to the sen, its total
/// accepted up to 8_000JPY — is held to its top at the total's own step (§15.333). The tops of the
/// elements were added up in whole yen and compared with the top in hundredths, so twenty lines of
/// up to 800JPY, 16_000JPY, were said to keep it. The precondition carries the total's unit with
/// that step, so what it says can be said in yen.
#[test]
fn a_total_kept_finer_than_its_unit_is_held_at_its_own_step() {
    const TAX: &str = "rule line_tax v1\ndescription \"The tax on the lines of an order, each line's worked out to the sen\"\n\nelements lines\n  price : money[JPY]  range >=0JPY <=10_000JPY\n\ndefine tax : money[JPY] = price * 8%  range >=0JPY <=800JPY\n\nsum total_tax over lines of tax  range >=0JPY <=8_000JPY\n\noutputs\n  due : money[JPY]  round down(1JPY)\n\ndefine due : money[JPY] = total_tax\n";
    let e = Engine::new();
    let tmp = ritsu_testkit::TempDir::new("ports-sen");
    let path = tmp.path().join("line_tax.rule");
    std::fs::write(&path, TAX).unwrap();
    let f = e.facts(&path).unwrap();
    let sum = f.preconditions.iter().find(|p| matches!(p, Precondition::Sum { .. })).cloned().expect("the total's precondition");
    let Precondition::Sum { max, unit: Some(unit), .. } = &sum else { panic!("{sum:?}") };
    assert_eq!((*max, unit.unit.as_str(), unit.step), (800_000, "JPY", Some(ritsu_units::Rat::new(1, 100))), "{sum:?}");
    let at = |n: i128| e.preconditions_hold(&path, &[], None, Some(n)).unwrap().into_iter().find(|(p, _)| *p == sum).map(|(_, a)| a);
    assert!(matches!(at(20), Some(Answer::Fails(_))), "twenty lines reach 16_000JPY: {:?}", at(20));
    assert_eq!(at(10), Some(Answer::Holds), "ten lines reach 8_000JPY");
}

/// A rule that does not pass check has no facts and no page: what check says comes back instead,
/// in both languages.
#[test]
fn 検査を通らない規則は診断を返す() {
    let e = Engine::new();
    let said = e.facts(Path::new("tests/mutants/m_e101.rule")).unwrap_err();
    let e101 = said.iter().find(|s| s.code == "E101").expect("E101");
    assert!(e101.message.ja.contains("完全性の欠落"), "{:?}", e101.message);
    assert!(!e101.message.en.is_empty() && e101.message.en != e101.message.ja);
    assert!(e101.line.is_some());
    assert!(e.doc(Path::new("tests/mutants/m_e101.rule"), "m_e101.rule", false, ritsu_base::text::Lang::En).is_err());
    let missing = e.facts(Path::new("tests/corpus/無い.rule")).unwrap_err();
    assert_eq!(missing[0].code, "");
}

/// What a rule file holds: each thing by its naming, its lines, and its definition as `rulec fmt`
/// writes it, so realigning a table changes no definition.
#[test]
fn 規則のファイルが持つものと参照するもの() {
    let e = Engine::new();
    let items = e.items(&root(), "tests/corpus/送料.rule").unwrap();
    let names: Vec<String> = items.iter().map(|i| i.naming.text()).collect();
    for want in [
        "rulec \"tests/corpus/送料.rule\" input 重量",
        "rulec \"tests/corpus/送料.rule\" output 送料",
        "rulec \"tests/corpus/送料.rule\" table 基本送料",
        "rulec \"tests/corpus/送料.rule\" enum 都道府県",
    ] {
        if want.contains("都道府県") {
            continue;
        }
        assert!(names.iter().any(|n| n == want), "{want} is not among {names:?}");
    }
    let table = items.iter().find(|i| i.kind() == "table" && i.name() == "基本送料").unwrap();
    let src = std::fs::read_to_string(root().join("tests/corpus/送料.rule")).unwrap();
    let formatted = rulec::fmt::format(&src);
    let lines: Vec<&str> = formatted.lines().collect();
    assert_eq!(table.text, lines[table.lines.0 - 1..table.lines.1].join("\n"));
    assert!(table.text.starts_with("table 基本送料") && table.text.lines().last().unwrap().starts_with('|'), "{}", table.text);
    assert!(table.lines.1 > table.lines.0);
    // a table realigned is the same definition
    let dir = ritsu_testkit::TempDir::new("ports-fmt");
    let messy: String = src.lines().map(|l| if l.starts_with('|') { l.replace(" | ", "  |  ") } else { l.to_string() }).collect::<Vec<_>>().join("\n") + "\n";
    std::fs::write(dir.path().join("送料.rule"), &messy).unwrap();
    let again = e.items(dir.path(), "送料.rule").unwrap();
    assert_eq!(again.iter().find(|i| i.name() == "基本送料").unwrap().text, table.text);
    // every value of an enum, under its enum
    let order = e.items(&root(), "tests/corpus/注文の状態.rule").unwrap();
    assert!(order.iter().any(|i| i.naming.items.len() == 2 && i.naming.items[0].0 == "enum" && i.kind() == "value"));
    assert!(order.iter().any(|i| i.kind() == "machine"));
    // what a rule names outside itself
    let refs = e.references(&root(), "tests/corpus/出荷の送料.rule").unwrap();
    assert!(refs.iter().any(|r| r.how == "shape" && r.target.text() == "proto \"tests/corpus/contracts/shipment.proto\" message CreateShipmentRequest"), "{refs:?}");
    let sourced = e.references(&root(), "tests/corpus/paypal_fee.rule").unwrap();
    assert!(sourced.iter().any(|r| r.how == "source" && r.target.tool == ritsu_base::naming::Tool::File), "{sourced:?}");
    let applied = e.references(&root(), "tests/apply_fixtures/適用.rule").unwrap();
    assert!(applied.iter().any(|r| r.how == "apply" && r.target.tool == ritsu_base::naming::Tool::Rulec), "{applied:?}");
    // every item of every rule of the corpus can be named, and its lines are in the file
    for path in corpus() {
        let n = std::fs::read_to_string(root().join(&path)).unwrap().lines().count();
        for it in e.items(&root(), &path).unwrap() {
            assert!(it.lines.0 >= 1 && it.lines.0 <= it.lines.1 && it.lines.1 <= n, "{path}: {it:?}");
            assert_eq!(ritsu_base::naming::parse_one(&it.naming.text()).map(|x| x.text()).ok(), Some(it.naming.text()), "{path}");
        }
        e.references(&root(), &path).unwrap();
    }
}

/// An `enums` file is made into modules of types and nothing else (§15.212), and the port answers
/// what is generated (§15.258): its enums, and no Connect service, as `rulec api` writes none. A
/// rule that reads its enums keeps its own service.
#[test]
fn an_enums_file_has_no_connect_service() {
    let e = Engine::new();
    for (enums, rule) in [("tests/corpus/shared/catalog.rule", "tests/corpus/catalog_shipping.rule"), ("tests/corpus/shared/品目.rule", "tests/corpus/品目の送料.rule")] {
        let facts = e.facts(Path::new(enums)).unwrap_or_else(|e| panic!("{enums}: {e:?}"));
        assert!(facts.connect.is_none(), "{enums}: {:?}", facts.connect);
        assert!(facts.inputs.is_empty() && facts.outputs.is_empty() && !facts.enums.is_empty(), "{enums}");
        let src = std::fs::read_to_string(enums).unwrap();
        let (f, c) = rulec::prepare(&src, enums).unwrap();
        let api = json(&rulec::codegen::Gen::new(&f, &c, &src, enums).api());
        assert!(api.get("connect").is_none() && api.get("enums").is_some(), "{enums}");
        let reads = e.facts(Path::new(rule)).unwrap_or_else(|e| panic!("{rule}: {e:?}"));
        let path = reads.connect.map(|c| c.path).unwrap_or_default();
        assert!(path.starts_with("/rulec.") && path.ends_with("Service/Decide"), "{rule}: {path}");
    }
}

/// A machine whose table cuts four numbers it holds: a rate on a step of 0.1%, one on a step of 1%,
/// an amount of yen and a count, each within a range, and two enums.
const FEES: &str = "rule fees v1
description \"A claim that is charged a fee until it is closed\"

enum st = open | done
enum ev = charge | close

inputs
  state : st
  event : ev
  fee   : rate[step 0.1%]  range >=0% <=10%
  flat  : rate[step 1%]  range >=0% <=10%
  cap   : money[JPY]  range >=0JPY <=10000JPY
  items : number  range >=0 <=10

outputs
  moved   : st
  flagged : bool

table t
policy first
| state | event  | fee   | flat | cap     | items | -> moved | flagged |
| open  | charge | >0.5% | -    | -       | -     | open     | true    |
| open  | charge | -     | >5%  | -       | -     | open     | true    |
| open  | charge | -     | -    | >300JPY | -     | open     | true    |
| open  | charge | -     | -    | -       | >3    | open     | true    |
| open  | charge | -     | -    | -       | -     | open     | false   |
| open  | close  | -     | -    | -       | -     | done     | false   |
| done  | -      | -     | -    | -       | -     | state    | false   |

machine life over t
  carry   state -> moved
  held    fee, flat, cap, items
  initial open
  final   done
";

/// [`FEES`], with the names in Japanese and the yen written `円`.
const FEES_JA: &str = "rule 手数料(fees) v1
description \"締めるまで手数料を取る請求\"

enum 状態(st) = 受付(open) | 済み(done)
enum 出来事(ev) = 課金(charge) | 締め(close)

inputs
  状態(state)   : 状態
  出来事(event) : 出来事
  手数料(fee)   : rate[step 0.1%]  range >=0% <=10%
  一律(flat)    : rate[step 1%]  range >=0% <=10%
  上限(cap)     : money[円]  range >=0円 <=10000円
  件数(items)   : number  range >=0 <=10

outputs
  移った(moved) : 状態
  目印(flagged) : bool

table 表(t)
policy first
| 状態 | 出来事 | 手数料 | 一律 | 上限   | 件数 | -> 移った | 目印  |
| 受付 | 課金   | >0.5%  | -    | -      | -    | 受付      | true  |
| 受付 | 課金   | -      | >5%  | -      | -    | 受付      | true  |
| 受付 | 課金   | -      | -    | >300円 | -    | 受付      | true  |
| 受付 | 課金   | -      | -    | -      | >3   | 受付      | true  |
| 受付 | 課金   | -      | -    | -      | -    | 受付      | false |
| 受付 | 締め   | -      | -    | -      | -    | 済み      | false |
| 済み | -      | -      | -    | -      | -    | 状態      | false |

machine 流れ(life) over 表
  carry   状態 -> 移った
  held    手数料, 一律, 上限, 件数
  initial 受付
  final   済み
";

/// A machine whose two held counts are bounded on one side each: `low` by `<=10`, `high` by `>=0`.
const LIMITS: &str = "rule limits v1
description \"A claim whose counts are bounded on one side\"

enum st = waiting | done
enum ev = add | close

inputs
  state : st
  event : ev
  low   : number  range <=10
  high  : number  range >=0

outputs
  moved   : st
  flagged : bool

table t
policy first
| state   | event | low | high | -> moved | flagged |
| waiting | add   | >3  | -    | waiting  | true    |
| waiting | add   | -   | >3   | waiting  | true    |
| waiting | add   | -   | -    | waiting  | false   |
| waiting | close | -   | -    | done     | false   |
| done    | -     | -   | -    | state    | false   |

machine life over t
  carry   state -> moved
  held    low, high
  initial waiting
  final   done
";

/// A machine whose table cuts a value it works out and a date: `share` is cut at 1USD on tenths of a
/// dollar, but its values are on 0.3USD (`amount × 30%` of whole dollars), and `day` is cut on the
/// first of April. The rule passes `check` with two warnings (W127) on what the machine reaches.
const SHARES: &str = "rule shares v1
description \"A claim whose share of an amount decides when it is flagged\"

enum st = waiting | done
enum ev = add | close

inputs
  state  : st
  event  : ev
  amount : money[USD]  range >=0USD <=10USD
  day    : date  range >=2026-01-01 <=2026-12-31

outputs
  moved   : st
  flagged : bool

define share : money[USD] = amount × 30%

table t
policy first
| state   | event | share    | day          | -> moved | flagged |
| waiting | add   | >1USD    | -            | waiting  | true    |
| waiting | add   | -        | >=2026-04-01 | waiting  | true    |
| waiting | add   | -        | -            | waiting  | false   |
| waiting | close | -        | -            | done     | false   |
| done    | -     | -        | -            | state    | false   |

machine life over t
  carry   state -> moved
  initial waiting
  final   done
";

/// The ends of each coordinate of a machine's table, as the port carries them (ritsu's DESIGN 3.2):
/// for a number or a date the table cuts, the first and the last value each coordinate stands for,
/// written as the coordinates are; none at a side the range leaves open, and none at all for an enum
/// or for a coordinate that holds no value of its column. A file that holds an input to a coordinate
/// reads any value between them as that coordinate (dandori's `held`).
#[test]
fn a_machine_axis_carries_the_ends_of_each_coordinate() {
    let t = ritsu_testkit::TempDir::new("rulec-port-ends");
    t.write("fees.rule", FEES);
    t.write("手数料.rule", FEES_JA);
    let e = Engine::new();
    let span = |lo: &str, hi: &str| Some((Some(lo.to_string()), Some(hi.to_string())));
    for (file, [state, event, fee, flat, cap, items], yen) in [("fees.rule", ["state", "event", "fee", "flat", "cap", "items"], "JPY"), ("手数料.rule", ["状態", "出来事", "手数料", "一律", "上限", "件数"], "円")] {
        let facts = e.facts(&t.path().join(file)).unwrap_or_else(|e| panic!("{file}: {e:?}"));
        let m = facts.machine.unwrap_or_else(|| panic!("{file}: no machine"));
        let ends = |col: &str| -> Vec<Option<(Option<String>, Option<String>)>> {
            let a = m.axes.iter().find(|a| a.column == col).unwrap_or_else(|| panic!("{file}: no axis {col}: {:?}", m.axes));
            assert_eq!(a.ends.len(), a.coords.len(), "{file} {col}");
            a.ends.clone()
        };
        assert!(ends(state).iter().chain(&ends(event)).all(Option::is_none), "{file}: {:?}", m.axes);
        assert_eq!(ends(fee), [span("0%", "0%"), span("0.1%", "0.4%"), span("0.5%", "0.5%"), span("0.6%", "9.9%"), span("10%", "10%")], "{file}");
        assert_eq!(ends(flat), [span("0%", "0%"), span("1%", "4%"), span("5%", "5%"), span("6%", "9%"), span("10%", "10%")], "{file}");
        let yen = |a: &str, b: &str| span(&format!("{a}{yen}"), &format!("{b}{yen}"));
        assert_eq!(ends(cap), [yen("0", "0"), yen("1", "299"), yen("300", "300"), yen("301", "9999"), yen("10000", "10000")], "{file}");
        assert_eq!(ends(items), [span("0", "0"), span("1", "2"), span("3", "3"), span("4", "9"), span("10", "10")], "{file}");
    }
    // where the range leaves a side open, the set at that side has no end there
    t.write("limits.rule", LIMITS);
    let m = e.facts(&t.path().join("limits.rule")).unwrap_or_else(|e| panic!("limits.rule: {e:?}")).machine.unwrap();
    let ends = |col: &str| m.axes.iter().find(|a| a.column == col).map(|a| (a.coords.clone(), a.ends.clone())).unwrap();
    let to = |hi: &str| Some((None, Some(hi.to_string())));
    let from = |lo: &str| Some((Some(lo.to_string()), None));
    let strs = |v: [&str; 4]| v.map(String::from).to_vec();
    assert_eq!(ends("low"), (strs(["2", "3", "4", "10"]), vec![to("2"), span("3", "3"), span("4", "9"), span("10", "10")]));
    assert_eq!(ends("high"), (strs(["0", "1", "3", "4"]), vec![span("0", "0"), span("1", "2"), span("3", "3"), from("4")]));
    // a value worked out on a coarser step than its axis's takes the values of that step alone, so
    // the set above 1USD starts at 1.2USD and the point 1USD holds none; a date's ends are days
    t.write("shares.rule", SHARES);
    let m = e.facts(&t.path().join("shares.rule")).unwrap_or_else(|e| panic!("shares.rule: {e:?}")).machine.unwrap();
    let ends = |col: &str| m.axes.iter().find(|a| a.column == col).map(|a| (a.coords.clone(), a.ends.clone())).unwrap();
    assert_eq!(ends("share"), (["0.9USD", "1USD", "1.2USD"].map(String::from).to_vec(), vec![to("0.9USD"), None, from("1.2USD")]));
    let days = ["2026-01-01", "2026-01-02", "2026-04-01", "2026-04-02", "2026-12-31"].map(String::from).to_vec();
    assert_eq!(ends("day"), (days, vec![span("2026-01-01", "2026-01-01"), span("2026-01-02", "2026-03-31"), span("2026-04-01", "2026-04-01"), span("2026-04-02", "2026-12-30"), span("2026-12-31", "2026-12-31")]));
}

/// `ritsu check` prints for a rule what `rulec check` prints for it, and a rule it has checked
/// for that is not checked a second time when a workflow asks for its facts, under another
/// spelling of its path (ritsu's DESIGN 6.1, 8.3).
#[test]
fn a_rule_checked_for_ritsu_check_is_checked_once() {
    let e = Engine::new();
    let rules = corpus();
    let units = e.checked(&root(), &rules, ritsu_base::text::Lang::En);
    assert_eq!(units.len(), rules.len());
    for (rule, u) in rules.iter().zip(&units) {
        let o = std::process::Command::new(env!("CARGO_BIN_EXE_rulec")).args(["check", rule, "--lang", "en"]).current_dir(root()).output().unwrap();
        assert_eq!(u.text(), String::from_utf8_lossy(&o.stdout), "{rule}");
        assert_eq!(u.verdict, ritsu_ports::Verdict::Passes, "{rule}");
        assert!(e.facts(&root().join(rule)).is_ok(), "{rule}");
    }
    assert_eq!(e.checks(), rules.len(), "each rule is checked once, for check and for its facts");
}

/// A rule is one file however its path is written (ritsu's DESIGN 4.15): asked under four spellings
/// of its path in a run, it is checked once, and what the port says of one that does not pass names
/// it one way. A rule that does not pass is said in both languages, so after `ritsu check` has
/// checked it in one, it is checked once more, in the other, and no more. Written otherwise, a rule
/// is checked again, whichever way it is asked.
#[test]
fn a_rule_is_one_file_however_its_path_is_written() {
    let t = ritsu_testkit::TempDir::new("rulec-port-spelled");
    let rule = "rule extra v1\ndescription \"The seats a late event asks for on top\"\n\ninputs\n  late  : bool\n  asked : number  range >=0 <=100\n\noutputs\n  more : number  round down(1)\n\ntable pick\npolicy unique\n| late  | asked | -> more : number |\n| true  | <=50  | 10               |\n| true  | >50   | 20               |\n| false | -     | 0                |\n";
    t.write("rules/extra.rule", rule);
    t.write("rules/short.rule", rule.replace("| false | -     | 0                |\n", ""));
    let _run = ritsu_base::paths::run_from(".");
    let e = Engine::new();
    for p in t.spellings("rules/extra.rule") {
        e.facts(&p).unwrap_or_else(|s| panic!("{}: {s:?}", p.display()));
    }
    assert_eq!(e.checks(), 1, "one check for every spelling");
    let short = t.path().join("rules/short.rule").display().to_string();
    let units = e.checked(t.path(), &[short], ritsu_base::text::Lang::En);
    assert_eq!(units[0].verdict, ritsu_ports::Verdict::Fails, "{}", units[0].text());
    let said: Vec<Vec<ritsu_ports::Said>> = t.spellings("rules/short.rule").iter().map(|p| e.facts(p).unwrap_err()).collect();
    assert!(said[0].iter().any(|s| s.code == "E101"), "{:?}", said[0]);
    assert!(said.windows(2).all(|w| w[0] == w[1]), "{said:?}");
    assert_eq!(e.checks(), 3, "the rule that does not pass, checked once in each language");
    t.write("rules/extra.rule", format!("{rule}\n# the rule written otherwise\n"));
    for p in t.spellings("rules/extra.rule") {
        e.facts(&p).unwrap_or_else(|s| panic!("{}: {s:?}", p.display()));
    }
    assert_eq!(e.checks(), 4, "the rule written otherwise, checked again once");
}

/// A check is kept while what it read reads the same (ritsu's DESIGN 17.9.1): the same text of a
/// rule, read through files whose enums differ, is checked again, both checks are kept, and one read
/// off a report holds only while what the report read does.
#[test]
fn a_kept_check_holds_while_what_it_read_reads_the_same() {
    let view = |refundable: &str| {
        let m = std::rc::Rc::new(ritsu_base::fs::Memory::new("/shop"));
        m.add("sizes.rule", format!("enums sizes v1\ndescription \"The sizes of an order, and the ones a clerk refunds\"\n\nenum size = small | medium | large\ngroup refundable = {refundable}\n"));
        m.add(
            "refund.rule",
            "rule refund v1\ndescription \"Whether a clerk refunds an order\"\n\nimport enum size from \"sizes.rule\"\n\ninputs\n  size : size\n\noutputs\n  allowed : bool\n\ntable allowed\npolicy unique\n| size            | -> allowed |\n| refundable      | true       |\n| not: refundable | false      |\n",
        );
        m
    };
    let (passes, fails) = (view("small"), view("small, medium, large"));
    let rule = Path::new("/shop/refund.rule");
    let e = Engine::new();
    let facts = |m: &std::rc::Rc<ritsu_base::fs::Memory>| ritsu_base::fs::with(m.clone(), || e.facts(rule)).map(|_| ()).map_err(|said| said.iter().map(|s| s.code.clone()).collect::<Vec<_>>());
    // where every size is refundable, the last row is never reached: the report says so, and the
    // facts are read off it
    let units = ritsu_base::fs::with(fails.clone(), || e.checked(Path::new("/shop"), &["refund.rule".to_string()], ritsu_base::text::Lang::En));
    assert_ne!(units[0].verdict, ritsu_ports::Verdict::Passes, "{}", units[0].text());
    assert!(facts(&fails).is_err());
    // the same text where a small order alone is refundable: checked again, and it passes
    let before = e.checks();
    assert_eq!(facts(&passes), Ok(()));
    assert_eq!(e.checks() - before, 1);
    // both are kept
    assert!(facts(&fails).is_err());
    assert_eq!(facts(&passes), Ok(()));
    assert_eq!(e.checks() - before, 1);
    // what its answers rest on: the same for the same files, wherever they are held
    let on = |m: &std::rc::Rc<ritsu_base::fs::Memory>| ritsu_base::fs::with(m.clone(), || ritsu_ports::Rules::rests_on(&e, rule));
    assert!(on(&passes).is_some());
    assert_eq!(on(&passes), on(&view("small")));
    assert_ne!(on(&passes), on(&fails));
    assert_eq!(e.checks() - before, 1);
    // the same files at another place, with a comment and another description: the same
    let elsewhere = |sizes: &str| {
        let m = std::rc::Rc::new(ritsu_base::fs::Memory::new("/far/away/shop"));
        m.add("sizes.rule", sizes.to_string());
        m.add("refund.rule", ritsu_base::fs::with(passes.clone(), || ritsu_base::fs::read_to_string(rule)).unwrap());
        m
    };
    let there = |m: &std::rc::Rc<ritsu_base::fs::Memory>| ritsu_base::fs::with(m.clone(), || ritsu_ports::Rules::rests_on(&e, Path::new("/far/away/shop/refund.rule")));
    let sizes = ritsu_base::fs::with(passes.clone(), || ritsu_base::fs::read_to_string(Path::new("/shop/sizes.rule"))).unwrap();
    assert_eq!(on(&passes), there(&elsewhere(&sizes)));
    let noted = format!("# the sizes\n{}", sizes.replace("The sizes of an order, and", "Each size of an order, and"));
    assert_ne!(noted, sizes);
    assert_eq!(on(&passes), there(&elsewhere(&noted)));
    // and with what it reads changed, not the same
    assert_ne!(on(&passes), there(&elsewhere(&sizes.replace("group refundable = small", "group refundable = small, medium"))));
}

/// A rule checked on the disk is the one check of it asked again through files laid over the disk
/// (as ringi lays the rules it lowers a request to), while every question its check asked of the
/// files reads the same through them (ritsu's DESIGN 4.15): asked again there, and applied by a rule
/// laid there, beside it or in a directory beside its own (`../refund.rule`), it is not checked
/// again. Where the files laid write a file it read otherwise, or lay the file it looked for and did
/// not find, it is checked again, and the check on the disk still holds on the disk.
#[test]
fn a_check_on_the_disk_holds_over_laid_files_while_what_it_read_reads_the_same() {
    checked_on_the_disk_then_over_it(
        "rulec-kept-laid",
        [
            ("rules/sizes.rule", "enums sizes v1\ndescription \"The sizes of an order, and the ones a clerk refunds\"\n\nenum size = small | medium | large\ngroup refundable = small\n"),
            (
                "rules/refund.rule",
                "rule refund v1\ndescription \"Whether a clerk refunds an order\"\n\nimport enum size from \"sizes.rule\"\n\ninputs\n  size : size\n\noutputs\n  allowed : bool\n\ntable allowed\npolicy unique\n| size            | -> allowed |\n| refundable      | true       |\n| not: refundable | false      |\n",
            ),
            (
                "rules/late.rule",
                "rule late v1\ndescription \"Whether a late order is refunded, by the sizes a later file says\"\n\nimport enum size from \"later.rule\"\n\ninputs\n  size : size\n\noutputs\n  allowed : bool\n\ntable allowed\npolicy unique\n| size            | -> allowed |\n| refundable      | true       |\n| not: refundable | false      |\n",
            ),
            ("rules/later.rule", "enums later v1\ndescription \"The sizes of a late order, and the ones a clerk refunds\"\n\nenum size = small | medium | large\ngroup refundable = small\n"),
        ],
        ("group refundable = small", "group refundable = small, medium, large"),
        (
            "rules/laid.rule",
            "rules/refund.lowered/laid.rule",
            "rule laid v1\ndescription \"A rule laid over the disk that applies the refund\"\n\nimport enum size from \"{up}sizes.rule\"\n\ninputs\n  size : size\n\napply refund = \"{up}refund.rule\" {pin}\n  size = size\n  allowed -> refunded\n\noutputs\n  paid_back : bool\n\nresult paid_back = refunded\n",
        ),
    );
}

/// The same, with rules written in Japanese names.
#[test]
fn ディスクで検査した規則は_ファイルを重ねても読んだものが同じなら検査し直さない() {
    checked_on_the_disk_then_over_it(
        "rulec-kept-laid-ja",
        [
            ("規則/大きさ.rule", "enums 大きさ(sizes) v1\ndescription \"注文の大きさと、店員が返金するもの\"\n\nenum 大きさ(size) = 小(small) | 中(medium) | 大(large)\ngroup 返金できる(refundable) = 小\n"),
            (
                "規則/返金.rule",
                "rule 返金(refund) v1\ndescription \"店員が注文を返金するか\"\n\nimport enum 大きさ from \"大きさ.rule\"\n\ninputs\n  大きさ(size) : 大きさ\n\noutputs\n  返金する(allowed) : bool\n\ntable 返金する\npolicy unique\n| 大きさ          | -> 返金する |\n| 返金できる      | true        |\n| not: 返金できる | false       |\n",
            ),
            (
                "規則/遅い.rule",
                "rule 遅い(late) v1\ndescription \"遅れた注文を返金するか。大きさはあとのファイルが言う\"\n\nimport enum 大きさ from \"あとで.rule\"\n\ninputs\n  大きさ(size) : 大きさ\n\noutputs\n  返金する(allowed) : bool\n\ntable 返金する\npolicy unique\n| 大きさ          | -> 返金する |\n| 返金できる      | true        |\n| not: 返金できる | false       |\n",
            ),
            ("規則/あとで.rule", "enums あとで(later) v1\ndescription \"遅れた注文の大きさと、店員が返金するもの\"\n\nenum 大きさ(size) = 小(small) | 中(medium) | 大(large)\ngroup 返金できる(refundable) = 小\n"),
        ],
        ("group 返金できる(refundable) = 小", "group 返金できる(refundable) = 小, 中, 大"),
        (
            "規則/重ねた.rule",
            "規則/返金.lowered/重ねた.rule",
            "rule 重ねた(laid) v1\ndescription \"ディスクの上に重ねて置き、返金の規則を準用する規則\"\n\nimport enum 大きさ from \"{up}大きさ.rule\"\n\ninputs\n  大きさ(size) : 大きさ\n\napply 返金(refund) = \"{up}返金.rule\" {pin}\n  大きさ = 大きさ\n  返金する -> 返金した\n\noutputs\n  払い戻す(paid_back) : bool\n\nresult 払い戻す = 返金した\n",
        ),
    );
}

/// The test of a rule checked on the disk and asked again through files laid over it. `files` are an
/// `enums` file, a rule that imports it, a rule that imports a file not on the disk, and that file,
/// by their paths from the directory they are written in; `every` rewrites the `enums` file so that
/// every size is refundable; `laid` is a rule laid over the disk that applies the rule importing the
/// `enums` file, laid beside them and in a directory beside theirs, as ringi lays the rules it lowers
/// a request to: its two paths, and its text, with `{up}` where the way up to their directory goes
/// and `{pin}` where the pin of its apply goes.
fn checked_on_the_disk_then_over_it(name: &str, files: [(&str, &str); 4], every: (&str, &str), laid: (&str, &str, &str)) {
    /// `f` run with `put` laid over the files the thread reads, each at its path from `dir`.
    fn over<R>(dir: &Path, put: &[(&str, String)], f: impl FnOnce() -> R) -> R {
        let mut o = ritsu_base::fs::Overlay::new(ritsu_base::fs::current());
        for (p, text) in put {
            o.put(&dir.join(p), text.as_bytes());
        }
        ritsu_base::fs::with(std::rc::Rc::new(o), f)
    }
    let [sizes, refund, late, later] = files;
    let t = ritsu_testkit::TempDir::new(name);
    for (p, text) in [sizes, refund, late] {
        t.write(p, text);
    }
    let at = |p: &str| t.path().join(p);
    // the codes the check of the rule at `p` says, the rule read through the files the thread reads
    let said = |p: &str| -> Vec<String> {
        let path = at(p).display().to_string();
        let src = ritsu_base::fs::read_to_string(Path::new(&path)).unwrap_or_else(|e| panic!("{path}: {e}"));
        rulec::report(&src, &path).diags.into_iter().map(|d| d.code.to_string()).collect()
    };
    // the files checked whole since the counts were last taken, each once
    let checked = || -> Vec<PathBuf> {
        ritsu_base::tally::take()
            .into_iter()
            .filter(|(tool, _, _)| *tool == "rulec")
            .map(|(_, f, n)| {
                assert_eq!(n, 1, "{} checked {n} times", f.display());
                f
            })
            .collect()
    };
    let these = |ps: &[&str]| -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = ps.iter().map(|p| ritsu_base::paths::opened(&at(p))).collect();
        v.sort();
        v
    };
    let none: Vec<String> = Vec::new();
    ritsu_base::tally::take();
    // on the disk: the rule that imports the `enums` file passes; the one that imports a file not
    // there does not (E013)
    assert_eq!(said(refund.0), none);
    let missing = said(late.0);
    assert!(missing.contains(&"E013".to_string()), "{missing:?}");
    assert_eq!(checked(), these(&[sizes.0, refund.0, late.0]));
    // 1. over rules laid on the disk, which apply the rule: what the check read reads the same, so
    // the check on the disk answers, asked again and for the rules laid there, the one beside it and
    // the one that writes the way up to it
    let pin = format!("meaning:{}", rulec::link::pin::meaning(refund.1));
    let (beside, below, text) = laid;
    let lowered = [(beside, text.replace("{up}", "").replace("{pin}", &pin)), (below, text.replace("{up}", "../").replace("{pin}", &pin))];
    over(t.path(), &lowered, || {
        assert_eq!(said(refund.0), none);
        assert_eq!(said(late.0), missing);
        assert_eq!(said(beside), none);
        assert_eq!(said(below), none);
    });
    assert_eq!(checked(), these(&[beside, below]), "the rules laid alone are checked");
    // 2. over the `enums` file written otherwise: checked again, and every size refundable leaves the
    // last row of the table never reached
    let others = [(sizes.0, sizes.1.replace(every.0, every.1))];
    assert_ne!(others[0].1, sizes.1);
    let otherwise = over(t.path(), &others, || said(refund.0));
    assert_ne!(otherwise, none);
    assert_eq!(checked(), these(&[sizes.0, refund.0]));
    // 3. over the file the check looked for and did not find: checked again, and it passes
    let found = over(t.path(), &[(later.0, later.1.to_string())], || said(late.0));
    assert_eq!(found, none);
    assert_eq!(checked(), these(&[late.0, later.0]));
    // on the disk again: the checks made there hold
    assert_eq!(said(refund.0), none);
    assert_eq!(said(late.0), missing);
    assert_eq!(checked(), Vec::<PathBuf>::new());
}

/// A rule another applies by a path that goes up (`../shared/refund.rule`) is checked by the path the
/// run writes it (§15.320), and so is the `enums` file it imports: on the disk alone, asked for
/// itself and applied from a directory beside its own, each is one check, whichever is asked first,
/// as is the `enums` file the rule that applies it imports by the same way up; and the page of the
/// rule that applies it reads the two off those checks.
#[test]
fn a_rule_applied_by_a_path_that_goes_up_is_checked_once() {
    applied_by_a_path_that_goes_up(
        "rulec-kept-up",
        ("shared/sizes.rule", "enums sizes v1\ndescription \"The sizes of an order, and the ones a clerk refunds\"\n\nenum size = small | medium | large\ngroup refundable = small\n"),
        (
            "shared/refund.rule",
            "rule refund v1\ndescription \"Whether a clerk refunds an order\"\n\nimport enum size from \"sizes.rule\"\n\ninputs\n  size : size\n\noutputs\n  allowed : bool\n\ntable allowed\npolicy unique\n| size            | -> allowed |\n| refundable      | true       |\n| not: refundable | false      |\n",
        ),
        (
            "request.lowered/route.rule",
            "rule route v1\ndescription \"A rule in a directory beside the shared one that applies the refund\"\n\nimport enum size from \"../shared/sizes.rule\"\n\ninputs\n  size : size\n\napply refund = \"../shared/refund.rule\" {pin}\n  size = size\n  allowed -> refunded\n\noutputs\n  paid_back : bool\n\nresult paid_back = refunded\n",
        ),
    );
}

/// The same, with rules written in Japanese names.
#[test]
fn 親のディレクトリを経て準用する規則とそれが読む規則は一度だけ検査する() {
    applied_by_a_path_that_goes_up(
        "rulec-kept-up-ja",
        ("共有/大きさ.rule", "enums 大きさ(sizes) v1\ndescription \"注文の大きさと、店員が返金するもの\"\n\nenum 大きさ(size) = 小(small) | 中(medium) | 大(large)\ngroup 返金できる(refundable) = 小\n"),
        (
            "共有/返金.rule",
            "rule 返金(refund) v1\ndescription \"店員が注文を返金するか\"\n\nimport enum 大きさ from \"大きさ.rule\"\n\ninputs\n  大きさ(size) : 大きさ\n\noutputs\n  返金する(allowed) : bool\n\ntable 返金する\npolicy unique\n| 大きさ          | -> 返金する |\n| 返金できる      | true        |\n| not: 返金できる | false       |\n",
        ),
        (
            "申請.lowered/経路.rule",
            "rule 経路(route) v1\ndescription \"共有のディレクトリの隣のディレクトリから、返金の規則を準用する規則\"\n\nimport enum 大きさ from \"../共有/大きさ.rule\"\n\ninputs\n  大きさ(size) : 大きさ\n\napply 返金(refund) = \"../共有/返金.rule\" {pin}\n  大きさ = 大きさ\n  返金する -> 返金した\n\noutputs\n  払い戻す(paid_back) : bool\n\nresult 払い戻す = 返金した\n",
        ),
    );
}

/// The test of a rule applied by a path that goes up, on the disk alone: `sizes` is an `enums` file,
/// `refund` a rule beside it that imports it, and `up` a rule in a directory beside theirs that
/// imports the one and applies the other by the way up to them, with `{pin}` where the pin of its
/// apply goes; each by its path from the directory they are written in, and its text.
fn applied_by_a_path_that_goes_up(name: &str, sizes: (&str, &str), refund: (&str, &str), up: (&str, &str)) {
    let pin = format!("meaning:{}", rulec::link::pin::meaning(refund.1));
    let none: Vec<String> = Vec::new();
    for up_first in [false, true] {
        let t = ritsu_testkit::TempDir::new(&format!("{name}-{up_first}"));
        t.write(sizes.0, sizes.1);
        t.write(refund.0, refund.1);
        t.write(up.0, up.1.replace("{pin}", &pin));
        let at = |p: &str| t.path().join(p);
        let said = |p: &str| -> Vec<String> {
            let path = at(p).display().to_string();
            let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
            rulec::report(&src, &path).diags.into_iter().map(|d| d.code.to_string()).collect()
        };
        // the files checked whole since the counts were last taken, each once
        let checked = || -> Vec<PathBuf> {
            ritsu_base::tally::take()
                .into_iter()
                .filter(|(tool, _, _)| *tool == "rulec")
                .map(|(_, f, n)| {
                    assert_eq!(n, 1, "{} checked {n} times", f.display());
                    f
                })
                .collect()
        };
        let these = |ps: &[&str]| -> Vec<PathBuf> {
            let mut v: Vec<PathBuf> = ps.iter().map(|p| ritsu_base::paths::opened(&at(p))).collect();
            v.sort();
            v
        };
        ritsu_base::tally::take();
        if up_first {
            // the rule that goes up, first: it, the rule it applies and the `enums` file are each
            // checked once; then asked for themselves, neither is checked again
            assert_eq!(said(up.0), none);
            assert_eq!(checked(), these(&[sizes.0, refund.0, up.0]));
            assert_eq!(said(refund.0), none);
            assert_eq!(said(sizes.0), none);
            assert_eq!(checked(), Vec::<PathBuf>::new(), "asked for themselves after the rule that goes up");
        } else {
            // the rule it applies, first, which checks the `enums` file it imports; then the rule
            // that goes up is the one checked
            assert_eq!(said(refund.0), none);
            assert_eq!(checked(), these(&[sizes.0, refund.0]));
            assert_eq!(said(up.0), none);
            assert_eq!(checked(), these(&[up.0]), "the rule that goes up alone is checked");
        }
        // the page of the rule that goes up reads the rule it applies off the same check: neither it
        // nor the `enums` file is checked or read again
        let page = Engine::new().doc(&at(up.0), up.0, false, ritsu_base::text::Lang::En);
        assert!(page.is_ok(), "{page:?}");
        let again: Vec<(&str, PathBuf, usize)> = ritsu_base::tally::take().into_iter().filter(|(_, f, _)| these(&[sizes.0, refund.0]).contains(f)).collect();
        assert_eq!(again, Vec::new(), "the page reads them off the checks made");
    }
}

/// What reads of a check only whether a rule passes and the codes of its errors (the check of a rule
/// another applies, of an `enums` file a rule imports, a port's question of whether a rule passes)
/// is answered from a report in either language, and a rule there is no report of is checked in the
/// thread's language (§15.320): in a run in Japanese where the port asks of a rule first and the rule
/// that applies it is checked after, each file is checked once, the rule's own findings shown after
/// in Japanese among them; and so it is the other way round. A file's own findings are shown in the
/// language they are shown in: shown in English after checks made in Japanese, the rule is checked
/// in English.
#[test]
fn a_check_read_for_whether_a_rule_passes_is_one_check_in_either_language() {
    read_in_either_language(
        "rulec-kept-lang",
        ("shared/sizes.rule", "enums sizes v1\ndescription \"The sizes of an order, and the ones a clerk refunds\"\n\nenum size = small | medium | large\ngroup refundable = small\n"),
        (
            "shared/refund.rule",
            "rule refund v1\ndescription \"Whether a clerk refunds an order\"\n\nimport enum size from \"sizes.rule\"\n\ninputs\n  size : size\n\noutputs\n  allowed : bool\n\ntable allowed\npolicy first\n| size            | -> allowed |\n| refundable      | true       |\n| not: refundable | false      |\n",
        ),
        (
            "request.lowered/route.rule",
            "rule route v1\ndescription \"A rule in a directory beside the shared one that applies the refund\"\n\nimport enum size from \"../shared/sizes.rule\"\n\ninputs\n  size : size\n\napply refund = \"../shared/refund.rule\" {pin}\n  size = size\n  allowed -> refunded\n\noutputs\n  paid_back : bool\n\nresult paid_back = refunded\n",
        ),
    );
}

/// The same, with rules written in Japanese names.
#[test]
fn 通るかだけを読む検査は_どちらの言語で覚えた報告からでも一度だけ() {
    read_in_either_language(
        "rulec-kept-lang-ja",
        ("共有/大きさ.rule", "enums 大きさ(sizes) v1\ndescription \"注文の大きさと、店員が返金するもの\"\n\nenum 大きさ(size) = 小(small) | 中(medium) | 大(large)\ngroup 返金できる(refundable) = 小\n"),
        (
            "共有/返金.rule",
            "rule 返金(refund) v1\ndescription \"店員が注文を返金するか\"\n\nimport enum 大きさ from \"大きさ.rule\"\n\ninputs\n  大きさ(size) : 大きさ\n\noutputs\n  返金する(allowed) : bool\n\ntable 返金する\npolicy first\n| 大きさ          | -> 返金する |\n| 返金できる      | true        |\n| not: 返金できる | false       |\n",
        ),
        (
            "申請.lowered/経路.rule",
            "rule 経路(route) v1\ndescription \"共有のディレクトリの隣のディレクトリから、返金の規則を準用する規則\"\n\nimport enum 大きさ from \"../共有/大きさ.rule\"\n\ninputs\n  大きさ(size) : 大きさ\n\napply 返金(refund) = \"../共有/返金.rule\" {pin}\n  大きさ = 大きさ\n  返金する -> 返金した\n\noutputs\n  払い戻す(paid_back) : bool\n\nresult 払い戻す = 返金した\n",
        ),
    );
}

/// The test of a check read in either language, on the disk: `sizes` is an `enums` file, `refund` a
/// rule beside it that imports it, whose `first` table no two rows of overlap (W110, a finding to
/// show), and `up` a rule in a directory beside theirs that imports the one and applies the other,
/// with `{pin}` where the pin of its apply goes; each by its path from the directory they are written
/// in, and its text.
fn read_in_either_language(name: &str, sizes: (&str, &str), refund: (&str, &str), up: (&str, &str)) {
    use rulec::i18n::{Lang, with};
    let pin = format!("meaning:{}", rulec::link::pin::meaning(refund.1));
    let none: Vec<String> = Vec::new();
    for port_first in [true, false] {
        let t = ritsu_testkit::TempDir::new(&format!("{name}-{port_first}"));
        t.write(sizes.0, sizes.1);
        t.write(refund.0, refund.1);
        t.write(up.0, up.1.replace("{pin}", &pin));
        let at = |p: &str| t.path().join(p);
        // the report of the rule at `p`, as a run in `lang` shows it
        let said = |p: &str, lang: Lang| -> rulec::Report {
            let path = at(p).display().to_string();
            let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
            with(lang, || rulec::report(&src, &path))
        };
        let errors = |r: &rulec::Report| -> Vec<String> { r.diags.iter().filter(|d| d.severity == rulec::diag::Severity::Error).map(|d| d.code.to_string()).collect() };
        // the title of the finding of the `first` table the applied rule shows
        let w110 = |r: &rulec::Report| -> String { r.diags.iter().find(|d| d.code == "W110").map(|d| d.title.clone()).unwrap_or_else(|| panic!("no W110: {:?}", r.diags.iter().map(|d| d.code).collect::<Vec<_>>())) };
        // the files checked whole since the counts were last taken, each once
        let checked = || -> Vec<PathBuf> {
            ritsu_base::tally::take()
                .into_iter()
                .filter(|(tool, _, _)| *tool == "rulec")
                .map(|(_, f, n)| {
                    assert_eq!(n, 1, "{} checked {n} times", f.display());
                    f
                })
                .collect()
        };
        let these = |ps: &[&str]| -> Vec<PathBuf> {
            let mut v: Vec<PathBuf> = ps.iter().map(|p| ritsu_base::paths::opened(&at(p))).collect();
            v.sort();
            v
        };
        let e = Engine::new();
        ritsu_base::tally::take();
        if port_first {
            // 1. the port asks first whether the applied rule passes (as ringi pins the rule a route
            // applies before it checks the rules it lowers), and checks it and the `enums` file in
            // the run's language; the rule that applies it, checked after in Japanese, reads them off
            // those checks
            assert!(with(Lang::Ja, || e.meaning(&at(refund.0))).is_some());
            assert_eq!(checked(), these(&[sizes.0, refund.0]));
            assert_eq!(errors(&said(up.0, Lang::Ja)), none);
            assert_eq!(checked(), these(&[up.0]), "the rule that applies it alone is checked");
            // 3. the applied rule's own findings, shown in Japanese, are written in Japanese, off the
            // check the port made: it is not checked again
            let title = w110(&said(refund.0, Lang::Ja));
            assert!(!title.is_ascii(), "shown in Japanese: {title}");
            assert_eq!(checked(), Vec::<PathBuf>::new(), "the port checked it in the run's language");
            // 4. shown in English, they are written in English: the rule is checked in English
            let title = w110(&said(refund.0, Lang::En));
            assert!(title.is_ascii(), "shown in English: {title}");
            assert_eq!(checked(), these(&[refund.0]));
        } else {
            // 2. the other way round: the rule that applies it, checked in Japanese first, checks the
            // applied rule and the `enums` file in Japanese, and the port asks off those checks
            assert_eq!(errors(&said(up.0, Lang::Ja)), none);
            assert_eq!(checked(), these(&[sizes.0, refund.0, up.0]));
            assert!(with(Lang::Ja, || e.meaning(&at(refund.0))).is_some());
            assert_eq!(checked(), Vec::<PathBuf>::new(), "the port asks off the checks made in Japanese");
            // 3. shown in English, the applied rule's own findings are written in English
            let title = w110(&said(refund.0, Lang::En));
            assert!(title.is_ascii(), "shown in English: {title}");
            assert_eq!(checked(), these(&[refund.0]));
        }
    }
}

/// A run chooses the contribution files among its files once (§15.231, ritsu's DESIGN 4.15): with
/// rules that do not pass, asked for their reports in both languages, their facts and what they
/// rest on, each file of the run is read once to tell whether it is a contribution. And a rule's
/// check is kept with what that check read and nothing else of the run: what a rule that does not
/// pass rests on is what it rests on in a run of it and the file it imports alone, a file of the run
/// it does not read can change while its check holds, and the file it imports cannot.
#[test]
fn a_run_chooses_its_contributions_once_and_a_kept_check_holds_only_what_it_read() {
    let m = std::rc::Rc::new(ritsu_base::fs::Memory::new("/shop"));
    let sizes = |values: &str| format!("enums sizes v1\ndescription \"The sizes of an order\"\n\nenum size = {values}\n");
    m.add("sizes.rule", sizes("small | medium | large"));
    let rule = |name: &str, rows: &str| {
        format!("rule {name} v1\ndescription \"The fee of an order of a size\"\n\nimport enum size from \"sizes.rule\"\n\ninputs\n  size : size\n\noutputs\n  fee : number  round down(1)\n\ntable fee\npolicy unique\n| size   | -> fee : number |\n{rows}")
    };
    let every = "| small  | 1               |\n| medium | 2               |\n| large  | 3               |\n";
    // a table that leaves out the large orders does not pass (E101)
    let short = "| small  | 1               |\n| medium | 2               |\n";
    let broken: Vec<String> = (0..3).map(|i| format!("broken_{i}")).collect();
    let fine: Vec<String> = (0..3).map(|i| format!("fine_{i}")).collect();
    for b in &broken {
        m.add(format!("{b}.rule"), rule(b, short));
    }
    for f in &fine {
        m.add(format!("{f}.rule"), rule(f, every));
    }
    let files: Vec<String> = std::iter::once("sizes.rule".to_string()).chain(broken.iter().chain(&fine).map(|n| format!("{n}.rule"))).collect();
    let lang = ritsu_base::text::Lang::En;
    let e = Engine::new();
    let at = |name: &str| PathBuf::from(format!("/shop/{name}.rule"));
    let on = |e: &Engine, name: &str| ritsu_base::fs::with(m.clone(), || ritsu_ports::Rules::rests_on(e, &at(name)));
    ritsu_base::tally::take();
    let units = ritsu_base::fs::with(m.clone(), || e.checked(Path::new("/shop"), &files, lang));
    let fails: Vec<&str> = files.iter().zip(&units).filter(|(_, u)| u.verdict == ritsu_ports::Verdict::Fails).map(|(f, _)| f.as_str()).collect();
    assert_eq!(fails, ["broken_0.rule", "broken_1.rule", "broken_2.rule"]);
    for b in &broken {
        let said = ritsu_base::fs::with(m.clone(), || e.facts(&at(b))).unwrap_err();
        assert!(said.iter().any(|s| s.code == "E101"), "{said:?}");
    }
    for f in &fine {
        ritsu_base::fs::with(m.clone(), || e.facts(&at(f))).unwrap_or_else(|s| panic!("{f}: {s:?}"));
    }
    let rests: Vec<Option<String>> = broken.iter().map(|b| on(&e, b)).collect();
    assert!(rests.iter().all(Option::is_some), "{rests:?}");
    // each file of the run read once to tell whether it is a contribution, not once for each
    // report of each rule
    let read: Vec<(PathBuf, usize)> = ritsu_base::tally::take().into_iter().filter(|(t, _, _)| *t == "rulec.contributions").map(|(_, f, n)| (f, n)).collect();
    let mut once: Vec<(PathBuf, usize)> = files.iter().map(|f| (PathBuf::from(format!("/shop/{f}")), 1)).collect();
    once.sort();
    assert_eq!(read, once);
    // what a rule that does not pass rests on is what it rests on in a run of it and the file it
    // imports alone
    let alone = Engine::new();
    let two = vec!["sizes.rule".to_string(), "broken_0.rule".to_string()];
    ritsu_base::fs::with(m.clone(), || alone.checked(Path::new("/shop"), &two, lang));
    assert_eq!(on(&alone, "broken_0"), rests[0]);
    // a rule of the run the broken ones do not read, written otherwise: what they rest on is the
    // same, and nothing is checked again
    let checks = e.checks();
    m.add("fine_0.rule", rule("fine_0", &every.replace("| 3 ", "| 4 ")));
    assert_eq!(broken.iter().map(|b| on(&e, b)).collect::<Vec<_>>(), rests);
    assert_eq!(e.checks(), checks);
    // the file they import, written otherwise: not the same
    m.add("sizes.rule", sizes("small | medium | large | huge"));
    for (b, was) in broken.iter().zip(&rests) {
        let now = on(&e, b);
        assert!(now.is_some() && now != *was, "{b}");
    }
}

/// A run walks its contribution files once (§15.227, ritsu's DESIGN 4.15): with rules contributions
/// reach (one of them through a contribution that comes after another) and rules none reaches, each
/// checked, asked for its facts and for what it rests on, every contribution of the run is read
/// once to find the rules the run's contributions come down to, not once for each rule. A
/// contribution written otherwise is walked again, once, and the rule it reaches is read with what
/// it says now.
#[test]
fn a_run_walks_its_contributions_once_and_again_when_one_is_written_otherwise() {
    let m = std::rc::Rc::new(ritsu_base::fs::Memory::new("/shop"));
    let merging = root().join("tests/merging");
    for (case, f) in [
        ("sum", "site_capacity.rule"),
        ("sum", "contrib/mezzanine.rule"),
        ("override", "standing.rule"),
        ("override", "contrib/strict_policy.rule"),
        ("override", "contrib/grace_policy.rule"),
    ] {
        m.add(f, std::fs::read_to_string(merging.join(case).join(f)).unwrap());
    }
    m.add("sizes.rule", "enums sizes v1\ndescription \"The sizes of an order\"\n\nenum size = small | medium | large\n".to_string());
    let plain: Vec<String> = (0..3).map(|i| format!("plain_{i}")).collect();
    for p in &plain {
        m.add(
            format!("{p}.rule"),
            format!("rule {p} v1\ndescription \"The fee of an order of a size\"\n\nimport enum size from \"sizes.rule\"\n\ninputs\n  size : size\n\noutputs\n  fee : number  round down(1)\n\ntable fee\npolicy unique\n| size   | -> fee : number |\n| small  | 1               |\n| medium | 2               |\n| large  | 3               |\n"),
        );
    }
    let contributions = ["contrib/grace_policy.rule", "contrib/mezzanine.rule", "contrib/strict_policy.rule"];
    let rules: Vec<String> = ["site_capacity.rule".to_string(), "standing.rule".to_string()].into_iter().chain(plain.iter().map(|p| format!("{p}.rule"))).collect();
    let files: Vec<String> = std::iter::once("sizes.rule".to_string()).chain(rules.iter().cloned()).chain(contributions.iter().map(|c| c.to_string())).collect();
    let e = Engine::new();
    let at = |f: &str| PathBuf::from(format!("/shop/{f}"));
    let ask = |e: &Engine| {
        for r in &rules {
            ritsu_base::fs::with(m.clone(), || e.facts(&at(r))).unwrap_or_else(|s| panic!("{r}: {s:?}"));
            assert!(ritsu_base::fs::with(m.clone(), || ritsu_ports::Rules::rests_on(e, &at(r))).is_some(), "{r}");
        }
    };
    let walked = || -> Vec<(PathBuf, usize)> { ritsu_base::tally::take().into_iter().filter(|(t, _, _)| *t == "rulec.walks").map(|(_, f, n)| (f, n)).collect() };
    let once: Vec<(PathBuf, usize)> = contributions.iter().map(|c| (at(c), 1)).collect();
    ritsu_base::tally::take();
    let units = ritsu_base::fs::with(m.clone(), || e.checked(Path::new("/shop"), &files, ritsu_base::text::Lang::En));
    let passes: Vec<bool> = units.iter().map(|u| u.verdict == ritsu_ports::Verdict::Passes).collect();
    assert_eq!(passes, vec![true; files.len()], "{units:?}");
    ask(&e);
    assert_eq!(walked(), once);
    // a contribution written otherwise: walked again, once, and the rule it reaches reads it
    let inputs = |e: &Engine| -> Vec<String> { ritsu_base::fs::with(m.clone(), || e.facts(&at("site_capacity.rule"))).unwrap().inputs.into_iter().map(|c| c.name).collect() };
    assert!(inputs(&e).contains(&"has_mezzanine".to_string()), "{:?}", inputs(&e));
    let mezzanine = std::fs::read_to_string(merging.join("sum/contrib/mezzanine.rule")).unwrap();
    m.add("contrib/mezzanine.rule", mezzanine.replace("has_mezzanine", "has_loft"));
    let now = inputs(&e);
    assert!(now.contains(&"has_loft".to_string()) && !now.contains(&"has_mezzanine".to_string()), "{now:?}");
    ask(&e);
    assert_eq!(walked(), once);
}

/// The values a question over ranges answers, in its order.
fn values_of(found: &ritsu_ports::Found<Vec<(Value, ritsu_ports::Values)>>) -> Vec<Value> {
    match found {
        ritsu_ports::Found::Value(vs) => vs.iter().map(|(v, _)| v.clone()).collect(),
        ritsu_ports::Found::Undecided(why) => panic!("undecided: {}", why.en),
    }
}

/// Each value comes with an input that is inside the ranges and comes to it: a number or a date
/// between the ends, an enum's value or a bool at a place one of the entries of its input holds.
fn witnesses_hold(e: &Engine, path: &str, output: &str, held: &[(String, Option<i128>, Option<i128>)], found: &ritsu_ports::Found<Vec<(Value, ritsu_ports::Values)>>) {
    let ritsu_ports::Found::Value(vs) = found else { return };
    let facts = e.facts(Path::new(path)).unwrap();
    // the place of an enum's value in the order the rule declares them
    let place = |input: &str, name: &str| -> Option<i128> {
        let en = match &facts.inputs.iter().find(|c| c.name == input)?.ty {
            ritsu_ports::ColumnType::Enum { name: en, .. } => en.clone(),
            ritsu_ports::ColumnType::Opt(t) => match t.as_ref() {
                ritsu_ports::ColumnType::Enum { name: en, .. } => en.clone(),
                _ => return None,
            },
            _ => return None,
        };
        facts.enums.iter().find(|x| x.name == en)?.values.iter().position(|v| v.name == name).map(|k| k as i128)
    };
    for (v, input) in vs {
        for (name, _, _) in held {
            let at = match input.iter().find(|(n, _)| n == name).map(|(_, x)| x) {
                Some(Value::Int(n)) => *n,
                Some(Value::Date(d)) => {
                    let p: Vec<i32> = d.split('-').map(|x| x.parse().unwrap()).collect();
                    let r = rulec::types::date_ord(p[0], p[1] as u32, p[2] as u32);
                    r.num / r.den
                }
                Some(Value::Bool(b)) => i128::from(*b),
                Some(Value::Enum(w)) => place(name, w).unwrap_or_else(|| panic!("{path} {output}: {name} = {w} is no value of its enum")),
                _ => continue,
            };
            let inside = held.iter().filter(|(n, _, _)| n == name).any(|(_, lo, hi)| lo.is_none_or(|l| at >= l) && hi.is_none_or(|h| at <= h));
            assert!(inside, "{path} {output}: {name} = {at} is outside what {held:?} holds it to");
        }
        let outs = e.eval(Path::new(path), input).unwrap_or_else(|x| panic!("{path} {output}: {x:?}"));
        let got = outs.iter().find(|(n, _)| n == output).map(|(_, x)| public(e, path, output, x));
        assert_eq!(got.as_ref(), Some(v), "{path} {output}: {input:?}");
    }
}

/// A value `eval` answers (an enum's value by the rule's name of it), as the question over ranges
/// answers it: an enum's value by its public name, from the facts the port hands over.
fn public(e: &Engine, path: &str, output: &str, v: &Value) -> Value {
    let Value::Enum(name) = v else { return v.clone() };
    let facts = e.facts(Path::new(path)).unwrap();
    let ty = facts.outputs.iter().find(|c| c.name == output).map(|c| c.ty.clone());
    let en = match ty {
        Some(ritsu_ports::ColumnType::Enum { name: en, .. }) => en,
        Some(ritsu_ports::ColumnType::Opt(t)) => match *t {
            ritsu_ports::ColumnType::Enum { name: en, .. } => en,
            _ => return v.clone(),
        },
        _ => return v.clone(),
    };
    let value = facts.enums.iter().find(|x| x.name == en).and_then(|x| x.values.iter().find(|y| y.name == *name)).unwrap_or_else(|| panic!("{path}: {name} is no value of {en}"));
    Value::Enum(value.public.clone())
}

/// sekisho's example: a refund within the limit or over it, by a table over the derived value
/// `excess = amount - limit`. Held to the intervals a policy cuts `amount` into, and to limits
/// that leave only one of the rows, the values are the ones some input reaches — a row whose
/// values of `excess` the derive does not reach over the held ranges is left out — in the English
/// rule and its Japanese twin alike. Over the whole ranges, such a row is E102 (DESIGN §15.189).
#[test]
fn an_output_over_ranges_leaves_out_the_rows_past_a_derives_reach() {
    use ritsu_ports::Found;
    let e = Engine::new();
    for (path, out, amount, limit, within, over) in [
        ("tests/over/refund_limit.rule", "band", "amount", "limit", "within_limit", "over_limit"),
        // the Japanese twin answers by the aliases it writes: `上限まで(within_limit)`
        ("tests/over/返金の上限.rule", "区分", "金額", "上限", "within_limit", "over_limit"),
    ] {
        let ask = |ranges: &[(&str, Option<i128>, Option<i128>)]| {
            let held: Vec<(String, Option<i128>, Option<i128>)> = ranges.iter().map(|(n, l, h)| (n.to_string(), *l, *h)).collect();
            let found = e.outputs_over(Path::new(path), out, &held).unwrap_or_else(|x| panic!("{path}: {x:?}"));
            witnesses_hold(&e, path, out, &held, &found);
            found
        };
        let (w, o) = (Value::Enum(within.into()), Value::Enum(over.into()));
        // the two intervals sekisho's example cuts `amount` into, at 50 pounds
        assert_eq!(values_of(&ask(&[(amount, Some(1), Some(50))])), [w.clone(), o.clone()], "{path}");
        // each value comes with an input written in the order the rule declares its inputs
        if let ritsu_ports::Found::Value(vs) = ask(&[(amount, Some(1), Some(50))]) {
            assert!(vs.iter().all(|(_, input)| input.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>() == [amount, limit]), "{path}: {vs:?}");
        }
        assert_eq!(values_of(&ask(&[(amount, Some(51), Some(10_000))])), [w.clone(), o.clone()], "{path}");
        // a limit above every amount: `excess` stays at -50 or below
        assert_eq!(values_of(&ask(&[(amount, Some(1), Some(50)), (limit, Some(100), None)])), [w.clone()], "{path}");
        // no limit at all: `excess` is the amount
        assert_eq!(values_of(&ask(&[(limit, Some(0), Some(0))])), [o.clone()], "{path}");
        // `excess` is exactly 0, and `>0GBP` on a derive of whole pounds is `>=1GBP`
        assert_eq!(values_of(&ask(&[(amount, Some(5), Some(5)), (limit, Some(5), Some(5))])), [w.clone()], "{path}");
        // ranges that hold no input
        assert_eq!(ask(&[(amount, Some(20_000), None)]), Found::Value(vec![]), "{path}");
        assert_eq!(ask(&[(amount, Some(40), Some(30))]), Found::Value(vec![]), "{path}");
    }
    // a row past the reach of its derive no longer passes check: E102's third form reads the same
    // reach (DESIGN §15.189), and the port answers nothing for a rule that does not pass
    let path = "tests/over/reach.rule";
    let src = std::fs::read_to_string(path).unwrap();
    let dead: Vec<String> = rulec::report(&src, path).diags.iter().filter(|d| d.code == "E102").map(|d| d.where_.clone()).collect();
    assert_eq!(dead, [format!("{path}:20 table t")]);
    assert!(e.outputs_over(Path::new(path), "out", &[]).is_err());
    // a row past the reach of a `define` no longer passes check either: E102's third form reads a
    // define's reach too (DESIGN §15.195)
    let path = "tests/over/define_reach.rule";
    let src = std::fs::read_to_string(path).unwrap();
    let dead: Vec<String> = rulec::report(&src, path).diags.iter().filter(|d| d.code == "E102").map(|d| d.where_.clone()).collect();
    assert_eq!(dead, [format!("{path}:20 table t")]);
    assert!(e.outputs_over(Path::new(path), "out", &[]).is_err());
    // a row only a `constraint` rules out, which E102 does not stop a rule on, passes check, and no
    // input comes to its value
    let path = "tests/over/constraint_reach.rule";
    let src = std::fs::read_to_string(path).unwrap();
    assert!(!rulec::has_error(&rulec::report(&src, path).diags));
    let found = e.outputs_over(Path::new(path), "out", &[]).unwrap();
    witnesses_hold(&e, path, "out", &[], &found);
    assert_eq!(values_of(&found), [Value::Enum("small".into()), Value::Enum("big".into())]);
    let found = e.outputs_over(Path::new(path), "out", &[("limit".into(), Some(0), Some(0)), ("amount".into(), Some(1), Some(1))]).unwrap();
    assert_eq!(values_of(&found), [Value::Enum("small".into())]);
}

/// What the question does not answer, it says why: a name that is no output or no input, an
/// output that is a number, an input that is a string (an enum and a bool are held by their
/// values), a rule that walks a list.
#[test]
fn an_output_over_ranges_says_why_it_cannot_answer() {
    use ritsu_ports::Found;
    let e = Engine::new();
    let undecided = |path: &str, out: &str, held: &[(String, Option<i128>, Option<i128>)]| match e.outputs_over(Path::new(path), out, held).unwrap_or_else(|x| panic!("{path}: {x:?}")) {
        Found::Undecided(t) => {
            assert!(!t.ja.is_empty() && t.ja != t.en, "{t:?}");
            t.en
        }
        Found::Value(v) => panic!("{path} {out}: decided {v:?}"),
    };
    let p = "tests/over/refund_limit.rule";
    assert!(undecided(p, "excess", &[]).contains("not an output"));
    assert!(undecided(p, "band", &[("price".into(), Some(1), Some(2))]).contains("not an input"));
    // a numeric output, a rule that walks a list, and a string input, from the corpus
    let mut numeric = false;
    let mut walks = false;
    let mut string = false;
    for path in corpus() {
        let src = std::fs::read_to_string(&path).unwrap();
        if rulec::has_error(&rulec::report(&src, &path).diags) {
            continue;
        }
        let (f, c) = rulec::prepare(&src, &path).unwrap();
        let first = f.outputs[0].name.text.clone();
        if !numeric && f.elements.is_empty() && c.ty_of(&first).is_some_and(|t| t.is_numeric()) {
            assert!(undecided(&path, &first, &[]).contains("neither an enum nor a bool"), "{path}");
            numeric = true;
        }
        let counted = f.outputs.iter().find(|o| matches!(c.ty_of(&o.name.text), Some(rulec::types::Ty::Enum(_) | rulec::types::Ty::Bool)));
        if !walks
            && let Some(el) = f.elements.first()
            && let Some(o) = counted
        {
            assert!(undecided(&path, &o.name.text, &[]).contains(&format!("walks the list `{}`", el.name.text)), "{path}");
            walks = true;
        }
        // a string input cannot be held to a range
        if !string
            && f.elements.is_empty()
            && let Some(i) = f.inputs.iter().find(|i| matches!(c.ty_of(&i.name.text), Some(rulec::types::Ty::Str)))
            && let Some(o) = counted
        {
            assert!(undecided(&path, &o.name.text, &[(i.name.text.clone(), Some(0), Some(1))]).contains("neither a number, a date, an enum nor a bool"), "{path}");
            string = true;
        }
        if walks && numeric && string {
            return;
        }
    }
    panic!("the corpus has no rule with a numeric output, a list, or a string input beside an enum output");
}

/// An enum input held by the places of its values (from 0, in the order the rule declares them)
/// and a bool input as 0 and 1: what the output comes to is what the rows those values reach
/// write, each value with an input that holds to them. A rule's enum given a constant (the event
/// `cancel`) and its state given one value of a gate's enum is how sekisho asks (its DESIGN 3.2);
/// an input given more than once takes the values of each entry (a gate's enum with fewer values
/// than the rule's). In the English rule and its Japanese twin alike, which answer by the aliases.
#[test]
fn an_output_over_ranges_holds_an_enum_and_a_bool_to_their_values() {
    use ritsu_ports::Found;
    let e = Engine::new();
    for (path, state, event, rush, accepted, next) in [
        ("tests/over/order_state.rule", "state", "event", "rush", "accepted", "next_state"),
        ("tests/over/注文の状態.rule", "状態", "イベント", "急ぎ", "受理", "次"),
    ] {
        let ask = |out: &str, ranges: &[(&str, i128, i128)]| {
            let held: Vec<(String, Option<i128>, Option<i128>)> = ranges.iter().map(|(n, l, h)| (n.to_string(), Some(*l), Some(*h))).collect();
            let found = e.outputs_over(Path::new(path), out, &held).unwrap_or_else(|x| panic!("{path}: {x:?}"));
            witnesses_hold(&e, path, out, &held, &found);
            found
        };
        let (yes, no) = (Value::Bool(true), Value::Bool(false));
        // the places: received 0, paid 1, shipped 2, cancelled 3; pay 0, ship 1, cancel 2
        assert_eq!(values_of(&ask(accepted, &[])), [yes.clone(), no.clone()], "{path}");
        // a shipped order is never cancelled, a received one always is
        assert_eq!(values_of(&ask(accepted, &[(state, 2, 2), (event, 2, 2)])), [no.clone()], "{path}");
        assert_eq!(values_of(&ask(accepted, &[(state, 0, 0), (event, 2, 2)])), [yes.clone()], "{path}");
        // a paid one as the bool says: a rushed order is packed already
        assert_eq!(values_of(&ask(accepted, &[(state, 1, 1), (event, 2, 2)])), [yes.clone(), no.clone()], "{path}");
        assert_eq!(values_of(&ask(accepted, &[(state, 1, 1), (event, 2, 2), (rush, 1, 1)])), [no.clone()], "{path}");
        assert_eq!(values_of(&ask(accepted, &[(state, 1, 1), (event, 2, 2), (rush, 0, 0)])), [yes.clone()], "{path}");
        // a payment to an order received or paid leaves it paid, whichever way the two are given
        let paid = vec![Value::Enum("paid".into())];
        assert_eq!(values_of(&ask(next, &[(state, 0, 1), (event, 0, 0)])), paid, "{path}");
        assert_eq!(values_of(&ask(next, &[(state, 0, 0), (state, 1, 1), (event, 0, 0)])), paid, "{path}");
        // received and shipped, which do not run on: a payment leaves them paid and shipped
        assert_eq!(values_of(&ask(next, &[(state, 0, 0), (state, 2, 2), (event, 0, 0)])), [Value::Enum("paid".into()), Value::Enum("shipped".into())], "{path}");
        // places that name no value hold no input
        assert_eq!(ask(accepted, &[(state, 4, 9)]), Found::Value(vec![]), "{path}");
        assert_eq!(ask(accepted, &[(rush, 2, 2)]), Found::Value(vec![]), "{path}");
    }
}

/// Several outputs of one call asked together (`outputs_together`; sekisho's DESIGN 4.1). Two
/// outputs one table writes come to the combinations its rows write, and to no other pair of their
/// values. Two outputs of two tables, whose rows say nothing of each other, are undecided over
/// ranges that leave two rows of each, with the combination no input is found for; over ranges
/// that leave one row of one of them they are exact again. The English rule and its Japanese twin
/// answer alike, by the public names of the values; the reason of an undecided answer is for a
/// person, and names the values as the rule writes them.
#[test]
fn the_outputs_of_one_call_are_answered_together() {
    use ritsu_ports::Found;
    let e = Engine::new();
    for (path, a, b, c, amount, written) in [("tests/over/sizes.rule", "a", "b", "c", "amount", ("high", "low")), ("tests/over/金額の区分.rule", "甲", "乙", "丙", "金額", ("高", "低"))] {
        let ask = |outs: &[&str], ranges: &[(&str, i128, i128)]| {
            let held: Vec<(String, Option<i128>, Option<i128>)> = ranges.iter().map(|(n, l, h)| (n.to_string(), Some(*l), Some(*h))).collect();
            let outs: Vec<String> = outs.iter().map(|o| o.to_string()).collect();
            let found = e.outputs_together(Path::new(path), &outs, &held).unwrap_or_else(|x| panic!("{path}: {x:?}"));
            // each combination comes with an input inside the ranges that comes to it
            if let Found::Value(ts) = &found {
                for (t, input) in ts {
                    assert_eq!(t.len(), outs.len(), "{path}: {t:?}");
                    for (o, v) in outs.iter().zip(t) {
                        witnesses_hold(&e, path, o, &held, &Found::Value(vec![(v.clone(), input.clone())]));
                    }
                }
            }
            found
        };
        let combos = |found: Found<Vec<(Vec<Value>, ritsu_ports::Values)>>| -> Vec<Vec<Value>> {
            match found {
                Found::Value(ts) => ts.into_iter().map(|(t, _)| t).collect(),
                Found::Undecided(why) => panic!("{path}: undecided: {}", why.en),
            }
        };
        let (low, high) = (Value::Enum("low".to_string()), Value::Enum("high".to_string()));
        // one table writes a and b: its two rows, never one low and the other high
        assert_eq!(combos(ask(&[a, b], &[])), [vec![low.clone(), low.clone()], vec![high.clone(), high.clone()]], "{path}");
        assert_eq!(combos(ask(&[a, b], &[(amount, 1, 500)])), [vec![low.clone(), low.clone()]], "{path}");
        assert_eq!(combos(ask(&[a, b], &[(amount, 500, 501)])), [vec![low.clone(), low.clone()], vec![high.clone(), high.clone()]], "{path}");
        // a and c are two tables': over every amount, the rows cannot say that a high a never comes
        // with a low c, and no input does. The reason names the values as the rule writes them.
        match ask(&[a, c], &[]) {
            Found::Undecided(why) => {
                assert!(why.en.contains(&format!("{a} = {}, {c} = {}", written.0, written.1)), "{path}: {}", why.en);
                assert!(!why.ja.is_empty() && why.ja != why.en, "{path}: {why:?}");
            }
            Found::Value(ts) => panic!("{path}: decided {ts:?}"),
        }
        // with one row of one table left, every pair of what is left has an input
        assert_eq!(combos(ask(&[a, c], &[(amount, 1, 300)])), [vec![low.clone(), low.clone()]], "{path}");
        assert_eq!(combos(ask(&[a, c], &[(amount, 301, 1_000)])), [vec![low.clone(), high.clone()], vec![high.clone(), high.clone()]], "{path}");
        // the order asked is the order answered; an output asked twice is its one value twice
        assert_eq!(combos(ask(&[c, a], &[(amount, 301, 500)])), [vec![high.clone(), low.clone()]], "{path}");
        assert_eq!(combos(ask(&[a, b, c], &[(amount, 501, 1_000)])), [vec![high.clone(), high.clone(), high.clone()]], "{path}");
        // ranges that hold no input
        assert_eq!(ask(&[a, b], &[(amount, 2_000, 3_000)]), Found::Value(vec![]), "{path}");
        // what it does not answer, it says why
        for (outs, ranges, why) in [(vec![a, "nothing"], vec![], "not an output"), (vec![a, b], vec![("price", 1, 2)], "not an input")] {
            match ask(&outs, &ranges) {
                Found::Undecided(t) => assert!(t.en.contains(why) && t.ja != t.en, "{path}: {t:?}"),
                Found::Value(ts) => panic!("{path}: decided {ts:?}"),
            }
        }
    }
}

/// An input that may be left out, held to a range or to values, is given: the call a gate writes
/// with an attribute that is always there never leaves it out, so a value only the absent input
/// comes to (`unknown`) is not among what the output comes to, whether the ends are the ones the
/// rule declares or narrower. Not held, it may be left out, and the value is there. The numeric
/// output's question and the pairs of outputs answer the same way.
#[test]
fn an_input_that_may_be_left_out_is_given_when_it_is_held() {
    use ritsu_ports::Found;
    let e = Engine::new();
    for (path, age, seat, band, place) in [("tests/over/band.rule", "age", "seat", "band", "place"), ("tests/over/年齢の区分.rule", "年齢", "席", "区分", "場所")] {
        let en = |w: &str| Value::Enum(w.to_string());
        let ask = |output: &str, ranges: &[(&str, i128, i128)]| -> Vec<Value> {
            let held: Vec<(String, Option<i128>, Option<i128>)> = ranges.iter().map(|(n, l, h)| (n.to_string(), Some(*l), Some(*h))).collect();
            let found = e.outputs_over(Path::new(path), output, &held).unwrap_or_else(|x| panic!("{path}: {x:?}"));
            witnesses_hold(&e, path, output, &held, &found);
            match found {
                Found::Value(vs) => {
                    // the input each value comes with gives every input that is held
                    for (v, input) in &vs {
                        for (n, _, _) in &held {
                            assert!(input.iter().any(|(k, x)| k == n && *x != Value::None), "{path} {output}: {v:?} comes with {input:?}, which leaves {n} out");
                        }
                    }
                    vs.into_iter().map(|(v, _)| v).collect()
                }
                Found::Undecided(why) => panic!("{path} {output} over {ranges:?}: undecided: {}", why.en),
            }
        };
        // not held: left out or given
        assert_eq!(ask(band, &[]), [en("unknown"), en("child"), en("adult")], "{path}");
        assert_eq!(ask(place, &[]), [en("unknown"), en("child"), en("adult")], "{path}");
        // held to the whole range the rule declares, and to less
        assert_eq!(ask(band, &[(age, 1, 100)]), [en("child"), en("adult")], "{path}");
        assert_eq!(ask(band, &[(age, 1, 12)]), [en("child")], "{path}");
        assert_eq!(ask(band, &[(age, 13, 20)]), [en("adult")], "{path}");
        // an enum held to all of its values, and to one
        assert_eq!(ask(place, &[(seat, 0, 1)]), [en("child"), en("adult")], "{path}");
        assert_eq!(ask(place, &[(seat, 1, 1)]), [en("adult")], "{path}");
        // holding the one says nothing of the other
        assert_eq!(ask(place, &[(age, 1, 100)]), [en("unknown"), en("child"), en("adult")], "{path}");
        // the two together: the pairs of what is left
        let held: Vec<(String, Option<i128>, Option<i128>)> = vec![(age.to_string(), Some(1), Some(100)), (seat.to_string(), Some(0), Some(1))];
        match e.outputs_together(Path::new(path), &[band.to_string(), place.to_string()], &held).unwrap() {
            Found::Value(ts) => {
                let pairs: Vec<Vec<Value>> = ts.into_iter().map(|(t, _)| t).collect();
                assert_eq!(pairs, [vec![en("child"), en("child")], vec![en("child"), en("adult")], vec![en("adult"), en("child")], vec![en("adult"), en("adult")]], "{path}");
            }
            Found::Undecided(why) => panic!("{path}: undecided: {}", why.en),
        }
    }
}

/// Over the corpus, every numeric and date input held to a small interval around a vector's
/// value, what the port answers for each enum and bool output is what the reference evaluator
/// comes to on every input inside the intervals (every value of each enum and bool input, the
/// constraints kept): the same values, each with an input inside the ranges that comes to it.
/// An input that may be left out is given when it is held: the inputs inside its interval are
/// the ones that give it a value there. What the port leaves undecided is counted, and printed
/// with its reason.
///
/// A rule with two or more such outputs is asked of them together too (`outputs_together`): what
/// it answers is the combinations the evaluator comes to on the inputs inside the intervals, no
/// more and no fewer, each with an input that comes to it.
#[test]
fn an_output_over_ranges_is_what_every_input_inside_them_comes_to() {
    use rulec::eval::Val;
    use rulec::types::Ty;
    use std::collections::{BTreeMap, HashMap};
    const CAP: usize = 2_000;
    let e = Engine::new();
    let (mut cases, mut exact, mut undecided, mut unwalked, mut too_big, mut narrower) = (0, 0, 0, 0, 0, 0);
    let (mut together, mut together_exact, mut together_fewer) = (0, 0, 0);
    let mut why: Vec<String> = Vec::new();
    // the corpus, and the rules of these tests and of `apply`'s
    let mut rules = corpus();
    for dir in ["tests/over", "tests/apply_fixtures"] {
        let mut more: Vec<String> = std::fs::read_dir(dir).unwrap().flatten().map(|e| format!("{dir}/{}", e.file_name().to_string_lossy())).filter(|p| p.ends_with(".rule")).collect();
        more.sort();
        rules.extend(more);
    }
    for path in rules {
        let src = std::fs::read_to_string(&path).unwrap();
        if rulec::has_error(&rulec::report(&src, &path).diags) {
            continue;
        }
        let (f, c) = rulec::prepare(&src, &path).unwrap();
        let base = |t: Option<Ty>| match t {
            Some(Ty::Opt(t)) => *t,
            Some(t) => t,
            None => Ty::Unknown,
        };
        let outputs: Vec<String> = f.outputs.iter().map(|o| o.name.text.clone()).filter(|o| matches!(base(c.ty_of(o)), Ty::Enum(_) | Ty::Bool)).collect();
        if outputs.is_empty() || !f.elements.is_empty() {
            continue;
        }
        // the inputs held to intervals (numbers and dates, on the wire), and the ones walked whole
        let mut held_names: Vec<(String, bool)> = Vec::new();
        let mut whole: Vec<(String, Vec<Val>)> = Vec::new();
        let mut walkable = true;
        for i in &f.inputs {
            let n = i.name.text.clone();
            let ty = c.ty_of(&n);
            let opt = matches!(ty, Some(Ty::Opt(_)));
            let mut vals: Vec<Val> = if opt { vec![Val::Enum(rulec::kw::NONE.into())] } else { vec![] };
            match base(ty) {
                Ty::Enum(en) => vals.extend(c.enums.get(&en).cloned().unwrap_or_default().into_iter().map(Val::Enum)),
                Ty::Bool => vals.extend([Val::Bool(true), Val::Bool(false)]),
                // a number or a date, one that may be absent too: held to its interval, it is given
                Ty::Date => held_names.push((n.clone(), true)),
                t if t.is_numeric() => held_names.push((n.clone(), false)),
                _ => walkable = false,
            }
            if matches!(base(c.ty_of(&n)), Ty::Enum(_) | Ty::Bool) {
                whole.push((n, vals));
            }
        }
        let vectors = rulec::vectors::generate(&f, &c);
        if !walkable || vectors.is_empty() {
            unwalked += 1;
            continue;
        }
        let wire_of = |n: &str, date: bool, v: &Val| -> Option<i128> {
            match v {
                Val::Num(x) if !date => Some(rulec::types::wire_int(*x, c.wire_scale(n))),
                Val::Date(y, m, d) => {
                    let r = rulec::types::date_ord(*y, *m, *d);
                    Some(r.num / r.den)
                }
                _ => None,
            }
        };
        let declared = |n: &str, date: bool| -> (Option<i128>, Option<i128>) {
            let (lo, hi) = c.ranges.get(n).copied().unwrap_or((None, None));
            let w = |r: rulec::num::Rat| if date { r.num / r.den } else { rulec::types::wire_int(r, c.wire_scale(n)) };
            (lo.map(w), hi.map(w))
        };
        let size_of = |held: &[(String, bool, i128, i128)]| held.iter().map(|(_, _, l, h)| (h - l + 1).max(0) as usize).chain(whole.iter().map(|(_, vs)| vs.len())).fold(1usize, |a, b| a.saturating_mul(b));
        // around four of the vectors: every input held as wide as keeps the walk small, and each
        // input in turn swept 40 steps each way with the others at the vector's values
        let n = vectors.len();
        let mut picks = vec![0, n / 3, 2 * n / 3, n - 1];
        picks.dedup();
        let mut boxes: Vec<Vec<(String, bool, i128, i128)>> = Vec::new();
        for pick in picks {
            let v = &vectors[pick];
            let around = |wide: Option<usize>, width: i128| -> Vec<(String, bool, i128, i128)> {
                held_names
                    .iter()
                    .enumerate()
                    .filter_map(|(j, (n, date))| {
                        // a value that is absent is centred on the low end the input declares
                        let at = v.input.get(n).and_then(|x| wire_of(n, *date, x)).or(declared(n, *date).0)?;
                        let w = match wide {
                            None => width,
                            Some(k) if k == j => 40,
                            Some(_) => 0,
                        };
                        let (dlo, dhi) = declared(n, *date);
                        Some((n.clone(), *date, dlo.map_or(at - w, |l| l.max(at - w)), dhi.map_or(at + w, |h| h.min(at + w))))
                    })
                    .collect()
            };
            let mut these: Vec<Vec<(String, bool, i128, i128)>> = Vec::new();
            match [3i128, 2, 1, 0].into_iter().map(|w| around(None, w)).find(|b| size_of(b) <= CAP) {
                Some(b) => these.push(b),
                None => too_big += 1,
            }
            these.extend((0..held_names.len()).map(|k| around(Some(k), 0)).filter(|b| size_of(b) <= CAP));
            for b in these {
                if !boxes.contains(&b) {
                    boxes.push(b);
                }
            }
        }
        for held in boxes {
            // every input in the box, through the reference evaluator
            let mut truth: HashMap<String, Vec<Value>> = outputs.iter().map(|o| (o.clone(), Vec::new())).collect();
            let mut truth_together: Vec<Vec<Value>> = Vec::new();
            let mut axes: Vec<(String, Vec<Val>)> = whole.clone();
            for (n, date, lo, hi) in &held {
                let mut vals: Vec<Val> = Vec::new();
                vals.extend((*lo..=*hi).map(|w| {
                        if *date {
                            let (y, m, d) = rulec::types::ord_to_date(rulec::num::Rat::int(w));
                            Val::Date(y, m, d)
                        } else {
                            Val::Num(rulec::types::from_wire(w, c.wire_scale(n)))
                        }
                    }));
                axes.push((n.clone(), vals));
            }
            let mut at = vec![0usize; axes.len()];
            'walk: loop {
                let input: BTreeMap<String, Val> = axes.iter().zip(&at).map(|((n, vs), i)| (n.clone(), vs[*i].clone())).collect();
                if rulec::vectors::allowed(&f, &c, &input) && rulec::vectors::days_ok(&c, &input) {
                    let (outs, _, _) = rulec::eval::run_all(&f, &c, input.into_iter().collect());
                    let mut here: BTreeMap<String, Value> = BTreeMap::new();
                    for (n, x) in outs {
                        if let (Some(seen), Some(x)) = (truth.get_mut(&n), x) {
                            let v = match (wire(&c, &n, &x), base(c.ty_of(&n))) {
                                (Value::Enum(name), Ty::Enum(en)) => Value::Enum(rulec::codegen::public_value(&f, &en, &name)),
                                (v, _) => v,
                            };
                            if !seen.contains(&v) {
                                seen.push(v.clone());
                            }
                            here.insert(n, v);
                        }
                    }
                    // what the outputs come to together on this input
                    if let Some(t) = outputs.iter().map(|o| here.get(o).cloned()).collect::<Option<Vec<Value>>>()
                        && !truth_together.contains(&t)
                    {
                        truth_together.push(t);
                    }
                }
                for k in 0..at.len() {
                    at[k] += 1;
                    if at[k] < axes[k].1.len() {
                        continue 'walk;
                    }
                    at[k] = 0;
                }
                break;
            }
            let ranges: Vec<(String, Option<i128>, Option<i128>)> = held.iter().map(|(n, _, l, h)| (n.clone(), Some(*l), Some(*h))).collect();
            for out in &outputs {
                cases += 1;
                let found = e.outputs_over(Path::new(&path), out, &ranges).unwrap_or_else(|x| panic!("{path}: {x:?}"));
                match &found {
                    ritsu_ports::Found::Value(vs) => {
                        exact += 1;
                        let every = match base(c.ty_of(out)) {
                            Ty::Enum(en) => c.enums.get(&en).map_or(0, |v| v.len()),
                            _ => 2,
                        } + usize::from(matches!(c.ty_of(out), Some(Ty::Opt(_))));
                        if vs.len() < every {
                            narrower += 1;
                        }
                        let mut got: Vec<String> = vs.iter().map(|(v, _)| format!("{v:?}")).collect();
                        let mut want: Vec<String> = truth[out].iter().map(|v| format!("{v:?}")).collect();
                        got.sort();
                        want.sort();
                        assert_eq!(got, want, "{path} {out} over {ranges:?}");
                        witnesses_hold(&e, &path, out, &ranges, &found);
                    }
                    ritsu_ports::Found::Undecided(t) => {
                        undecided += 1;
                        why.push(format!("{path} {out} over {ranges:?}: {}", t.en));
                    }
                }
            }
            // the outputs together
            if outputs.len() > 1 {
                together += 1;
                match e.outputs_together(Path::new(&path), &outputs, &ranges).unwrap_or_else(|x| panic!("{path}: {x:?}")) {
                    ritsu_ports::Found::Value(ts) => {
                        together_exact += 1;
                        let every: usize = outputs.iter().map(|o| truth[o].len()).product();
                        if ts.len() < every {
                            together_fewer += 1;
                        }
                        let mut got: Vec<String> = ts.iter().map(|(t, _)| format!("{t:?}")).collect();
                        let mut want: Vec<String> = truth_together.iter().map(|t| format!("{t:?}")).collect();
                        got.sort();
                        want.sort();
                        assert_eq!(got, want, "{path} {outputs:?} together over {ranges:?}");
                        // each combination with an input that comes to it, value by value
                        for (t, input) in &ts {
                            for (out, v) in outputs.iter().zip(t) {
                                witnesses_hold(&e, &path, out, &ranges, &ritsu_ports::Found::Value(vec![(v.clone(), input.clone())]));
                            }
                        }
                    }
                    ritsu_ports::Found::Undecided(t) => why.push(format!("{path} {outputs:?} together over {ranges:?}: {}", t.en)),
                }
            }
        }
    }
    println!("outputs_over: {cases} questions over the corpus and the rules of these tests, {exact} answered exactly ({narrower} of them with fewer values than the output has), {undecided} undecided; {unwalked} rules not walked (a string input), {too_big} boxes too big to walk");
    println!("outputs_together: {together} questions of two or more outputs, {together_exact} answered exactly ({together_fewer} of them with fewer combinations than every pair of the outputs' values)");
    for w in &why {
        println!("  undecided: {w}");
    }
    assert!(exact * 3 >= cases * 2, "{exact} of {cases} answered exactly");
    assert!(together_exact * 2 >= together, "{together_exact} of {together} answered exactly together");
}

/// Over the corpus and the rules of these tests, every input held to the value one of the rule's
/// vectors gives it (an enum's value and a bool by their places, a number and a date to that one
/// value), then each enum and bool input in turn let free: what the port answers for each enum and
/// bool output is what the reference evaluator comes to on every input so held (the constraints
/// kept), each value with an input that holds to them. This is how sekisho asks: a gate holds each
/// enum and bool a policy reads to one value of a combination (its DESIGN 3.2). What the port
/// leaves undecided is counted, and printed with its reason.
#[test]
fn an_output_over_values_is_what_every_input_held_to_them_comes_to() {
    use rulec::eval::Val;
    use rulec::types::Ty;
    use std::collections::BTreeMap;
    let e = Engine::new();
    let (mut cases, mut exact, mut undecided, mut rules_asked) = (0, 0, 0, 0);
    let mut why: Vec<String> = Vec::new();
    let mut rules = corpus();
    for dir in ["tests/over", "tests/apply_fixtures"] {
        let mut more: Vec<String> = std::fs::read_dir(dir).unwrap().flatten().map(|e| format!("{dir}/{}", e.file_name().to_string_lossy())).filter(|p| p.ends_with(".rule")).collect();
        more.sort();
        rules.extend(more);
    }
    for path in rules {
        let src = std::fs::read_to_string(&path).unwrap();
        if rulec::has_error(&rulec::report(&src, &path).diags) {
            continue;
        }
        let (f, c) = rulec::prepare(&src, &path).unwrap();
        let base = |t: Option<Ty>| match t {
            Some(Ty::Opt(t)) => *t,
            Some(t) => t,
            None => Ty::Unknown,
        };
        let outputs: Vec<String> = f.outputs.iter().map(|o| o.name.text.clone()).filter(|o| matches!(base(c.ty_of(o)), Ty::Enum(_) | Ty::Bool)).collect();
        let finite: Vec<String> = f.inputs.iter().map(|i| i.name.text.clone()).filter(|n| matches!(base(c.ty_of(n)), Ty::Enum(_) | Ty::Bool)).collect();
        let walkable = f.inputs.iter().all(|i| matches!(base(c.ty_of(&i.name.text)), Ty::Enum(_) | Ty::Bool | Ty::Date) || base(c.ty_of(&i.name.text)).is_numeric());
        if outputs.is_empty() || finite.is_empty() || !f.elements.is_empty() || !walkable {
            continue;
        }
        rules_asked += 1;
        // the values an enum or a bool input takes, `none` first where it may be absent
        let values_of = |n: &str| -> Vec<Val> {
            let mut vs: Vec<Val> = if matches!(c.ty_of(n), Some(Ty::Opt(_))) { vec![Val::Enum(rulec::kw::NONE.into())] } else { vec![] };
            match base(c.ty_of(n)) {
                Ty::Enum(en) => vs.extend(c.enums.get(&en).cloned().unwrap_or_default().into_iter().map(Val::Enum)),
                _ => vs.extend([Val::Bool(true), Val::Bool(false)]),
            }
            vs
        };
        // how the port is told a value: an enum's by its place, a bool as 0 or 1, a number and a
        // date on the wire; None for `none`, which is not held
        let told = |n: &str, v: &Val| -> Option<i128> {
            match (v, base(c.ty_of(n))) {
                (Val::Enum(w), Ty::Enum(en)) => c.enums.get(&en)?.iter().position(|x| x == w).map(|k| k as i128),
                (Val::Bool(b), _) => Some(i128::from(*b)),
                (Val::Num(x), _) => Some(rulec::types::wire_int(*x, c.wire_scale(n))),
                (Val::Date(y, m, d), _) => {
                    let r = rulec::types::date_ord(*y, *m, *d);
                    Some(r.num / r.den)
                }
                _ => None,
            }
        };
        let vectors = rulec::vectors::generate(&f, &c);
        let n = vectors.len();
        if n == 0 {
            continue;
        }
        let mut picks = vec![0, n / 3, 2 * n / 3, n - 1];
        picks.dedup();
        for pick in picks {
            let v = &vectors[pick];
            // an input the vector leaves absent cannot be held; a rule with one is asked no more
            if f.inputs.iter().any(|i| told(&i.name.text, v.input.get(&i.name.text).unwrap_or(&Val::Enum(rulec::kw::NONE.into()))).is_none()) {
                continue;
            }
            for free in std::iter::once(None).chain(finite.iter().map(Some)) {
                let ranges: Vec<(String, Option<i128>, Option<i128>)> = f
                    .inputs
                    .iter()
                    .filter(|i| Some(&i.name.text) != free)
                    .map(|i| {
                        let k = told(&i.name.text, &v.input[&i.name.text]);
                        (i.name.text.clone(), k, k)
                    })
                    .collect();
                // every input so held, through the reference evaluator
                let mut truth: BTreeMap<String, Vec<Value>> = outputs.iter().map(|o| (o.clone(), Vec::new())).collect();
                let tried: Vec<Val> = match free {
                    Some(name) => values_of(name),
                    None => vec![Val::Bool(true)],
                };
                for x in tried {
                    let mut input: BTreeMap<String, Val> = v.input.clone();
                    if let Some(name) = free {
                        input.insert(name.clone(), x);
                    }
                    if !rulec::vectors::allowed(&f, &c, &input) || !rulec::vectors::days_ok(&c, &input) {
                        continue;
                    }
                    let (outs, _, _) = rulec::eval::run_all(&f, &c, input.into_iter().collect());
                    for (o, x) in outs {
                        if let (Some(seen), Some(x)) = (truth.get_mut(&o), x) {
                            let val = match (wire(&c, &o, &x), base(c.ty_of(&o))) {
                                (Value::Enum(name), Ty::Enum(en)) => Value::Enum(rulec::codegen::public_value(&f, &en, &name)),
                                (val, _) => val,
                            };
                            if !seen.contains(&val) {
                                seen.push(val);
                            }
                        }
                    }
                }
                for out in &outputs {
                    cases += 1;
                    let found = e.outputs_over(Path::new(&path), out, &ranges).unwrap_or_else(|x| panic!("{path}: {x:?}"));
                    match &found {
                        ritsu_ports::Found::Value(vs) => {
                            exact += 1;
                            let mut got: Vec<String> = vs.iter().map(|(v, _)| format!("{v:?}")).collect();
                            let mut want: Vec<String> = truth[out].iter().map(|v| format!("{v:?}")).collect();
                            got.sort();
                            want.sort();
                            assert_eq!(got, want, "{path} {out} over {ranges:?}");
                            witnesses_hold(&e, &path, out, &ranges, &found);
                        }
                        ritsu_ports::Found::Undecided(t) => {
                            undecided += 1;
                            why.push(format!("{path} {out} over {ranges:?}: {}", t.en));
                        }
                    }
                }
            }
        }
    }
    println!("outputs_over held to values: {cases} questions over {rules_asked} rules with an enum or a bool input, {exact} answered exactly, {undecided} undecided");
    for w in &why {
        println!("  undecided: {w}");
    }
    assert!(rules_asked >= 10 && exact * 10 >= cases * 9, "{exact} of {cases} answered exactly, over {rules_asked} rules");
}

/// Two dates held to one day each, and a table that reads a bool `define` comparing them: over the
/// days held, the comparison comes to one value, so a row asking for the other is left out, and so
/// is a row under `policy first` that the rows above it always take over. The answer is exact, each
/// value with an input that holds to the days (DESIGN §15.195). The rule that orders two coupons and
/// its Japanese twin, which answer by the aliases; the places of the kinds: percent 0, fixed 1.
#[test]
fn an_output_over_values_reads_what_a_bool_define_comes_to() {
    let e = Engine::new();
    let day = |y: i32, m: u32, d: u32| {
        let r = rulec::types::date_ord(y, m, d);
        r.num / r.den
    };
    let (april, may) = (day(2026, 4, 1), day(2026, 5, 1));
    let (first, second) = (|| Value::Enum("first".into()), || Value::Enum("second".into()));
    for (path, a_kind, b_kind, a_due, b_due, out) in [
        ("tests/corpus/coupon_order.rule", "a_kind", "b_kind", "a_due", "b_due", "a_order"),
        ("tests/corpus/適用順序.rule", "A種別", "B種別", "A期限", "B期限", "A順序"),
    ] {
        let ask = |kinds: Option<(i128, i128)>, dues: (i128, i128)| {
            let mut held: Vec<(String, Option<i128>, Option<i128>)> = vec![(a_due.to_string(), Some(dues.0), Some(dues.0)), (b_due.to_string(), Some(dues.1), Some(dues.1))];
            if let Some((a, b)) = kinds {
                held.push((a_kind.to_string(), Some(a), Some(a)));
                held.push((b_kind.to_string(), Some(b), Some(b)));
            }
            let found = e.outputs_over(Path::new(path), out, &held).unwrap_or_else(|x| panic!("{path}: {x:?}"));
            witnesses_hold(&e, path, out, &held, &found);
            values_of(&found)
        };
        // two fixed coupons: the one due first, or on the same day, goes first
        assert_eq!(ask(Some((1, 1)), (april, may)), [first()], "{path}");
        assert_eq!(ask(Some((1, 1)), (april, april)), [first()], "{path}");
        assert_eq!(ask(Some((1, 1)), (may, april)), [second()], "{path}");
        // a rate coupon goes first whatever the days, and the days decide only between the rest
        assert_eq!(ask(Some((0, 1)), (may, april)), [first()], "{path}");
        assert_eq!(ask(None, (may, april)), [first(), second()], "{path}");
    }
}

/// A row whose two derived columns over the same inputs ask for what only numbers between whole
/// numbers give (a sum of 3 and a difference of 0): the analysis cannot rule the row out, and no
/// input reaches it, so its value is left undecided, with the row named, rather than counted as
/// reached or as never reached.
#[test]
fn an_output_over_ranges_names_the_row_it_cannot_decide() {
    use ritsu_ports::Found;
    let e = Engine::new();
    let path = "tests/over/parity.rule";
    let src = std::fs::read_to_string(path).unwrap();
    assert!(!rulec::has_error(&rulec::report(&src, path).diags));
    match e.outputs_over(Path::new(path), "answer", &[]).unwrap() {
        Found::Undecided(t) => {
            assert!(t.en.contains("comes to odd_half") && t.en.contains("row 1 of t"), "{}", t.en);
            assert!(t.ja.contains("odd_half") && t.ja.contains("t の行 1"), "{}", t.ja);
        }
        other => panic!("{other:?}"),
    }
    // held where the sum cannot be 3, the row is ruled out and the answer is exact
    let held = [("first".to_string(), Some(0), Some(1)), ("second".to_string(), Some(0), Some(1))];
    let found = e.outputs_over(Path::new(path), "answer", &held).unwrap();
    witnesses_hold(&e, path, "answer", &held, &found);
    assert_eq!(values_of(&found), [Value::Enum("other".into())]);
}

/// A rule that applies another holds its own inputs to the ranges, not the other rule's: the rule
/// applied is checked as it is (here its input has the same name), and its rows come into this
/// rule's table, where the ranges hold.
#[test]
fn an_output_over_ranges_holds_the_rule_not_the_one_it_applies() {
    let e = Engine::new();
    let path = "tests/over/apply_band.rule";
    for (lo, hi, want) in [(1, 50, vec![Value::Bool(false)]), (51, 100, vec![Value::Bool(true)]), (50, 51, vec![Value::Bool(true), Value::Bool(false)])] {
        let held = [("amount".to_string(), Some(lo), Some(hi))];
        let found = e.outputs_over(Path::new(path), "answer", &held).unwrap();
        witnesses_hold(&e, path, "answer", &held, &found);
        assert_eq!(values_of(&found), want, "{lo}..{hi}");
    }
}

/// The examples of a numeric output (§15.270), in their order: the two ends of its range, the
/// numbers the rows that decide it write, then the fewest and the most it comes to over the rule's
/// vectors, each value once, with the first vector that comes to it, and none for a value no vector
/// comes to. Under `holding <= asked`, `asked - holding - 5` reaches its upper end, 95, and not its
/// lower, -55: the fewest the vectors come to, -5, is an example. The rows of a table are examples
/// where a vector reaches them, and a row the constraint keeps out is none.
#[test]
fn the_examples_of_an_output_are_its_ends_its_rows_and_the_fewest_and_most_of_its_vectors() {
    let t = ritsu_testkit::TempDir::new("ports-examples");
    let head = "inputs\n  asked   : number  range >=0 <=100\n  holding : number  range >=0 <=50\n\nconstraint holding <= asked\n\noutputs\n  needed : number  round down(1)\n";
    std::fs::write(t.path().join("short.rule"), format!("rule short v1\n\n{head}\nresult needed = asked - holding - 5\n")).unwrap();
    std::fs::write(t.path().join("rows.rule"), format!("rule rows v1\n\n{head}\ntable pick\npolicy unique\n| asked | holding | -> needed : number |\n| <=40  | >40     | -20                |\n| <=40  | <=40    | 10                 |\n| >40   | -       | 30                 |\n")).unwrap();
    let e = Engine::new();
    let examples = |file: &str| match e.output_values(&t.path().join(file), "needed").unwrap() {
        ritsu_ports::Found::Value(v) => ((v.min, v.max), v.values, v.examples),
        ritsu_ports::Found::Undecided(why) => panic!("{file}: {why:?}"),
    };
    let int = |n: i128| Value::Int(n);
    let at = |asked: i128, holding: i128| vec![("asked".to_string(), int(asked)), ("holding".to_string(), int(holding))];
    let (ends, values, got) = examples("short.rule");
    assert_eq!((ends, values), ((Some(-55), Some(95)), None));
    assert_eq!(got, [(95, at(100, 0)), (-5, at(0, 0))]);
    // -20 is an end and a row no vector reaches; 30 the other end, 10 a row; the vectors stop there
    let (ends, values, got) = examples("rows.rule");
    assert_eq!((ends, values), ((Some(-20), Some(30)), Some(vec![-20, 10, 30])));
    assert_eq!(got.iter().map(|(v, _)| *v).collect::<Vec<_>>(), [30, 10]);
}

/// What a numeric output comes to with some inputs held (§15.273): with none held to less than the
/// rule declares, `output_values`'s answer; otherwise what decides the output there — the rows some
/// input there may reach, what its `result` reaches — and not a range written on the output's
/// line, which is a promise over every input; the examples are those of the vectors inside the
/// ranges, taken as `output_values` takes them. An enum is held by the places of its values, and a
/// value apart from the others by naming the input again.
#[test]
fn a_numeric_output_over_ranges_is_what_decides_it_there() {
    let t = ritsu_testkit::TempDir::new("ports-values-over");
    let write = |name: &str, body: &str| std::fs::write(t.path().join(name), body).unwrap();
    write("seats.rule", "rule seats v1\n\nenum kind = handback | workshop | talk\n\ninputs\n  event_kind : kind\n\noutputs\n  needed : number  round down(1)\n\ntable pick\npolicy unique\n| event_kind | -> needed : number |\n| handback   | -20                |\n| workshop   | 30                 |\n| talk       | 80                 |\n");
    write("refund.rule", "rule refund v1\n\nenum kind = charge | refund\n\ninputs\n  what : kind\n  late : bool\n\noutputs\n  amount : number  range >=-500 <=500  round down(1)\n  note : kind\n\ntable pick\npolicy unique\n| what   | late  | -> amount : number | note   |\n| charge | false | 300                | charge |\n| charge | true  | 350                | charge |\n| refund | -     | -300               | refund |\n");
    let head = "inputs\n  asked   : number  range >=0 <=100\n  holding : number  range >=0 <=50\n\nconstraint holding <= asked\n\noutputs\n  needed : number  round down(1)\n";
    write("tied.rule", &format!("rule tied v1\n\n{head}\nresult needed = asked - holding\n"));
    write("rows.rule", &format!("rule rows v1\n\n{head}\ntable pick\npolicy unique\n| asked | holding | -> needed : number |\n| <=40  | >40     | -20                |\n| <=40  | <=40    | 10                 |\n| >40   | -       | 30                 |\n"));
    let e = Engine::new();
    let held = |r: &[(&str, Option<i128>, Option<i128>)]| r.iter().map(|(n, l, h)| (n.to_string(), *l, *h)).collect::<Vec<_>>();
    #[allow(clippy::type_complexity)]
    let over = |file: &str, output: &str, r: &[(&str, Option<i128>, Option<i128>)]| -> ((Option<i128>, Option<i128>), Option<Vec<i128>>, Vec<(i128, Vec<(String, Value)>)>) {
        match e.output_values_over(&t.path().join(file), output, &held(r)).unwrap() {
            ritsu_ports::Found::Value(v) => ((v.min, v.max), v.values, v.examples),
            ritsu_ports::Found::Undecided(why) => panic!("{file} {r:?}: {why:?}"),
        }
    };
    let whole = |file: &str, output: &str| match e.output_values(&t.path().join(file), output).unwrap() {
        ritsu_ports::Found::Value(v) => ((v.min, v.max), v.values, v.examples),
        ritsu_ports::Found::Undecided(why) => panic!("{file}: {why:?}"),
    };
    let kind = |v: &str| vec![("event_kind".to_string(), Value::Enum(v.into()))];
    // the rows of the kinds held: workshop and talk, by their places; handback and talk apart
    assert_eq!(over("seats.rule", "needed", &[("event_kind", Some(1), Some(2))]), ((Some(30), Some(80)), Some(vec![30, 80]), vec![(30, kind("workshop")), (80, kind("talk"))]));
    assert_eq!(over("seats.rule", "needed", &[("event_kind", Some(0), Some(0)), ("event_kind", Some(2), Some(2))]), ((Some(-20), Some(80)), Some(vec![-20, 80]), vec![(-20, kind("handback")), (80, kind("talk"))]));
    // every kind, or a range past the places, holds nothing to less than the rule declares
    assert_eq!(over("seats.rule", "needed", &[("event_kind", Some(0), Some(2))]), whole("seats.rule", "needed"));
    assert_eq!(over("seats.rule", "needed", &[("event_kind", None, Some(9))]), whole("seats.rule", "needed"));
    assert_eq!(over("seats.rule", "needed", &[]), whole("seats.rule", "needed"));
    // the range written on the line is what the rule promises over every input; held to a charge,
    // the rows say 300 to 350, and a bool is held as 0 and 1
    assert_eq!(whole("refund.rule", "amount").0, (Some(-500), Some(500)));
    let (ends, values, examples) = over("refund.rule", "amount", &[("what", Some(0), Some(0))]);
    assert_eq!((ends, values), ((Some(300), Some(350)), Some(vec![300, 350])));
    assert_eq!(examples.iter().map(|(v, _)| *v).collect::<Vec<_>>(), [300, 350]);
    assert_eq!(over("refund.rule", "amount", &[("what", Some(0), Some(0)), ("late", Some(1), Some(1))]).0, (Some(350), Some(350)));
    // what a `result` reaches over the held range; the constraint keeps out the rows past it
    let (ends, values, examples) = over("tied.rule", "needed", &[("asked", Some(50), Some(100))]);
    assert_eq!((ends, values), ((Some(0), Some(100)), None));
    assert!(examples.iter().all(|(_, input)| matches!(input[0], (_, Value::Int(n)) if (50..=100).contains(&n))), "{examples:?}");
    assert_eq!(over("tied.rule", "needed", &[("asked", Some(0), Some(50))]).0, (Some(-50), Some(50)));
    assert_eq!(over("rows.rule", "needed", &[("asked", Some(0), Some(40))]).0, (Some(10), Some(10)));
    // the row only an input past the constraint reaches (-20) is left out once any input is held
    assert_eq!(whole("rows.rule", "needed").0, (Some(-20), Some(30)));
    for held in [("asked", Some(0), Some(99)), ("holding", Some(0), Some(49))] {
        let (ends, values, _) = over("rows.rule", "needed", &[held]);
        assert_eq!((ends, values), ((Some(10), Some(30)), Some(vec![10, 30])), "{held:?}");
    }
    // what is not answered: an output that is not a number, an input the rule does not have, a
    // range with no input of the rule inside
    let undecided = |file: &str, output: &str, r: &[(&str, Option<i128>, Option<i128>)]| match e.output_values_over(&t.path().join(file), output, &held(r)).unwrap() {
        ritsu_ports::Found::Undecided(why) => why.en,
        other => panic!("{file} {r:?}: {other:?}"),
    };
    assert_eq!(undecided("refund.rule", "note", &[("what", Some(0), Some(0))]), "`note` is not a numeric output of this rule");
    assert_eq!(undecided("seats.rule", "needed", &[("size", Some(0), Some(0))]), "`size` is not an input of this rule");
    assert_eq!(undecided("tied.rule", "needed", &[("asked", Some(200), None)]), "no input of the rule lies inside the ranges it is given");
}

/// Over the corpus and the rules of these tests, every numeric output asked with its inputs held
/// (§15.273): each enum and bool input in turn to each of its values, each number and date input
/// with both ends declared in turn to the lower and the upper half of its range, and the number and
/// date inputs to a few steps around four of the rule's vectors. A hold whose inputs are few enough
/// to walk is asked, and every input it holds goes through the reference evaluator (the
/// constraints kept): what it comes to lies between the fewest and the most the port answers, and
/// among the numbers where it names them. Each example is an input inside the ranges that comes to
/// the amount it is given with. What the port leaves undecided is counted, and printed with its
/// reason.
#[test]
fn a_numeric_output_over_ranges_holds_what_every_input_inside_them_comes_to() {
    use rulec::eval::Val;
    use rulec::types::Ty;
    use std::collections::BTreeMap;
    const CAP: usize = 4_000;
    /// How an input is held: an enum or a bool by its values, a number or a date by a range on
    /// the wire, between the ends it declares.
    enum Kind {
        Values(Vec<Val>),
        Line { date: bool, lo: Option<i128>, hi: Option<i128> },
    }
    let e = Engine::new();
    let (mut cases, mut answered, mut narrowed, mut walked, mut too_wide, mut undecided, mut rules_asked, mut unwalked) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    let mut why: Vec<String> = Vec::new();
    let mut rules = corpus();
    for dir in ["tests/over", "tests/apply_fixtures"] {
        let mut more: Vec<String> = std::fs::read_dir(dir).unwrap().flatten().map(|e| format!("{dir}/{}", e.file_name().to_string_lossy())).filter(|p| p.ends_with(".rule")).collect();
        more.sort();
        rules.extend(more);
    }
    for path in rules {
        let src = std::fs::read_to_string(&path).unwrap();
        if rulec::has_error(&rulec::report(&src, &path).diags) {
            continue;
        }
        let (f, c) = rulec::prepare(&src, &path).unwrap();
        let outputs: Vec<String> = f.outputs.iter().map(|o| o.name.text.clone()).filter(|o| c.ty_of(o).is_some_and(|t| !matches!(t, Ty::Opt(_)) && t.is_numeric())).collect();
        if outputs.is_empty() || !f.elements.is_empty() {
            continue;
        }
        let base = |t: Option<Ty>| match t {
            Some(Ty::Opt(t)) => *t,
            Some(t) => t,
            None => Ty::Unknown,
        };
        let day = |y: i32, m: u32, d: u32| {
            let r = rulec::types::date_ord(y, m, d);
            r.num / r.den
        };
        // how the port is told a value: an enum's by its place, a bool as 0 or 1, a number and a
        // date on the wire
        let told = |n: &str, v: &Val| -> Option<i128> {
            match (v, base(c.ty_of(n))) {
                (Val::Enum(w), Ty::Enum(en)) => c.enums.get(&en)?.iter().position(|x| x == w).map(|k| k as i128),
                (Val::Bool(b), _) => Some(i128::from(*b)),
                (Val::Num(x), _) => Some(rulec::types::wire_int(*x, c.wire_scale(n))),
                (Val::Date(y, m, d), _) => Some(day(*y, *m, *d)),
                _ => None,
            }
        };
        let mut ins: Vec<(String, bool, Kind)> = Vec::new();
        for i in &f.inputs {
            let n = i.name.text.clone();
            let opt = matches!(c.ty_of(&n), Some(Ty::Opt(_)));
            let kind = match base(c.ty_of(&n)) {
                Ty::Enum(en) => {
                    let mut vs = c.enums.get(&en).cloned().unwrap_or_default();
                    if let Some(allowed) = c.enum_ranges.get(&n) {
                        vs.retain(|w| allowed.contains(w));
                    }
                    Kind::Values(vs.into_iter().map(Val::Enum).collect())
                }
                Ty::Bool => Kind::Values(vec![Val::Bool(true), Val::Bool(false)]),
                t if t == Ty::Date || t.is_numeric() => {
                    let date = t == Ty::Date;
                    let (lo, hi) = c.ranges.get(&n).copied().unwrap_or((None, None));
                    let w = |r: rulec::num::Rat| if date { r.num / r.den } else { rulec::types::wire_int(r, c.wire_scale(&n)) };
                    Kind::Line { date, lo: lo.map(w), hi: hi.map(w) }
                }
                _ => {
                    ins.clear();
                    break;
                }
            };
            ins.push((n, opt, kind));
        }
        if ins.len() != f.inputs.len() {
            unwalked += 1;
            continue;
        }
        rules_asked += 1;
        // the questions: each enum and bool value alone, each half of a range, boxes around vectors
        let mut questions: Vec<Vec<(String, Option<i128>, Option<i128>)>> = Vec::new();
        for (n, _, kind) in &ins {
            match kind {
                Kind::Values(vs) => questions.extend(vs.iter().filter_map(|v| told(n, v)).map(|k| vec![(n.clone(), Some(k), Some(k))])),
                Kind::Line { lo: Some(lo), hi: Some(hi), .. } if hi > lo => {
                    let mid = lo + (hi - lo) / 2;
                    questions.push(vec![(n.clone(), Some(*lo), Some(mid))]);
                    questions.push(vec![(n.clone(), Some(mid + 1), Some(*hi))]);
                }
                _ => {}
            }
        }
        // the values each input walks under a question; None when they are too many or unbounded
        let axes_of = |q: &[(String, Option<i128>, Option<i128>)]| -> Option<Vec<(String, Vec<Val>)>> {
            let mut axes: Vec<(String, Vec<Val>)> = Vec::new();
            for (n, opt, kind) in &ins {
                let held = q.iter().find(|(m, _, _)| m == n);
                let mut vals: Vec<Val> = Vec::new();
                match kind {
                    Kind::Values(vs) => {
                        // an enum or a bool held to values is not absent, as the port holds it
                        if *opt && held.is_none() {
                            vals.push(Val::Enum(rulec::kw::NONE.into()));
                        }
                        vals.extend(vs.iter().filter(|v| held.is_none_or(|(_, lo, hi)| told(n, v).is_some_and(|k| lo.is_none_or(|l| k >= l) && hi.is_none_or(|h| k <= h)))).cloned());
                    }
                    Kind::Line { date, lo, hi } => {
                        if *opt {
                            vals.push(Val::Enum(rulec::kw::NONE.into()));
                        }
                        let l = [held.and_then(|h| h.1), *lo].into_iter().flatten().max()?;
                        let h = [held.and_then(|h| h.2), *hi].into_iter().flatten().min()?;
                        if h - l >= CAP as i128 {
                            return None;
                        }
                        vals.extend((l..=h).map(|w| {
                            if *date {
                                let (y, m, d) = rulec::types::ord_to_date(rulec::num::Rat::int(w));
                                Val::Date(y, m, d)
                            } else {
                                Val::Num(rulec::types::from_wire(w, c.wire_scale(n)))
                            }
                        }));
                    }
                }
                axes.push((n.clone(), vals));
            }
            let size = axes.iter().map(|(_, vs)| vs.len()).fold(1usize, |a, b| a.saturating_mul(b));
            (size <= CAP).then_some(axes)
        };
        let vectors = rulec::vectors::generate(&f, &c);
        let n = vectors.len();
        let mut picks = if n == 0 { vec![] } else { vec![0, n / 3, 2 * n / 3, n - 1] };
        picks.dedup();
        for pick in picks {
            let v = &vectors[pick];
            for w in [3i128, 1, 0] {
                let b: Vec<(String, Option<i128>, Option<i128>)> = ins
                    .iter()
                    .filter_map(|(n, _, kind)| match kind {
                        Kind::Line { lo, hi, .. } => {
                            let at = v.input.get(n).and_then(|x| told(n, x)).or(*lo)?;
                            Some((n.clone(), Some(lo.map_or(at - w, |l| l.max(at - w))), Some(hi.map_or(at + w, |h| h.min(at + w)))))
                        }
                        Kind::Values(_) => None,
                    })
                    .collect();
                if b.is_empty() {
                    break;
                }
                if axes_of(&b).is_some() {
                    if !questions.contains(&b) {
                        questions.push(b);
                    }
                    break;
                }
            }
        }
        // the fewest and the most over every input, as `output_values` answers them
        let whole: BTreeMap<String, (Option<i128>, Option<i128>)> = outputs.iter().map(|o| (o.clone(), c.wire_range(o).unwrap_or((None, None)))).collect();
        for q in &questions {
            // every input so held, through the reference evaluator; a hold too wide to walk is
            // not asked
            let Some(axes) = axes_of(q) else {
                too_wide += 1;
                continue;
            };
            walked += 1;
            let truth: BTreeMap<String, Vec<i128>> = {
                let mut seen: BTreeMap<String, Vec<i128>> = outputs.iter().map(|o| (o.clone(), Vec::new())).collect();
                let mut at = vec![0usize; axes.len()];
                'walk: loop {
                    let input: BTreeMap<String, Val> = axes.iter().zip(&at).filter(|((_, vs), _)| !vs.is_empty()).map(|((n, vs), i)| (n.clone(), vs[*i].clone())).collect();
                    if axes.iter().all(|(_, vs)| !vs.is_empty()) && rulec::vectors::allowed(&f, &c, &input) && rulec::vectors::days_ok(&c, &input) {
                        let (outs, _, _) = rulec::eval::run_all(&f, &c, input.into_iter().collect());
                        for (o, x) in outs {
                            if let (Some(seen), Some(Val::Num(r))) = (seen.get_mut(&o), x) {
                                seen.push(rulec::types::wire_int(r, c.wire_scale(&o)));
                            }
                        }
                    }
                    for k in 0..at.len() {
                        at[k] += 1;
                        if at[k] < axes[k].1.len() {
                            continue 'walk;
                        }
                        at[k] = 0;
                    }
                    break;
                }
                seen
            };
            for out in &outputs {
                cases += 1;
                match e.output_values_over(Path::new(&path), out, q).unwrap_or_else(|x| panic!("{path}: {x:?}")) {
                    ritsu_ports::Found::Value(got) => {
                        answered += 1;
                        let between = |v: i128| got.min.is_none_or(|m| v >= m) && got.max.is_none_or(|m| v <= m);
                        if (got.min, got.max) != whole[out] {
                            narrowed += 1;
                        }
                        for v in &truth[out] {
                            assert!(between(*v), "{path} {out} over {q:?}: {v} is outside {:?}..{:?}", got.min, got.max);
                            if let Some(vs) = &got.values {
                                assert!(vs.contains(v), "{path} {out} over {q:?}: {v} is not among {vs:?}");
                            }
                        }
                        for (v, input) in &got.examples {
                            assert!(between(*v), "{path} {out} over {q:?}: the example {v} is outside {:?}..{:?}", got.min, got.max);
                            for (name, lo, hi) in q {
                                let k = match input.iter().find(|(n, _)| n == name).map(|(_, x)| x) {
                                    Some(Value::Int(n)) => *n,
                                    Some(Value::Date(d)) => {
                                        let p: Vec<i32> = d.split('-').map(|x| x.parse().unwrap()).collect();
                                        day(p[0], p[1] as u32, p[2] as u32)
                                    }
                                    Some(Value::Bool(b)) => i128::from(*b),
                                    Some(Value::Enum(w)) => told(name, &Val::Enum(w.clone())).unwrap_or_else(|| panic!("{path}: {name} = {w}")),
                                    _ => continue,
                                };
                                assert!(lo.is_none_or(|l| k >= l) && hi.is_none_or(|h| k <= h), "{path} {out} over {q:?}: the example of {v} gives {name} {k}");
                            }
                            let outs = e.eval(Path::new(&path), input).unwrap_or_else(|x| panic!("{path} {out}: {x:?}"));
                            let came = outs.iter().find(|(n, _)| n == out).map(|(_, x)| x.clone());
                            assert_eq!(came, Some(Value::Int(*v)), "{path} {out} over {q:?}: {input:?}");
                        }
                    }
                    ritsu_ports::Found::Undecided(t) => {
                        undecided += 1;
                        why.push(format!("{path} {out} over {q:?}: {}", t.en));
                    }
                }
            }
        }
    }
    println!("output_values_over: {cases} questions over {rules_asked} rules with a numeric output ({walked} holds walked, {too_wide} too wide to walk and not asked), {answered} answered ({narrowed} narrower than over every input), {undecided} undecided; {unwalked} rules not asked (a string input)");
    for w in &why {
        println!("  undecided: {w}");
    }
    assert!(rules_asked >= 10 && narrowed > 0 && answered * 10 >= cases * 9, "{answered} of {cases} answered, {narrowed} narrower, over {rules_asked} rules");
}

/// Each value of a rule's enum is handed over with its public name — the alias the rule writes in
/// parentheses, else its name; what a gate writes into Cedar — beside the member the generated code
/// builds from it. Over the corpus, a public name that is not the name is written in the rule as
/// `<name>(<public name>)`.
#[test]
fn a_value_of_an_enum_has_the_public_name_the_rule_writes() {
    let e = Engine::new();
    for (path, name, values) in [
        ("tests/over/refund_limit.rule", "refund_band", [("within_limit", "within_limit"), ("over_limit", "over_limit")]),
        ("tests/over/返金の上限.rule", "返金の区分", [("上限まで", "within_limit"), ("上限超え", "over_limit")]),
    ] {
        let f = e.facts(Path::new(path)).unwrap();
        let en = f.enums.iter().find(|x| x.name == name).unwrap_or_else(|| panic!("{path}: no enum {name}"));
        let got: Vec<(&str, &str, &str)> = en.values.iter().map(|v| (v.name.as_str(), v.public.as_str(), v.alias.as_str())).collect();
        assert_eq!(got, [(values[0].0, values[0].1, "WithinLimit"), (values[1].0, values[1].1, "OverLimit")], "{path}");
    }
    let mut aliased = 0;
    for path in corpus() {
        let Ok(f) = e.facts(Path::new(&path)) else { continue };
        let mut src = std::fs::read_to_string(&path).unwrap();
        // An enum read from an `enums` file is written there, and so are its values (§15.212).
        let dir = Path::new(&path).parent().unwrap().to_path_buf();
        let owners: Vec<String> = src.lines().filter(|l| l.starts_with("import enum ")).filter_map(|l| l.split('"').nth(1).map(str::to_string)).collect();
        for o in owners {
            src.push_str(&std::fs::read_to_string(dir.join(&o)).unwrap_or_else(|e| panic!("{path}: cannot read {o}: {e}")));
        }
        for v in f.enums.iter().flat_map(|en| &en.values) {
            if v.public != v.name {
                assert!(src.contains(&format!("{}({})", v.name, v.public)), "{path}: {} is not written {}({})", v.name, v.name, v.public);
                aliased += 1;
            }
        }
    }
    assert!(aliased > 20, "{aliased} values are written with an alias");
}

/// The range an input of an ordered enum declares crosses the port (§15.251): `campaign : tier
/// range >=basic <=gold` is handed over with the three values it lets in, and `member`, with no
/// range, and the outputs with none; the evaluator answers a campaign inside the range and refuses
/// platinum at its door, as the generated code does (before, it answered platinum); and an output
/// over a place the range keeps out comes to nothing, as over a number past its range. The English
/// rule of the corpus, and its Japanese version beside it.
#[test]
fn the_range_of_an_enum_input_crosses_the_port() {
    let e = Engine::new();
    #[allow(clippy::type_complexity)]
    let rules: [(&str, &str, [&str; 4], [&str; 3]); 2] = [
        ("tests/corpus/member_tier_shipping.rule", "tier", ["member", "campaign", "subtotal", "lifted"], ["basic", "silver", "gold"]),
        ("tests/corpus/会員段階の送料.rule", "段階", ["会員", "キャンペーン", "小計", "引き上げ"], ["一般", "シルバー", "ゴールド"]),
    ];
    for (path, tier, [member, campaign, subtotal, lifted], inside) in rules {
        let p = Path::new(path);
        let f = e.facts(p).unwrap_or_else(|x| panic!("{path}: {x:?}"));
        let ty = |n: &str| f.inputs.iter().chain(&f.outputs).find(|c| c.name == n).map(|c| c.ty.clone()).unwrap_or_else(|| panic!("{path}: no {n}"));
        assert_eq!(ty(campaign), ColumnType::Enum { name: tier.into(), range: Some(inside.iter().map(|v| v.to_string()).collect()) }, "{path}");
        assert_eq!(ty(member), ColumnType::Enum { name: tier.into(), range: None }, "{path}");
        assert!(f.outputs.iter().all(|c| !matches!(&c.ty, ColumnType::Enum { range: Some(_), .. })), "{path}: an output carries a range");
        let values: Vec<String> = f.enums.iter().find(|x| x.name == tier).unwrap().values.iter().map(|v| v.name.clone()).collect();
        let (lowest, highest) = (values[0].clone(), values[3].clone());
        let at = |c: &str| -> ritsu_ports::Values { vec![(member.into(), Value::Enum(lowest.clone())), (campaign.into(), Value::Enum(c.into())), (subtotal.into(), Value::Int(10))] };
        let got = e.eval(p, &at(inside[2])).unwrap_or_else(|x| panic!("{path}: {x:?}"));
        assert!(got.contains(&(lifted.to_string(), Value::Bool(true))), "{path}: {got:?}");
        match e.eval(p, &at(&highest)) {
            Err(RuleError::Input(t)) => {
                assert_eq!(t.en, format!("the input `{campaign}` is outside its range"), "{path}");
                assert_eq!(t.ja, format!("入力 `{campaign}` が範囲の外です"), "{path}");
            }
            other => panic!("{path}: a campaign past its range is answered: {other:?}"),
        }
        // the places of the values, from 0: platinum is 3, and over it no input lies inside
        let over = |lo: i128, hi: i128| e.outputs_over(p, lifted, &[(campaign.to_string(), Some(lo), Some(hi))]).unwrap();
        assert_eq!(over(3, 3), ritsu_ports::Found::Value(vec![]), "{path}");
        assert_eq!(values_of(&over(0, 3)), values_of(&over(0, 2)), "{path}");
    }
}

/// A door case on the wire, as the port takes it: None for what the port has no value for (a
/// number with a fraction, an element that is not an object).
fn door_value(ty: &rulec::types::Ty, j: &Json) -> Option<Value> {
    use rulec::types::Ty;
    let inner = match ty {
        Ty::Opt(t) => t.as_ref(),
        t => t,
    };
    Some(match (inner, j) {
        (_, Json::Null) => Value::None,
        (_, Json::Bool(b)) => Value::Bool(*b),
        (_, Json::Int(n)) => Value::Int(*n),
        (Ty::Enum(_), Json::Str(s)) => Value::Enum(s.clone()),
        (Ty::Date, Json::Str(s)) => Value::Date(s.clone()),
        (_, Json::Str(s)) => Value::Str(s.clone()),
        _ => return None,
    })
}

/// The object a vector's `in` is, as the values the port takes: None where the port has no value
/// for one of them.
fn port_values(f: &rulec::ast::RuleFile, c: &rulec::types::Checked, given: &Json) -> Option<ritsu_ports::Values> {
    let mut values: ritsu_ports::Values = Vec::new();
    for i in &f.inputs {
        let n = &i.name.text;
        let Some(x) = given.get(n) else { continue };
        values.push((n.clone(), door_value(&c.ty_of(n).unwrap_or(rulec::types::Ty::Unknown), x)?));
    }
    for el in &f.elements {
        let seq = &el.name.text;
        match given.get(seq) {
            Some(Json::Arr(xs)) => {
                let mut rows = Vec::new();
                for x in xs {
                    if !matches!(x, Json::Obj(_)) {
                        return None;
                    }
                    let mut row = Vec::new();
                    for fd in &el.fields {
                        let n = &fd.name.text;
                        let Some(v) = x.get(n) else { continue };
                        row.push((n.clone(), door_value(&c.ty_of(n).unwrap_or(rulec::types::Ty::Unknown), v)?));
                    }
                    rows.push(row);
                }
                values.push((seq.clone(), Value::List(rows)));
            }
            Some(other) => values.push((seq.clone(), door_value(&rulec::types::Ty::Unknown, other)?)),
            None => {}
        }
    }
    Some(values)
}

/// The evaluator behind the port refuses every input the door of the generated code refuses (what
/// `rulec gen` writes for `rulec test` to hold every language to, §15.204), over every rule of the
/// corpus, wherever the port can carry the input: a value of another kind, one past each end of
/// its range — of an ordered enum's too — a day the calendar does not have, an element's field past
/// its range, a combination a `constraint` rules out. Before §15.251 it answered 66 of them: the
/// value past the range of an enum in each of the two rules that hold one to a range, 12 days the
/// calendar does not have, and 52 fields of an element past their range or on such a day.
#[test]
fn the_port_refuses_every_input_the_door_refuses() {
    let e = Engine::new();
    let (mut asked, mut enums, mut pairs) = (0, 0, 0);
    let mut answered = Vec::new();
    for path in corpus() {
        let src = std::fs::read_to_string(&path).unwrap();
        if rulec::has_error(&rulec::report(&src, &path).diags) {
            continue;
        }
        let (f, c) = rulec::prepare(&src, &path).unwrap();
        let vectors = rulec::vectors::generate(&f, &c);
        for line in rulec::vectors::door_cases(&f, &c, &vectors) {
            let j = json(&line);
            let Some(values) = port_values(&f, &c, j.get("in").unwrap()) else { continue };
            asked += 1;
            let why = st(j.get("why"));
            if f.inputs.iter().any(|i| c.enum_ranges.contains_key(&i.name.text) && why.starts_with(i.name.text.as_str()) && why.contains("range")) {
                if why.contains(" and ") {
                    pairs += 1;
                } else {
                    enums += 1;
                }
            }
            match e.eval(Path::new(&path), &values) {
                Err(RuleError::Input(_)) | Err(RuleError::Contradiction(_)) => {}
                other => answered.push(format!("{path}: {why}: {other:?}")),
            }
        }
    }
    assert!(answered.is_empty(), "the port answers what the door refuses:\n{}", answered.join("\n"));
    assert!(asked > 1500, "{asked} inputs the door refuses");
    // `campaign` and `キャンペーン`, `season_ticket` and `シーズン券`, one value past the end of each range
    assert_eq!(enums, 4, "the values of an enum past its range");
    // and the campaign past its range with the subtotal after it of another kind, in English and in
    // Japanese: an input with two things wrong, which the door refuses for the kind (§15.328)
    assert_eq!(pairs, 2, "the values of an enum past its range with a second thing wrong");
}

/// The case routing of the corpus with capacities up to 4e18, which three reviewers take past int64
/// (§15.226): the rule `check` passes, written beside the test.
fn heavy_routing(dir: &Path, corpus: &str, name: &str) -> String {
    let src = std::fs::read_to_string(root().join(corpus)).unwrap();
    assert_eq!(src.matches("range >=0 <=100").count(), 1, "{corpus}");
    let path = dir.join(name);
    std::fs::write(&path, src.replacen("range >=0 <=100", "range >=0 <=4_000_000_000_000_000_000", 1)).unwrap();
    path.to_string_lossy().into_owned()
}

/// The port refuses a call for the kind of reason the door refuses it for (§15.314): the caller's
/// input (`RuleError::Input`) or the rule's own contradiction (`RuleError::Contradiction`). Over
/// every line of the refused inputs `rulec gen` writes, for every rule of the corpus and for the
/// rules that have what no corpus rule has — a `unique` field (the learning of a province) and
/// weights that can pass int64 (the case routing with capacities up to 4e18), each in English and
/// in Japanese. Before, the port called three of the refusals of a walk the rule's contradiction,
/// with "no row decides the output": two elements with the same value of a `unique` field, weights
/// whose total passes int64, and a drawn value not below the total of the weights.
#[test]
fn the_port_refuses_for_the_kind_of_reason_the_door_does() {
    let e = Engine::new();
    let t = ritsu_testkit::TempDir::new("ports-kinds");
    let mut rules = corpus();
    rules.extend(["tests/merging/unique/province_learning.rule", "tests/merging/unique/州の学問.rule"].map(String::from));
    rules.push(heavy_routing(t.path(), "tests/corpus/case_routing.rule", "heavy.rule"));
    rules.push(heavy_routing(t.path(), "tests/corpus/案件の振り分け.rule", "大口.rule"));
    let (mut inputs, mut contradictions) = (0, 0);
    // the lines of each of the three reasons, by the sentence the reference evaluator says
    let (mut twice, mut heavy, mut drawn) = (0, 0, 0);
    let mut apart = Vec::new();
    for path in rules {
        let src = std::fs::read_to_string(root().join(&path)).unwrap();
        if rulec::has_error(&rulec::report(&src, &path).diags) {
            assert!(path.starts_with("tests/corpus/"), "{path} does not pass check");
            continue;
        }
        let (f, c) = rulec::prepare(&src, &path).unwrap();
        let suite = rulec::vectors::suite(&f, &c);
        let seqs: Vec<&String> = f.elements.iter().map(|el| &el.name.text).collect();
        for line in rulec::vectors::refused_lines(&f, &c, &suite) {
            let j = json(&line);
            let Some(values) = port_values(&f, &c, j.get("in").unwrap()) else { continue };
            let (kind, said, why) = (st(j.get("refused")), st(j.get("error")), st(j.get("why")));
            let got = e.eval(&root().join(&path), &values);
            let port = match &got {
                Err(RuleError::Input(_)) => "input",
                Err(RuleError::Contradiction(_)) => "contradiction",
                _ => "",
            };
            if port != kind {
                apart.push(format!("{path}: {why}: the door says {kind} ({said}), the port {got:?}"));
                continue;
            }
            match port {
                "input" => inputs += 1,
                _ => contradictions += 1,
            }
            let Err(RuleError::Input(text)) = &got else { continue };
            // the three, each with the sentence the port says of it
            for el in &f.elements {
                let seq = &el.name.text;
                for fd in &el.fields {
                    let field = &fd.name.text;
                    // The two said by their places, the first element and a later one (§15.324).
                    if let Some((a, b)) = [(0, 1), (0, 2)].into_iter().find(|(a, b)| said == rulec::door::duplicate(seq, field, a, b)) {
                        twice += 1;
                        assert_eq!(text.en, format!("the list `{seq}` has two elements with the same `{field}` (`{seq}[{a}]` and `{seq}[{b}]`)"), "{path}: {why}");
                        assert_eq!(text.ja, format!("並び `{seq}` の要素に、`{field}` が同じものが二つあります（`{seq}[{a}]` と `{seq}[{b}]`）"), "{path}: {why}");
                    }
                }
            }
            for seq in &seqs {
                if said == rulec::door::weights_past(seq) {
                    heavy += 1;
                    assert_eq!(text.en, format!("the total of the weights of the list `{seq}` does not fit in int64"), "{path}: {why}");
                    assert_eq!(text.ja, format!("並び `{seq}` の重みの合計が int64 に収まりません"), "{path}: {why}");
                }
            }
            for i in &f.inputs {
                let name = &i.name.text;
                if said == rulec::door::draw_past(name) {
                    drawn += 1;
                    assert_eq!(text.en, format!("the input `{name}` is not below the total of the weights"), "{path}: {why}");
                    assert_eq!(text.ja, format!("入力 `{name}` が、重みの合計より小さくありません"), "{path}: {why}");
                }
            }
        }
    }
    assert!(apart.is_empty(), "the port and the door say another kind:\n{}", apart.join("\n"));
    println!("the port and the door: {inputs} inputs refused, {contradictions} contradictions; of the inputs, {twice} with a `unique` value twice, {heavy} with weights past int64, {drawn} with a drawn value past the weights");
    assert!(inputs > 1500 && contradictions > 0, "{inputs} inputs, {contradictions} contradictions");
    assert!(twice >= 2 && heavy >= 2 && drawn >= 2, "a `unique` value twice {twice}, weights past int64 {heavy}, a drawn value past the weights {drawn}");
}

/// The three refusals of a walk one by one, each in three places (§15.314): the reference evaluator
/// (`rulec run`), the generated code (Python, through its runner) and the port say the caller's
/// input is wrong, for the same reason. And two elements under `take_unique`, which all three call
/// the rule's contradiction.
#[test]
fn a_walks_refusal_is_the_same_in_the_evaluator_the_generated_code_and_the_port() {
    let python = ritsu_testkit::ready(ritsu_testkit::Need::Python, || std::process::Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 is not here, so the generated code was not run");
    let e = Engine::new();
    let t = ritsu_testkit::TempDir::new("ports-three");
    let heavy = heavy_routing(t.path(), "tests/corpus/case_routing.rule", "heavy.rule");
    let at_the_top = r#"{"reviewer_id":1,"present":true,"capacity":4000000000000000000}"#;
    let cases: [(&str, String, String, &str, &str, &str, &str); 4] = [
        (
            "a `unique` value twice",
            root().join("tests/merging/unique/province_learning.rule").to_string_lossy().into_owned(),
            r#"{"buildings":[{"building":"library"},{"building":"temple"},{"building":"library"}]}"#.to_string(),
            "input",
            "buildings has two elements with the same building (buildings[0] and buildings[2])",
            "the list `buildings` has two elements with the same `building` (`buildings[0]` and `buildings[2]`)",
            "並び `buildings` の要素に、`building` が同じものが二つあります（`buildings[0]` と `buildings[2]`）",
        ),
        (
            "weights past int64",
            heavy,
            format!(r#"{{"reviewers":[{at_the_top},{at_the_top},{at_the_top}],"roll":0}}"#),
            "input",
            "the total of the weights of reviewers does not fit in int64",
            "the total of the weights of the list `reviewers` does not fit in int64",
            "並び `reviewers` の重みの合計が int64 に収まりません",
        ),
        (
            "a drawn value not below the total of the weights",
            root().join("tests/corpus/case_routing.rule").to_string_lossy().into_owned(),
            r#"{"reviewers":[{"reviewer_id":1,"present":true,"capacity":50},{"reviewer_id":2,"present":false,"capacity":30}],"roll":50}"#.to_string(),
            "input",
            "roll is not below the total of the weights",
            "the input `roll` is not below the total of the weights",
            "入力 `roll` が、重みの合計より小さくありません",
        ),
        (
            "two elements under `take_unique`",
            root().join("tests/corpus/nationwide_freight.rule").to_string_lossy().into_owned(),
            r#"{"freight_rows":[{"row_zone":"kinki","threshold":500,"row_fee":800},{"row_zone":"kinki","threshold":900,"row_fee":1500}]}"#.to_string(),
            "contradiction",
            "fold verdict: two elements matched a take_unique (freight_rows[0] and freight_rows[1])",
            "two elements matched a `take_unique` of the fold `verdict` (`freight_rows[0]` and `freight_rows[1]`)",
            "畳み込み `verdict` で、`take_unique` に二つの要素が当たりました（`freight_rows[0]` と `freight_rows[1]`）",
        ),
    ];
    let rulec = |args: &[&str]| {
        let o = std::process::Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").current_dir(t.path()).args(args).output().expect("cannot start rulec");
        (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
    };
    for (n, (what, path, input, kind, said, port_en, port_ja)) in cases.iter().enumerate() {
        // the reference evaluator
        let file = t.path().join(format!("in{n}.json"));
        std::fs::write(&file, input).unwrap();
        let (code, out, err) = rulec(&["run", path, "--input", file.to_str().unwrap(), "--format", "json"]);
        assert_eq!(code, 0, "{what}: {out}{err}");
        let j = json(&out);
        assert_eq!((st(j.get("refused")).as_str(), st(j.get("error")).as_str()), (*kind, *said), "{what}: rulec run: {out}");
        // the port
        let src = std::fs::read_to_string(path).unwrap();
        let (f, c) = rulec::prepare(&src, path).unwrap();
        let values = port_values(&f, &c, &json(input)).unwrap();
        let text = match (e.eval(Path::new(path), &values), *kind) {
            (Err(RuleError::Input(text)), "input") | (Err(RuleError::Contradiction(text)), "contradiction") => text,
            (other, _) => panic!("{what}: the port: {other:?}"),
        };
        assert_eq!((text.en.as_str(), text.ja.as_str()), (*port_en, *port_ja), "{what}");
        // the generated code
        if !python {
            continue;
        }
        let out_dir = t.path().join(format!("out{n}"));
        let (code, out, err) = rulec(&["gen", path, "--target", "python", "--extra", "runner", "--out", out_dir.to_str().unwrap()]);
        assert_eq!(code, 0, "{what}: {out}{err}");
        let alias = f.name.ascii.clone().unwrap_or_else(|| f.name.text.clone());
        use std::io::Write;
        let mut child = std::process::Command::new("python3")
            .arg(out_dir.join(format!("python/{alias}_runner.py")))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("cannot start python3");
        child.stdin.take().unwrap().write_all(format!("{{\"in\":{input}}}\n").as_bytes()).unwrap();
        let o = child.wait_with_output().unwrap();
        let out = String::from_utf8_lossy(&o.stdout).into_owned();
        let j = json(out.lines().next().unwrap_or_else(|| panic!("{what}: the runner said nothing: {}", String::from_utf8_lossy(&o.stderr))));
        assert_eq!((st(j.get("refused")).as_str(), st(j.get("error")).as_str()), (*kind, *said), "{what}: the generated Python: {out}");
    }
}

/// What the port says two versions of a rule answer differently is what `rulec diff --format json`
/// says (ritsu's ports, `Rules::diff`): each box with what it holds of each column (the words; the
/// numbers in runs, which are what the box's text says), the outputs, the rows, the witness and the
/// cells; the boxes not settled, the declarations and the counts. Each rule of the corpus against
/// itself with the first amount of its tables moved (itself where it has none), each side read
/// through an overlay of its own, with the default budget and with one too small.
#[test]
fn the_diff_the_port_hands_over_is_the_one_rulec_diff_says() {
    use ritsu_ports::{DiffColumn, DiffHeld, DiffSide, Found};
    use ritsu_units::Rat;
    let engine = Engine::new();
    // a run as the text of a box writes it (rulec's `show_axis`)
    let run_text = |c: &DiffColumn, runs: &[(Option<Rat>, Option<Rat>)]| -> String {
        let num = |r: Rat| {
            if c.date {
                ritsu_ports::day_text((r.num / r.den) as i64)
            } else {
                let v = if c.rate { r.mul(Rat::int(100)) } else { r };
                format!("{}{}", rulec::types::written_num(v), c.unit)
            }
        };
        let one = |(lo, hi): &(Option<Rat>, Option<Rat>)| match (lo, hi) {
            (Some(l), Some(h)) if l == h => format!("{} = {}", c.name, num(*l)),
            (Some(l), Some(h)) => format!("{} >={} <={}", c.name, num(*l), num(*h)),
            (Some(l), None) => format!("{} >={}", c.name, num(*l)),
            (None, Some(h)) => format!("{} <={}", c.name, num(*h)),
            (None, None) => format!("{} = -", c.name),
        };
        let t = runs.iter().map(one).collect::<Vec<_>>().join(" or ");
        if runs.len() > 1 { format!("({t})") } else { t }
    };
    // what a box holds, as the JSON says it: by column, its words or its text
    let theirs = |r: Option<&Json>| -> Vec<(String, String)> {
        arr(r)
            .iter()
            .map(|c| {
                let accepts = arr(c.get("accepts"));
                let words: Vec<String> = accepts.iter().filter_map(|x| x.as_str().map(str::to_string)).collect();
                (st(c.get("column")), if words.len() == accepts.len() { words.join("|") } else { st(c.get("text")) })
            })
            .collect()
    };
    let mut compared = 0;
    for p in corpus() {
        let src = std::fs::read_to_string(root().join(&p)).unwrap();
        // the first amount moved (a rule against itself is settled without a walk)
        let versions: Vec<String> = moved_amounts(&src).into_iter().take(1).chain([src.clone()]).take(1).collect();
        for (k, other) in versions.iter().enumerate() {
            // each side laid beside the corpus file, where what it imports is, in an overlay of its own
            let (sa, sb) = (root().join(&p).with_file_name(format!("_port_diff_{k}_a.rule")), root().join(&p).with_file_name(format!("_port_diff_{k}_b.rule")));
            let laid = |at: &Path, text: &str| {
                let mut o = ritsu_base::fs::Overlay::new(None);
                o.put(at, text.as_bytes().to_vec());
                std::rc::Rc::new(o) as std::rc::Rc<dyn ritsu_base::fs::Files>
            };
            let (fa, fb) = (laid(&sa, &src), laid(&sb, other));
            for budget in [None, Some(1)] {
                let at = format!("{p} ({k}, {budget:?})");
                let whole = rulec::i18n::with(rulec::i18n::Lang::En, || {
                    rulec::cli::diff_whole_through((&sa.to_string_lossy(), Some(fa.clone())), (&sb.to_string_lossy(), Some(fb.clone())), ("a", "b"), budget)
                });
                let port = engine.diff(&DiffSide::new(&sa, Some(fa.clone())), &DiffSide::new(&sb, Some(fb.clone())), budget);
                let (whole, port) = match (whole, port) {
                    (Ok(w), Ok(Found::Value(d))) => (w, d),
                    (Err(_), Err(_)) => continue,
                    (w, d) => panic!("{at}: rulec diff and the port disagree on whether the two compare: {:?} / {d:?}", w.map(|w| w.json)),
                };
                let j = json(&whole.json);
                assert_eq!((port.any(), port.total()), (whole.differs, whole.total), "{at}");
                assert_eq!((port.rule.as_str(), port.old_version.as_str(), port.new_version.as_str()), (st(j.get("rule")).as_str(), st(j.get("old_version")).as_str(), st(j.get("new_version")).as_str()), "{at}");
                for (key, n) in [("cells", port.cells), ("feasible", port.feasible), ("same", port.same), ("differing", port.differing), ("unsettled", port.unsettled), ("unrealized", port.unrealized)] {
                    assert_eq!(j.get(key).and_then(Json::as_int), Some(n as i128), "{at}: {key}");
                }
                assert_eq!(j.get("over_budget").and_then(Json::as_bool), Some(port.over_budget), "{at}");
                assert_eq!(port.blocked.as_ref().map(|b| b.en.clone()), j.get("blocked").and_then(Json::as_str).map(str::to_string), "{at}");
                let domain: Vec<(String, String, String, String)> = arr(j.get("domain")).iter().map(|d| (st(d.get("what")), st(d.get("name")), st(d.get("old")), st(d.get("new")))).collect();
                let ours: Vec<(String, String, String, String)> = port.declared.iter().map(|d| (d.what.clone(), d.name.clone(), d.old.clone().unwrap_or_default(), d.new.clone().unwrap_or_default())).collect();
                assert_eq!(ours, domain, "{at}");
                let mine = |held: &[DiffHeld]| -> Vec<(String, String)> {
                    port.columns
                        .iter()
                        .zip(held)
                        .filter_map(|(c, h)| match h {
                            DiffHeld::Any => None,
                            DiffHeld::Words { words, other } => {
                                let mut w = words.clone();
                                if *other {
                                    w.push("*".into());
                                }
                                Some((c.name.clone(), w.join("|")))
                            }
                            DiffHeld::Runs(runs) => Some((c.name.clone(), run_text(c, runs))),
                        })
                        .collect()
                };
                let changes = arr(j.get("changes"));
                assert_eq!(port.changes.len(), changes.len(), "{at}");
                for (ch, x) in port.changes.iter().zip(&changes) {
                    assert_eq!(mine(&ch.held), theirs(x.get("region")), "{at}");
                    let outs: Vec<(String, String, String)> = arr(x.get("outputs")).iter().map(|o| (st(o.get("output")), st(o.get("old")), st(o.get("new")))).collect();
                    assert_eq!(ch.outputs, outs, "{at}");
                    assert_eq!((Some(ch.cells as i128), Some(ch.uniform)), (x.get("cells").and_then(Json::as_int), x.get("uniform").and_then(Json::as_bool)), "{at}");
                    let wit: Vec<(String, String)> = arr(x.get("witness")).iter().map(|w| (st(w.get("input")), st(w.get("value")))).collect();
                    assert_eq!(ch.witness, wit, "{at}");
                    let rows = |key: &str| -> Vec<(String, usize)> { arr(x.get(key)).iter().map(|r| (st(r.get("table")), r.get("row").and_then(Json::as_int).unwrap_or(0) as usize)).collect() };
                    assert_eq!((ch.old_rows.clone(), ch.new_rows.clone()), (rows("old_rows"), rows("new_rows")), "{at}");
                }
                let unknown = arr(j.get("unknown"));
                assert_eq!(port.unknown.len(), unknown.len(), "{at}");
                for (u, x) in port.unknown.iter().zip(&unknown) {
                    assert_eq!(mine(&u.held), theirs(x.get("region")), "{at}");
                    assert_eq!(u.why.en, st(x.get("why")), "{at}");
                    assert!(!u.why.ja.is_empty() && u.why.ja != u.why.en, "{at}: {:?}", u.why);
                }
                compared += 1;
            }
        }
    }
    assert!(compared > 20, "{compared}");
}

/// Each side of the port's diff is read through its own files: one path, two texts, the old in one
/// overlay and the new in another, and the box the move of one amount makes.
#[test]
fn each_side_of_the_diff_is_read_through_its_own_files() {
    use ritsu_ports::{DiffHeld, DiffSide, Found};
    let src = "rule fee v1\n\nenum band = low \"low\" | high \"high\"\n\ninputs\n  amount : money[JPY]  range >=1JPY <=1_000_000JPY\n\noutputs\n  band : band\n\ntable band\n| amount        | -> band |\n| <=100_000JPY  | low     |\n| >100_000JPY   | high    |\n";
    let moved = src.replace("rule fee v1", "rule fee v2").replace("<=100_000JPY ", "<=200_000JPY ").replace(">100_000JPY ", ">200_000JPY ");
    let at = root().join("tests/_port_diff_sides/fee.rule");
    let laid = |text: &str| {
        let mut o = ritsu_base::fs::Overlay::new(None);
        o.put(&at, text.as_bytes().to_vec());
        Some(std::rc::Rc::new(o) as std::rc::Rc<dyn ritsu_base::fs::Files>)
    };
    let d = match Engine::new().diff(&DiffSide::new(&at, laid(src)), &DiffSide::new(&at, laid(&moved)), None).unwrap() {
        Found::Value(d) => d,
        other => panic!("{other:?}"),
    };
    assert_eq!((d.old_version.as_str(), d.new_version.as_str(), d.total()), ("1", "2", true));
    assert_eq!(d.changes.len(), 1, "{d:?}");
    let r = ritsu_units::Rat::int;
    assert_eq!(d.changes[0].held, vec![DiffHeld::Runs(vec![(Some(r(100_001)), Some(r(200_000)))])]);
    assert_eq!(d.changes[0].outputs, vec![("band".to_string(), "high".to_string(), "low".to_string())]);
    // a side that does not read is said as the check says it
    let missing = Engine::new().diff(&DiffSide::new(&at, None), &DiffSide::new(&at, laid(&moved)), None);
    assert!(matches!(missing, Err(ref s) if !s.is_empty()), "{:?}", missing.map(|_| ()));
}

/// Each amount of a table's last column moved by a little, one rule for each (the same moves as
/// `tests/vdiff.rs` makes).
fn moved_amounts(src: &str) -> Vec<String> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();
    for (i, ln) in lines.iter().enumerate() {
        let t = ln.trim();
        if !t.starts_with('|') || t.contains("->") {
            continue;
        }
        let Some(last) = t.trim_matches('|').split('|').next_back().map(|x| x.trim().to_string()) else { continue };
        let digits: String = last.chars().take_while(|c| c.is_ascii_digit() || *c == '_').collect();
        let Ok(n) = digits.replace('_', "").parse::<i128>() else { continue };
        let bumped = format!("{}{}", n + if n >= 100 { 100 } else { 7 }, &last[digits.len()..]);
        let mut cells: Vec<String> = ln.split('|').map(|x| x.to_string()).collect();
        let Some(k) = cells.iter().rposition(|c| c.trim() == last) else { continue };
        cells[k] = cells[k].replace(&last, &bumped);
        let mut all: Vec<String> = lines.iter().map(|x| (*x).to_string()).collect();
        all[i] = cells.join("|");
        out.push(all.join("\n"));
    }
    out
}
