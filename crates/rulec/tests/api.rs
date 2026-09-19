//! `rulec api`: the inventory of how to call the generated code (§8.6).
//!
//! An inventory that is written by hand rots; one that is merely *computed* from the same AST
//! can still drift from what the emitters actually write. So these tests do not compare the
//! inventory with the generator's intentions — they compare it with the **generated files**,
//! and where a toolchain is available they make the generated code answer.
//!
//! - Python: import the module and hold `inspect.signature` to the inventory.
//! - Go: build a calling program mechanically from the inventory alone and run `go vet`
//!   over it. If a single name in the inventory were wrong, it would not compile.
//! - Swift: the same, type-checked by `swiftc`.

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn have(cmd: &str) -> bool {
    ["--version", "version"]
        .iter()
        .any(|a| Command::new(cmd).arg(a).output().map(|o| o.status.success()).unwrap_or(false))
}

fn api(rule: &str) -> rulec::json::Json {
    let (c, out, e) = run(&["api", rule]);
    assert_eq!(c, 0, "{e}");
    rulec::json::parse(out.trim()).unwrap_or_else(|x| panic!("JSON として読めない: {x}\n{out}"))
}

fn s(j: &rulec::json::Json, k: &str) -> String {
    j.get(k).and_then(|v| v.as_str()).unwrap_or_else(|| panic!("{k} が無い")).to_string()
}

fn arr<'a>(j: &'a rulec::json::Json, k: &str) -> &'a Vec<rulec::json::Json> {
    match j.get(k) {
        Some(rulec::json::Json::Arr(a)) => a,
        _ => panic!("{k} が配列でない"),
    }
}

/// Generate a rule into a fresh directory and return it with its inventory.
fn setup(tag: &str, rule: &str) -> (PathBuf, rulec::json::Json) {
    let dir = std::env::temp_dir().join(format!("rulec-api-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = dir.to_string_lossy().to_string();
    let (c, _, e) = run(&["gen", rule, "--out", &out]);
    assert_eq!(c, 0, "{e}");
    (dir, api(rule))
}

const RULES: &[(&str, &str)] = &[
    ("single", "tests/corpus/送料.rule"),
    ("multi", "tests/corpus/クーポン一枚.rule"),
    ("date", "tests/corpus/期間区分.rule"),
];

#[test]
fn 署名とガードが生成物と一致する() {
    // The cheapest check that cannot be fooled: every name and number the inventory states
    // has to occur in the file the generator wrote.
    for (tag, rule) in RULES {
        let (dir, j) = setup(tag, rule);
        let py_j = j.get("python").unwrap();
        let go_j = j.get("go").unwrap();
        let py = std::fs::read_to_string(dir.join("python").join(format!("{}.py", s(&j, "alias")))).unwrap();
        let go = std::fs::read_to_string(
            dir.join("go").join(s(go_j, "package")).join(format!("{}.go", s(&j, "alias"))),
        )
        .unwrap();

        let ts_j = j.get("typescript").unwrap();
        let ts = std::fs::read_to_string(dir.join("typescript").join(s(ts_j, "module"))).unwrap();
        let js_j = j.get("javascript").unwrap();
        let js = std::fs::read_to_string(dir.join("javascript").join(s(js_j, "module"))).unwrap();
        let rs_j = j.get("rust").unwrap();
        let rs = std::fs::read_to_string(dir.join("rust").join(s(rs_j, "module"))).unwrap();
        let sw_j = j.get("swift").unwrap();
        let sw = std::fs::read_to_string(dir.join("swift").join(s(sw_j, "module"))).unwrap();
        let php_j = j.get("php").unwrap();
        let php = std::fs::read_to_string(dir.join("php").join(s(php_j, "module"))).unwrap();
        let jv_j = j.get("java").unwrap();
        let jv = std::fs::read_to_string(dir.join("java").join(s(jv_j, "module"))).unwrap();

        assert!(py.contains(&s(py_j, "signature")), "python の署名が違う: {}", s(py_j, "signature"));
        assert!(ts.contains(&s(ts_j, "signature")), "typescript の署名が違う: {}", s(ts_j, "signature"));
        assert!(rs.contains(&s(rs_j, "signature")), "rust の署名が違う: {}", s(rs_j, "signature"));
        assert!(go.contains(&s(go_j, "signature")), "go の署名が違う: {}", s(go_j, "signature"));
        assert!(sw.contains(&s(sw_j, "signature")), "swift の署名が違う: {}", s(sw_j, "signature"));
        assert!(php.contains(&s(php_j, "signature")), "php の署名が違う: {}", s(php_j, "signature"));
        assert!(jv.contains(&s(jv_j, "signature")), "java の署名が違う: {}", s(jv_j, "signature"));
        // The traced twin (§15.33) is part of the inventory too, and has to be in the file
        // exactly as the inventory spells it.
        assert!(js.contains(&s(js_j, "signature")), "javascript の署名が違う: {}", s(js_j, "signature"));
        for (lang, file, j) in [("python", &py, py_j), ("typescript", &ts, ts_j), ("javascript", &js, js_j), ("rust", &rs, rs_j), ("go", &go, go_j), ("swift", &sw, sw_j), ("php", &php, php_j), ("java", &jv, jv_j)] {
            let sig = s(j, "traced_signature");
            assert!(file.contains(&sig), "{lang} の traced の署名が違う: {sig}");
            assert!(sig.contains(&s(j, "traced")), "{lang}: traced の名前が署名に無い: {sig}");
            // And the record function (§15.35), the same way.
            let sig = s(j, "record_signature");
            assert!(file.contains(&sig), "{lang} の record の署名が違う: {sig}");
            assert!(sig.contains(&s(j, "record")), "{lang}: record の名前が署名に無い: {sig}");
        }

        // The entry guards. A range in the inventory that the guard does not enforce would
        // send a caller values the code then rejects.
        for p in arr(py_j, "params") {
            if let Some(r) = p.get("range") {
                let (lo, hi) = (r.get("min").unwrap(), r.get("max").unwrap());
                let alias = s(p, "alias");
                assert!(
                    py.contains(&format!("if not {lo} <= {alias} <= {hi}:")),
                    "python のガードが範囲と食い違う: {alias} {lo}..{hi}"
                );
            }
        }
        // PHP and Java declare the kind of every parameter, so their guard is the range
        // alone — the one Go, Rust and Swift emit (§15.77, §15.78).
        for p in arr(php_j, "params") {
            if let Some(r) = p.get("range") {
                let (lo, hi) = (r.get("min").unwrap(), r.get("max").unwrap());
                let v = format!("${}", s(p, "alias"));
                assert!(
                    php.contains(&format!("if ({v} < {lo} || {v} > {hi}) {{")),
                    "php のガードが範囲と食い違う: {v} {lo}..{hi}\n{php}"
                );
            }
        }
        for p in arr(jv_j, "params") {
            if let Some(r) = p.get("range") {
                let (lo, hi) = (r.get("min").unwrap(), r.get("max").unwrap());
                let v = s(p, "alias");
                assert!(
                    jv.contains(&format!("if ({v} < {lo}L || {v} > {hi}L) {{")),
                    "java のガードが範囲と食い違う: {v} {lo}..{hi}\n{jv}"
                );
            }
        }
        for p in arr(go_j, "input_fields") {
            if let Some(r) = p.get("range") {
                let (lo, hi) = (r.get("min").unwrap(), r.get("max").unwrap());
                let f = format!("in.{}", s(p, "alias"));
                assert!(
                    go.contains(&format!("if int64({f}) < {lo} || int64({f}) > {hi} {{")),
                    "go のガードが範囲と食い違う: {f} {lo}..{hi}\n{go}"
                );
            }
        }

        for p in arr(ts_j, "params") {
            if let Some(r) = p.get("range") {
                let (lo, hi) = (r.get("min").unwrap(), r.get("max").unwrap());
                let alias = s(p, "alias");
                assert!(
                    ts.contains(&format!("if ({alias} < {lo}n || {alias} > {hi}n) {{")),
                    "typescript のガードが範囲と食い違う: {alias} {lo}..{hi}\n{ts}"
                );
            }
        }

        for p in arr(rs_j, "params") {
            if let Some(r) = p.get("range") {
                let (lo, hi) = (r.get("min").unwrap(), r.get("max").unwrap());
                let a = s(p, "alias");
                // A branded input is an i64 inside, so the guard reads it through `.0`.
                let v = if s(p, "type").chars().next().is_some_and(|c| c.is_uppercase()) {
                    format!("{a}.0")
                } else {
                    a.clone()
                };
                assert!(
                    rs.contains(&format!("if {v} < {lo} || {v} > {hi} {{")),
                    "rust のガードが範囲と食い違う: {v} {lo}..{hi}\n{rs}"
                );
            }
        }

        for p in arr(sw_j, "params") {
            if let Some(r) = p.get("range") {
                let (lo, hi) = (r.get("min").unwrap(), r.get("max").unwrap());
                let a = s(p, "alias");
                // A branded input is an Int64 inside, so the guard reads it through
                // `.value`. `Int64` itself is capitalised, so the test cannot go by case
                // the way the Rust one does — it goes by the three names that are not
                // brands.
                let t = s(p, "type");
                let v = if matches!(t.as_str(), "Int64" | "Bool" | "String") {
                    a.clone()
                } else {
                    format!("{a}.value")
                };
                assert!(
                    sw.contains(&format!("if {v} < {lo} || {v} > {hi} {{")),
                    "swift のガードが範囲と食い違う: {v} {lo}..{hi}\n{sw}"
                );
            }
        }

        // Enum members, under the spelling each language gives them.
        for e in arr(rs_j, "enums") {
            assert!(rs.contains(&format!("pub enum {} {{", s(e, "alias"))), "{}", s(e, "alias"));
            for v in arr(e, "values") {
                assert!(
                    rs.contains(&format!("    {},", s(v, "alias"))),
                    "rust の列挙値が違う: {}",
                    s(v, "alias")
                );
            }
        }
        for e in arr(ts_j, "enums") {
            assert!(ts.contains(&format!("export const {} = {{", s(e, "alias"))), "{}", s(e, "alias"));
            for v in arr(e, "values") {
                assert!(
                    ts.contains(&format!("  {}: \"{}\",", s(v, "alias"), s(v, "name"))),
                    "typescript の列挙値が違う: {}",
                    s(v, "alias")
                );
            }
        }
        for e in arr(py_j, "enums") {
            assert!(py.contains(&format!("class {}(enum.Enum):", s(e, "alias"))), "{}", s(e, "alias"));
            for v in arr(e, "values") {
                assert!(
                    py.contains(&format!("    {} = \"{}\"", s(v, "alias"), s(v, "name"))),
                    "python の列挙値が違う: {}",
                    s(v, "alias")
                );
            }
        }
        for e in arr(go_j, "enums") {
            for v in arr(e, "values") {
                assert!(go.contains(&s(v, "alias")), "go の列挙値が違う: {}", s(v, "alias"));
            }
        }
        for e in arr(sw_j, "enums") {
            assert!(
                sw.contains(&format!("public enum {}: String, CaseIterable, Sendable {{", s(e, "alias"))),
                "{}",
                s(e, "alias")
            );
            for v in arr(e, "values") {
                // The raw value is the source name, which is what the wire carries.
                assert!(
                    sw.contains(&format!("    case {} = \"{}\"", s(v, "alias"), s(v, "name"))),
                    "swift の列挙値が違う: {}",
                    s(v, "alias")
                );
            }
        }

        // The errors Python can raise are classes it really defines.
        for err in arr(py_j, "errors") {
            let n = err.as_str().unwrap();
            assert!(py.contains(&format!("class {n}(")), "{n} が生成物に無い");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn pythonの実物の署名と一致する() {
    if !have("python3") {
        eprintln!("注意: python3 が無いので飛ばした");
        return;
    }
    for (tag, rule) in RULES {
        let (dir, j) = setup(&format!("py{tag}"), rule);
        let py_j = j.get("python").unwrap();
        let module = s(py_j, "module");
        let func = s(py_j, "function");
        let names: Vec<String> =
            arr(py_j, "params").iter().map(|p| s(p, "alias")).collect();
        let outs: Vec<String> = arr(py_j, "outputs").iter().map(|p| s(p, "alias")).collect();
        let script = format!(
            "import importlib, inspect, json, sys\n\
             m = importlib.import_module({module:?})\n\
             f = getattr(m, {func:?})\n\
             sig = inspect.signature(f)\n\
             got = list(sig.parameters)\n\
             assert got == {names:?}, (got, {names:?})\n\
             # With several outputs the return is a NamedTuple whose fields are the outputs.\n\
             outs = {outs:?}\n\
             if len(outs) > 1:\n\
             \x20   assert list(m.Output._fields) == outs, (m.Output._fields, outs)\n\
             for e in {enums:?}:\n\
             \x20   t = getattr(m, e[0])\n\
             \x20   assert [v.name for v in t] == e[1], ([v.name for v in t], e[1])\n\
             for err in {errs:?}:\n\
             \x20   assert issubclass(getattr(m, err), Exception)\n\
             print('ok')\n",
            enums = arr(py_j, "enums")
                .iter()
                .map(|e| (s(e, "alias"), arr(e, "values").iter().map(|v| s(v, "alias")).collect::<Vec<_>>()))
                .collect::<Vec<_>>(),
            errs = arr(py_j, "errors").iter().map(|e| e.as_str().unwrap().to_string()).collect::<Vec<_>>(),
        );
        let o = Command::new("python3")
            .current_dir(dir.join("python"))
            .args(["-B", "-c", &script])
            .output()
            .unwrap();
        assert!(
            o.status.success(),
            "{rule}: api と実物の署名が食い違う\n{}",
            String::from_utf8_lossy(&o.stderr)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn goは目録から組んだ呼び出しがvetを通る() {
    if !have("go") {
        eprintln!("注意: go が無いので飛ばした");
        return;
    }
    for (tag, rule) in RULES {
        let (dir, j) = setup(&format!("go{tag}"), rule);
        let go_j = j.get("go").unwrap();
        let pkg = s(go_j, "package");
        // Build the call from the inventory alone: the field names, their types, and the
        // enum members. A single wrong name here does not compile.
        let mut body = String::new();
        body.push_str(&format!("package main\n\nimport (\n\t\"fmt\"\n\n\t\"{pkg}\"\n)\n\nfunc main() {{\n"));
        body.push_str(&format!("\tin := {pkg}.{}{{}}\n", s(go_j, "input_type")));
        for f in arr(go_j, "input_fields") {
            let (name, ty) = (s(f, "alias"), s(f, "type"));
            let v = match ty.as_str() {
                "bool" => "false".to_string(),
                "string" => "\"\"".to_string(),
                // A builtin is not qualified by the package.
                "int64" | "int" => {
                    f.get("range").and_then(|r| r.get("min")).map(|v| format!("{v}")).unwrap_or("0".into())
                }
                // An enum takes its first member; anything numeric takes the low end of the
                // range the inventory states.
                t if arr(go_j, "enums").iter().any(|e| s(e, "alias") == t) => {
                    let e = arr(go_j, "enums").iter().find(|e| s(e, "alias") == t).unwrap();
                    format!("{pkg}.{}", s(&arr(e, "values")[0], "alias"))
                }
                t => {
                    let lo = f.get("range").and_then(|r| r.get("min")).map(|v| format!("{v}")).unwrap_or("0".into());
                    format!("{pkg}.{t}({lo})")
                }
            };
            body.push_str(&format!("\tin.{name} = {v}\n"));
        }
        body.push_str(&format!("\tout, err := {pkg}.{}(in)\n", s(go_j, "func")));
        body.push_str("\tif err != nil {\n\t\tfmt.Println(err)\n\t\treturn\n\t}\n");
        let outs = arr(go_j, "output_fields");
        if outs.len() > 1 {
            for f in outs {
                body.push_str(&format!("\tfmt.Println(out.{})\n", s(f, "alias")));
            }
        } else {
            body.push_str("\tfmt.Println(out)\n");
        }
        body.push_str("}\n");

        let caller = dir.join("go").join("apicaller");
        std::fs::create_dir_all(&caller).unwrap();
        std::fs::write(caller.join("main.go"), &body).unwrap();
        std::fs::write(
            caller.join("go.mod"),
            format!("module apicaller\n\ngo 1.25\n\nrequire {pkg} v0.0.0\n\nreplace {pkg} => ../{pkg}\n"),
        )
        .unwrap();
        let o = Command::new("go")
            .current_dir(&caller)
            .args(["vet", "./..."])
            // The generated code has no external dependencies, and neither does this caller.
            .envs([("GOPROXY", "off"), ("GOFLAGS", "-mod=mod")])
            .output()
            .unwrap();
        assert!(
            o.status.success(),
            "{rule}: 目録から組んだ呼び出しが通らない\n{body}\n{}",
            String::from_utf8_lossy(&o.stderr)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn 検査を通らない規則からは目録を出さない() {
    let (c, _, e) = run(&["api", "tests/mutants/m_e103.rule"]);
    assert_eq!(c, 1, "{e}");
}

/// The range the inventory states for an output that comes from a rate column is the
/// column's own — its cells run to 150%, so the output runs to 15,000 (§15.34). An assumed
/// 0..100% used to put 10,000 here.
#[test]
fn 出力の範囲は表の列のセルから来る() {
    let j = api("tests/corpus/会員特典.rule");
    let py = j.get("python").unwrap();
    let pts = arr(py, "outputs").iter().find(|o| s(o, "name") == "付与点").expect("付与点 が無い");
    let r = pts.get("range").expect("付与点 に範囲が無い");
    assert_eq!(r.get("min").and_then(|v| v.as_int()), Some(0));
    assert_eq!(r.get("max").and_then(|v| v.as_int()), Some(15000), "{}", r.get("max").map(|v| v.as_int()).flatten().unwrap_or(-1));
}

/// `docs/generated-code.md` is the prose companion; it must not promise a name the tool does
/// not emit.
#[test]
fn 生成物の文書が実物の名前を使っている() {
    let doc = std::fs::read_to_string(root().join("docs/generated-code.md")).unwrap();
    let (dir, j) = setup("doc", "tests/corpus/クーポン一枚.rule");
    let py_j = j.get("python").unwrap();
    let go_j = j.get("go").unwrap();
    let php_j = j.get("php").unwrap();
    let jv_j = j.get("java").unwrap();
    for want in [
        s(py_j, "signature"),
        s(go_j, "signature"),
        s(py_j, "traced_signature"),
        s(go_j, "traced_signature"),
        s(php_j, "traced_signature"),
        s(jv_j, "traced_signature"),
        "RuleInputError".into(),
        "RuleContradictionError".into(),
        "rulec api".into(),
        "gen --check".into(),
    ] {
        assert!(doc.contains(&want), "docs/generated-code.md に `{want}` が無い");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The inventory has to be enough to *call* the TypeScript without opening the generated
/// file: assemble a caller from the inventory alone and let node run it. A name the
/// inventory gets wrong fails here, the same way `go vet` catches it on the Go side.
#[test]
fn typescriptは目録から組んだ呼び出しが動く() {
    if !have("node") {
        eprintln!("注意: node が無いので飛ばした");
        return;
    }
    for (tag, rule) in RULES {
        let (dir, j) = setup(&format!("ts{tag}"), rule);
        let ts_j = j.get("typescript").unwrap();
        let func = s(ts_j, "function");
        let module = s(ts_j, "module");
        // One argument per parameter, built from what the inventory says about it: an enum
        // member by its own spelling, a boolean, a number at the bottom of its range.
        let mut args: Vec<String> = Vec::new();
        let mut names: Vec<String> = vec![func.clone()];
        for p in arr(ts_j, "params") {
            let ty = s(p, "type");
            let lo = p.get("range").and_then(|r| r.get("min")).map(|v| format!("{v:?}"));
            let lo = lo.unwrap_or_else(|| "0".into());
            let lo = lo.trim_start_matches("Int(").trim_end_matches(')').to_string();
            args.push(match ty.as_str() {
                "boolean" => "true".to_string(),
                "bigint" => format!("{lo}n"),
                t if t.chars().next().is_some_and(|c| c.is_uppercase()) && !arr(ts_j, "enums").is_empty()
                    && arr(ts_j, "enums").iter().any(|e| s(e, "alias") == t) =>
                {
                    let e = arr(ts_j, "enums").iter().find(|e| s(e, "alias") == t).unwrap();
                    if !names.contains(&t.to_string()) {
                        names.push(t.to_string());
                    }
                    format!("{t}.{}", s(&arr(e, "values")[0], "alias"))
                }
                t => {
                    // A branded bigint: the brand is a type, so it goes in an `import type`.
                    let _ = t;
                    format!("{lo}n as never")
                }
            });
        }
        let outs: Vec<String> = arr(ts_j, "outputs").iter().map(|p| s(p, "alias")).collect();
        let script = format!(
            "import {{ {} }} from \"./{module}\";\n\
             const r = {func}({});\n\
             const outs = {outs:?};\n\
             if (outs.length > 1) {{\n  \
                 for (const o of outs) {{\n    \
                     if (!(o in (r as object))) {{ throw new Error(`missing ${{o}}`); }}\n  }}\n}}\n\
             console.log(\"ok\");\n",
            names.join(", "),
            args.join(", ")
        );
        let p = dir.join("typescript").join("_api_call.ts");
        std::fs::write(&p, script).unwrap();
        let o = std::process::Command::new("node")
            .current_dir(dir.join("typescript"))
            .args(["--no-warnings", "_api_call.ts"])
            .output()
            .expect("node を起動できない");
        assert!(
            o.status.success(),
            "{tag}: 目録から組んだ TypeScript の呼び出しが動かない:\n{}",
            String::from_utf8_lossy(&o.stderr)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Ruby, the same way: the inventory alone has to describe the module well enough to reach
/// the method, the enum constants and the error classes. Ruby has no static check to lean
/// on, so reflection stands in for `go vet` — `method(:x).parameters` is the real signature.
#[test]
fn rubyの実物の署名と一致する() {
    if !have("ruby") {
        eprintln!("注意: ruby が無いので飛ばした");
        return;
    }
    for (tag, rule) in RULES {
        let (dir, j) = setup(&format!("rb{tag}"), rule);
        let rb_j = j.get("ruby").unwrap();
        let module = s(rb_j, "module");
        let func = s(rb_j, "function");
        let file = s(j.get("python").unwrap(), "module");
        let names: Vec<String> = arr(rb_j, "params").iter().map(|p| s(p, "alias")).collect();
        let outs: Vec<String> = arr(rb_j, "outputs").iter().map(|p| s(p, "alias")).collect();
        let enums: Vec<(String, Vec<String>)> = arr(rb_j, "enums")
            .iter()
            .map(|e| (s(e, "alias"), arr(e, "values").iter().map(|v| s(v, "alias").to_uppercase()).collect()))
            .collect();
        let errs: Vec<String> =
            arr(rb_j, "errors").iter().map(|e| e.as_str().unwrap().to_string()).collect();
        // Every Ruby literal is bound to a variable first: interpolating an array into a
        // `raise` message would put a double quote inside a double-quoted string.
        let script = format!(
            "require_relative {file:?}\n\
             want = {names:?}\n\
             outs = {outs:?}\n\
             enums = {enums}\n\
             errs = {errs:?}\n\
             m = Object.const_get({module:?})\n\
             got = m.method({func:?}).parameters.map {{ |_, n| n.to_s }}\n\
             raise \"params: #{{got}} vs #{{want}}\" unless got == want\n\
             if outs.size > 1\n  \
               ms = m.const_get(:Output).members.map(&:to_s)\n  \
               raise \"members: #{{ms}} vs #{{outs}}\" unless ms == outs\n\
             end\n\
             enums.each do |name, vals|\n  \
               t = m.const_get(name)\n  \
               vals.each {{ |v| raise \"enum #{{name}}::#{{v}}\" unless t.const_defined?(v) }}\n  \
               raise \"ALL #{{name}}\" unless t::ALL.size == vals.size\n\
             end\n\
             errs.each {{ |e| raise \"error #{{e}}\" unless m.const_get(e) < Exception }}\n\
             puts 'ok'\n",
            enums = format!(
                "[{}]",
                enums
                    .iter()
                    .map(|(n, vs)| format!("[{n:?}, {vs:?}]"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        );
        let o = Command::new("ruby")
            .current_dir(dir.join("ruby"))
            .args(["-e", &script])
            .output()
            .unwrap();
        assert!(
            o.status.success(),
            "{rule}: api と実物の Ruby が食い違う\n{}",
            String::from_utf8_lossy(&o.stderr)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Swift, the way Go and TypeScript are done: assemble a caller out of the inventory alone
/// and let `swiftc` type-check it beside the generated module. A name the inventory gets
/// wrong — a function, an argument label, a brand, an enum case, an error — does not
/// compile, and nothing here reads the generated file to find out what to write.
#[test]
fn swiftは目録から組んだ呼び出しが動く() {
    if !have("swiftc") {
        eprintln!("注意: swiftc が無いので飛ばした");
        return;
    }
    for (tag, rule) in RULES {
        let (dir, j) = setup(&format!("sw{tag}"), rule);
        let sw_j = j.get("swift").unwrap();
        let enums = arr(sw_j, "enums");
        // One argument per parameter, built from what the inventory says about it: an enum
        // case by its own spelling, a boolean, a number at the bottom of its range, and a
        // brand wrapped around that number.
        let mut args: Vec<String> = Vec::new();
        for p in arr(sw_j, "params") {
            let ty = s(p, "type");
            let lo = p
                .get("range")
                .and_then(|r| r.get("min"))
                .map(|v| format!("{v:?}"))
                .unwrap_or_else(|| "0".into());
            let lo = lo.trim_start_matches("Int(").trim_end_matches(')').to_string();
            let v = match ty.as_str() {
                "Bool" => "true".to_string(),
                "Int64" => lo,
                "String" => "\"\"".to_string(),
                t if enums.iter().any(|e| s(e, "alias") == t) => {
                    let e = enums.iter().find(|e| s(e, "alias") == t).unwrap();
                    format!("{t}.{}", s(&arr(e, "values")[0], "alias"))
                }
                t => format!("{t}({lo})"),
            };
            // A keyword is taken as an argument label as it stands, so the inventory's
            // spelling goes in unquoted.
            args.push(format!("{}: {v}", s(p, "alias").trim_matches('`')));
        }
        let outs = arr(sw_j, "outputs");
        let mut body = format!("let got = try {}({})\n", s(sw_j, "function"), args.join(", "));
        if outs.len() == 1 {
            body.push_str("_ = got\n");
        } else {
            for o in outs {
                body.push_str(&format!("_ = got.{}\n", s(o, "alias")));
            }
        }
        // Every case of every enum, and both errors, named the way the inventory spells them.
        for e in enums {
            for v in arr(e, "values") {
                body.push_str(&format!("_ = {}.{}\n", s(e, "alias"), s(v, "alias")));
            }
        }
        for err in arr(sw_j, "errors") {
            body.push_str(&format!("_ = {}(\"\")\n", err.as_str().unwrap()));
        }
        // Top-level code is only allowed in a file called `main.swift`, so that is the name
        // — beside the module, not in place of the runner, which is left out of this build.
        let p = dir.join("swift").join("main.swift");
        std::fs::write(&p, format!("do {{\n{body}}} catch {{\n    print(error)\n}}\n")).unwrap();
        let o = Command::new("swiftc")
            .current_dir(dir.join("swift"))
            .args(["-typecheck", &s(sw_j, "module"), "main.swift"])
            .output()
            .expect("swiftc を起動できない");
        assert!(
            o.status.success(),
            "{rule}: api から組んだ Swift の呼び出しが通らない\n{}",
            String::from_utf8_lossy(&o.stderr)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The documents a rule transcribes are in the inventory too, as the generated header names
/// them (§15.71).
#[test]
fn 出典は目録に載る() {
    let (c, out, _) = run(&["api", "tests/corpus/印紙税の本則と軽減.rule"]);
    assert_eq!(c, 0, "{out}");
    let j = rulec::json::parse(&out).unwrap();
    let Some(rulec::json::Json::Arr(srcs)) = j.get("sources") else { panic!("sources が無い: {out}") };
    assert_eq!(srcs.len(), 2);
    assert_eq!(s(&srcs[1], "name"), "措置法");
    assert_eq!(s(&srcs[1], "kind"), "law");
    assert_eq!(s(&srcs[1], "id"), "332AC0000000026");
    assert_eq!(s(&srcs[1], "asof"), "2026-04-01");
    let Some(rulec::json::Json::Arr(pins)) = srcs[1].get("pins") else { panic!("pins が無い: {out}") };
    assert_eq!(s(&pins[0], "fragment"), "第91条");
    assert_eq!(s(&pins[0], "sha256"), "85faf53f6f6e8196");
}

/// The `wasi` entry inside the Rust entry (§15.63): the rule reached as a command that reads
/// stdin and writes stdout. The names have to be files `gen` really wrote, and where the
/// toolchain is installed the two lines have to build something that answers exactly what the
/// native runner answers — otherwise the inventory would be describing a shape nobody can get
/// to, which is the failure this file exists to prevent.
#[test]
fn wasiの項は実際に組めて同じ答えを返す() {
    let (dir, j) = setup("wasi", "tests/corpus/送料.rule");
    let rs = j.get("rust").expect("rust の項が無い");
    let w = rs.get("wasi").expect("rust の中に wasi の項が無い");
    let rust = dir.join("rust");
    assert!(rust.join(s(w, "source")).exists(), "{} が無い", s(w, "source"));
    assert_eq!(s(w, "run"), format!("wasmtime {}", s(w, "module")));
    // The column is closed at one: no other language claims the shape.
    for id in ["python", "typescript", "javascript", "ruby", "php", "go", "swift", "java", "sql", "wasm"] {
        if let Some(e) = j.get(id) {
            assert!(e.get("wasi").is_none(), "{id} に wasi の項がある");
        }
    }

    if !(have("rustc") && have("wasmtime") && rulec::backend::rust_target("wasm32-wasip1")) {
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    let build = s(w, "build");
    let words: Vec<&str> = build.split(' ').collect();
    let o = Command::new(words[0]).current_dir(&rust).args(&words[1..]).output().expect("rustc を起動できない");
    assert!(o.status.success(), "api の build 行が通らない: {build}\n{}", String::from_utf8_lossy(&o.stderr));

    // The same runner built natively, over the same vectors: the two have to agree byte for byte.
    let native = s(rs, "module").replace(".rs", "");
    let o = Command::new("rustc")
        .current_dir(&rust)
        .args(["--edition", "2021", "-O", &format!("{native}_runner.rs"), "-o", &native])
        .output()
        .expect("rustc を起動できない");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));

    let vectors = dir.join("vectors").join(format!("{native}.jsonl"));
    let pipe = || std::process::Stdio::from(std::fs::File::open(&vectors).expect("ベクタが無い"));
    let a = Command::new(format!("./{native}")).current_dir(&rust).stdin(pipe()).output().unwrap();
    let run = s(w, "run");
    let rw: Vec<&str> = run.split(' ').collect();
    let b = Command::new(rw[0]).current_dir(&rust).args(&rw[1..]).stdin(pipe()).output().expect("wasmtime を起動できない");
    assert!(b.status.success(), "api の run 行が通らない: {run}\n{}", String::from_utf8_lossy(&b.stderr));
    assert_eq!(
        String::from_utf8_lossy(&a.stdout),
        String::from_utf8_lossy(&b.stdout),
        "WASI の答えがネイティブと違う"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
