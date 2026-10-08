//! `rulec api` names the errors every language's code raises or returns, and says which is which
//! and what each carries (§15.202).
//!
//! `errors` named the two in eight of the ten entries whose code has errors of its own. NumPy's
//! runtime raises the two the Python module raises (§15.200), and Go returns a pointer to one of
//! two types of its own, and neither entry named them: a caller had to read the generated file to
//! learn what to catch. Every one of the ten now has `errors`, and beside it `error_types`: each
//! error's `kind` (`input`, refused at the door; `contradiction`, the W114 guard), its name and
//! the fields a caller reads. Both are held here to the generated files — every name declared
//! there, every field one the error really has — and, where the language is installed, to the
//! code itself: Python and NumPy build the errors and read the fields, NumPy refuses a column and
//! says which element, and Go compiles a caller that tells the two apart with `errors.As`.
//!
//! The Wasm entry has no errors, since the module raises nothing: what it answers is its only way
//! of saying a refusal, so the entry says the two shapes of that answer instead (§15.206), and the
//! module built the way the entry says is held to them.
//!
//! Every case is written twice: first in English, then the Japanese version beside it.

use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, ready};
use rulec::json::Json;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rulec(lang: &str, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", lang)
        .current_dir(root())
        .args(args)
        .output()
        .expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn have(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd).args(args).output().map(|o| o.status.success()).unwrap_or(false)
}

/// The English rule first, the Japanese version beside it: a fare with a `constraint` between
/// two amounts, so the door has a refusal that is about no one value.
const RULES: [(&str, &str); 2] = [("en", "tests/corpus/express_delivery_quote.rule"), ("ja", "tests/corpus/速達の見積.rule")];

/// The entries whose code raises or returns errors of its own.
const ENTRIES: [&str; 10] = ["python", "typescript", "javascript", "rust", "ruby", "php", "go", "swift", "java", "numpy"];

fn s(j: &Json, k: &str) -> String {
    j.get(k).and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| panic!("{k} is missing: {}", rulec::json::show(j)))
}

fn arr<'a>(j: &'a Json, k: &str) -> &'a [Json] {
    match j.get(k) {
        Some(Json::Arr(a)) => a,
        _ => panic!("{k} is not an array: {}", rulec::json::show(j)),
    }
}

/// `(kind, name, fields)` of each error an entry lists.
fn error_types(entry: &Json) -> Vec<(String, String, Vec<String>)> {
    arr(entry, "error_types")
        .iter()
        .map(|e| (s(e, "kind"), s(e, "name"), arr(e, "fields").iter().map(|f| f.as_str().unwrap().to_string()).collect()))
        .collect()
}

/// The text from `start` to the first `end` after it.
fn between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let a = text.find(start)?;
    let b = text[a..].find(end).map(|b| a + b).unwrap_or(text.len());
    Some(&text[a..b])
}

/// Where a language declares one error, and the text that names one field of it there. What
/// is read is the generated source as it is, so a name or a field the inventory made up fails.
fn declared(lang: &str, src: &str, name: &str, field: &str) -> bool {
    let bare = name.trim_start_matches('*');
    match lang {
        "python" | "numpy" => between(src, &format!("class {bare}("), "\n\n\n").is_some_and(|c| c.contains(&format!("self.{field} = "))),
        "typescript" => between(src, &format!("export class {bare} extends"), "\n}\n").is_some_and(|c| c.contains(&format!("readonly {field}:"))),
        "javascript" => between(src, &format!("export class {bare} extends"), "\n}\n").is_some_and(|c| c.contains(&format!("this.{field} = "))),
        "ruby" => between(src, &format!("class {bare} <"), "\n  end\n").is_some_and(|c| {
            c.lines().any(|l| l.trim_start().starts_with("attr_reader") && l.split([' ', ',']).any(|w| w == format!(":{field}")))
        }),
        "php" => between(src, &format!("final class {bare} extends"), "\n}\n").is_some_and(|c| c.contains(&format!("${field}"))),
        "java" => between(src, &format!("class {bare} extends"), "\n    }\n").is_some_and(|c| {
            c.lines().any(|l| l.trim_start().starts_with("public final ") && l.trim_end().ends_with(&format!(" {field};")))
        }),
        "go" => between(src, &format!("type {bare} struct {{"), "\n}\n").is_some_and(|c| c.lines().any(|l| l.trim_start().starts_with(&format!("{field} ")))),
        // A variant of the enum: `Input { what: …, value: … }` in Rust, `case input(what: …)` in Swift.
        "rust" => {
            let variant = bare.rsplit("::").next().unwrap();
            between(src, "pub enum RuleError {", "\n}\n").is_some_and(|c| c.lines().any(|l| l.trim_start().starts_with(&format!("{variant} {{")) && l.contains(&format!("{field}:"))))
        }
        "swift" => {
            let case = bare.rsplit('.').next().unwrap();
            between(src, "public enum RuleError", "\n}\n").is_some_and(|c| c.lines().any(|l| l.trim_start().starts_with(&format!("case {case}(")) && l.contains(&format!("{field}:"))))
        }
        _ => panic!("no such entry: {lang}"),
    }
}

/// The file an entry's errors are declared in, as the inventory names it.
fn source(out: &Path, lang: &str, entry: &Json, alias: &str) -> String {
    let p = match lang {
        "python" => out.join("python").join(format!("{}.py", s(entry, "module"))),
        "numpy" => out.join("numpy").join(s(entry, "runtime")),
        "go" => out.join("go").join(s(entry, "package")).join(format!("{alias}.go")),
        "ruby" => out.join("ruby").join(format!("{alias}.rb")),
        _ => out.join(lang).join(s(entry, "module")),
    };
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

/// Each of the ten entries names its two errors in `errors`, the same two in `error_types` in the
/// same order — the input first, the contradiction second — and every name and every field is
/// declared in the file that entry's code is in.
#[test]
fn every_entry_names_its_errors_and_what_they_carry() {
    for (lang, rule) in RULES {
        let t = TempDir::new(&format!("api-errors-{lang}"));
        let out = t.path().to_path_buf();
        let (c, _, e) = rulec(lang, &["gen", rule, "--out", &out.to_string_lossy()]);
        assert_eq!(c, 0, "{rule}: {e}");
        let (c, api, e) = rulec(lang, &["api", rule]);
        assert_eq!(c, 0, "{rule}: {e}");
        let j = rulec::json::parse(api.trim()).unwrap();
        let alias = s(&j, "alias");
        for entry_name in ENTRIES {
            let entry = j.get(entry_name).unwrap_or_else(|| panic!("{rule}: no {entry_name} entry"));
            let types = error_types(entry);
            let kinds: Vec<&str> = types.iter().map(|(k, ..)| k.as_str()).collect();
            assert_eq!(kinds, ["input", "contradiction"], "{rule}: {entry_name}");
            let names: Vec<String> = arr(entry, "errors").iter().map(|n| n.as_str().unwrap().to_string()).collect();
            assert_eq!(names, types.iter().map(|(_, n, _)| n.clone()).collect::<Vec<_>>(), "{rule}: {entry_name}: errors and error_types name different errors");
            let src = source(&out, entry_name, entry, &alias);
            for (_, name, fields) in &types {
                assert!(!fields.is_empty(), "{rule}: {entry_name}: {name} carries nothing");
                for field in fields {
                    assert!(declared(entry_name, &src, name, field), "{rule}: {entry_name}: {name} has no field {field} in the generated file");
                }
            }
        }
        // The entries whose code has no errors of its own say how it refuses in their own way.
        for other in ["connect", "sql", "wasm"] {
            assert!(j.get(other).is_some_and(|e| e.get("error_types").is_none()), "{rule}: {other}");
        }
    }
}

/// The fields are attributes the Python errors really have, and the NumPy runtime refuses a
/// column with an error whose fields say which element and which value: `row` is its place.
#[test]
fn python_and_numpy_errors_carry_the_fields_the_inventory_lists() {
    if !ready(Need::Python, || have("python3", &["-c", "import numpy"]), "python3 with numpy is not here; the errors are not built") {
        return;
    }
    for (lang, rule) in RULES {
        let t = TempDir::new(&format!("api-errors-py-{lang}"));
        let out = t.path().to_path_buf();
        let (c, _, e) = rulec(lang, &["gen", rule, "--out", &out.to_string_lossy()]);
        assert_eq!(c, 0, "{rule}: {e}");
        let (_, api, _) = rulec(lang, &["api", rule]);
        let j = rulec::json::parse(api.trim()).unwrap();
        let py = j.get("python").unwrap();
        let np = j.get("numpy").unwrap();
        let fields = |entry: &Json| -> String {
            let pairs: Vec<String> = error_types(entry).iter().map(|(k, n, f)| format!("({k:?}, {n:?}, {f:?})")).collect();
            format!("[{}]", pairs.join(", "))
        };
        // Each error built as the code builds it, and every field the inventory lists read back.
        let script = format!(
            "import importlib, json, sys\n\
             m = importlib.import_module({module:?})\n\
             for kind, name, fields in {py_types}:\n\
             \x20   e = getattr(m, name)('x', 1) if kind == 'input' else getattr(m, name)('x')\n\
             \x20   assert all(hasattr(e, f) for f in fields), (name, fields)\n\
             sys.path.insert(0, {np_dir:?})\n\
             import rulec_np\n\
             for kind, name, fields in {np_types}:\n\
             \x20   e = getattr(rulec_np, name)('x', 1, 0) if kind == 'input' else getattr(rulec_np, name)('x', 0)\n\
             \x20   assert all(hasattr(e, f) for f in fields), (name, fields)\n\
             plan = rulec_np.load({plan:?})\n\
             cols = json.loads(sys.argv[1])\n\
             try:\n\
             \x20   plan(**cols)\n\
             except rulec_np.RuleInputError as e:\n\
             \x20   v = e.value.item() if hasattr(e.value, 'item') else e.value\n\
             \x20   print(json.dumps({{'what': e.what, 'value': v, 'row': e.row, 'word': e.word}}, ensure_ascii=False))\n",
            module = s(py, "module"),
            py_types = fields(py),
            np_dir = out.join("numpy").to_string_lossy(),
            np_types = fields(np),
            plan = out.join("numpy").join(s(np, "plan")).to_string_lossy(),
        );
        // Two elements; the second is under the low end of the first number column's range.
        let mut cols: Vec<String> = Vec::new();
        let mut pushed = false;
        let mut first = String::new();
        for c in arr(np, "columns") {
            let name = s(c, "name");
            let v = match s(c, "type").as_str() {
                "ndarray[bool]" => "[false, false]".to_string(),
                _ => {
                    let lo = c.get("range").and_then(|r| r.get("min")).and_then(|m| m.as_int()).unwrap_or(0);
                    if pushed {
                        format!("[{lo}, {lo}]")
                    } else {
                        pushed = true;
                        first = name.clone();
                        format!("[{lo}, {}]", lo - 1)
                    }
                }
            };
            cols.push(format!("{}: {v}", rulec::json::quote(&name)));
        }
        let o = Command::new("python3")
            .current_dir(out.join("python"))
            .env("TMPDIR", tmpdir_in(&out))
            .args(["-B", "-c", &script, &format!("{{{}}}", cols.join(", "))])
            .output()
            .unwrap();
        assert!(o.status.success(), "{rule}: {}", String::from_utf8_lossy(&o.stderr));
        let got = String::from_utf8_lossy(&o.stdout).into_owned();
        let e = rulec::json::parse(got.trim()).unwrap_or_else(|x| panic!("{rule}: the column was not refused ({x}): {got}"));
        assert_eq!(e.get("row").and_then(|r| r.as_int()), Some(1), "{rule}: {got}");
        assert!(s(&e, "what").starts_with(&first), "{rule}: the sentence is not about {first}: {got}");
        assert_eq!(s(&e, "word"), if lang == "ja" { "行" } else { "row" }, "{rule}: {got}");
    }
}

/// A Go caller built from the inventory alone tells the two errors apart with `errors.As` and
/// reads every field the inventory lists; a name or a field that is wrong does not compile.
#[test]
fn a_go_caller_built_from_the_inventory_reads_each_error() {
    if !ready(Need::Go, || have("go", &["version"]), "go is not here; the caller is not built") {
        return;
    }
    for (lang, rule) in RULES {
        let t = TempDir::new(&format!("api-errors-go-{lang}"));
        let out = t.path().to_path_buf();
        let (c, _, e) = rulec(lang, &["gen", rule, "--out", &out.to_string_lossy()]);
        assert_eq!(c, 0, "{rule}: {e}");
        let (_, api, _) = rulec(lang, &["api", rule]);
        let j = rulec::json::parse(api.trim()).unwrap();
        let go = j.get("go").unwrap();
        let pkg = s(go, "package");
        let mut body = format!("package main\n\nimport (\n\t\"errors\"\n\t\"fmt\"\n\n\t\"{pkg}\"\n)\n\nfunc read(err error) {{\n");
        for (i, (_, name, fields)) in error_types(go).iter().enumerate() {
            let ty = name.trim_start_matches('*');
            body.push_str(&format!("\tvar e{i} *{pkg}.{ty}\n\tif errors.As(err, &e{i}) {{\n"));
            for f in fields {
                body.push_str(&format!("\t\tfmt.Println(e{i}.{f})\n"));
            }
            body.push_str("\t}\n");
        }
        body.push_str(&format!("}}\n\nfunc main() {{\n\tread(&{pkg}.RuleInputError{{}})\n}}\n"));
        let caller = out.join("go").join("errcaller");
        std::fs::create_dir_all(&caller).unwrap();
        std::fs::write(caller.join("main.go"), &body).unwrap();
        std::fs::write(caller.join("go.mod"), format!("module errcaller\n\ngo 1.25\n\nrequire {pkg} v0.0.0\n\nreplace {pkg} => ../{pkg}\n")).unwrap();
        let o = Command::new("go")
            .current_dir(&caller)
            .args(["vet", "./..."])
            .envs([("GOPROXY", "off"), ("GOFLAGS", "-mod=mod")])
            .env("GOTMPDIR", tmpdir_in(&out))
            .output()
            .unwrap();
        assert!(o.status.success(), "{rule}: the caller built from the inventory does not compile\n{body}\n{}", String::from_utf8_lossy(&o.stderr));
    }
}

/// The keys of a JSON object, in the order they were written.
fn keys(j: &Json) -> Vec<String> {
    match j {
        Json::Obj(m) => m.iter().map(|(k, _)| k.clone()).collect(),
        _ => panic!("not an object: {}", rulec::json::show(j)),
    }
}

fn strings(j: &Json, k: &str) -> Vec<String> {
    arr(j, k).iter().map(|v| v.as_str().unwrap_or_else(|| panic!("{k}: {}", rulec::json::show(v))).to_string()).collect()
}

/// The Wasm entry says what `call` answers (§15.206): the record, with the keys every language's
/// record function writes, and in its place the line of a refusal, with its two keys and the
/// values `refused` takes. The module built the way the entry says answers in exactly those
/// shapes: a case it decides with the record's keys, an input its door refuses with `input`, and a
/// walk the rule contradicts itself on with `contradiction`, each with the sentence. The WASI
/// runner's `wire` says the same line.
#[test]
fn the_wasm_entry_says_what_its_module_answers() {
    // A walk, so that both refusals can be had: (the rule, an element its door takes, an element
    // past its range and the sentence, two elements both taking and the sentence).
    for (lang, rule, take, (past, past_said), (both, both_said)) in [
        (
            "en",
            "tests/corpus/nationwide_freight.rule",
            r#"{"freight_rows":[{"row_zone":"kinki","threshold":1000,"row_fee":500}]}"#,
            (r#"{"freight_rows":[{"row_zone":"kinki","threshold":1000,"row_fee":200000}]}"#, "row_fee is out of range: 200000"),
            (
                r#"{"freight_rows":[{"row_zone":"kinki","threshold":1000,"row_fee":500},{"row_zone":"kinki","threshold":900,"row_fee":700}]}"#,
                "fold verdict: two elements matched a take_unique",
            ),
        ),
        (
            "ja",
            "tests/corpus/全国運賃.rule",
            r#"{"運賃行":[{"行ゾーン":"近畿圏","閾値":1000,"行運賃":500}]}"#,
            (r#"{"運賃行":[{"行ゾーン":"近畿圏","閾値":1000,"行運賃":200000}]}"#, "行運賃 が範囲の外です: 200000"),
            (
                r#"{"運賃行":[{"行ゾーン":"近畿圏","閾値":1000,"行運賃":500},{"行ゾーン":"近畿圏","閾値":900,"行運賃":700}]}"#,
                "畳み込み 採用: take_unique に二件当たりました",
            ),
        ),
    ] {
        let (c, api, e) = rulec(lang, &["api", rule]);
        assert_eq!(c, 0, "{rule}: {e}");
        let j = rulec::json::parse(api.trim()).unwrap();
        let w = j.get("wasm").unwrap_or_else(|| panic!("{rule}: no wasm entry"));
        let answers = arr(w, "answers");
        let kinds: Vec<String> = answers.iter().map(|a| s(a, "kind")).collect();
        assert_eq!(kinds, ["record", "refusal"], "{rule}");
        let (record_keys, refusal_keys) = (strings(&answers[0], "keys"), strings(&answers[1], "keys"));
        assert_eq!(record_keys, ["in", "observed", "trace"], "{rule}");
        assert_eq!(refusal_keys, ["refused", "error"], "{rule}");
        let refused = strings(&answers[1], "refused");
        assert_eq!(refused, ["input", "contradiction"], "{rule}: the values of refused are the kinds of error_types");
        let wire = j.get("rust").and_then(|r| r.get("wasi")).and_then(|x| x.get("wire")).and_then(|x| x.as_str()).unwrap_or_default();
        assert!(wire.contains("\"refused\"") && wire.contains("\"error\""), "{rule}: the WASI runner's wire does not say the refusal line: {wire}");

        let toolchain = || have("node", &["--version"]) && have("rustc", &["--version"]) && rulec::backend::rust_target("wasm32-unknown-unknown");
        if !ready(Need::Rustc, toolchain, "node, rustc or its wasm32-unknown-unknown target is missing; the Wasm module's answers are not read") {
            continue;
        }
        let t = TempDir::new(&format!("api-wasm-{lang}"));
        let out = t.path().to_path_buf();
        let (c, _, e) = rulec(lang, &["gen", rule, "--out", &out.to_string_lossy()]);
        assert_eq!(c, 0, "{rule}: {e}");
        // Built the way the entry says.
        let line = s(w, "build");
        let words: Vec<&str> = line.split(' ').collect();
        let o = Command::new(words[0]).current_dir(out.join("wasm")).args(&words[1..]).env("TMPDIR", tmpdir_in(&out)).output().unwrap();
        assert!(o.status.success(), "{rule}: {}", String::from_utf8_lossy(&o.stderr));
        // A host of its own, called with the names the entry gives.
        let host = format!(
            "import {{ readFileSync }} from \"node:fs\";\n\
             const {{ instance }} = await WebAssembly.instantiate(readFileSync({module:?}), {{}});\n\
             const ex = instance.exports;\n\
             function call(text) {{\n\
               const b = new TextEncoder().encode(text);\n\
               const ptr = ex.{realloc}(0, 0, 1, b.length);\n\
               new Uint8Array(ex.{memory}.buffer, ptr, b.length).set(b);\n\
               const ret = ex.{call}(ptr, b.length);\n\
               const [p, n] = new Uint32Array(ex.{memory}.buffer, ret, 2);\n\
               const out = new TextDecoder().decode(new Uint8Array(ex.{memory}.buffer, p, n));\n\
               ex.{post}(ret);\n\
               return out;\n\
             }}\n\
             for (const line of process.argv.slice(2)) console.log(call(line));\n",
            module = s(w, "module"),
            realloc = s(w, "realloc"),
            memory = s(w, "memory"),
            call = s(w, "call"),
            post = s(w, "post_return"),
        );
        std::fs::write(out.join("wasm").join("host.mjs"), host).unwrap();
        let o = Command::new("node").current_dir(out.join("wasm")).args(["host.mjs", take, past, both]).output().unwrap();
        assert!(o.status.success(), "{rule}: {}", String::from_utf8_lossy(&o.stderr));
        let said: Vec<Json> = String::from_utf8_lossy(&o.stdout).lines().map(|l| rulec::json::parse(l).unwrap_or_else(|x| panic!("{rule}: {x}: {l}"))).collect();
        assert_eq!(said.len(), 3, "{rule}: {said:?}");
        assert_eq!(keys(&said[0]), record_keys, "{rule}: the record");
        for (a, kind, sentence) in [(&said[1], "input", past_said), (&said[2], "contradiction", both_said)] {
            assert_eq!(keys(a), refusal_keys, "{rule}: {}", rulec::json::show(a));
            assert_eq!(s(a, "refused"), kind, "{rule}");
            assert!(refused.iter().any(|r| r == kind), "{rule}: {kind}");
            assert_eq!(s(a, "error"), sentence, "{rule}");
        }
    }
}
