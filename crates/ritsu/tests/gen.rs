//! `ritsu gen` (DESIGN 9.3; PLAN E.7): a project's rules, dates, clients of the books and workflows
//! as one package for each of TypeScript, Python and Go.
//!
//! - The packages pass the target's own type checker: `tsc --strict`, `mypy --strict`, `go vet` (and
//!   gofmt), with the clients of the books on PostgreSQL and on TigerBeetle.
//! - A workflow reads the rules, the dates and the books from its package: its imports name the
//!   package's rules/, dates/ and books/, the books its transport takes are the package's clients,
//!   and the rule's and the date's activities, run, answer through the package's modules.
//! - `--check` says what is stale, missing or left over, and writing again puts it right.
//! - Every file begins with the head of DESIGN 9.2, naming the project's file it is made from.
//!
//! The project is `tests/projects/stockroom` (English): a rule of rulec's, a dates file and a
//! calendar of koyomi's, a book of chobo's, and a flow of dandori's that uses all three. The
//! Japanese one, `tests/projects/通販`, is generated too, where it can be: its two flows have
//! Japanese names, so the Go of both would be the package `workflow`, which ritsu gen refuses.

use ritsu_testkit::{Need, TempDir, golden, ready};
use std::path::{Path, PathBuf};
use std::process::Command;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The root of the workspace, where the languages keep the tools their tests use.
fn workspace() -> PathBuf {
    here().parent().and_then(Path::parent).expect("crates/ritsu is two below the root").to_path_buf()
}

const STOCKROOM: &str = "tests/projects/stockroom";
const SHOP: &str = "tests/projects/通販";
/// The module the Go is vetted in, and the import path of the package's directory in it.
const MODULE: &str = "gocheck/generated";

/// `ritsu`, run in `dir`, with no language asked of the environment.
fn ritsu_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(dir).args(args);
    for v in ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG", "CHOBO_LANG", "GEAS_LANG", "YUEN_LANG", "SAKAI_LANG"] {
        c.env_remove(v);
    }
    let o = c.output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// `ritsu gen` of the project at `project` (from this crate), its root the project's directory,
/// writing under `out`.
fn generate(project: &Path, out: &Path, more: &[&str]) -> (i32, String, String) {
    let o = out.to_str().unwrap();
    let mut args = vec!["gen", "--root", ".", "--out", o, "--module", MODULE];
    args.extend_from_slice(more);
    ritsu_in(project, &args)
}

/// Every file under `dir`, from it, in order.
fn files(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, d: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.push(p.strip_prefix(base).unwrap().to_string_lossy().to_string());
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

fn sha256(bytes: &[u8]) -> String {
    ritsu_base::sha256::hex(bytes)
}

// ── the type checkers ────────────────────────────────────────────────────────────────────────────

/// tsc (TypeScript 7), as dandori's tests run it, and the packages it reads: Temporal's SDK from
/// dandori's runner, tigerbeetle-node from chobo's.
fn tsc() -> Option<(PathBuf, Vec<PathBuf>)> {
    let tsc = workspace().join("crates/dandori/tools/temporal/node_modules/.bin/tsc");
    let temporal = workspace().join("crates/dandori/tools/temporal/node_modules");
    let tigerbeetle = workspace().join("crates/chobo/tools/runner/node_modules/tigerbeetle-node");
    (tsc.exists() && tigerbeetle.exists()).then(|| {
        let mut modules: Vec<PathBuf> = std::fs::read_dir(&temporal).unwrap().flatten().map(|e| e.path()).filter(|p| !p.ends_with(".bin")).collect();
        modules.sort();
        modules.push(tigerbeetle);
        (tsc, modules)
    })
}

/// `tsc --strict` on the TypeScript package at `dir`. The packages the default transport loads only
/// when a task needs them (the AWS SDK's clients, the agents' SDKs) are declared as modules of any
/// type, as dandori's own test of its TypeScript does.
fn type_check_typescript(dir: &Path, tsc: &Path, modules: &[PathBuf]) {
    let nm = dir.join("node_modules");
    std::fs::create_dir_all(&nm).unwrap();
    for m in modules {
        std::os::unix::fs::symlink(m, nm.join(m.file_name().unwrap())).unwrap();
    }
    std::fs::write(dir.join("shims.d.ts"), "declare module \"@anthropic-ai/sdk\";\ndeclare module \"openai\";\ndeclare module \"@openai/agents\";\ndeclare module \"@aws-sdk/*\";\n").unwrap();
    let types = workspace().join("crates/dandori/tools/temporal/node_modules/@types");
    let tsconfig = serde_json::json!({
        "compilerOptions": {
            "strict": true, "noEmit": true, "module": "nodenext", "moduleResolution": "nodenext", "target": "es2022",
            "skipLibCheck": true, "types": ["node"], "typeRoots": [types]
        },
        "include": ["**/*.ts"],
        "exclude": ["node_modules"]
    });
    std::fs::write(dir.join("tsconfig.json"), serde_json::to_string_pretty(&tsconfig).unwrap()).unwrap();
    let out = Command::new(tsc).arg("-p").arg(dir.join("tsconfig.json")).output().unwrap();
    assert!(out.status.success(), "{}: tsc --strict finds fault with the package:\n{}{}", dir.display(), String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}

/// mypy (koyomi's, or one on the PATH), the Python whose packages it reads (dandori's runner's, with
/// Temporal's SDK), and chobo's runner's tigerbeetle package.
fn mypy() -> Option<(PathBuf, PathBuf, PathBuf)> {
    let mypy = Some(workspace().join("crates/koyomi/tools/.venv/bin/mypy")).filter(|p| p.exists()).or_else(|| {
        Command::new("mypy").arg("--version").output().ok().filter(|o| o.status.success()).map(|_| PathBuf::from("mypy"))
    })?;
    let python = workspace().join("crates/dandori/tools/temporal-python/.venv/bin/python");
    let lib = workspace().join("crates/chobo/tools/runner/.venv/lib");
    let tigerbeetle = std::fs::read_dir(&lib).ok()?.flatten().map(|e| e.path().join("site-packages/tigerbeetle")).find(|p| p.exists())?;
    python.exists().then_some((mypy, python, tigerbeetle))
}

/// `mypy --strict` on the Python package at `dir` (the directory of pyproject.toml). tigerbeetle is
/// read from chobo's runner, and followed without its own errors said, as an installed package is.
fn type_check_python(dir: &Path, mypy: &Path, python: &Path, tigerbeetle: &Path) {
    let extra = dir.join("extra");
    std::fs::create_dir_all(&extra).unwrap();
    std::os::unix::fs::symlink(tigerbeetle, extra.join("tigerbeetle")).unwrap();
    std::fs::write(dir.join("mypy.ini"), "[mypy]\n\n[mypy-tigerbeetle.*]\nfollow_imports = silent\n").unwrap();
    let out = Command::new(mypy)
        .args(["--strict", "--config-file", "mypy.ini", "--no-incremental", "--cache-dir"])
        .arg(dir.join(".mypy_cache"))
        .arg("--python-executable")
        .arg(python)
        .args(["-p", "generated"])
        .env("MYPYPATH", &extra)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}: mypy --strict finds fault with the package:\n{}{}", dir.display(), String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}

/// The go.mod and go.sum of a module that requires what the Go of a package imports: those of
/// dandori's runner (Temporal's SDK) and chobo's (pgx, tigerbeetle-go), put together.
fn go_module() -> Option<(String, String)> {
    let dandori = workspace().join("crates/dandori/tools/temporal-go");
    let chobo = workspace().join("crates/chobo/tools/runner/go");
    if !dandori.join("go.mod").exists() || !chobo.join("go.mod").exists() {
        return None;
    }
    let mut go = String::new();
    let mut requires: Vec<(String, String)> = Vec::new();
    for dir in [&dandori, &chobo] {
        let text = read(&dir.join("go.mod"));
        let mut inside = false;
        for line in text.lines() {
            let l = line.trim();
            if let Some(v) = l.strip_prefix("go ").filter(|_| go.is_empty()) {
                go = v.to_string();
            } else if l == "require (" {
                inside = true;
            } else if l == ")" {
                inside = false;
            } else if inside {
                let mut parts = l.split_whitespace();
                if let (Some(p), Some(v)) = (parts.next(), parts.next())
                    && !requires.iter().any(|(q, _)| q == p)
                {
                    requires.push((p.to_string(), v.to_string()));
                }
            }
        }
    }
    requires.sort();
    let lines: String = requires.iter().map(|(p, v)| format!("\t{p} {v}\n")).collect();
    let mut sums: Vec<String> = [&dandori, &chobo].iter().flat_map(|d| read(&d.join("go.sum")).lines().map(str::to_string).collect::<Vec<_>>()).collect();
    sums.sort();
    sums.dedup();
    Some((format!("module gocheck\n\ngo {go}\n\nrequire (\n{lines})\n"), sums.join("\n") + "\n"))
}

/// The module `gocheck` in `dir`, with the Go package at `package` as gocheck/generated.
fn go_module_in(dir: &Path, package: &Path, gomod: &(String, String)) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("go.mod"), &gomod.0).unwrap();
    std::fs::write(dir.join("go.sum"), &gomod.1).unwrap();
    ritsu_testkit::tmp::copy_dir(package, &dir.join("generated"));
}

fn go_in(dir: &Path) -> Command {
    let mut go = Command::new("go");
    // the modules come from the module cache, as the tests of dandori and chobo leave it
    go.current_dir(dir).env("GOWORK", "off").env("GOFLAGS", "-mod=mod -trimpath").env("GOPROXY", "off");
    go
}

/// `go vet` and gofmt on the Go package at `package`, in a module made in `dir`.
fn type_check_go(dir: &Path, package: &Path, gomod: &(String, String)) {
    go_module_in(dir, package, gomod);
    let out = go_in(dir).args(["vet", "./..."]).output().unwrap();
    assert!(out.status.success(), "{}: go vet finds fault with the package:\n{}{}", package.display(), String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let fmt = Command::new("gofmt").arg("-l").arg("generated").current_dir(dir).output().unwrap();
    assert!(fmt.status.success() && fmt.stdout.is_empty(), "{}: gofmt would write these otherwise:\n{}{}", package.display(), String::from_utf8_lossy(&fmt.stdout), String::from_utf8_lossy(&fmt.stderr));
}

fn have_go() -> bool {
    Command::new("go").arg("version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// The packages of the English project, with the books on PostgreSQL and on TigerBeetle, and the
/// TypeScript and the Python of the Japanese one, pass `tsc --strict`, `mypy --strict` and `go vet`.
#[test]
fn the_packages_pass_the_type_checkers() {
    let t = TempDir::new("gen-types");
    let mut made = Vec::new();
    for books in ["postgres", "tigerbeetle"] {
        let out = t.path().join(format!("stockroom-{books}"));
        let (code, _, err) = generate(&here().join(STOCKROOM), &out, &["--books", books]);
        assert_eq!(code, 0, "ritsu gen of the stockroom ({books}):\n{err}");
        made.push(out);
    }
    let shop = t.path().join("shop");
    let (code, _, err) = generate(&here().join(SHOP), &shop, &["--target", "typescript"]);
    assert_eq!(code, 0, "ritsu gen --target typescript of 通販:\n{err}");
    if ritsu_testkit::need(Need::Node) {
        match tsc() {
            Some((tsc, modules)) => {
                for out in made.iter().chain([&shop]) {
                    type_check_typescript(&out.join("typescript"), &tsc, &modules);
                }
                println!("tsc --strict passes the TypeScript of three packages");
            }
            None => ritsu_testkit::skip("TypeScript (crates/dandori/tools/temporal) or tigerbeetle-node (crates/chobo/tools/runner) is not installed; the TypeScript of the packages is not type-checked"),
        }
    }
    if ritsu_testkit::need(Need::Python) {
        match mypy() {
            Some((mypy, python, tigerbeetle)) => {
                for out in &made {
                    type_check_python(&out.join("python"), &mypy, &python, &tigerbeetle);
                }
                println!("mypy --strict passes the Python of two packages");
            }
            None => ritsu_testkit::skip("mypy, Temporal's Python SDK (crates/dandori/tools/temporal-python) or tigerbeetle (crates/chobo/tools/runner) is not installed; the Python of the packages is not type-checked"),
        }
    }
    if ready(Need::Go, have_go, "go is not installed; the Go of the packages is not vetted") {
        match go_module() {
            Some(gomod) => {
                for out in &made {
                    let name = out.file_name().unwrap().to_string_lossy().to_string();
                    type_check_go(&t.path().join(format!("go-{name}")), &out.join("go"), &gomod);
                }
                println!("go vet and gofmt pass the Go of two packages");
            }
            None => ritsu_testkit::skip("the go.mod of crates/dandori/tools/temporal-go or crates/chobo/tools/runner/go is missing; the Go of the packages is not vetted"),
        }
    }
}

// ── reading from the package ─────────────────────────────────────────────────────────────────────

fn assert_has(file: &Path, what: &str) {
    let text = read(file);
    assert!(text.contains(what), "{} does not have {what:?}:\n{text}", file.display());
}

/// The workflow's imports name the package's own modules, and nothing of rulec's, koyomi's or
/// chobo's is put beside the workflow.
#[test]
fn a_workflow_imports_the_rules_dates_and_books_of_its_package() {
    let t = TempDir::new("gen-reads");
    let out = t.path().join("out");
    let (code, _, err) = generate(&here().join(STOCKROOM), &out, &[]);
    assert_eq!(code, 0, "{err}");
    let ts = out.join("typescript/flows/order");
    assert_has(&ts.join("rules.ts"), "from \"../../rules/delivery\";");
    assert_has(&ts.join("rules.ts"), "from \"../../dates/payment_terms\";");
    assert_has(&ts.join("io.ts"), "import type { Book as Book0 } from \"../../books/stock\";");
    assert_has(&ts.join("io.ts"), "  books?: Books;\n");
    let py = out.join("python/generated/flows/order");
    assert_has(&py.join("rules.py"), "from ...rules.delivery import ");
    assert_has(&py.join("rules.py"), "from ...dates.payment_terms import payment, payment_at\n");
    assert_has(&py.join("io.py"), "from ...books.stock import Book as _Book0\n");
    assert_has(&py.join("io.py"), "        books: Books | None = None,\n");
    let go = out.join("go/flows/order");
    assert_has(&go.join("rules.go"), &format!("\t\"{MODULE}/dates/paymentterms\"\n\t\"{MODULE}/rules/delivery\"\n"));
    assert_has(&go.join("io_books.go"), &format!("\tddBook0 \"{MODULE}/books/stock\"\n"));
    assert_has(&go.join("io_books.go"), "\tStock *ddBook0.Book\n");
    for target in ["typescript", "python/generated", "go"] {
        let under = files(&out.join(target).join("flows"));
        assert!(under.iter().all(|f| !f.contains("rulec/") && !f.contains("koyomi/") && !f.contains("chobo/")), "{target}/flows holds code of another language's: {under:?}");
    }
}

/// The rule's and the date's activities of the workflow, run, answer through the package's modules:
/// in Python, called as the worker calls them; in Go, from a test of the workflow's package. And the
/// books the Python transport takes are the package's clients.
#[test]
fn the_activities_of_a_workflow_answer_through_its_package() {
    let t = TempDir::new("gen-run");
    let out = t.path().join("out");
    let (code, _, err) = generate(&here().join(STOCKROOM), &out, &[]);
    assert_eq!(code, 0, "{err}");
    let python = workspace().join("crates/dandori/tools/temporal-python/.venv/bin/python");
    if ready(Need::Python, || python.exists(), "Temporal's Python SDK (crates/dandori/tools/temporal-python) is not installed; the Python activities are not run") {
        let script = "import asyncio, json\n\
                      from generated.flows.order import io, rules\n\
                      from generated.books import stock\n\
                      print(json.dumps([asyncio.run(rules.rule_delivery({'member': False, 'amount': 299})), asyncio.run(rules.rule_delivery({'member': False, 'amount': 300})), asyncio.run(rules.dates_terms_payment({'received': '2026-04-01'})), io.Books.__annotations__['stock'] is stock.Book]))\n";
        let o = Command::new(&python).args(["-B", "-c", script]).current_dir(out.join("python")).output().unwrap();
        assert!(o.status.success(), "the Python activities do not run:\n{}", String::from_utf8_lossy(&o.stderr));
        let got = String::from_utf8_lossy(&o.stdout).trim().to_string();
        assert_eq!(got, r#"[{"carrier": "standard"}, {"carrier": "next_day"}, {"day": "2026-05-08", "at": "2026-05-08T09:00:00Z"}, true]"#);
        println!("the Python activities answer through the package: {got}");
    }
    if ready(Need::Go, have_go, "go is not installed; the Go activities are not run") {
        let Some(gomod) = go_module() else {
            ritsu_testkit::skip("the go.mod of crates/dandori/tools/temporal-go or crates/chobo/tools/runner/go is missing; the Go activities are not run");
            return;
        };
        let module = t.path().join("go");
        go_module_in(&module, &out.join("go"), &gomod);
        let test = format!(
            "package order\n\nimport (\n\t\"context\"\n\t\"encoding/json\"\n\t\"fmt\"\n\t\"testing\"\n\n\tstock \"{MODULE}/books/stock\"\n)\n\n\
             func TestTheActivitiesAnswerThroughThePackage(t *testing.T) {{\n\
             \tvar got []any\n\
             \tfor _, c := range []struct {{\n\t\tname string\n\t\targs map[string]any\n\t}}{{\n\
             \t\t{{\"rule_delivery\", map[string]any{{\"member\": false, \"amount\": 299}}}},\n\
             \t\t{{\"rule_delivery\", map[string]any{{\"member\": false, \"amount\": 300}}}},\n\
             \t\t{{\"dates_terms_payment\", map[string]any{{\"received\": \"2026-04-01\"}}}},\n\
             \t}} {{\n\
             \t\tf := ddRules[c.name].(func(context.Context, map[string]any) (any, error))\n\
             \t\tv, err := f(context.Background(), c.args)\n\
             \t\tif err != nil {{\n\t\t\tt.Fatal(err)\n\t\t}}\n\
             \t\tgot = append(got, v)\n\
             \t}}\n\
             \tvar b Books\n\
             \tb.Stock = (*stock.Book)(nil)\n\
             \tgot = append(got, fmt.Sprintf(\"%T\", b.Stock))\n\
             \ttext, _ := json.Marshal(got)\n\
             \tfmt.Println(string(text))\n\
             }}\n"
        );
        std::fs::write(module.join("generated/flows/order/package_test.go"), test).unwrap();
        let o = go_in(&module).args(["test", "-count=1", "-run", "TestTheActivitiesAnswerThroughThePackage", "-v", "./generated/flows/order/"]).output().unwrap();
        let text = String::from_utf8_lossy(&o.stdout).to_string();
        assert!(o.status.success(), "the Go activities do not run:\n{text}{}", String::from_utf8_lossy(&o.stderr));
        let line = text.lines().find(|l| l.starts_with('[')).unwrap_or_default().to_string();
        assert_eq!(line, r#"[{"carrier":"standard"},{"carrier":"next_day"},{"at":"2026-05-08T09:00:00Z","day":"2026-05-08"},"*stock.Book"]"#);
        println!("the Go activities answer through the package: {line}");
    }
}

// ── --check ──────────────────────────────────────────────────────────────────────────────────────

/// `--check` writes nothing, and says what is stale, missing, or left over from an earlier gen; gen
/// puts it right, removing what it wrote before and writes no more. A change to the project is a
/// change to the packages.
#[test]
fn gen_check_says_what_is_stale() {
    let t = TempDir::new("gen-check");
    let project = t.path().join("stockroom");
    ritsu_testkit::tmp::copy_dir(&here().join(STOCKROOM), &project);
    let out = t.path().join("out");
    let (code, _, err) = generate(&project, &out, &[]);
    assert_eq!(code, 0, "{err}");
    let (code, stdout, _) = generate(&project, &out, &["--check"]);
    assert_eq!((code, stdout.as_str()), (0, ""), "a package just written is not stale");
    // a file edited, one taken away, and one an earlier gen wrote that this one does not
    let rule = out.join("typescript/rules/delivery.ts");
    std::fs::write(&rule, read(&rule).replace("next_day", "next-day")).unwrap();
    std::fs::remove_file(out.join("python/generated/dates/payment_terms.py")).unwrap();
    std::fs::copy(out.join("go/rules/delivery/delivery.go"), out.join("go/rules/delivery/old.go")).unwrap();
    let o = out.display().to_string();
    let (code, stdout, _) = generate(&project, &out, &["--check"]);
    assert_eq!(code, 1);
    assert_eq!(
        stdout,
        format!("missing: {o}/python/generated/dates/payment_terms.py\ngenerated file is stale or hand-edited: {o}/typescript/rules/delivery.ts\ngenerated no more, and still there: {o}/go/rules/delivery/old.go\n")
    );
    let (code, stdout, _) = generate(&project, &out, &[]);
    assert_eq!(code, 0);
    assert_eq!(stdout, format!("generated: {o}/typescript/rules/delivery.ts\ngenerated: {o}/python/generated/dates/payment_terms.py\nremoved, as it is generated no more: {o}/go/rules/delivery/old.go\n"));
    let (code, stdout, _) = generate(&project, &out, &["--check"]);
    assert_eq!((code, stdout.as_str()), (0, ""));
    // the rule changed: its module in every language, and nothing else
    let src = project.join("rules/delivery.rule");
    std::fs::write(&src, read(&src).replace("Written for the example", "Written for the example, again")).unwrap();
    let (code, stdout, _) = generate(&project, &out, &["--check"]);
    assert_eq!(code, 1);
    assert_eq!(
        stdout,
        format!("generated file is stale or hand-edited: {o}/typescript/rules/delivery.ts\ngenerated file is stale or hand-edited: {o}/python/generated/rules/delivery.py\ngenerated file is stale or hand-edited: {o}/go/rules/delivery/delivery.go\n")
    );
    // in Japanese: the prose of the generated code is in the language asked, as `rulec gen`'s is, so a
    // package written in Japanese is checked in Japanese
    let ja = t.path().join("ja");
    let (code, _, err) = generate(&project, &ja, &["--lang", "ja"]);
    assert_eq!(code, 0, "{err}");
    let (code, stdout, _) = generate(&project, &ja, &["--check", "--lang", "ja"]);
    assert_eq!((code, stdout.as_str()), (0, ""));
    let (code, stdout, _) = generate(&project, &ja, &["--check"]);
    assert_eq!(code, 1, "a package written in Japanese is not what the English writes: {stdout}");
    let rule = ja.join("typescript/rules/delivery.ts");
    std::fs::write(&rule, read(&rule).replace("next_day", "next-day")).unwrap();
    std::fs::remove_file(ja.join("python/generated/dates/payment_terms.py")).unwrap();
    std::fs::copy(ja.join("go/rules/delivery/delivery.go"), ja.join("go/rules/delivery/old.go")).unwrap();
    let j = ja.display().to_string();
    let (code, stdout, _) = generate(&project, &ja, &["--check", "--lang", "ja"]);
    assert_eq!(code, 1);
    assert_eq!(
        stdout,
        format!("ありません: {j}/python/generated/dates/payment_terms.py\n生成物が古いか手で編集されています: {j}/typescript/rules/delivery.ts\nもう生成しないファイルが残っています: {j}/go/rules/delivery/old.go\n")
    );
    let (code, stdout, _) = generate(&project, &ja, &["--lang", "ja"]);
    assert_eq!(code, 0);
    assert_eq!(stdout, format!("生成しました: {j}/typescript/rules/delivery.ts\n生成しました: {j}/python/generated/dates/payment_terms.py\nもう生成しないので消しました: {j}/go/rules/delivery/old.go\n"));
}

// ── the heads ────────────────────────────────────────────────────────────────────────────────────

/// Every file of a package begins with the head of DESIGN 9.2: what wrote it, with ritsu's version,
/// and for a file made from one of the project's, that file by its path from the project's root and
/// the digest of its bytes. The golden is the shape of the three packages: each file, what wrote it,
/// and what it is made from.
#[test]
fn every_file_says_what_wrote_it_and_what_it_is_made_from() {
    let t = TempDir::new("gen-heads");
    let out = t.path().join("out");
    let (code, _, err) = generate(&here().join(STOCKROOM), &out, &[]);
    assert_eq!(code, 0, "{err}");
    let version = env!("CARGO_PKG_VERSION");
    let mut shape = String::new();
    for target in ["typescript", "python", "go"] {
        shape.push_str(&format!("{target}:\n"));
        for f in files(&out.join(target)) {
            let text = read(&out.join(target).join(&f));
            let mut lines = text.lines();
            let (first, second) = (lines.next().unwrap_or(""), lines.next().unwrap_or(""));
            let by = if f.ends_with("py.typed") {
                "(empty)".to_string()
            } else if f.ends_with("package.json") {
                assert!(text.contains(&format!("Code generated by ritsu {version}. DO NOT EDIT.")), "{f}");
                "ritsu".to_string()
            } else {
                let tool = ["rulec", "koyomi", "chobo", "dandori", "ritsu"].into_iter().find(|t| first.ends_with(&format!("Code generated by {t} {version}. DO NOT EDIT."))).unwrap_or_else(|| panic!("{target}/{f} begins otherwise: {first}"));
                tool.to_string()
            };
            let from = match second.split_once("Source: ") {
                Some((_, s)) => {
                    let path = s.split(" (").next().unwrap();
                    let digest = s.rsplit("sha256:").next().unwrap().trim_end_matches(')');
                    let bytes = std::fs::read(here().join(STOCKROOM).join(path)).unwrap_or_else(|_| panic!("{target}/{f} names {path}, which is not a file of the project"));
                    assert_eq!(digest, &sha256(&bytes)[..16], "{target}/{f}: the digest of {path}");
                    format!(" <- {path}")
                }
                None => String::new(),
            };
            shape.push_str(&format!("  {f}: {by}{from}\n"));
        }
    }
    golden(here().join("tests/golden/gen/stockroom.txt"), &shape.replace(version, "<version>"));
}

/// The head names the file a part is made from as its path, which may hold a line break: Unix lets
/// a file's name have one. The head writes it as `U+000A` (ritsu-emit's `one_line`), so the rest of the
/// name stays in the comment. Written as it was, the rest of this name was a line of the package's
/// Python, a statement before everything else in the module (DESIGN 9.2).
#[test]
fn a_files_name_stays_in_the_head() {
    let t = TempDir::new("gen-name");
    let project = t.path().join("project");
    ritsu_testkit::tmp::copy_dir(&here().join(STOCKROOM), &project);
    let rule = read(&project.join("rules/delivery.rule")).replace("rule delivery v1", "rule pickup v1");
    std::fs::write(project.join("rules/pickup\nprint('ran') #.rule"), rule).unwrap();
    let out = t.path().join("out");
    let (code, _, err) = generate(&project, &out, &[]);
    assert_eq!(code, 0, "{err}");
    let mut seen = 0;
    for target in ["typescript", "python", "go"] {
        for f in files(&out.join(target)) {
            for line in read(&out.join(target).join(&f)).lines().filter(|l| l.contains("print('ran')")) {
                assert!(line.starts_with("// Source: rules/pickupU+000Aprint('ran') #.rule (") || line.starts_with("# Source: rules/pickupU+000Aprint('ran') #.rule ("), "{target}/{f}: {line}");
                seen += 1;
            }
        }
    }
    assert_eq!(seen, 3, "the head of the rule's module in each language");
}

// ── what it refuses ──────────────────────────────────────────────────────────────────────────────

/// Two files of the project that would write one file of a package (the Go of two flows with
/// Japanese names, both the package `workflow`), a file that does not pass its check, a flow that
/// reads a file the project does not hold, a module named by a word the target keeps, and a name no
/// package can take: each says so and writes nothing.
#[test]
fn what_cannot_be_a_package_is_refused() {
    let t = TempDir::new("gen-refused");
    let out = t.path().join("out");
    let (code, stdout, err) = generate(&here().join(SHOP), &out, &["--target", "go"]);
    assert_eq!((code, stdout.as_str()), (2, ""));
    assert_eq!(
        err,
        "error: `delivery/配送の手配.flow` and `ordering/受注.flow` both write go/flows/workflow/doc.go of the package; give one of them another name (a rule or a dates file another alias, a book another name, a workflow a name or a file name in ASCII)\n"
    );
    let (code, _, err) = generate(&here().join(SHOP), &out, &["--target", "go", "--lang", "ja"]);
    assert_eq!(code, 2);
    assert_eq!(
        err,
        "エラー: `delivery/配送の手配.flow` と `ordering/受注.flow` が、パッケージの同じ go/flows/workflow/doc.go を書きます。どちらかの名前を変えてください（規則と日付のファイルは別名、帳簿は名前、ワークフローは名前かファイルの名前を ASCII で）\n"
    );
    assert!(!out.exists(), "nothing is written when a package is refused");
    // a rule that does not pass its check
    let project = t.path().join("broken");
    ritsu_testkit::tmp::copy_dir(&here().join(STOCKROOM), &project);
    let rule = project.join("rules/delivery.rule");
    std::fs::write(&rule, read(&rule).replace("| false  | <300GBP  | standard   |\n", "")).unwrap();
    let (code, stdout, err) = generate(&project, &out, &[]);
    assert_eq!((code, stdout.as_str()), (1, ""));
    assert!(err.contains("[rulec E"), "the rule's diagnostics, with the tool's word: {err}");
    // and the flow that reads it
    assert!(err.contains("[dandori E005]"), "{err}");
    assert!(err.ends_with("error: 2 file(s) do not pass check, so nothing is generated: rules/delivery.rule, orders/order.flow\n"), "{err}");
    assert!(!out.exists());
    // the flow alone: what it reads is not given
    let (code, _, err) = ritsu_in(&here().join(STOCKROOM), &["gen", "--root", ".", "orders/order.flow", "--out", out.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(err.starts_with("error: `orders/order.flow` reads "), "{err}");
    assert!(err.ends_with(", which is not one of the project's files; a package is made of the project's files, so give ritsu gen that file too\n"), "{err}");
    assert!(!out.exists());
    // a rule whose alias is a word Python keeps: rulec only warns of it (W121), and the package's
    // Python cannot import it
    let kept = t.path().join("kept");
    ritsu_testkit::tmp::copy_dir(&here().join(STOCKROOM), &kept);
    let rule = kept.join("rules/delivery.rule");
    std::fs::write(&rule, read(&rule).replace("rule delivery v1", "rule pass v1")).unwrap();
    let (code, _, err) = generate(&kept, &out, &["--target", "python"]);
    assert_eq!(code, 2);
    assert_eq!(err, "error: `rules/delivery.rule` makes the module `pass`, a word Python keeps, which the package cannot hold in its rules/; give it another name (an alias)\n");
    let (code, _, err) = generate(&kept, &out, &["--target", "typescript"]);
    assert_eq!(code, 0, "TypeScript takes the name: {err}");
    std::fs::remove_dir_all(&out).unwrap();
    let (code, _, err) = generate(&here().join(STOCKROOM), &out, &["--name", "Stock-Room"]);
    assert_eq!(code, 2);
    assert_eq!(err, "error: `--name Stock-Room` cannot name a package: it starts with a lowercase letter and has only lowercase letters, digits and `_` (and is not a word Python keeps)\n");
}
