//! The fields of an element that may be absent, under each language's strict checker (§15.343 of
//! DESIGN.md).
//!
//! The door looks at each element twice: once of its kind, before any input is held to its range,
//! and once of its range where the walk comes to it. Python and TypeScript make a whole float the
//! integer it is on a copy of the element, `elem = elem._replace(spare=…)` and
//! `elem = { ...elem, spare: … }`, and the checker reads the copy's field as the optional it is
//! declared, though the test is inside the `is not None` (`!== null`) of the field: mypy said
//! `Unsupported operand types for <= ("int" and "None")` and tsc `'elem.spare' is possibly 'null'`.
//! The test says again that the field is there. Rust's record function wrote a string that may be
//! absent with `e.note.map(…)`, which takes it out of the element it reads through a reference
//! (E0507); it borrows the string now. The values of the other types are Copy, and are written as
//! they were. Go's record function wrote each such field with a function literal of two statements
//! on one line, which gofmt writes on lines of its own: a function of each type writes it now, and
//! the line calls it by its name.
//!
//! A column a table writes that may be absent is held under its unit's type in Rust, Go and Swift
//! (`Option<JPY>`, `*JPY`, `JPY?`), as an input that may be absent passes on as it stands. A rule
//! none of whose inputs, fields and outputs carried that unit declared no such type, and the code
//! named one it never defined: it declares the type now.
//!
//! The materials are in `tests/absent_fields`, English first, with the Japanese version beside it:
//! an element with a field of every type, and beside each one of the same type that may be absent,
//! two of those named like a builtin (`len`, `abs`); and a table that writes an amount, a weight and
//! a rate that may be absent, each of a unit no other value of the rule carries.

use ritsu_testkit::run::finish;
use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, need, ready, skip};
use rulec::json::Json;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The materials of the fields, each with the language `rulec` speaks for it.
const FIELDS: [(&str, &str); 2] = [("line_fields", "en"), ("明細の項目", "ja")];

/// The materials of the columns, each with the language `rulec` speaks for it.
const COLUMNS: [(&str, &str); 2] = [("optional_columns", "en"), ("値の無いことがある列", "ja")];

/// How long a run of rulec may take: `rulec test` talks to the servers of the generated code and
/// waits for their answers without a limit of its own.
const RULEC_LIMIT: std::time::Duration = std::time::Duration::from_secs(20 * 60);

fn rulec(lang: &str, tmp: Option<&Path>, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_rulec"));
    c.env("RULEC_LANG", lang).current_dir(root()).args(args).args(["--lang", lang]);
    if let Some(t) = tmp {
        c.env("TMPDIR", t);
    }
    let o = finish(&mut c, RULEC_LIMIT, &format!("rulec {}", args.join(" ")));
    (o.code.unwrap_or(-1), o.stdout, o.stderr)
}

/// Whether a command answers. swiftc leaves an empty directory in its TMPDIR almost every time it
/// answers `--version`, so the probe gets a TMPDIR of its own, which goes when this returns.
fn have(cmd: &str, args: &[&str]) -> bool {
    let tmp = TempDir::new("absent-probe");
    Command::new(cmd).args(args).env("TMPDIR", tmp.path()).output().map(|o| o.status.success()).unwrap_or(false)
}

/// `mypy`, installed or through `uvx`.
fn mypy() -> Option<Vec<String>> {
    if have("mypy", &["--version"]) {
        return Some(vec!["mypy".into()]);
    }
    have("uvx", &["mypy", "--version"]).then(|| vec!["uvx".into(), "mypy".into()])
}

/// `rulec test` over a generated directory, as JSON, with a SKIP line for whatever it did not run.
fn rulec_test(out: &Path) -> Json {
    let (_, said, e) = rulec("en", Some(&tmpdir_in(out)), &["test", &out.to_string_lossy(), "--format", "json"]);
    let j = rulec::json::parse(said.lines().next().unwrap_or_else(|| panic!("no result\n{e}"))).unwrap_or_else(|x| panic!("{x}: {said}"));
    for why in j.get("skipped").and_then(|v| v.as_arr()).unwrap_or_default() {
        skip(&format!("rulec test: {}", why.as_str().unwrap_or_default()));
    }
    j
}

/// What each language's strict checker says of the code generated for a rule into `out`, its
/// module `alias`: an empty list where each is silent. A checker that is not here is a SKIP line.
fn strictly(out: &Path, alias: &str) -> Vec<String> {
    let mut said = Vec::new();
    let mut ran = |what: &str, c: &mut Command| {
        let o = c.output().unwrap_or_else(|e| panic!("cannot start {what}: {e}"));
        let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        let quiet = text.lines().all(|l| l.trim().is_empty() || l.trim() == "Syntax OK" || l.starts_with("Success: no issues found") || l.starts_with("No syntax errors"));
        if !o.status.success() || !quiet {
            said.push(format!("{what}:\n{text}"));
        }
    };
    let tmp = tmpdir_in(out);
    if need(Need::Python) {
        match mypy() {
            None => skip("mypy is not here, nor uvx; the Python is not held to mypy --strict"),
            Some(my) => {
                let py = out.join("python");
                let mut files: Vec<String> = std::fs::read_dir(&py)
                    .unwrap()
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .filter(|n| n.ends_with(".py") && !n.ends_with("_service.py") && !n.ends_with("_connect_runner.py"))
                    .collect();
                files.sort();
                ran("mypy --strict", Command::new(&my[0]).args(&my[1..]).args(["--strict", "--no-color-output", "--no-incremental", "--cache-dir"]).arg(tmp.join("mypy")).args(&files).current_dir(&py));
            }
        }
    }
    if need(Need::Node) {
        match ritsu_testkit::tools::find("TSC", Some(Path::new("tools/node_modules/.bin/tsc")), "tsc", &["--version"]) {
            None => skip("tsc is not here (npm ci --prefix tools puts it there); the TypeScript is not held to tsc --strict"),
            Some(tsc) => {
                let ts = out.join("typescript");
                std::fs::write(ts.join("package.json"), "{\"type\": \"module\"}\n").unwrap();
                let mut c = Command::new(&tsc);
                c.args(["--strict", "--noUnusedLocals", "--noUnusedParameters", "--noEmit", "--allowImportingTsExtensions", "--erasableSyntaxOnly", "--module", "nodenext", "--target", "es2022", "--types", "node"]);
                if let Ok(types) = std::fs::canonicalize(root().join("tools/node_modules/@types")) {
                    c.arg("--typeRoots").arg(types);
                }
                ran("tsc --strict", c.args([format!("{alias}.ts"), format!("{alias}_runner.ts"), format!("{alias}_mcp.ts")]).current_dir(&ts));
            }
        }
    }
    if ready(Need::Rustc, || have("rustc", &["--version"]), "rustc is not here; the Rust is not built with -D warnings") {
        let clippy = have("clippy-driver", &["--version"]);
        if !clippy {
            skip("clippy-driver is not here (rustup component add clippy); the Rust is not held to clippy's lints");
        }
        // The module as a library, the runner as a program, and the WASI module as a library.
        let builds = [("module", "rust", format!("{alias}.rs"), true), ("runner", "rust", format!("{alias}_runner.rs"), false), ("wasm", "wasm", format!("{alias}_wasm.rs"), true)];
        for (what, dir, file, lib) in &builds {
            for tool in ["rustc", "clippy-driver"].into_iter().filter(|t| *t == "rustc" || clippy) {
                let mut c = Command::new(tool);
                c.args(["--edition", "2021", "-D", "warnings", "--emit=metadata", "--out-dir"]).arg(tmp.join(format!("{tool}-{what}")));
                if *lib {
                    c.args(["--crate-type", "lib"]);
                }
                ran(&format!("{tool} -D warnings ({what})"), c.arg(file).current_dir(out.join(dir)));
            }
        }
    }
    if ready(Need::Go, || have("go", &["version"]), "go is not here; the Go is not vetted") {
        let pkg = out.join("go").join(alias.replace('_', ""));
        ran("go vet", Command::new("go").args(["vet", "./..."]).env("GOFLAGS", "-mod=mod").env("GOPROXY", "off").current_dir(&pkg));
        ran("go build", Command::new("go").args(["build", "./..."]).env("GOFLAGS", "-mod=mod").env("GOPROXY", "off").current_dir(&pkg));
        // gofmt names each file it would write otherwise (§8.1).
        ran("gofmt -l", Command::new("gofmt").arg("-l").arg(out.join("go")));
    }
    if ready(Need::Swift, || have("swiftc", &["--version"]), "swiftc is not here; the Swift is not checked with -warnings-as-errors") {
        ran("swiftc -warnings-as-errors", Command::new("swiftc").args(["-typecheck", "-warnings-as-errors"]).arg(format!("{alias}.swift")).env("TMPDIR", &tmp).current_dir(out.join("swift")));
    }
    if ready(Need::Ruby, || have("ruby", &["--version"]), "ruby is not here; the Ruby is not read with -w") {
        ran("ruby -wc", Command::new("ruby").args(["-wc"]).arg(format!("{alias}.rb")).current_dir(out.join("ruby")));
    }
    if ready(Need::Java, || have("javac", &["-version"]), "javac is not here; the Java is not compiled with -Xlint:all -Werror") {
        let class: String = alias.split('_').map(|w| w[..1].to_uppercase() + &w[1..]).collect();
        ran("javac -Xlint:all -Werror", Command::new("javac").args(["-Xlint:all", "-Werror", "-d"]).arg(tmp.join("classes")).arg(format!("{class}.java")).current_dir(out.join("java")));
    }
    if ready(Need::Php, || have("php", &["--version"]), "php is not here; the PHP is not read with -l") {
        ran("php -l", Command::new("php").arg("-l").arg(format!("{alias}.php")).current_dir(out.join("php")));
    }
    said
}

/// Each material builds under each language's strict checker, and every language answers the rule's
/// vectors as the reference evaluator does: as a runner, as a WASI module, as an MCP server.
fn holds(materials: [(&str, &str); 2], alias: &str) {
    if !ready(Need::Python, || have("python3", &["--version"]), "python3 is not here") {
        return;
    }
    for (stem, lang) in materials {
        let t = TempDir::new("absent-fields");
        let out = t.path().join("out");
        let (code, said, e) = rulec(lang, None, &rulec::targets::up_to_0_40(&["gen", &format!("tests/absent_fields/{stem}.rule"), "--out", &out.to_string_lossy()]));
        assert_eq!(code, 0, "{stem}: {said}{e}");
        let said = strictly(&out, alias);
        assert!(said.is_empty(), "{stem}: the generated code is not silent under each strict checker:\n{}", said.join("\n"));
        let j = rulec_test(&out);
        let results = j.get("results").and_then(|v| v.as_arr()).unwrap_or_default();
        let ran: Vec<String> = results
            .iter()
            .filter(|r| r.get("ran").and_then(|v| v.as_bool()) == Some(true))
            .map(|r| format!("{}/{}", r.get("lang").and_then(|v| v.as_str()).unwrap_or_default(), r.get("via").and_then(|v| v.as_str()).unwrap_or_default()))
            .collect();
        for r in results {
            assert_ne!(r.get("ok").and_then(|v| v.as_bool()), Some(false), "{stem}: {r:?}");
        }
        assert!(ran.len() >= 2, "{stem}: {j:?}");
        println!("absent_fields: {stem} answered alike by {}", ran.join(", "));
    }
}

/// The fields of an element that may be absent build under each language's strict checker, the
/// English material and the Japanese version, and every language answers as the reference
/// evaluator does.
#[test]
fn the_fields_of_an_element_that_may_be_absent_build_under_each_strict_checker() {
    holds(FIELDS, "line_fields");
}

/// The columns a table writes that may be absent, each of a unit nothing else of the rule carries,
/// name types the module declares: the code builds under each language's strict checker, and every
/// language answers as the reference evaluator does.
#[test]
fn a_column_a_table_may_leave_unwritten_names_a_type_its_module_declares() {
    holds(COLUMNS, "optional_columns");
}
