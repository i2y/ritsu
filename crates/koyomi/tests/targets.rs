//! The generated code, held to the reference interpreter (PLAN C.8, DESIGN 6.4). For each of
//! the five targets, every example that passes check and every `tests/fixtures/helpers_*`
//! file is generated, the target's own tools look at it (tsc, mypy, gofmt and go vet, rustc on
//! two editions, PostgreSQL), and its runner is given every line of `koyomi vectors`. Every
//! line it prints is compared with what the reference interpreter gives.
//!
//! The vectors are made in this process and piped to the runner as they are made; nothing as
//! large as them is written to disk. A tool that is missing prints `SKIP:` and the test passes.
//! `-- --nocapture` shows a `compared <target> <file>: <n> lines` line for every comparison.

mod common;

use common::{Cluster, TempDir, have, on_path, pg_bin, pipe, run, tool, version};
use koyomi::check::{Checked, check};
use koyomi::codegen;
use koyomi::i18n::Lang;
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
    path: &'static str,
    checked: Checked,
    alias: String,
}

fn subjects() -> Vec<Subject> {
    FILES
        .iter()
        .map(|p| {
            let mut o = check(p).unwrap();
            assert!(!o.has_errors(), "{p} passes check");
            let checked = o.checked.take().unwrap();
            let alias = codegen::unit_of(&checked, Lang::En).alias;
            Subject { path: p, checked, alias }
        })
        .collect()
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

/// The files the `--lang ja` code is run on as well: one with the rows outside the range
/// (`range`), one with the days outside the data range (`data`). The Japanese code differs from
/// the English only in its comments and in the messages of its errors.
const JA_FILES: &[&str] = &["examples/支払_20日締め翌月10日払い.cal", "examples/calendars/東京の営業日.cal"];

/// Run the `--lang ja` runners of `JA_FILES` and fail if one disagrees.
fn compare_ja(target: &str, ss: &[Subject], job: impl Fn(&Subject) -> Command + Sync) {
    for s in ss.iter().filter(|s| JA_FILES.contains(&s.path)) {
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
fn ok_or_fail(what: &str, r: &common::Ran) {
    assert!(r.ok && !r.timed_out, "{what} failed:\n{}{}", r.stdout, r.stderr);
}

#[test]
fn typescript() {
    if !have("node") {
        println!("SKIP: node is not on the PATH; the TypeScript target is not run");
        return;
    }
    let ss = subjects();
    let (dir, ja) = (TempDir::new("ts"), TempDir::new("ts-ja"));
    for (d, lang) in [(&dir, Lang::En), (&ja, Lang::Ja)] {
        generate(d.path(), Target::TypeScript, &ss, lang);
        // Node strips the types of a .ts file inside a package of ES modules.
        std::fs::write(d.path().join("typescript/package.json"), "{\"type\": \"module\"}\n").unwrap();
    }
    println!("typescript: {}", version("node", "--version"));
    match tool("KOYOMI_TSC", "tools/node_modules/.bin/tsc", "tsc") {
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
        None => println!("SKIP: tsc is not installed (npm ci --prefix tools); the TypeScript is run but not type-checked"),
    }
    let runner = |d: &TempDir, s: &Subject| {
        let mut c = Command::new("node");
        c.arg("--disable-warning=ExperimentalWarning").arg(d.path().join(format!("typescript/{}_runner.ts", s.alias)));
        c
    };
    compare_all("typescript", &ss, |s| runner(&dir, s));
    compare_ja("typescript", &ss, |s| runner(&ja, s));
}

#[test]
fn python() {
    if !have("python3") {
        println!("SKIP: python3 is not on the PATH; the Python target is not run");
        return;
    }
    let ss = subjects();
    let (dir, ja) = (TempDir::new("py"), TempDir::new("py-ja"));
    generate(dir.path(), Target::Python, &ss, Lang::En);
    generate(ja.path(), Target::Python, &ss, Lang::Ja);
    println!("python: {}", version("python3", "--version"));
    match tool("KOYOMI_MYPY", "tools/.venv/bin/mypy", "mypy") {
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
        None => println!("SKIP: mypy is not installed (tools/requirements.txt); the Python is run but not type-checked"),
    }
    let runner = |d: &TempDir, s: &Subject| {
        let mut c = Command::new("python3");
        c.arg(d.path().join(format!("python/{}_runner.py", s.alias)));
        c
    };
    compare_all("python", &ss, |s| runner(&dir, s));
    compare_ja("python", &ss, |s| runner(&ja, s));
}

#[test]
fn go() {
    if !on_path("go") || !on_path("gofmt") {
        println!("SKIP: go or gofmt is not on the PATH; the Go target is not run");
        return;
    }
    let ss = subjects();
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
}

#[test]
fn rust() {
    if !have("rustc") {
        println!("SKIP: rustc is not on the PATH; the Rust target is not run");
        return;
    }
    let ss = subjects();
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
                    if lang == Lang::Ja && !JA_FILES.contains(&s.path) {
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
}

#[test]
fn sql() {
    let Some(bin) = pg_bin() else {
        println!("SKIP: initdb is not found (KOYOMI_PG_BIN or the PATH); the SQL target is not run");
        return;
    };
    let ss = subjects();
    let (dir, ja) = (TempDir::new("sql"), TempDir::new("sql-ja"));
    generate(dir.path(), Target::Sql, &ss, Lang::En);
    generate(ja.path(), Target::Sql, &ss, Lang::Ja);
    let cluster = match Cluster::start(&bin) {
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
}
