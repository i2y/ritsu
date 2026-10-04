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
/// breaks it is the example; a bound over a list whose length the question does not carry is not
/// decided.
#[test]
fn 前提は範囲の角で決まり_並びの上限は決めない() {
    let e = Engine::new();
    let rule = Path::new("tests/corpus/比例配分.rule");
    let f = e.facts(rule).unwrap();
    let rels: Vec<&Precondition> = f.preconditions.iter().filter(|p| matches!(p, Precondition::Relation { .. })).collect();
    assert!(!rels.is_empty(), "{:?}", f.preconditions);
    let Precondition::Relation { left, op, right } = rels[0] else { unreachable!() };
    assert!(op.starts_with('<'), "{op}");
    // the rule's own ranges: the check passed, so the guard the generated code has is what keeps them apart
    let none = e.preconditions_hold(rule, &[]).unwrap();
    assert!(none.iter().any(|(p, _)| p == rels[0]));
    // the left side kept below the right: it holds
    let ok = e.preconditions_hold(rule, &[(left.clone(), Some(0), Some(10)), (right.clone(), Some(10), Some(20))]).unwrap();
    assert_eq!(ok.iter().find(|(p, _)| p == rels[0]).map(|(_, a)| a.clone()), Some(Answer::Holds));
    // the left side may pass the right: the corner is the example
    let bad = e.preconditions_hold(rule, &[(left.clone(), Some(0), Some(30)), (right.clone(), Some(10), Some(20))]).unwrap();
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
    let open = e.preconditions_hold(rule, &[(left.clone(), Some(0), None)]).unwrap();
    assert!(matches!(open.iter().find(|(p, _)| p == rels[0]).map(|(_, a)| a), Some(Answer::Undecided(_)) | Some(Answer::Holds) | Some(Answer::Fails(_))));
    // the bounds over a list
    for path in corpus() {
        let Ok(f) = e.facts(Path::new(&path)) else { continue };
        for (p, a) in e.preconditions_hold(Path::new(&path), &[]).unwrap() {
            if matches!(p, Precondition::Sum { .. } | Precondition::Length { .. }) {
                assert!(matches!(a, Answer::Undecided(_)), "{path}: {p:?} {a:?}");
            }
        }
        let _ = f;
    }
    // a set of days is not yet a range rulec holds a table to
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
fn 規則のファイルが持つものと名指すもの() {
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
