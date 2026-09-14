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
        let rs_j = j.get("rust").unwrap();
        let rs = std::fs::read_to_string(dir.join("rust").join(s(rs_j, "module"))).unwrap();

        assert!(py.contains(&s(py_j, "signature")), "python の署名が違う: {}", s(py_j, "signature"));
        assert!(ts.contains(&s(ts_j, "signature")), "typescript の署名が違う: {}", s(ts_j, "signature"));
        assert!(rs.contains(&s(rs_j, "signature")), "rust の署名が違う: {}", s(rs_j, "signature"));
        assert!(go.contains(&s(go_j, "signature")), "go の署名が違う: {}", s(go_j, "signature"));

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

/// `docs/generated-code.md` is the prose companion; it must not promise a name the tool does
/// not emit.
#[test]
fn 生成物の文書が実物の名前を使っている() {
    let doc = std::fs::read_to_string(root().join("docs/generated-code.md")).unwrap();
    let (dir, j) = setup("doc", "tests/corpus/クーポン一枚.rule");
    let py_j = j.get("python").unwrap();
    let go_j = j.get("go").unwrap();
    for want in [
        s(py_j, "signature"),
        s(go_j, "signature"),
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
