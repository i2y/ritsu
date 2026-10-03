//! The settings sakai writes, run by the tools themselves (DESIGN 7.6; PLAN C.13). For each of
//! the four, the settings written from the example's map keep the example's imports, catch each
//! of the four imports the map does not allow (`tests/code/<language>/`), and are seen not to pass
//! in silence: import-linter analyzed every module, dependency-cruiser cruised every file and read
//! TypeScript, ArchUnit found every rule, go-arch-lint held every file to a component. The nested
//! map (`tests/maps/入れ子`) goes the same way. What the tools say about the imports they catch
//! is held in `tests/golden/imports/<tool>.txt`.
//!
//! A tool that is not there is told with `SKIP:` and its test passes; `tools/README.md` says how
//! to put each in `tools/`.

mod common;

use common::TempDir;
use sakai::build::{self, Target};
use ritsu_base::text::Lang;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// A map to write the settings of, and where the imports it does not allow are.
struct Case {
    dir: &'static str,
    map: &'static str,
    mutants: &'static str,
}

const CASES: [Case; 2] = [
    Case { dir: common::EXAMPLE, map: "通販.ctx", mutants: "tests/code" },
    Case { dir: "tests/maps/入れ子", map: "入れ子.ctx", mutants: "tests/code/入れ子" },
];

fn code_dir(t: Target) -> &'static str {
    match t {
        Target::ImportLinter => "py",
        Target::DependencyCruiser => "ts",
        Target::ArchUnit => "java/src/main/java",
        Target::GoArchLint => "go",
    }
}

fn language(t: Target) -> &'static str {
    match t {
        Target::ImportLinter => "python",
        Target::DependencyCruiser => "typescript",
        Target::ArchUnit => "java",
        Target::GoArchLint => "go",
    }
}

fn extension(t: Target) -> &'static str {
    match t {
        Target::ImportLinter => ".py",
        Target::DependencyCruiser => ".ts",
        Target::ArchUnit => ".java",
        Target::GoArchLint => ".go",
    }
}

/// The imports a case does not allow, in a language: the name, and the files to lay over the code.
fn mutants(case: &Case, t: Target) -> Vec<(String, PathBuf)> {
    let d = Path::new(case.mutants).join(language(t));
    let mut v: Vec<(String, PathBuf)> = std::fs::read_dir(&d).unwrap().filter_map(|e| e.ok()).filter(|e| e.path().is_dir()).map(|e| (e.file_name().to_string_lossy().to_string(), e.path())).collect();
    v.sort();
    v
}

/// The code files under a directory, counted.
fn count(dir: &Path, ext: &str) -> usize {
    let mut n = 0;
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            n += count(&p, ext);
        } else if p.to_string_lossy().ends_with(ext) {
            n += 1;
        }
    }
    n
}

/// A copy of the case with the settings sakai writes (`--lang ja`), and the mutant's files over
/// its code, added after the settings were written.
fn prepared(case: &Case, t: Target, mutant: Option<&Path>) -> TempDir {
    let dir = TempDir::new(case.dir.rsplit('/').next().unwrap_or("case"));
    common::copy_dir(Path::new(case.dir), dir.path());
    let b = build::run(dir.path(), case.map, t, None, false, Lang::Ja).unwrap();
    let text: String = b.outcome.diags.iter().map(|d| d.render(Lang::En)).collect();
    assert!(!b.outcome.has_errors() && b.done.is_some(), "{}: {text}", t.word());
    if let Some(m) = mutant {
        common::copy_dir(m, &dir.path().join(code_dir(t)));
    }
    dir
}

/// What one run of a tool came to.
struct Said {
    ok: bool,
    /// What it says about each import it caught, a line each.
    lines: Vec<String>,
    /// How much it read: the modules, files or rules it says it went through.
    read: usize,
    /// All it printed, for a failure's message.
    raw: String,
}

/// Run the tool on each case and mutant, and hold what it says to its golden file. `files` says
/// how many of the code files a run must have read, from the number there is.
fn each(t: Target, run: &(dyn Fn(&Path) -> Said + Sync), at_least: &(dyn Fn(usize, &Path) -> usize + Sync)) {
    let mut jobs: Vec<(String, &Case, Option<PathBuf>)> = Vec::new();
    for case in &CASES {
        jobs.push((format!("{} (as written)", case.dir), case, None));
        for (name, p) in mutants(case, t) {
            jobs.push((format!("{} {name}", case.dir), case, Some(p)));
        }
    }
    let results: Vec<(String, bool, Said, usize)> = std::thread::scope(|s| {
        let hs: Vec<_> = jobs
            .iter()
            .map(|(label, case, m)| {
                s.spawn(move || {
                    let dir = prepared(case, t, m.as_deref());
                    let code = dir.path().join(code_dir(t));
                    let want = at_least(count(&code, extension(t)), dir.path());
                    let said = run(dir.path());
                    (label.clone(), m.is_some(), said, want)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut failures = Vec::new();
    let mut golden = String::new();
    for (label, mutated, said, want) in results {
        if said.read < want {
            failures.push(format!("{label}: {} read {} where there are {want}; it may have passed in silence:\n{}", t.word(), said.read, said.raw));
        }
        if !mutated {
            if !said.ok || !said.lines.is_empty() {
                failures.push(format!("{label}: {} does not keep the imports the map allows:\n{}", t.word(), said.raw));
            }
            continue;
        }
        if said.ok || said.lines.is_empty() {
            failures.push(format!("{label}: {} does not catch the import the map does not allow:\n{}", t.word(), said.raw));
        }
        golden.push_str(&format!("== {label}\n"));
        for l in &said.lines {
            golden.push_str(l);
            golden.push('\n');
        }
    }
    if let Some(f) = common::golden(&format!("tests/golden/imports/{}.txt", t.word()), &golden) {
        failures.push(f);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn import_linter_keeps_the_map() {
    if !common::linters() {
        return;
    }
    let Some(tool) = common::lint_imports() else {
        common::skip("import-linter is not there (SAKAI_LINT_IMPORTS, or tools/.venv: uv venv --python 3.13 tools/.venv && uv pip install --python tools/.venv/bin/python --require-hashes -r tools/requirements.txt)");
        return;
    };
    let run = |dir: &Path| {
        let r = common::run(
            Command::new(&tool).args(["--no-logo", "--no-cache", "--config", ".importlinter"]).current_dir(dir.join("py")).env("COLUMNS", "300").env("PYTHONDONTWRITEBYTECODE", "1"),
            Duration::from_secs(120),
        );
        let out = format!("{}{}", r.stdout, r.stderr);
        let read = out.lines().find_map(|l| l.strip_prefix("Analyzed ").and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok())).unwrap_or(0);
        let lines = out.lines().map(str::trim).filter(|l| l.starts_with("Illegal imports of protected package") || (l.starts_with("- ") && l.contains(" -> "))).map(String::from).collect();
        Said { ok: r.ok, lines, read, raw: out }
    };
    each(Target::ImportLinter, &run, &|n, _| n);
}

#[test]
fn dependency_cruiser_keeps_the_map() {
    if !common::linters() {
        return;
    }
    let Some(tool) = common::depcruise() else {
        common::skip("dependency-cruiser is not there (SAKAI_DEPCRUISE, or tools/node_modules: npm ci --prefix tools)");
        return;
    };
    // dependency-cruiser 16 reads TypeScript under 6 only; with another, it cruises no .ts file and
    // passes (DESIGN 7.3). That is a failure, not a skip.
    let info = common::run(Command::new(&tool).arg("--info"), Duration::from_secs(60));
    let ts = info.stdout.lines().map(str::trim).find(|l| l.contains(" typescript ")).unwrap_or("").to_string();
    assert!(ts.starts_with('✔'), "dependency-cruiser does not read the TypeScript it finds, and would pass every .ts file in silence: {ts}");
    let run = |dir: &Path| {
        let r = common::run(Command::new(&tool).args(["--config", ".dependency-cruiser.cjs", "--output-type", "json", "."]).current_dir(dir.join("ts")), Duration::from_secs(120));
        let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap_or(serde_json::Value::Null);
        let read = v["summary"]["totalCruised"].as_u64().unwrap_or(0) as usize;
        let lines = v["summary"]["violations"]
            .as_array()
            .map(|vs| vs.iter().map(|x| format!("{} {}: {} → {}", x["rule"]["severity"].as_str().unwrap_or(""), x["rule"]["name"].as_str().unwrap_or(""), x["from"].as_str().unwrap_or(""), x["to"].as_str().unwrap_or(""))).collect())
            .unwrap_or_default();
        // The JSON reporter exits 0 whatever it finds (the `err` reporter, for CI, exits with the
        // number of errors): what it found is in the JSON.
        let ok = r.ok && v["summary"]["violations"].as_array().is_some_and(|a| a.is_empty());
        Said { ok, lines, read, raw: format!("{}{}", r.stdout, r.stderr) }
    };
    each(Target::DependencyCruiser, &run, &|n, _| n);
}

#[test]
fn archunit_keeps_the_map() {
    if !common::linters() {
        return;
    }
    let (Some(java), Some(javac), Some(lib)) = (common::java("java"), common::java("javac"), common::archunit_lib()) else {
        common::skip("Java or the jars of ArchUnit are not there (SAKAI_JAVA and SAKAI_JAVAC, or JAVA_HOME; SAKAI_ARCHUNIT_LIB, or tools/java/lib: tools/java/fetch.sh)");
        return;
    };
    let jars: Vec<String> = ["archunit-1.5.1.jar", "archunit-junit5-api-1.5.1.jar", "archunit-junit5-engine-1.5.1.jar", "archunit-junit5-engine-api-1.5.1.jar", "slf4j-api-2.0.17.jar"]
        .iter()
        .map(|j| lib.join(j).to_string_lossy().to_string())
        .collect();
    let run = |dir: &Path| {
        let (main, test) = (dir.join("classes/main"), dir.join("classes/test"));
        let mut sources = Vec::new();
        collect(&dir.join("java/src/main/java"), ".java", &mut sources);
        let limit = Duration::from_secs(180);
        let a = common::run(Command::new(&javac).args(["--release", "21", "-d"]).arg(&main).args(&sources), limit);
        assert!(a.ok, "javac: {}", a.stderr);
        let b = common::run(
            Command::new(&javac).args(["--release", "21", "-cp"]).arg(format!("{}:{}", main.display(), jars.join(":"))).arg("-d").arg(&test).arg(dir.join("java/src/test/java/SakaiContextsTest.java")),
            limit,
        );
        assert!(b.ok, "javac: {}", b.stderr);
        let cp = format!("{}:{}:{}", main.display(), test.display(), jars.join(":"));
        let r = common::run(
            Command::new(&java)
                .arg("-jar")
                .arg(lib.join("junit-platform-console-standalone-6.1.3.jar"))
                .args(["execute", "--class-path", &cp, "--select-class", "SakaiContextsTest", "--disable-banner", "--details=tree", "--disable-ansi-colors"]),
            limit,
        );
        let out = format!("{}{}", r.stdout, r.stderr);
        let found = out.lines().filter(|l| l.contains("tests found")).find_map(|l| l.split_whitespace().find_map(|w| w.parse::<usize>().ok())).unwrap_or(0);
        let mut lines: Vec<String> = Vec::new();
        for l in out.lines() {
            let l = l.trim_start_matches(|c: char| c.is_whitespace() || "│├└─".contains(c));
            let line = if let Some(i) = l.find("Rule '") {
                Some(l[i..].to_string())
            } else if l.contains("> in (") && l.trim_end().ends_with(')') && l.contains(".java:") {
                Some(l.trim().to_string())
            } else {
                None
            };
            if let Some(x) = line
                && !lines.contains(&x)
            {
                lines.push(x);
            }
        }
        Said { ok: r.ok, lines, read: found, raw: out }
    };
    // Every rule the settings hold is found and run.
    let rules = |_: usize, dir: &Path| std::fs::read_to_string(dir.join("java/src/test/java/SakaiContextsTest.java")).unwrap().matches("@ArchTest").count().max(1);
    each(Target::ArchUnit, &run, &rules);
}

fn collect(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, ext, out);
        } else if p.to_string_lossy().ends_with(ext) {
            out.push(p);
        }
    }
}

#[test]
fn go_arch_lint_keeps_the_map() {
    if !common::linters() {
        return;
    }
    let (Some(gal), Some(go)) = (common::go_arch_lint(), common::go()) else {
        common::skip("go or go-arch-lint is not there (SAKAI_GO and SAKAI_GO_ARCH_LINT, or tools/go/bin: tools/go/install.sh)");
        return;
    };
    // go-arch-lint runs `go list`; go's caches go in a directory of the test.
    let cache = TempDir::new("go-cache");
    // go-arch-lint finds go on the PATH: the go named by SAKAI_GO goes first.
    let path = match Path::new(&go).parent().filter(|d| !d.as_os_str().is_empty()) {
        Some(d) => format!("{}:{}", d.display(), std::env::var("PATH").unwrap_or_default()),
        None => std::env::var("PATH").unwrap_or_default(),
    };
    let run = |dir: &Path| {
        let r = common::run(
            Command::new(&gal)
                .args(["check", "--json"])
                .current_dir(dir.join("go"))
                .env("PATH", &path)
                .env("GOCACHE", cache.path().join("build"))
                .env("GOMODCACHE", cache.path().join("mod"))
                .env("GOPATH", cache.path().join("gopath"))
                .env("GOTOOLCHAIN", "local")
                .env("GOFLAGS", "-trimpath")
                .env("GOWORK", "off"),
            Duration::from_secs(300),
        );
        let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap_or(serde_json::Value::Null);
        let p = &v["Payload"];
        let deps = p["ArchWarningsDeps"].as_array().cloned().unwrap_or_default();
        let unmatched = p["ArchWarningsNotMatched"].as_array().map(|a| a.len()).unwrap_or(usize::MAX);
        let lines = deps
            .iter()
            .map(|w| {
                let f = w["FileRelativePath"].as_str().unwrap_or("").trim_start_matches('/');
                format!("Component {} shouldn't depend on {} in {f}:{}", w["ComponentName"].as_str().unwrap_or(""), w["ResolvedImportName"].as_str().unwrap_or(""), w["Reference"]["Line"])
            })
            .collect();
        // A file no component holds is not checked; every file is held, or the run is a failure.
        let read = if unmatched == 0 && p.is_object() { usize::MAX } else { 0 };
        Said { ok: r.ok, lines, read, raw: format!("{}{}", r.stdout, r.stderr) }
    };
    each(Target::GoArchLint, &run, &|n, _| n);
}
