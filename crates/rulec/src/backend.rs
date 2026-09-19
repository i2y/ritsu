//! The one place that knows which languages rulec generates (§15.20).
//!
//! Adding Ruby as the fifth target took 540 lines of generator and **22 hand-edited
//! files**, because the set of backends was written out again in seven separate lists —
//! `gen`'s output table, `runtest`'s five steps, `api`'s entries, and four test suites —
//! and then again in the diagrams and the prose. §15.13's "one `Lang` enum is enough" was
//! true only inside `guards()`.
//!
//! So the set lives here, once. Everything that has to walk it walks [`ALL`], and a sixth
//! language is a row in this file plus its own emitter. What cannot be expressed as data —
//! the emitter itself — stays in `codegen.rs`; what can is here.

use crate::codegen::{Gen, Lang};

/// What a backend has to say about itself for the shared machinery to drive it.
pub struct Backend {
    /// The stable identifier: the directory under the output, the key in `rulec api`, the
    /// name in `--format json`. Never translated, never renamed.
    pub id: &'static str,
    /// The name a person reads in a report.
    pub name: &'static str,
    /// The command that has to be on PATH to run what this backend writes. `rulec test`
    /// skips the language and says so when it is missing.
    pub tool: &'static str,
    /// Which arm of the generator emits it.
    pub lang: Lang,
    /// The files it writes, as (path under the output directory, contents).
    pub files: fn(&Gen, &str, &str) -> Vec<(String, String)>,
    /// What this backend names a rule's files, when that is not the alias. Java's public
    /// class has to be the name of the file it lives in, so its module is `ShippingFee.java`
    /// where the other ten write `shipping_fee.…`; `rulec test` finds what was written for a
    /// rule by this stem, and `None` means the alias itself.
    pub stem: Option<fn(&str) -> String>,
    /// How to run the generated runner over the vectors, relative to the output directory.
    pub run: fn(&str, &str) -> Plan,
    /// How to run the unit vectors of the rounding helpers.
    pub round: fn(&str) -> Plan,
    /// Whether this backend can write the walk a `fold` declares (§15.56). A rule that walks
    /// a sequence is generated only for the backends that say yes, and the others are
    /// refused by name — a generated file that cannot run is worse than a missing one.
    pub folds: bool,
    /// How to run the same runner as a WASI module under wasmtime (§15.63), for the languages
    /// whose output compiles to `wasm32-wasip1` unchanged. `rulec test` runs it as a pass of its
    /// own when wasmtime and the target's standard library are installed.
    ///
    /// **The column is closed at one.** It exists to prove the *shape* — a rule reached as a
    /// command that reads stdin and writes stdout — and one language proving it is the whole
    /// claim. TinyGo, Javy or ruby.wasm here would add a toolchain to `rulec test` and widen
    /// nothing, so the `None`s elsewhere are a decision and not a list of things to do. Not to be
    /// confused with the `wasm` backend below, which is the other shape: a module that exports
    /// a function for a host to call (§15.64).
    pub wasi: Option<fn(&str, &str) -> Plan>,
    /// How to start the rule as an MCP server (§15.44), for the languages that get one.
    /// `rulec test` drives it over the vectors like the runner and holds its answers to the
    /// same expected records.
    pub mcp: Option<fn(&str) -> Plan>,
    /// What else has to be there beyond `tool`, checked before the language is run; the Err is
    /// the note `rulec test` prints when it skips the language for that reason.
    pub ready: Option<fn() -> Result<(), String>>,
}

/// Whether a command answers `--version` or `version`.
pub fn have(cmd: &str) -> bool {
    ["--version", "version"]
        .iter()
        .any(|a| std::process::Command::new(cmd).arg(a).output().map(|o| o.status.success()).unwrap_or(false))
}

/// Whether the Rust toolchain can build for `target`: its standard library sits under the
/// sysroot once `rustup target add <target>` has been run.
pub fn rust_target(target: &str) -> bool {
    let Ok(o) = std::process::Command::new("rustc").args(["--print", "sysroot"]).output() else { return false };
    if !o.status.success() {
        return false;
    }
    let root = String::from_utf8_lossy(&o.stdout).trim().to_string();
    std::path::Path::new(&root).join("lib").join("rustlib").join(target).join("lib").is_dir()
}

/// One command to run, with an optional build that has to succeed first.
pub struct Plan {
    /// Directory under the output root.
    pub cwd: String,
    /// A compile step, for the languages that need one.
    pub build: Option<(String, Vec<String>)>,
    pub cmd: String,
    pub args: Vec<String>,
}

impl Plan {
    fn new(cwd: &str, cmd: &str, args: &[&str]) -> Self {
        Plan {
            cwd: cwd.into(),
            build: None,
            cmd: cmd.into(),
            args: args.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    fn built(mut self, cmd: &str, args: &[&str]) -> Self {
        self.build = Some((cmd.into(), args.iter().map(|s| (*s).to_string()).collect()));
        self
    }
}

/// The `go` line of the generated `go.mod`.
///
/// Since Go 1.21 that line is not only a minimum version but a **toolchain switch**: with the
/// default `GOTOOLCHAIN=auto`, a `go` older than it downloads the named version and hands over.
/// In a sandbox with no network that fails, and `rulec test` reports the download error as the
/// generated code disagreeing with the reference evaluator.
///
/// So the line states what the output actually needs, and the output needs very little: `bufio`,
/// `encoding/json`, `fmt`, `os`, `time`, `testing`, no generics, no `slices` or `maps`, and
/// `min`/`max` written out by hand. 1.21 is the oldest release this was run against; asking for
/// more would only switch toolchains on somebody's machine for nothing.
const GO_MIN: &str = "1.21";

/// How the generated Java is compiled, in `rulec test` and in the `build` line of
/// `rulec api` alike, so that the two cannot part company (the same arrangement the two
/// Wasm shapes have).
pub const JAVAC_FLAGS: &[&str] = &["--release", "17", "-encoding", "UTF-8", "-d", "classes"];

/// Every language `rulec gen` writes, in the order they are reported.
pub const ALL: &[Backend] = &[
    Backend {
        id: "python",
        name: "Python",
        tool: "python3",
        lang: Lang::Py,
        files: |g, alias, _pkg| {
            vec![
                (format!("python/{alias}.py"), g.python()),
                (format!("python/{alias}_runner.py"), g.python_runner()),
                (format!("python/{alias}_mcp.py"), g.py_mcp()),
                ("python/_round_test.py".into(), crate::codegen::round_tests_python()),
                (format!("python/{alias}_page.html"), g.page()),
            ]
        },
        stem: None,
        // `-B` and a cleared cache: a `.pyc` counts as fresh when the source has the same
        // length and the same whole-second mtime, so a same-length edit within a second of
        // the last run would otherwise execute the old module and report it as ok.
        run: |alias, _| Plan::new("python", "python3", &["-B", &format!("{alias}_runner.py")]),
        round: |_| Plan::new("python", "python3", &["-B", "_round_test.py"]),
        folds: true,
        wasi: None,
        mcp: Some(|alias| Plan::new("python", "python3", &["-B", &format!("{alias}_mcp.py")])),
        ready: None,
    },
    // The twelfth target is the one that is not a language: the rule travels as data and a
    // fixed evaluator reads it. It is here rather than in a package of its own because the
    // claim is the same claim — `rulec test` runs it over the vectors like the others, and a
    // plan that disagreed with the reference evaluator would be as red as generated code is.
    Backend {
        id: "numpy",
        name: "NumPy",
        tool: "python3",
        lang: Lang::Py,
        files: |g, alias, _pkg| {
            vec![
                (format!("numpy/{alias}.json"), g.np_plan()),
                ("numpy/rulec_np.py".into(), crate::codegen::np_runtime()),
                (format!("numpy/{alias}_runner.py"), g.np_runner()),
                ("numpy/_round_test.py".into(), crate::codegen::round_tests_numpy()),
            ]
        },
        stem: None,
        run: |alias, _| Plan::new("numpy", "python3", &["-B", &format!("{alias}_runner.py")]),
        round: |_| Plan::new("numpy", "python3", &["-B", "_round_test.py"]),
        // A walk carries state from element to element, which is not a column operation.
        folds: false,
        wasi: None,
        mcp: None,
        ready: Some(|| {
            let ok = std::process::Command::new("python3")
                .args(["-c", "import numpy"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if ok {
                Ok(())
            } else {
                Err(tr!("numpy が無いので NumPy 側を飛ばしました", "numpy not found; skipped the NumPy side"))
            }
        }),
    },
    Backend {
        id: "typescript",
        name: "TypeScript",
        tool: "node",
        lang: Lang::Ts,
        files: |g, alias, _pkg| {
            vec![
                (format!("typescript/{alias}.ts"), g.typescript()),
                (format!("typescript/{alias}_runner.ts"), g.ts_runner()),
                (format!("typescript/{alias}_mcp.ts"), g.ts_mcp()),
                ("typescript/_round_test.ts".into(), crate::codegen::round_tests_typescript()),
                (format!("typescript/{alias}_page.html"), g.page()),
            ]
        },
        stem: None,
        run: |alias, _| {
            Plan::new("typescript", "node", &["--no-warnings", &format!("{alias}_runner.ts")])
        },
        round: |_| Plan::new("typescript", "node", &["--no-warnings", "_round_test.ts"]),
        folds: true,
        wasi: None,
        mcp: Some(|alias| Plan::new("typescript", "node", &["--no-warnings", &format!("{alias}_mcp.ts")])),
        ready: None,
    },
    Backend {
        id: "javascript",
        name: "JavaScript",
        tool: "node",
        lang: Lang::Ts,
        // The TypeScript with its types taken off (§15.36). `.mjs`, so that node reads it
        // as a module without a package.json, and a browser takes it as it stands.
        files: |g, alias, _pkg| {
            vec![
                (format!("javascript/{alias}.mjs"), g.javascript()),
                (format!("javascript/{alias}_runner.mjs"), g.js_runner()),
                (format!("javascript/{alias}_mcp.mjs"), g.js_mcp()),
                ("javascript/_round_test.mjs".into(), crate::codegen::round_tests_javascript()),
                (format!("javascript/{alias}_page.html"), g.page()),
            ]
        },
        stem: None,
        run: |alias, _| Plan::new("javascript", "node", &[&format!("{alias}_runner.mjs")]),
        round: |_| Plan::new("javascript", "node", &["_round_test.mjs"]),
        folds: true,
        wasi: None,
        mcp: Some(|alias| Plan::new("javascript", "node", &[&format!("{alias}_mcp.mjs")])),
        ready: None,
    },
    Backend {
        id: "rust",
        name: "Rust",
        tool: "rustc",
        lang: Lang::Rs,
        files: |g, alias, _pkg| {
            vec![
                (format!("rust/{alias}.rs"), g.rust()),
                (format!("rust/{alias}_runner.rs"), g.rs_runner()),
                ("rust/_round_test.rs".into(), crate::codegen::round_tests_rust()),
            ]
        },
        stem: None,
        // rustc takes no dependencies and needs no project file, so one invocation builds
        // the rule and its runner together through a `#[path] mod`.
        run: |alias, _| {
            Plan::new("rust", &format!("./{alias}"), &[]).built(
                "rustc",
                &["--edition", "2021", "-O", &format!("{alias}_runner.rs"), "-o", alias],
            )
        },
        round: |_| {
            Plan::new("rust", "./_round_test", &[]).built(
                "rustc",
                &["--edition", "2021", "-O", "_round_test.rs", "-o", "_round_test"],
            )
        },
        folds: true,
        // The same runner, compiled for WASI and run under wasmtime. Nothing in the generated
        // Rust is platform-specific, so the source is the one above, unchanged.
        wasi: Some(|alias, _| {
            let args = wasi_rustc(&format!("{alias}_runner.rs"), &format!("{alias}_runner.wasm"));
            let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            Plan::new("rust", "wasmtime", &[&format!("{alias}_runner.wasm")]).built("rustc", &refs)
        }),
        mcp: None,
        ready: None,
    },
    Backend {
        id: "ruby",
        name: "Ruby",
        tool: "ruby",
        lang: Lang::Rb,
        files: |g, alias, _pkg| {
            vec![
                (format!("ruby/{alias}.rb"), g.ruby()),
                (format!("ruby/{alias}_runner.rb"), g.ruby_runner()),
                // The signature goes under sig/, where steep looks by default.
                (format!("ruby/sig/{alias}.rbs"), g.rbs()),
                ("ruby/_round_test.rb".into(), crate::codegen::round_tests_ruby()),
            ]
        },
        stem: None,
        run: |alias, _| Plan::new("ruby", "ruby", &[&format!("{alias}_runner.rb")]),
        round: |_| Plan::new("ruby", "ruby", &["_round_test.rb"]),
        folds: true,
        wasi: None,
        mcp: None,
        ready: None,
    },
    Backend {
        id: "php",
        name: "PHP",
        tool: "php",
        lang: Lang::Php,
        files: |g, alias, _pkg| {
            vec![
                (format!("php/{alias}.php"), g.php()),
                (format!("php/{alias}_runner.php"), g.php_runner()),
                ("php/_round_test.php".into(), crate::codegen::round_tests_php()),
            ]
        },
        stem: None,
        // `-n` ignores the machine's php.ini, the way `-B` keeps Python off a stale cache:
        // what runs here must not depend on somebody's local settings. `ext/json` is compiled
        // into every 8.x build and cannot be switched off, so nothing is lost by it.
        run: |alias, _| Plan::new("php", "php", &["-n", &format!("{alias}_runner.php")]),
        round: |_| Plan::new("php", "php", &["-n", "_round_test.php"]),
        folds: true,
        wasi: None,
        mcp: None,
        ready: None,
    },
    Backend {
        id: "go",
        name: "Go",
        tool: "go",
        lang: Lang::Go,
        files: |g, alias, pkg| {
            vec![
                (format!("go/{pkg}/{alias}.go"), g.go()),
                (format!("go/{pkg}/go.mod"), format!("module {pkg}\n\ngo {GO_MIN}\n")),
                (format!("go/{pkg}runner/main.go"), g.go_runner()),
                (
                    format!("go/{pkg}runner/go.mod"),
                    format!("module {pkg}runner\n\ngo {GO_MIN}\n\nrequire {pkg} v0.0.0\n\nreplace {pkg} => ../{pkg}\n"),
                ),
                (format!("go/{pkg}/round_test.go"), crate::codegen::round_tests_go(pkg)),
            ]
        },
        stem: None,
        run: |_, pkg| Plan::new(&format!("go/{pkg}runner"), "go", &["run", "."]),
        round: |pkg| Plan::new(&format!("go/{pkg}"), "go", &["test", "./..."]),
        folds: true,
        wasi: None,
        mcp: None,
        ready: None,
    },
    Backend {
        id: "swift",
        name: "Swift",
        tool: "swiftc",
        lang: Lang::Sw,
        files: |g, alias, _pkg| {
            vec![
                (format!("swift/{alias}.swift"), g.swift()),
                (format!("swift/{alias}_runner.swift"), g.swift_runner()),
                ("swift/_round_test.swift".into(), crate::codegen::round_tests_swift()),
            ]
        },
        stem: None,
        // Two files, one module: top-level code is only allowed in `main.swift`, so the
        // runner carries `@main` instead and the pair compiles as it stands. `-Onone` is
        // deliberate — nothing here is measured for speed, and the optimizer is the slowest
        // part of a build whose only job is to answer whether the vectors agree.
        run: |alias, _| {
            Plan::new("swift", &format!("./{alias}"), &[]).built(
                "swiftc",
                &["-Onone", &format!("{alias}.swift"), &format!("{alias}_runner.swift"), "-o", alias],
            )
        },
        round: |_| {
            Plan::new("swift", "./_round_test", &[])
                .built("swiftc", &["-Onone", "_round_test.swift", "-o", "_round_test"])
        },
        folds: true,
        wasi: None,
        mcp: None,
        ready: None,
    },
    Backend {
        id: "java",
        name: "Java",
        // `javac` builds and `java` runs; `ready` asks for the second one, because a JDK is
        // what has to be there and a JRE alone would compile nothing.
        tool: "javac",
        lang: Lang::Java,
        // The public class has to be the file's name, so the file is named after the class
        // and not after the alias the other backends use.
        files: |g, _alias, _pkg| {
            let cls = g.java_class();
            vec![
                (format!("java/{cls}.java"), g.java()),
                (format!("java/{cls}Runner.java"), g.java_runner()),
                ("java/_RoundTest.java".into(), crate::codegen::round_tests_java()),
            ]
        },
        stem: Some(crate::codegen::java_class),
        // `--release 17` is the floor the output claims (§15.78) rather than whatever JDK is
        // installed, `-encoding UTF-8` is what a JDK before 18 needs to read a Japanese
        // identifier, and `-d classes` keeps the build's output out of the source directory.
        run: |alias, _| {
            let cls = crate::codegen::java_class(alias);
            Plan::new("java", "java", &["-cp", "classes", &format!("{cls}Runner")]).built(
                "javac",
                &JAVAC_FLAGS
                    .iter()
                    .copied()
                    .chain([format!("{cls}.java"), format!("{cls}Runner.java")].iter().map(|s| s.as_str()))
                    .collect::<Vec<&str>>(),
            )
        },
        round: |_| {
            Plan::new("java", "java", &["-cp", "classes", "_RoundTest"]).built(
                "javac",
                &JAVAC_FLAGS.iter().copied().chain(["_RoundTest.java"]).collect::<Vec<&str>>(),
            )
        },
        folds: true,
        wasi: None,
        mcp: None,
        ready: Some(|| {
            if have("java") {
                Ok(())
            } else {
                Err(tr!("java が無いので Java 側を飛ばしました", "java not found; skipped the Java side"))
            }
        }),
    },
    Backend {
        id: "sql",
        name: "SQL",
        // The query runs on PostgreSQL; the agreement check runs it on the SQLite that ships
        // inside python3, so that is the tool it needs (§15.46).
        tool: "python3",
        lang: Lang::Sql,
        files: |g, alias, _pkg| {
            vec![
                (format!("sql/{alias}.sql"), g.sql()),
                (format!("sql/{alias}_runner.py"), g.sql_runner()),
                ("sql/_round_test.py".into(), crate::codegen::round_tests_sql()),
            ]
        },
        stem: None,
        run: |alias, _| Plan::new("sql", "python3", &["-B", &format!("{alias}_runner.py")]),
        round: |_| Plan::new("sql", "python3", &["-B", "_round_test.py"]),
        folds: false,
        wasi: None,
        mcp: None,
        ready: None,
    },
    Backend {
        id: "wasm",
        name: "Wasm",
        tool: "rustc",
        lang: Lang::Rs,
        // The module is the Rust one; what differs is the door. `<alias>_wasm.rs` is the crate
        // root that exports the canonical ABI, `<alias>.wit` names the world, and the Node
        // runner drives the built module through that ABI (§15.64).
        files: |g, alias, _pkg| {
            vec![
                (format!("wasm/{alias}.rs"), g.rust()),
                (format!("wasm/{alias}_wasm.rs"), g.rs_wasm()),
                (format!("wasm/{alias}.wit"), g.wit()),
                (format!("wasm/{alias}_runner.mjs"), crate::codegen::wasm_runner_js(alias)),
                ("wasm/_round.rs".into(), crate::codegen::round_wasm_rust()),
                ("wasm/_round_test.mjs".into(), crate::codegen::round_tests_wasm_js()),
            ]
        },
        stem: None,
        run: |alias, _| {
            let args = wasm_rustc(&format!("{alias}_wasm.rs"), &format!("{alias}.wasm"));
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            Plan::new("wasm", "node", &[&format!("{alias}_runner.mjs")]).built("rustc", &refs)
        },
        round: |_| {
            let args = wasm_rustc("_round.rs", "_round.wasm");
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            Plan::new("wasm", "node", &["_round_test.mjs"]).built("rustc", &refs)
        },
        folds: true,
        wasi: None,
        mcp: None,
        ready: Some(|| {
            if !have("node") {
                return Err(tr!("node が無いので Wasm 側を飛ばしました", "node not found; skipped the Wasm side"));
            }
            if !rust_target("wasm32-unknown-unknown") {
                return Err(tr!(
                    "wasm32-unknown-unknown の標準ライブラリが無いので Wasm 側を飛ばしました（rustup target add wasm32-unknown-unknown）",
                    "the wasm32-unknown-unknown standard library is not installed; skipped the Wasm side (rustup target add wasm32-unknown-unknown)"
                ));
            }
            Ok(())
        }),
    },
];

/// The rustc invocation for the runner as a WASI command: the flags of
/// `codegen::WASI_RUSTC_FLAGS`, so `rulec test` and the `wasi` entry of `rulec api` cannot
/// build it two different ways.
fn wasi_rustc(src: &str, out: &str) -> Vec<String> {
    let mut v: Vec<String> = vec!["--edition".into(), "2021".into()];
    v.extend(crate::codegen::WASI_RUSTC_FLAGS.split(' ').map(|s| s.to_string()));
    v.extend([src.to_string(), "-o".to_string(), out.to_string()]);
    v
}

/// The rustc invocation for a Wasm module: the flags of `codegen::WASM_RUSTC_FLAGS`, the
/// source, the output.
fn wasm_rustc(src: &str, out: &str) -> Vec<String> {
    let mut v: Vec<String> = vec!["--edition".into(), "2021".into()];
    v.extend(crate::codegen::WASM_RUSTC_FLAGS.split(' ').map(|s| s.to_string()));
    v.extend([src.to_string(), "-o".to_string(), out.to_string()]);
    v
}

/// What this backend calls the files it wrote for `alias`.
pub fn stem(b: &Backend, alias: &str) -> String {
    b.stem.map(|f| f(alias)).unwrap_or_else(|| alias.to_string())
}

/// The backend with this id, for the places that address one by name.
pub fn by_id(id: &str) -> Option<&'static Backend> {
    ALL.iter().find(|b| b.id == id)
}

/// The ids, in order. `rulec api` keys its entries with these, the output directory is
/// named with them, and the documentation lists them in this order.
pub fn ids() -> Vec<&'static str> {
    ALL.iter().map(|b| b.id).collect()
}

/// The names as a person reads them, in the output language. Every sentence that lists the
/// targets takes them from here, so that adding one is still a row in this file (§15.21).
pub fn names() -> String {
    if crate::i18n::ja() {
        names_ja()
    } else {
        names_en()
    }
}

/// The names as a person reads them, joined the way the prose does: `A, B and C`.
pub fn names_en() -> String {
    let ns: Vec<&str> = ALL.iter().map(|b| b.name).collect();
    match ns.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => ns.join(""),
    }
}

/// The same for Japanese, which separates with a middle dot and needs no conjunction.
pub fn names_ja() -> String {
    ALL.iter().map(|b| b.name).collect::<Vec<_>>().join("・")
}
