//! Every scenario of every book, on every target, against the reference interpreter (PLAN C5):
//! the SQL itself through psql, the PostgreSQL clients in TypeScript, Python and Go, and the
//! TigerBeetle clients in the three. Each answer, the balances at the end and the holds must be
//! the reference's (one of its ways to come out, with `together`), and what each client sends
//! must be what `chobo run --show` says, field by field.
//!
//! One throwaway PostgreSQL cluster and one TigerBeetle replica serve the whole test. The
//! clients are built from a copy of each book with every expiry at 3 seconds, and `pass` is
//! waited for, for real.

mod common;
use common::runners::*;
use common::servers::{Pg, Postgres, Tb, TigerBeetle};
use common::*;
use serde_json::Value;
use std::process::Command;
use std::time::Instant;

/// Compare the runner's answers for every book with the reference, and what it sent with the
/// show; one `compared:` line a book, and every disagreement in `failures`.
fn check(cases: &[Case], combo: &str, backend: &str, out: &Value, sent: bool, failures: &mut Vec<String>) {
    let books = out["books"].as_array().cloned().unwrap_or_default();
    for (c, b) in cases.iter().zip(&books) {
        let scenarios = b["scenarios"].as_array().cloned().unwrap_or_default();
        let mut n = 0;
        for (i, got) in scenarios.iter().enumerate() {
            let name = &c.scenarios[i].name;
            if let Some(e) = got["error"].as_str() {
                failures.push(format!("{} {combo} ({name}): {e}", c.stem));
                continue;
            }
            if !agrees(&c.reference[i], &got["result"]) {
                failures.push(format!(
                    "{} {combo} ({name}): the result is not the reference's\n  got       {}\n  reference {}",
                    c.stem, got["result"], c.reference[i]
                ));
                continue;
            }
            if sent {
                if let Err(e) = compare_sent(c, backend, i, got["tenant"].as_str().unwrap_or(""), &got["sent"]) {
                    failures.push(format!("{} {combo} ({name}): {e}", c.stem));
                    continue;
                }
            }
            n += 1;
        }
        if scenarios.len() != c.scenarios.len() {
            failures.push(format!("{} {combo}: {} scenarios came back of {}", c.stem, scenarios.len(), c.scenarios.len()));
        }
        eprintln!("compared: {} {combo} {n} scenarios", c.stem);
    }
}

fn input(backend: &str, conn: Value, combo: &str, cases: &[Case], clients: &[String]) -> Value {
    let books: Vec<Value> = cases.iter().zip(clients).map(|(c, client)| book_input(c, client, combo)).collect();
    let mut v = serde_json::json!({"backend": backend, "books": books});
    v[backend] = conn;
    v
}

/// The PostgreSQL side: the SQL of every book in the cluster, then each of the four ways to call it.
fn postgres(pg: &Postgres, cases: &[Case], work: &std::path::Path, go: Option<&GoRunner>, failures: &mut Vec<String>) {
    let sql = work.join("sql");
    for (c, f) in cases.iter().zip(build_all(cases, chobo::target::Target::Postgres, &sql)) {
        let out = pg.db().arg("-f").arg(&f).output().unwrap();
        assert!(out.status.success(), "{}: the SQL does not load:\n{}", c.stem, String::from_utf8_lossy(&out.stderr));
    }
    let started = Instant::now();
    let out = run_sql(pg, cases, "postgres");
    check(cases, "postgres", "postgres", &out, false, failures);
    eprintln!("ran postgres in {:.1} s", started.elapsed().as_secs_f64());

    match node() {
        Ok(()) => {
            let dir = ts_dir(work).join("postgres");
            let clients: Vec<String> = build_all(cases, chobo::target::Target::PostgresTypeScript, &dir).iter().map(|p| p.display().to_string()).collect();
            let mut cmd = Command::new("node");
            cmd.arg("--no-warnings").arg(runner_dir().join("runner.ts"));
            let started = Instant::now();
            match run_runner(cmd, &input("postgres", pg.connection(), "postgres-typescript", cases, &clients), work, "postgres-typescript") {
                Ok(out) => check(cases, "postgres-typescript", "postgres", &out, true, failures),
                Err(e) => failures.push(e),
            }
            eprintln!("ran postgres-typescript in {:.1} s", started.elapsed().as_secs_f64());
        }
        Err(why) => skip(&format!("{why}; postgres-typescript is not run")),
    }
    match python() {
        Ok(py) => {
            let dir = work.join("py/postgres");
            let clients: Vec<String> = build_all(cases, chobo::target::Target::PostgresPython, &dir).iter().map(|p| p.display().to_string()).collect();
            let mut cmd = Command::new(py);
            cmd.arg(runner_dir().join("runner.py"));
            let started = Instant::now();
            match run_runner(cmd, &input("postgres", pg.connection(), "postgres-python", cases, &clients), work, "postgres-python") {
                Ok(out) => check(cases, "postgres-python", "postgres", &out, true, failures),
                Err(e) => failures.push(e),
            }
            eprintln!("ran postgres-python in {:.1} s", started.elapsed().as_secs_f64());
        }
        Err(why) => skip(&format!("{why}; postgres-python is not run")),
    }
    match go {
        Some(g) => {
            let clients: Vec<String> = cases.iter().map(|c| g.keys[&(c.stem.clone(), "postgres")].clone()).collect();
            let started = Instant::now();
            match run_runner(Command::new(&g.bin), &input("postgres", pg.connection(), "postgres-go", cases, &clients), work, "postgres-go") {
                Ok(out) => check(cases, "postgres-go", "postgres", &out, true, failures),
                Err(e) => failures.push(e),
            }
            eprintln!("ran postgres-go in {:.1} s", started.elapsed().as_secs_f64());
        }
        None => skip("the Go runner is not built; postgres-go is not run"),
    }
    // every balance is what its entries add up to
    for c in cases {
        let s = chobo::postgres::ident(&c.copy.name);
        let off = pg.exec(&format!(
            "select count(*) from {s}.accounts a left join (select tenant, account, sum(d_posted) p, sum(d_held_in) i, sum(d_held_out) o from {s}.entries group by tenant, account) e on e.tenant = a.tenant and e.account = a.id where a.posted <> coalesce(e.p, 0) or a.held_in <> coalesce(e.i, 0) or a.held_out <> coalesce(e.o, 0)"
        ));
        if off.trim() != "0" {
            failures.push(format!("{}: {} account(s) whose balance is not what its entries add up to", c.stem, off.trim()));
        }
    }
}

fn tigerbeetle(tb: &TigerBeetle, cases: &[Case], work: &std::path::Path, go: Option<&GoRunner>, failures: &mut Vec<String>) {
    match node() {
        Ok(()) => {
            let dir = ts_dir(work).join("tigerbeetle");
            let clients: Vec<String> = build_all(cases, chobo::target::Target::TigerBeetleTypeScript, &dir).iter().map(|p| p.display().to_string()).collect();
            let mut cmd = Command::new("node");
            cmd.arg("--no-warnings").arg(runner_dir().join("runner.ts"));
            let started = Instant::now();
            match run_runner(cmd, &input("tigerbeetle", tb.connection(), "tigerbeetle-typescript", cases, &clients), work, "tigerbeetle-typescript") {
                Ok(out) => check(cases, "tigerbeetle-typescript", "tigerbeetle", &out, true, failures),
                Err(e) => failures.push(e),
            }
            eprintln!("ran tigerbeetle-typescript in {:.1} s", started.elapsed().as_secs_f64());
        }
        Err(why) => skip(&format!("{why}; tigerbeetle-typescript is not run")),
    }
    match python() {
        Ok(py) => {
            let dir = work.join("py/tigerbeetle");
            let clients: Vec<String> = build_all(cases, chobo::target::Target::TigerBeetlePython, &dir).iter().map(|p| p.display().to_string()).collect();
            let mut cmd = Command::new(py);
            cmd.arg(runner_dir().join("runner.py"));
            let started = Instant::now();
            match run_runner(cmd, &input("tigerbeetle", tb.connection(), "tigerbeetle-python", cases, &clients), work, "tigerbeetle-python") {
                Ok(out) => check(cases, "tigerbeetle-python", "tigerbeetle", &out, true, failures),
                Err(e) => failures.push(e),
            }
            eprintln!("ran tigerbeetle-python in {:.1} s", started.elapsed().as_secs_f64());
        }
        Err(why) => skip(&format!("{why}; tigerbeetle-python is not run")),
    }
    match go {
        Some(g) => {
            let clients: Vec<String> = cases.iter().map(|c| g.keys[&(c.stem.clone(), "tigerbeetle")].clone()).collect();
            let started = Instant::now();
            match run_runner(Command::new(&g.bin), &input("tigerbeetle", tb.connection(), "tigerbeetle-go", cases, &clients), work, "tigerbeetle-go") {
                Ok(out) => check(cases, "tigerbeetle-go", "tigerbeetle", &out, true, failures),
                Err(e) => failures.push(e),
            }
            eprintln!("ran tigerbeetle-go in {:.1} s", started.elapsed().as_secs_f64());
        }
        None => skip("the Go runner is not built; tigerbeetle-go is not run"),
    }
}

#[test]
fn every_scenario_matches_on_every_target() {
    if !need(Need::Postgres) || !need(Need::TigerBeetle) {
        return;
    }
    let started = Instant::now();
    let cases = cases();
    let work = TempDir::new("backends");
    let go = match go() {
        Ok(()) => Some(build_go(&cases, work.path()).unwrap_or_else(|e| panic!("{e}"))),
        Err(why) => {
            skip(&format!("{why}; the Go clients are not run"));
            None
        }
    };
    let pg = Postgres::start();
    let tb = TigerBeetle::start();
    let mut failures: Vec<String> = Vec::new();
    let mut tb_failures: Vec<String> = Vec::new();
    std::thread::scope(|s| {
        s.spawn(|| match &tb {
            Ok(tb) => tigerbeetle(tb, &cases, work.path(), go.as_ref(), &mut tb_failures),
            Err(why) => skip(&format!("{why}; no TigerBeetle target is run")),
        });
        match &pg {
            Ok(pg) => postgres(pg, &cases, work.path(), go.as_ref(), &mut failures),
            Err(why) => skip(&format!("{why}; no PostgreSQL target is run")),
        }
    });
    failures.extend(tb_failures);
    eprintln!("the backends took {:.1} s", started.elapsed().as_secs_f64());
    if !failures.is_empty() {
        if let Ok(pg) = &pg {
            let log = pg.log();
            let errors: Vec<&str> = log.lines().filter(|l| l.contains("ERROR") || l.contains("FATAL")).take(20).collect();
            eprintln!("PostgreSQL's log, its errors:\n{}", errors.join("\n"));
        }
        let shown: Vec<String> = failures.iter().take(15).cloned().collect();
        panic!("{} disagreement(s):\n{}", failures.len(), shown.join("\n"));
    }
}
