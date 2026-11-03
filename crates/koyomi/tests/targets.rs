//! The generated code, held to the reference interpreter (PLAN C.8, DESIGN 6.4). For each of
//! the six targets, every example that passes check and every `tests/fixtures/helpers_*`
//! file is generated, the target's own tools look at it (tsc, mypy, gofmt and go vet, rustc on
//! two editions, PostgreSQL, rustc for wasm32-unknown-unknown), and its runner is given every
//! line of `koyomi vectors`. Every line it prints is compared with what the reference
//! interpreter gives. What the Wasm module answers as JSON, and the component the `.wit` makes
//! of it, are tests/wasm.rs's.
//!
//! The files of `tests/spellings/` (DESIGN 15) go through the targets too: each through the
//! targets its W001 does not name, its renamed one (the alias the note gives) through all six, and
//! each through the ones it names, which have to stop where `common::SPELLING_FILES` says, or load
//! and answer wrongly (`breaks_typescript`, `breaks_python`, `breaks_go`, `breaks_sql`, `breaks_rust`,
//! `breaks_wasm`). These are the files `gen` does not write (E016), made as `gen` made them before it
//! stopped. So do the files of `tests/spellings/across/` (DESIGN 17): the versions with the alias the
//! note gives go through all six, and two files loaded into one PostgreSQL database show why the
//! later one's W001 is there (`across_sql`).
//!
//! The vectors are made in this process and piped to the runner as they are made; nothing as
//! large as them is written to disk. A tool that is missing prints `SKIP:` and the test passes
//! (ritsu-testkit's: the tools are found, the runners run and PostgreSQL is started by it).
//! `-- --nocapture` shows a `compared <target> <file>: <n> lines` line for every comparison.

mod common;

use ritsu_testkit::pg::{self, Postgres};
use ritsu_testkit::run::{Ran, pipe, run};
use ritsu_testkit::tools::{find, on_path, runs, version};
use ritsu_testkit::{Need, TempDir, need, skip};
use koyomi::check::{Checked, check};
use koyomi::codegen::{self, wasm};
use ritsu_base::text::Lang;
use koyomi::naming::{Target, go_package};
use koyomi::vectors::{self, Expect, Row};
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Every file the targets are held to.
pub const FILES: &[&str] = common::TARGET_FILES;

/// How long one runner may take before it is killed.
const LIMIT: Duration = Duration::from_secs(900);

/// The runners of one target that run at the same time.
const AT_ONCE: usize = 4;

struct Subject {
    path: String,
    checked: Checked,
    alias: String,
}

fn subject(p: &str) -> Subject {
    let mut o = check(p).unwrap();
    assert!(!o.has_errors(), "{p} passes check");
    let checked = o.checked.take().unwrap();
    let alias = codegen::unit_of(&checked, Lang::En).alias;
    Subject { path: p.to_string(), checked, alias }
}

/// The targets a subject's W001 names (DESIGN 15).
fn named(s: &Subject) -> Vec<Target> {
    match &s.checked {
        Checked::Dates(m, _) => koyomi::spellings::find(m).into_iter().flat_map(|f| f.targets).collect(),
        Checked::Calendar(_) => Vec::new(),
    }
}

/// What `t` is held to: every file of `FILES`, the files of tests/spellings whose W001 does not
/// name `t`, and every renamed one.
fn subjects(t: Target) -> Vec<Subject> {
    let mut v: Vec<Subject> = FILES.iter().map(|p| subject(p)).collect();
    for (p, _) in common::SPELLING_FILES {
        let s = subject(p);
        let n = named(&s);
        assert!(!n.is_empty(), "{p} gives W001");
        if !n.contains(&t) {
            v.push(s);
        }
        v.push(subject(&common::renamed(p)));
    }
    let mut seen = Vec::new();
    for (first, later, _) in common::ACROSS {
        for p in [first.to_string(), later.to_string(), common::renamed_across(later)] {
            if seen.contains(&p) {
                continue;
            }
            seen.push(p.clone());
            // the first file and the renamed ones are subjects of every target; of a later file,
            // the targets its pair does not make one
            let named = common::ACROSS.iter().any(|(_, l, n)| *l == p && n.contains(&t));
            if !named {
                v.push(subject(&p));
            }
        }
    }
    v
}

/// The files of tests/spellings whose W001 names `t`, with what its tool says of each.
fn broken(t: Target) -> Vec<(Subject, &'static str)> {
    common::SPELLING_FILES.iter().filter_map(|(p, says)| says.iter().find(|(x, _)| *x == t).map(|(_, said)| (subject(p), *said))).collect()
}

/// Write each subject's code for `t` under `dir`, its comments and messages in `lang`.
fn generate(dir: &Path, t: Target, ss: &[Subject], lang: Lang) {
    for s in ss {
        let u = codegen::unit_of(&s.checked, lang);
        for (rel, body) in codegen::files(&u, t) {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        }
    }
}

/// The rows of a subject's vectors, made as they are read.
fn rows(c: &Checked) -> Box<dyn Iterator<Item = Row> + '_> {
    match c {
        Checked::Dates(m, _) => Box::new(vectors::DatesRows::new(m)),
        Checked::Calendar(cal) => Box::new(vectors::calendar_rows(cal)),
    }
}

/// A row as the runner reads it.
fn line_of(c: &Checked, lines: Option<&vectors::DatesLines>, r: &Row) -> String {
    match c {
        Checked::Dates(..) => lines.unwrap().line(r),
        Checked::Calendar(_) => vectors::calendar_line(r),
    }
}

/// A row as the runner prints it.
fn want_of(r: &Row) -> String {
    match &r.expect {
        Expect::Values(v) => v.join(" "),
        Expect::Open(o) => o.to_string(),
        Expect::Error(k) => format!("error {k}"),
    }
}

/// Feed a subject's vectors to the runner `cmd` and compare every line it prints. The number
/// of lines compared, or what differs.
fn compare(s: &Subject, mut cmd: Command) -> Result<usize, String> {
    let lines = match &s.checked {
        Checked::Dates(m, _) => Some(vectors::DatesLines::new(m)),
        Checked::Calendar(_) => None,
    };
    let mut expect = rows(&s.checked);
    let mut n = 0usize;
    let mut diffs: Vec<String> = Vec::new();
    let mut extra = 0usize;
    let ran = pipe(
        &mut cmd,
        LIMIT,
        |w| {
            for r in rows(&s.checked) {
                if writeln!(w, "{}", line_of(&s.checked, lines.as_ref(), &r)).is_err() {
                    return;
                }
            }
        },
        |got| match expect.next() {
            Some(r) => {
                n += 1;
                let want = want_of(&r);
                if got != want && diffs.len() < 5 {
                    diffs.push(format!("  input {}\n    reference: {want}\n    generated: {got}", line_of(&s.checked, lines.as_ref(), &r)));
                } else if got != want {
                    diffs.push(String::new());
                }
            }
            None => extra += 1,
        },
    );
    let missing = expect.count();
    let wrong = diffs.len();
    if ran.timed_out {
        return Err(format!("{}: the runner took longer than {LIMIT:?}", s.path));
    }
    if !ran.ok || wrong > 0 || missing > 0 || extra > 0 {
        let shown: Vec<String> = diffs.into_iter().filter(|d| !d.is_empty()).collect();
        return Err(format!(
            "{}: {wrong} lines differ, {missing} lines missing, {extra} lines too many (exit ok: {})\n{}\n  stderr:\n{}",
            s.path,
            ran.ok,
            shown.join("\n"),
            ran.stderr
        ));
    }
    Ok(n)
}

/// The files the `--lang ja` code is run on as well: in English and in Japanese, one with the rows
/// outside the range (`range`) and one with the days outside the data range (`data`). The Japanese
/// code differs from the English only in its comments and in the messages of its errors.
const JA_FILES: &[&str] = &[
    "examples/payment_20th_close_next_10th.cal",
    "examples/calendars/england_and_wales.cal",
    "examples/payment_20th_close_next_10th.ja.cal",
    "examples/calendars/東京の営業日.cal",
];

/// Run the `--lang ja` runners of `JA_FILES` and fail if one disagrees.
fn compare_ja(target: &str, ss: &[Subject], job: impl Fn(&Subject) -> Command + Sync) {
    for s in ss.iter().filter(|s| JA_FILES.contains(&s.path.as_str())) {
        match compare(s, job(s)) {
            Ok(n) => println!("{target} --lang ja: {} agrees on its {n} lines", s.path),
            Err(e) => panic!("{target}'s --lang ja code disagrees with the reference interpreter:\n{e}"),
        }
    }
}

/// Run `job` for every subject, `AT_ONCE` at a time; print a `compared` line for each that
/// matches and fail with every one that does not.
fn compare_all(target: &str, ss: &[Subject], job: impl Fn(&Subject) -> Command + Sync) {
    let queue: Mutex<Vec<&Subject>> = Mutex::new(ss.iter().rev().collect());
    let failures: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let start = Instant::now();
    std::thread::scope(|sc| {
        for _ in 0..AT_ONCE {
            sc.spawn(|| {
                loop {
                    let Some(s) = queue.lock().unwrap().pop() else { break };
                    let t = Instant::now();
                    match compare(s, job(s)) {
                        Ok(n) => println!("compared {target} {}: {n} lines ({:.1} s)", s.path, t.elapsed().as_secs_f64()),
                        Err(e) => failures.lock().unwrap().push(e),
                    }
                }
            });
        }
    });
    println!("{target}: every file compared in {:.1} s", start.elapsed().as_secs_f64());
    let f = failures.into_inner().unwrap();
    assert!(f.is_empty(), "{target} disagrees with the reference interpreter:\n{}", f.join("\n"));
}

/// Fail with what a tool said, unless it succeeded.
fn ok_or_fail(what: &str, r: &Ran) {
    assert!(r.ok && !r.timed_out, "{what} failed:\n{}{}", r.stdout, r.stderr);
}

#[test]
fn typescript() {
    if !need(Need::Node) {
        return;
    }
    if !runs("node", &["--version"]) {
        skip("node is not on the PATH; the TypeScript target is not run");
        return;
    }
    let ss = subjects(Target::TypeScript);
    let (dir, ja) = (TempDir::new("ts"), TempDir::new("ts-ja"));
    for (d, lang) in [(&dir, Lang::En), (&ja, Lang::Ja)] {
        generate(d.path(), Target::TypeScript, &ss, lang);
        // Node strips the types of a .ts file inside a package of ES modules.
        std::fs::write(d.path().join("typescript/package.json"), "{\"type\": \"module\"}\n").unwrap();
    }
    println!("typescript: {}", version("node", "--version"));
    match find("TSC", Some(Path::new("tools/node_modules/.bin/tsc")), "tsc", &["--version"]) {
        Some(tsc) => {
            let types = std::fs::canonicalize("tools/node_modules/@types").ok();
            for d in [&dir, &ja] {
                let mut c = Command::new(&tsc);
                c.args(["--strict", "--noEmit", "--allowImportingTsExtensions", "--erasableSyntaxOnly", "--module", "nodenext", "--target", "es2022", "--types", "node"]);
                if let Some(t) = &types {
                    c.arg("--typeRoots").arg(t);
                }
                for s in &ss {
                    c.arg(d.path().join(format!("typescript/{}.ts", s.alias)));
                    c.arg(d.path().join(format!("typescript/{}_runner.ts", s.alias)));
                }
                ok_or_fail("tsc --strict", &run(&mut c, LIMIT));
            }
            println!("typescript: tsc --strict --erasableSyntaxOnly passes, in English and in Japanese ({})", version(&tsc, "--version"));
        }
        None => skip("tsc is not installed (npm ci --prefix tools); the TypeScript is run but not type-checked"),
    }
    let runner = |d: &TempDir, s: &Subject| {
        let mut c = Command::new("node");
        c.arg("--disable-warning=ExperimentalWarning").arg(d.path().join(format!("typescript/{}_runner.ts", s.alias)));
        c
    };
    compare_all("typescript", &ss, |s| runner(&dir, s));
    compare_ja("typescript", &ss, |s| runner(&ja, s));
    breaks_typescript();
}

/// Each file of tests/spellings whose W001 names TypeScript, generated alone: Node, which strips the
/// types and loads the module, stops where `common::SPELLING_FILES` says.
fn breaks_typescript() {
    let bs = broken(Target::TypeScript);
    for (s, says) in &bs {
        let d = TempDir::new("ts-breaks");
        generate(d.path(), Target::TypeScript, std::slice::from_ref(s), Lang::En);
        std::fs::write(d.path().join("typescript/package.json"), "{\"type\": \"module\"}\n").unwrap();
        let r = run(Command::new("node").arg("--disable-warning=ExperimentalWarning").arg(d.path().join(format!("typescript/{}_runner.ts", s.alias))).stdin(std::process::Stdio::null()), LIMIT);
        let said = format!("{}{}", r.stdout, r.stderr);
        assert!(!r.ok && said.contains(says), "{}: node was to stop with {says:?}, and said:\n{said}", s.path);
        println!("typescript: {} stops node ({says})", s.path);
    }
    assert!(bs.len() >= 2, "{} files of tests/spellings name TypeScript", bs.len());
}

#[test]
fn python() {
    if !need(Need::Python) {
        return;
    }
    if !runs("python3", &["--version"]) {
        skip("python3 is not on the PATH; the Python target is not run");
        return;
    }
    let ss = subjects(Target::Python);
    let (dir, ja) = (TempDir::new("py"), TempDir::new("py-ja"));
    generate(dir.path(), Target::Python, &ss, Lang::En);
    generate(ja.path(), Target::Python, &ss, Lang::Ja);
    println!("python: {}", version("python3", "--version"));
    match find("MYPY", Some(Path::new("tools/.venv/bin/mypy")), "mypy", &["--version"]) {
        Some(mypy) => {
            for d in [&dir, &ja] {
                let mut c = Command::new(&mypy);
                c.args(["--strict", "--no-incremental", "--cache-dir", &d.path().join(".mypy_cache").to_string_lossy()]).current_dir(d.path().join("python"));
                for s in &ss {
                    c.arg(format!("{}.py", s.alias)).arg(format!("{}_runner.py", s.alias));
                }
                ok_or_fail("mypy --strict", &run(&mut c, LIMIT));
            }
            println!("python: mypy --strict passes, in English and in Japanese ({})", version(&mypy, "--version"));
        }
        None => skip("mypy is not installed (tools/requirements.txt); the Python is run but not type-checked"),
    }
    let runner = |d: &TempDir, s: &Subject| {
        let mut c = Command::new("python3");
        c.arg(d.path().join(format!("python/{}_runner.py", s.alias)));
        c
    };
    compare_all("python", &ss, |s| runner(&dir, s));
    compare_ja("python", &ss, |s| runner(&ja, s));
    breaks_python();
}

/// Each file of tests/spellings whose W001 names Python, generated alone, its runner run from
/// beside the module as every runner is: it stops where `common::SPELLING_FILES` says (`import
/// <alias>` gives the standard library's module of that name, or the generated module hides a
/// built-in it uses); or, when it answers as it should, mypy --strict stops there.
fn breaks_python() {
    let bs = broken(Target::Python);
    for (s, says) in &bs {
        let d = TempDir::new("py-breaks");
        generate(d.path(), Target::Python, std::slice::from_ref(s), Lang::En);
        let mut c = Command::new("python3");
        c.arg(d.path().join(format!("python/{}_runner.py", s.alias)));
        match compare(s, c) {
            Ok(_) => {
                let Some(mypy) = find("MYPY", Some(Path::new("tools/.venv/bin/mypy")), "mypy", &["--version"]) else {
                    skip("mypy is not installed (tools/requirements.txt); what it says of a built-in the module hides is not looked at");
                    continue;
                };
                let r = run(Command::new(&mypy).args(["--strict", "--no-incremental", "--cache-dir", &d.path().join(".mypy_cache").to_string_lossy()]).arg(format!("{}.py", s.alias)).current_dir(d.path().join("python")), LIMIT);
                let said = format!("{}{}", r.stdout, r.stderr);
                assert!(!r.ok && said.contains(says), "{}: mypy was to stop with {says:?}, and said:\n{said}", s.path);
                println!("python: {} answers as it should, and stops mypy ({says})", s.path);
            }
            Err(e) => {
                assert!(e.contains(says), "{}: the runner was to stop with {says:?}, and said:\n{e}", s.path);
                println!("python: {} stops its runner ({says})", s.path);
            }
        }
    }
    assert!(bs.len() >= 2, "{} files of tests/spellings name Python", bs.len());
}

#[test]
fn go() {
    if !need(Need::Go) {
        return;
    }
    if on_path("go").is_none() || on_path("gofmt").is_none() {
        skip("go or gofmt is not on the PATH; the Go target is not run");
        return;
    }
    let ss = subjects(Target::Go);
    let (dir, ja) = (TempDir::new("go"), TempDir::new("go-ja"));
    println!("go: {}", version("go", "version"));
    for (d, lang) in [(&dir, Lang::En), (&ja, Lang::Ja)] {
        generate(d.path(), Target::Go, &ss, lang);
        std::fs::write(d.path().join("go.mod"), "module koyomigenerated\n\ngo 1.21\n").unwrap();
        let go = |args: &[&str]| {
            let mut c = Command::new("go");
            // No download and no other toolchain: the generated code needs the standard library only.
            c.args(args).current_dir(d.path()).env("GOTOOLCHAIN", "local").env("GOPROXY", "off").env("GOFLAGS", "-mod=mod");
            c
        };
        let fmt = run(Command::new("gofmt").arg("-l").arg(d.path().join("go")), LIMIT);
        assert!(fmt.ok && fmt.stdout.trim().is_empty(), "gofmt -l lists files it would change:\n{}{}", fmt.stdout, fmt.stderr);
        ok_or_fail("go vet", &run(&mut go(&["vet", "-trimpath", "./..."]), LIMIT));
        std::fs::create_dir_all(d.path().join("bin")).unwrap();
        ok_or_fail("go test -c", &run(&mut go(&["test", "-c", "-trimpath", "-o", "bin/", "./..."]), LIMIT));
    }
    println!("go: gofmt -l lists nothing and go vet passes, in English and in Japanese");
    let runner = |d: &TempDir, s: &Subject| {
        let mut c = Command::new(d.path().join(format!("bin/{}.test", go_package(&s.alias))));
        c.env("KOYOMI_RUNNER", "1");
        c
    };
    compare_all("go", &ss, |s| runner(&dir, s));
    compare_ja("go", &ss, |s| runner(&ja, s));
    breaks_go();
}

/// Each file of tests/spellings whose W001 names Go, generated alone: `go vet` stops where
/// `common::SPELLING_FILES` says. A package named `main` is vetted with a package that imports
/// it, since a program is what Go makes of it.
fn breaks_go() {
    let bs = broken(Target::Go);
    for (s, says) in &bs {
        let d = TempDir::new("go-breaks");
        generate(d.path(), Target::Go, std::slice::from_ref(s), Lang::En);
        std::fs::write(d.path().join("go.mod"), "module koyomigenerated\n\ngo 1.21\n").unwrap();
        let pkg = go_package(&s.alias);
        if pkg == "main" {
            std::fs::create_dir_all(d.path().join("uses")).unwrap();
            let f = codegen::unit_of(&s.checked, Lang::En);
            let first = koyomi::naming::pascal(&f.dates[0].alias);
            std::fs::write(d.path().join("uses/uses.go"), format!("package uses\n\nimport k \"koyomigenerated/go/main\"\n\nvar _ = k.{first}\n")).unwrap();
        }
        let r = run(Command::new("go").args(["vet", "-trimpath", "./..."]).current_dir(d.path()).env("GOTOOLCHAIN", "local").env("GOPROXY", "off").env("GOFLAGS", "-mod=mod"), LIMIT);
        let said = format!("{}{}", r.stdout, r.stderr);
        assert!(!r.ok && said.contains(says), "{}: go vet was to stop with {says:?}, and said:\n{said}", s.path);
        println!("go: {} stops go vet ({says})", s.path);
    }
    assert!(bs.len() >= 12, "{} files of tests/spellings name Go", bs.len());
}

#[test]
fn rust() {
    if !need(Need::Rustc) {
        return;
    }
    if !runs("rustc", &["--version"]) {
        skip("rustc is not on the PATH; the Rust target is not run");
        return;
    }
    let ss = subjects(Target::Rust);
    let (dir, ja) = (TempDir::new("rs"), TempDir::new("rs-ja"));
    println!("rust: {}", version("rustc", "--version"));
    for (d, lang) in [(&dir, Lang::En), (&ja, Lang::Ja)] {
        generate(d.path(), Target::Rust, &ss, lang);
        let out = d.path().join("rust/out");
        std::fs::create_dir_all(&out).unwrap();
        // The module alone, as a library of either edition, and the runner built with it (the
        // Japanese runners only for the files they are run on).
        std::thread::scope(|sc| {
            for s in &ss {
                let out = &out;
                sc.spawn(move || {
                    let src = d.path().join(format!("rust/{}.rs", s.alias));
                    for e in ["2021", "2024"] {
                        let r = run(Command::new("rustc").args(["--edition", e, "--crate-type", "lib", "-D", "warnings", "--emit=metadata", "--out-dir"]).arg(out.join(e)).arg(&src), LIMIT);
                        ok_or_fail(&format!("rustc --edition {e} {}", s.path), &r);
                    }
                    if lang == Lang::Ja && !JA_FILES.contains(&s.path.as_str()) {
                        return;
                    }
                    let r = run(
                        Command::new("rustc")
                            .args(["--edition", "2021", "-D", "warnings", "-O", "-o"])
                            .arg(out.join(format!("{}_runner", s.alias)))
                            .arg(d.path().join(format!("rust/{}_runner.rs", s.alias))),
                        LIMIT,
                    );
                    ok_or_fail(&format!("rustc {}_runner.rs", s.alias), &r);
                });
            }
        });
    }
    println!("rust: every module builds on the 2021 and 2024 editions with -D warnings, in English and in Japanese");
    compare_all("rust", &ss, |s| Command::new(dir.path().join(format!("rust/out/{}_runner", s.alias))));
    compare_ja("rust", &ss, |s| Command::new(ja.path().join(format!("rust/out/{}_runner", s.alias))));
    breaks_rust();
}

/// Each file of tests/spellings whose W001 names Rust, generated alone, its runner (which declares
/// the module by the file's alias) checked with the module: rustc warns where
/// `common::SPELLING_FILES` says, and with `-D warnings` stops there.
fn breaks_rust() {
    let bs = broken(Target::Rust);
    for (s, says) in &bs {
        let d = TempDir::new("rs-breaks");
        generate(d.path(), Target::Rust, std::slice::from_ref(s), Lang::En);
        let runner = d.path().join(format!("rust/{}_runner.rs", s.alias));
        let out = d.path().join("rust/out");
        let warns = run(Command::new("rustc").args(["--edition", "2021", "--emit=metadata", "--out-dir"]).arg(&out).arg(&runner), LIMIT);
        assert!(warns.stderr.contains(says), "{}: rustc was to say {says:?}, and said:\n{}", s.path, warns.stderr);
        let r = run(Command::new("rustc").args(["--edition", "2021", "-D", "warnings", "--emit=metadata", "--out-dir"]).arg(&out).arg(&runner), LIMIT);
        let said = format!("{}{}", r.stdout, r.stderr);
        assert!(!r.ok && said.contains(says), "{}: rustc -D warnings was to stop with {says:?}, and said:\n{said}", s.path);
        println!("rust: {} {}, and stops rustc -D warnings ({says})", s.path, if warns.ok { "builds with a warning" } else { "does not build" });
    }
    assert!(bs.len() >= 8, "{} files of tests/spellings name Rust", bs.len());
}

#[test]
fn sql() {
    if !need(Need::Postgres) {
        return;
    }
    if let Err(why) = pg::bin() {
        skip(&format!("{why}; the SQL target is not run"));
        return;
    }
    let ss = subjects(Target::Sql);
    let (dir, ja) = (TempDir::new("sql"), TempDir::new("sql-ja"));
    generate(dir.path(), Target::Sql, &ss, Lang::En);
    generate(ja.path(), Target::Sql, &ss, Lang::Ja);
    let cluster = match Postgres::start() {
        Ok(c) => c,
        Err(e) => panic!("cannot start a PostgreSQL cluster: {e}"),
    };
    let v = run(cluster.psql("postgres").args(["-t", "-A", "-c", "SHOW server_version"]), LIMIT);
    println!("sql: PostgreSQL {}", v.stdout.trim());
    // The Japanese code goes in a database of its own: its schemas have the same names.
    ok_or_fail("CREATE DATABASE ja", &run(cluster.psql("postgres").args(["-c", "CREATE DATABASE ja"]), LIMIT));
    for (d, db) in [(&dir, "postgres"), (&ja, "ja")] {
        for s in &ss {
            ok_or_fail(&format!("psql -f {}.sql", s.alias), &run(cluster.psql(db).arg("-f").arg(d.path().join(format!("sql/{}.sql", s.alias))), LIMIT));
        }
    }
    println!("sql: every file loads, in English and in Japanese");
    let runner = |d: &TempDir, db: &str, s: &Subject| {
        let mut c = cluster.psql(db);
        c.arg("-f").arg(d.path().join(format!("sql/{}_runner.sql", s.alias)));
        c
    };
    compare_all("sql", &ss, |s| runner(&dir, "postgres", s));
    compare_ja("sql", &ss, |s| runner(&ja, "ja", s));
    breaks_sql(&cluster);
    information_schema(&cluster);
    across_sql(&cluster);
}

/// A file named `information_schema`: as a superuser it loads, but only a superuser creates a function
/// in that schema, and the role an application connects as stops there. There is one such name, so
/// the file is written here rather than kept as a file of tests/spellings with a Japanese version.
fn information_schema(cluster: &Postgres) {
    let d = TempDir::new("sql-information-schema");
    let p = d.path().join("information_schema.cal");
    std::fs::write(&p, "dates information_schema v1\n\ninputs\n  received : date  range >=2026-01-01 <=2026-01-31\n\ndate due = received\n  + 30 days\n").unwrap();
    let s = subject(&p.to_string_lossy());
    assert_eq!(named(&s), [Target::Sql]);
    generate(d.path(), Target::Sql, std::slice::from_ref(&s), Lang::En);
    let sql = d.path().join("sql/information_schema.sql");
    ok_or_fail("CREATE DATABASE", &run(cluster.psql("postgres").args(["-c", "CREATE DATABASE information_schema_db", "-c", "CREATE ROLE koyomi_application LOGIN", "-c", "GRANT CREATE ON DATABASE information_schema_db TO koyomi_application"]), LIMIT));
    let r = run(cluster.psql("information_schema_db").args(["-U", "koyomi_application"]).arg("-f").arg(&sql), LIMIT);
    let said = format!("{}{}", r.stdout, r.stderr);
    let says = "permission denied for schema information_schema";
    assert!(!r.ok && said.contains(says), "PostgreSQL was to stop with {says:?}, and said:\n{said}");
    ok_or_fail("psql -f as a superuser", &run(cluster.psql("information_schema_db").arg("-f").arg(&sql), LIMIT));
    println!("sql: the alias information_schema stops PostgreSQL, as a role that is no superuser ({says})");
}

/// The two files of `common::ACROSS` that are one schema, generated as `gen` made them before it
/// stopped and loaded into one database: each function that had an answer of its own gets the later
/// file's. With the alias the note gives, each answers for itself.
fn across_sql(cluster: &Postgres) {
    for (k, (first, later, named)) in common::ACROSS.iter().filter(|(_, _, n)| n.contains(&Target::Sql)).enumerate() {
        assert_eq!(*named, [Target::Sql]);
        for (j, (l, keeps)) in [(later.to_string(), false), (common::renamed_across(later), true)].into_iter().enumerate() {
            let d = TempDir::new("sql-across");
            let ss = [subject(first), subject(&l)];
            generate(d.path(), Target::Sql, &ss, Lang::En);
            let [a, b] = &ss;
            let db = format!("across{k}_{j}");
            ok_or_fail(&format!("CREATE DATABASE {db}"), &run(cluster.psql("postgres").args(["-c", &format!("CREATE DATABASE {db}")]), LIMIT));
            for s in [a, b] {
                ok_or_fail(&format!("psql -f {}.sql", s.alias), &run(cluster.psql(&db).arg("-f").arg(d.path().join(format!("sql/{}.sql", s.alias))), LIMIT));
            }
            let asked = run(cluster.psql(&db).args(["-At", "-c", &format!("SELECT {}.due(DATE '2026-01-01'), {}.due(DATE '2026-01-01')", a.alias, b.alias)]), LIMIT);
            ok_or_fail("SELECT", &asked);
            let want = if keeps { "2026-01-04|2026-01-06" } else { "2026-01-06|2026-01-06" };
            assert!(asked.stdout.lines().any(|x| x == want), "{first} and {l}: {want} was to be the answer, and PostgreSQL said:\n{}", asked.stdout);
            println!("sql: {first} and {l} in one database answer {want}");
        }
    }
}

/// Each file of tests/spellings whose W001 names PostgreSQL, generated alone and loaded into a
/// database of its own: PostgreSQL stops where `common::SPELLING_FILES` says, or, when it says
/// nothing, the file loads and its runner disagrees with the reference interpreter.
fn breaks_sql(cluster: &Postgres) {
    let bs = broken(Target::Sql);
    for (k, (s, says)) in bs.iter().enumerate() {
        let d = TempDir::new("sql-breaks");
        generate(d.path(), Target::Sql, std::slice::from_ref(s), Lang::En);
        let db = format!("breaks{k}");
        ok_or_fail(&format!("CREATE DATABASE {db}"), &run(cluster.psql("postgres").args(["-c", &format!("CREATE DATABASE {db}")]), LIMIT));
        let r = run(cluster.psql(&db).arg("-f").arg(d.path().join(format!("sql/{}.sql", s.alias))), LIMIT);
        if says.is_empty() {
            ok_or_fail(&format!("psql -f {}.sql", s.alias), &r);
            let mut c = cluster.psql(&db);
            c.arg("-f").arg(d.path().join(format!("sql/{}_runner.sql", s.alias)));
            let e = compare(s, c).expect_err(&format!("{}: the runner was to disagree with the reference interpreter", s.path));
            let head = e.lines().next().unwrap_or("");
            assert!(head.contains("(exit ok: true)") && !head.contains(": 0 lines differ"), "{}: the runner was to answer wrongly, not fail:\n{e}", s.path);
            println!("sql: {} loads, and its runner disagrees ({head})", s.path);
        } else {
            let said = format!("{}{}", r.stdout, r.stderr);
            assert!(!r.ok && said.contains(says), "{}: PostgreSQL was to stop with {says:?}, and said:\n{said}", s.path);
            println!("sql: {} stops PostgreSQL ({says})", s.path);
        }
    }
    assert!(bs.len() >= 8, "{} files of tests/spellings name PostgreSQL", bs.len());
}

#[test]
fn wasm() {
    if !need(Need::Rustc) || !need(Need::Node) {
        return;
    }
    if !runs("rustc", &["--version"]) || !common::rust_target("wasm32-unknown-unknown") {
        skip("rustc or its wasm32-unknown-unknown target is missing (rustup target add wasm32-unknown-unknown); the Wasm target is not run");
        return;
    }
    if !runs("node", &["--version"]) {
        skip("node is not on the PATH; the Wasm target is not run");
        return;
    }
    let ss = subjects(Target::Wasm);
    let (dir, ja) = (TempDir::new("wasm"), TempDir::new("wasm-ja"));
    println!("wasm: {}, node {}", version("rustc", "--version"), version("node", "--version"));
    for (d, lang) in [(&dir, Lang::En), (&ja, Lang::Ja)] {
        generate(d.path(), Target::Wasm, &ss, lang);
        let w = d.path().join("wasm");
        // Every crate root checked on the 2024 edition, and built by the line koyomi api gives
        // (the 2021 edition), both with -D warnings; the Japanese modules are built only for the
        // files they are run on.
        let queue: Mutex<Vec<&Subject>> = Mutex::new(ss.iter().collect());
        std::thread::scope(|sc| {
            for _ in 0..AT_ONCE {
                sc.spawn(|| {
                    loop {
                        let Some(s) = queue.lock().unwrap().pop() else { break };
                        let root = format!("{}_wasm.rs", s.alias);
                        let mut c = Command::new("rustc");
                        c.current_dir(&w).args(["--edition", "2024", "-D", "warnings", "--target", "wasm32-unknown-unknown", "--crate-type", "cdylib", "--emit=metadata", "--out-dir", "meta"]).arg(&root);
                        ok_or_fail(&format!("rustc --edition 2024 {root}"), &run(&mut c, LIMIT));
                        if lang == Lang::Ja && !JA_FILES.contains(&s.path.as_str()) {
                            continue;
                        }
                        let line = wasm::build_line(&s.alias);
                        let words: Vec<&str> = line.split(' ').collect();
                        let mut c = Command::new(words[0]);
                        c.current_dir(&w).args(["-D", "warnings"]).args(&words[1..]);
                        ok_or_fail(&format!("{line} (-D warnings)"), &run(&mut c, LIMIT));
                    }
                });
            }
        });
    }
    println!("wasm: every crate root builds as koyomi api says and checks on the 2024 edition, with -D warnings, in English and in Japanese");
    let runner = |d: &TempDir, s: &Subject| {
        let mut c = Command::new("node");
        c.arg(d.path().join(format!("wasm/{}_runner.mjs", s.alias)));
        c
    };
    compare_all("wasm", &ss, |s| runner(&dir, s));
    compare_ja("wasm", &ss, |s| runner(&ja, s));
    breaks_wasm();
}

/// Each file of tests/spellings whose W001 names Wasm, generated alone: checking its crate root on
/// the 2024 edition with `-D warnings` stops where `common::SPELLING_FILES` says.
fn breaks_wasm() {
    let bs = broken(Target::Wasm);
    for (s, says) in &bs {
        if *says == common::PROTO_MISSING {
            // The code builds: what it answers a host is tests/wasm.rs's.
            continue;
        }
        let d = TempDir::new("wasm-breaks");
        generate(d.path(), Target::Wasm, std::slice::from_ref(s), Lang::En);
        let mut c = Command::new("rustc");
        c.current_dir(d.path().join("wasm"))
            .args(["--edition", "2024", "-D", "warnings", "--target", "wasm32-unknown-unknown", "--crate-type", "cdylib", "--emit=metadata", "--out-dir", "meta"])
            .arg(format!("{}_wasm.rs", s.alias));
        let r = run(&mut c, LIMIT);
        let said = format!("{}{}", r.stdout, r.stderr);
        assert!(!r.ok && said.contains(says), "{}: rustc -D warnings was to stop with {says:?}, and said:\n{said}", s.path);
        println!("wasm: {} stops rustc -D warnings ({says})", s.path);
    }
    assert!(bs.len() >= 6, "{} files of tests/spellings name Wasm", bs.len());
}
