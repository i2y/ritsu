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
    /// How to run the generated runner over the vectors, relative to the output directory.
    pub run: fn(&str, &str) -> Plan,
    /// How to run the unit vectors of the rounding helpers.
    pub round: fn(&str) -> Plan,
    /// How to start the rule as an MCP server (§15.44), for the languages that get one.
    /// `rulec test` drives it over the vectors like the runner and holds its answers to the
    /// same expected records.
    pub mcp: Option<fn(&str) -> Plan>,
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
            ]
        },
        // `-B` and a cleared cache: a `.pyc` counts as fresh when the source has the same
        // length and the same whole-second mtime, so a same-length edit within a second of
        // the last run would otherwise execute the old module and report it as ok.
        run: |alias, _| Plan::new("python", "python3", &["-B", &format!("{alias}_runner.py")]),
        round: |_| Plan::new("python", "python3", &["-B", "_round_test.py"]),
        mcp: Some(|alias| Plan::new("python", "python3", &["-B", &format!("{alias}_mcp.py")])),
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
            ]
        },
        run: |alias, _| {
            Plan::new("typescript", "node", &["--no-warnings", &format!("{alias}_runner.ts")])
        },
        round: |_| Plan::new("typescript", "node", &["--no-warnings", "_round_test.ts"]),
        mcp: Some(|alias| Plan::new("typescript", "node", &["--no-warnings", &format!("{alias}_mcp.ts")])),
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
            ]
        },
        run: |alias, _| Plan::new("javascript", "node", &[&format!("{alias}_runner.mjs")]),
        round: |_| Plan::new("javascript", "node", &["_round_test.mjs"]),
        mcp: Some(|alias| Plan::new("javascript", "node", &[&format!("{alias}_mcp.mjs")])),
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
        mcp: None,
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
        run: |alias, _| Plan::new("ruby", "ruby", &[&format!("{alias}_runner.rb")]),
        round: |_| Plan::new("ruby", "ruby", &["_round_test.rb"]),
        mcp: None,
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
        run: |_, pkg| Plan::new(&format!("go/{pkg}runner"), "go", &["run", "."]),
        round: |pkg| Plan::new(&format!("go/{pkg}"), "go", &["test", "./..."]),
        mcp: None,
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
        mcp: None,
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
        run: |alias, _| Plan::new("sql", "python3", &["-B", &format!("{alias}_runner.py")]),
        round: |_| Plan::new("sql", "python3", &["-B", "_round_test.py"]),
        mcp: None,
    },
];

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
