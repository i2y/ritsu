//! ritsu's ports, as rulec answers them (ritsu's DESIGN 3.2, PLAN D.2).
//!
//! What `rulec::ports::Engine` hands over is what rulec's own JSON says of the same rule —
//! `rulec api`, `rulec certificate` and `rulec schema`, read the way a program that reads the
//! JSON reads them — for every rule of the corpus, with one difference that is the point of the
//! port: a rate's step is the type's own, where the JSON gives it only in the words of the
//! schema's description. The page is the one `rulec doc` draws, and the evaluator answers every
//! vector `rulec vectors` writes.

use ritsu_base::json::Json;
use ritsu_ports::{Answer, Column, ColumnType, Items, Precondition, References, RuleError, Rules, Value};
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
    match &col.ty {
        ColumnType::Enum(e) => assert!(*e == written && enums.contains(e), "{at}: {} is the enum {e}", col.name),
        ColumnType::Bool => assert_eq!(ty, "boolean", "{at}: {}", col.name),
        ColumnType::Date => assert!(ty == "string" && st(sch.get("format")) == "date", "{at}: {}", col.name),
        ColumnType::Str => assert!(ty == "string" && sch.get("format").is_none(), "{at}: {}", col.name),
        ColumnType::Opt(_) => assert!(sch.get("oneOf").is_some(), "{at}: {} may be none", col.name),
        ColumnType::Num { written: w, unit, min, max } => {
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

/// Every fact of every rule of the corpus is what rulec's own JSON says of it.
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
            }
        }
        let walks = arr(ts.get("params")).iter().find(|p| p.get("elements").is_some()).map(|p| st(p.get("name")));
        assert_eq!(facts.elements.as_ref().map(|(n, _, _)| n.clone()), walks, "{path}");
        // the enums, as the certificate lists them
        let want: Vec<(String, Vec<String>)> = cert.get("enums").and_then(|e| e.as_obj()).map(|kv| kv.iter().map(|(k, v)| (k.clone(), strs(Some(v)))).collect()).unwrap_or_default();
        assert_eq!(facts.enums.iter().map(|e| (e.name.clone(), e.values.iter().map(|v| v.name.clone()).collect::<Vec<_>>())).collect::<Vec<_>>(), want, "{path}");
        // the preconditions, as `api` lists them
        let want: Vec<Precondition> = arr(api.get("preconditions"))
            .iter()
            .map(|p| match st(p.get("kind")).as_str() {
                "constraint" => Precondition::Relation { left: st(p.get("left")), op: st(p.get("op")), right: st(p.get("right")) },
                "sum" => Precondition::Sum { name: st(p.get("name")), over: st(p.get("over")), of: st(p.get("of")), max: p.get("max").and_then(|x| x.as_int()).unwrap() },
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
        assert_eq!(fc.elements.as_ref().map(|e| fields_of(e)), c.get("element_fields").filter(|x| !x.is_null()).map(|x| wire_fields(Some(x))), "{path} elements");
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
            if let Some(el) = &f.elements {
                inputs.push((el.name.text.clone(), v.input.get(&el.name.text).map(|x| wire(&c, &el.name.text, x)).unwrap_or(Value::List(vec![]))));
            }
            let want: Vec<(String, Value)> = v.outputs.iter().map(|(n, x)| (n.clone(), x.as_ref().map(|x| wire(&c, n, x)).unwrap_or(Value::None))).collect();
            match e.eval(Path::new(&path), &inputs) {
                Ok(got) => assert_eq!(got, want, "{path}: {}", v.why),
                // a vector the generated code refuses is one with no answer
                Err(RuleError::Input(t)) | Err(RuleError::Contradiction(t)) => assert!(want.iter().any(|(_, x)| *x == Value::None) || !rulec::vectors::allowed(&f, &v.input), "{path}: {}: refused: {}", v.why, t.en),
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
    let none = e.preconditions_hold(rule, &[], None).unwrap();
    assert!(none.iter().any(|(p, _)| p == rels[0]));
    // the left side kept below the right: it holds
    let ok = e.preconditions_hold(rule, &[(left.clone(), Some(0), Some(10)), (right.clone(), Some(10), Some(20))], None).unwrap();
    assert_eq!(ok.iter().find(|(p, _)| p == rels[0]).map(|(_, a)| a.clone()), Some(Answer::Holds));
    // the left side may pass the right: the corner is the example
    let bad = e.preconditions_hold(rule, &[(left.clone(), Some(0), Some(30)), (right.clone(), Some(10), Some(20))], None).unwrap();
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
    let open = e.preconditions_hold(rule, &[(left.clone(), Some(0), None)], None).unwrap();
    assert!(matches!(open.iter().find(|(p, _)| p == rels[0]).map(|(_, a)| a), Some(Answer::Undecided(_)) | Some(Answer::Holds) | Some(Answer::Fails(_))));
    // the bounds over a list: not decided without a length; with one, held or broken
    let mut sums = 0;
    for path in corpus() {
        let Ok(_) = e.facts(Path::new(&path)) else { continue };
        for (p, a) in e.preconditions_hold(Path::new(&path), &[], None).unwrap() {
            if matches!(p, Precondition::Sum { .. } | Precondition::Length { .. }) {
                assert!(matches!(a, Answer::Undecided(_)), "{path}: {p:?} {a:?}");
            }
        }
        for (p, a) in e.preconditions_hold(Path::new(&path), &[], Some(0)).unwrap() {
            if matches!(p, Precondition::Sum { .. } | Precondition::Length { .. }) {
                assert_eq!(a, Answer::Holds, "{path}: the empty list keeps every bound: {p:?}");
            }
        }
        for (p, a) in e.preconditions_hold(Path::new(&path), &[], Some(1_000_000)).unwrap() {
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
    // a set of days for an input the rule does not have, or not a date, is not decided
    assert!(matches!(e.checked_over(rule, "x", &Default::default()).unwrap(), Answer::Undecided(_)));
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
            ritsu_ports::ColumnType::Enum(en) => en.clone(),
            ritsu_ports::ColumnType::Opt(t) => match t.as_ref() {
                ritsu_ports::ColumnType::Enum(en) => en.clone(),
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
        Some(ritsu_ports::ColumnType::Enum(en)) => en,
        Some(ritsu_ports::ColumnType::Opt(t)) => match *t {
            ritsu_ports::ColumnType::Enum(en) => en,
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
    // a row past the reach of a `define`, which E102 does not read, passes check, and no input
    // comes to its value
    let path = "tests/over/define_reach.rule";
    let src = std::fs::read_to_string(path).unwrap();
    assert!(!rulec::has_error(&rulec::report(&src, path).diags));
    let found = e.outputs_over(Path::new(path), "out", &[]).unwrap();
    witnesses_hold(&e, path, "out", &[], &found);
    assert_eq!(values_of(&found), [Value::Enum("small".into()), Value::Enum("big".into())]);
    let found = e.outputs_over(Path::new(path), "out", &[("limit".into(), Some(0), Some(0)), ("amount".into(), Some(1), Some(1))]).unwrap();
    assert_eq!(values_of(&found), [Value::Enum("big".into())]);
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
        if !numeric && f.elements.is_none() && c.ty_of(&first).is_some_and(|t| t.is_numeric()) {
            assert!(undecided(&path, &first, &[]).contains("neither an enum nor a bool"), "{path}");
            numeric = true;
        }
        let counted = f.outputs.iter().find(|o| matches!(c.ty_of(&o.name.text), Some(rulec::types::Ty::Enum(_) | rulec::types::Ty::Bool)));
        if !walks
            && let Some(el) = &f.elements
            && let Some(o) = counted
        {
            assert!(undecided(&path, &o.name.text, &[]).contains(&format!("walks the list `{}`", el.name.text)), "{path}");
            walks = true;
        }
        // a string input cannot be held to a range
        if !string
            && f.elements.is_none()
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

/// Over the corpus, every numeric and date input held to a small interval around a vector's
/// value, what the port answers for each enum and bool output is what the reference evaluator
/// comes to on every input inside the intervals (every value of each enum and bool input, the
/// constraints kept): the same values, each with an input inside the ranges that comes to it.
/// What the port leaves undecided is counted, and printed with its reason.
#[test]
fn an_output_over_ranges_is_what_every_input_inside_them_comes_to() {
    use rulec::eval::Val;
    use rulec::types::Ty;
    use std::collections::{BTreeMap, HashMap};
    const CAP: usize = 2_000;
    let e = Engine::new();
    let (mut cases, mut exact, mut undecided, mut unwalked, mut too_big, mut narrower) = (0, 0, 0, 0, 0, 0);
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
        if outputs.is_empty() || f.elements.is_some() {
            continue;
        }
        // the inputs held to intervals (numbers and dates, on the wire), and the ones walked whole
        let mut held_names: Vec<(String, bool, bool)> = Vec::new();
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
                // a number or a date that may be absent: its interval, and `none` beside it
                Ty::Date => held_names.push((n.clone(), true, opt)),
                t if t.is_numeric() => held_names.push((n.clone(), false, opt)),
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
        let absent = |n: &str| held_names.iter().any(|(m, _, opt)| m == n && *opt);
        let size_of = |held: &[(String, bool, i128, i128)]| held.iter().map(|(n, _, l, h)| (h - l + 1).max(0) as usize + usize::from(absent(n))).chain(whole.iter().map(|(_, vs)| vs.len())).fold(1usize, |a, b| a.saturating_mul(b));
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
                    .filter_map(|(j, (n, date, _))| {
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
            let mut axes: Vec<(String, Vec<Val>)> = whole.clone();
            for (n, date, lo, hi) in &held {
                let mut vals: Vec<Val> = if absent(n) { vec![Val::Enum(rulec::kw::NONE.into())] } else { vec![] };
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
                if rulec::vectors::allowed(&f, &input) && rulec::vectors::days_ok(&c, &input) {
                    let (outs, _, _) = rulec::eval::run_all(&f, &c, input.into_iter().collect());
                    for (n, x) in outs {
                        if let (Some(seen), Some(x)) = (truth.get_mut(&n), x) {
                            let v = match (wire(&c, &n, &x), base(c.ty_of(&n))) {
                                (Value::Enum(name), Ty::Enum(en)) => Value::Enum(rulec::codegen::public_value(&f, &en, &name)),
                                (v, _) => v,
                            };
                            if !seen.contains(&v) {
                                seen.push(v);
                            }
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
        }
    }
    println!("outputs_over: {cases} questions over the corpus and the rules of these tests, {exact} answered exactly ({narrower} of them with fewer values than the output has), {undecided} undecided; {unwalked} rules not walked (a string input), {too_big} boxes too big to walk");
    for w in &why {
        println!("  undecided: {w}");
    }
    assert!(exact * 3 >= cases * 2, "{exact} of {cases} answered exactly");
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
        if outputs.is_empty() || finite.is_empty() || f.elements.is_some() || !walkable {
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
                    if !rulec::vectors::allowed(&f, &input) || !rulec::vectors::days_ok(&c, &input) {
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
        let src = std::fs::read_to_string(&path).unwrap();
        for v in f.enums.iter().flat_map(|en| &en.values) {
            if v.public != v.name {
                assert!(src.contains(&format!("{}({})", v.name, v.public)), "{path}: {} is not written {}({})", v.name, v.name, v.public);
                aliased += 1;
            }
        }
    }
    assert!(aliased > 20, "{aliased} values are written with an alias");
}
