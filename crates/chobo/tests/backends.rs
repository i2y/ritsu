//! Every scenario of every book, on every target, against the reference interpreter (PLAN C5):
//! the SQL itself through psql, the PostgreSQL clients in TypeScript, Python and Go, the
//! TigerBeetle clients in the three, and the clients of the MySQL family in the three, on MySQL
//! and on TiDB (DESIGN 4.5). Each answer, the balances at the end and the holds must be
//! the reference's (one of its ways to come out, with `together`), and what each client sends
//! must be what `chobo run --show` says, field by field.
//!
//! One throwaway PostgreSQL cluster and one TigerBeetle replica serve the whole test. The
//! clients are built from a copy of each book with every expiry at 3 seconds, and `pass` is
//! waited for, for real. The reference lets no time pass but at a `pass`, so a scenario whose step
//! or read came after the expiry of a hold it made since its last `pass` (`late`, which the runner
//! measures) is not held to it on that run: when it disagrees, it runs again, on tenants of its
//! own, with only the others that were late beside it, and late and wrong twice, it fails.

mod common;
use common::runners::*;
use common::servers::{My, MySql, Pg, Postgres, Tb, TigerBeetle};
use common::*;
use serde_json::Value;
use std::process::Command;
use std::time::Instant;

/// Each book's scenarios a run has, by their place in the book's: every one, or those named.
type Only<'a> = Option<&'a [Vec<usize>]>;

/// Compare the runner's answers for the scenarios `only` names (every one when None) with the
/// reference, and what it sent with the show; one `compared:` line a book, and every disagreement
/// in `failures`. A late scenario that disagrees goes in `again` instead, when there is one.
fn check(cases: &[Case], combo: &str, backend: &str, out: &Value, sent: bool, only: Only, mut again: Option<&mut Vec<Vec<usize>>>, failures: &mut Vec<String>) {
    let books = out["books"].as_array().cloned().unwrap_or_default();
    for (k, (c, b)) in cases.iter().zip(&books).enumerate() {
        let all: Vec<usize> = (0..c.scenarios.len()).collect();
        let which: &[usize] = only.map(|o| o[k].as_slice()).unwrap_or(&all);
        let scenarios = b["scenarios"].as_array().cloned().unwrap_or_default();
        let mut n = 0;
        for (&i, got) in which.iter().zip(&scenarios) {
            let name = &c.scenarios[i].name;
            if let Some(e) = got["error"].as_str() {
                failures.push(format!("{} {combo} ({name}): {e}", c.stem));
                continue;
            }
            if !agrees(&c.reference[i], &got["result"]) {
                let late = got["late"].as_bool().unwrap_or(false);
                if late && let Some(a) = again.as_deref_mut() {
                    a[k].push(i);
                    continue;
                }
                let when = if late { ", late again: a step or a read came after the expiry of a hold it made" } else { "" };
                failures.push(format!(
                    "{} {combo} ({name}): the result is not the reference's{when}\n  got       {}\n  reference {}",
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
        if scenarios.len() != which.len() {
            failures.push(format!("{} {combo}: {} scenarios came back of {}", c.stem, scenarios.len(), which.len()));
        }
        if only.is_none() {
            eprintln!("compared: {} {combo} {n} scenarios", c.stem);
        } else if !which.is_empty() {
            eprintln!("compared again: {} {combo} {n} scenario(s)", c.stem);
        }
    }
}

/// Run a combo and hold it to the reference; then, again, the scenarios that were late and
/// disagreed. `run` runs the scenarios `only` names on the tenants of the combo it is given.
fn run_and_check(cases: &[Case], combo: &str, backend: &str, sent: bool, failures: &mut Vec<String>, run: &dyn Fn(Only, &str) -> Result<Value, String>) {
    let out = match run(None, combo) {
        Ok(out) => out,
        Err(e) => return failures.push(e),
    };
    let mut again: Vec<Vec<usize>> = vec![Vec::new(); cases.len()];
    check(cases, combo, backend, &out, sent, None, Some(&mut again), failures);
    let late: Vec<String> = cases.iter().zip(&again).flat_map(|(c, a)| a.iter().map(move |&i| format!("{} ({})", c.stem, c.scenarios[i].name))).collect();
    if late.is_empty() {
        return;
    }
    eprintln!("run again: {combo}: {} scenario(s) disagreed after a hold made since their last pass had expired: {}", late.len(), late.join(", "));
    match run(Some(&again), &format!("{combo}/again")) {
        Ok(out) => check(cases, combo, backend, &out, sent, Some(&again), None, failures),
        Err(e) => failures.push(e),
    }
}

fn input(backend: &str, conn: Value, combo: &str, cases: &[Case], clients: &[String], only: Only) -> Value {
    let books: Vec<Value> = cases.iter().zip(clients).enumerate().map(|(k, (c, client))| book_input_of(c, client, combo, only.map(|o| o[k].as_slice()))).collect();
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
    run_and_check(cases, "postgres", "postgres", false, failures, &|only, combo| Ok(run_sql_of(pg, cases, combo, only)));
    eprintln!("ran postgres in {:.1} s", started.elapsed().as_secs_f64());

    match node() {
        Ok(()) => {
            let dir = ts_dir(work).join("postgres");
            let clients: Vec<String> = build_all(cases, chobo::target::Target::PostgresTypeScript, &dir).iter().map(|p| p.display().to_string()).collect();
            let run = |only: Only, combo: &str| {
                let mut cmd = Command::new("node");
                cmd.arg("--no-warnings").arg(runner_dir().join("runner.ts"));
                run_runner(cmd, &input("postgres", pg.connection(), combo, cases, &clients, only), work, combo)
            };
            let started = Instant::now();
            run_and_check(cases, "postgres-typescript", "postgres", true, failures, &run);
            eprintln!("ran postgres-typescript in {:.1} s", started.elapsed().as_secs_f64());
        }
        Err(why) => skip(&format!("{why}; postgres-typescript is not run")),
    }
    match python() {
        Ok(py) => {
            let dir = work.join("py/postgres");
            let clients: Vec<String> = build_all(cases, chobo::target::Target::PostgresPython, &dir).iter().map(|p| p.display().to_string()).collect();
            let run = |only: Only, combo: &str| {
                let mut cmd = Command::new(&py);
                cmd.arg(runner_dir().join("runner.py"));
                run_runner(cmd, &input("postgres", pg.connection(), combo, cases, &clients, only), work, combo)
            };
            let started = Instant::now();
            run_and_check(cases, "postgres-python", "postgres", true, failures, &run);
            eprintln!("ran postgres-python in {:.1} s", started.elapsed().as_secs_f64());
        }
        Err(why) => skip(&format!("{why}; postgres-python is not run")),
    }
    match go {
        Some(g) => {
            let clients: Vec<String> = cases.iter().map(|c| g.keys[&(c.stem.clone(), "postgres")].clone()).collect();
            let run = |only: Only, combo: &str| run_runner(Command::new(&g.bin), &input("postgres", pg.connection(), combo, cases, &clients, only), work, combo);
            let started = Instant::now();
            run_and_check(cases, "postgres-go", "postgres", true, failures, &run);
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
            let run = |only: Only, combo: &str| {
                let mut cmd = Command::new("node");
                cmd.arg("--no-warnings").arg(runner_dir().join("runner.ts"));
                run_runner(cmd, &input("tigerbeetle", tb.connection(), combo, cases, &clients, only), work, combo)
            };
            let started = Instant::now();
            run_and_check(cases, "tigerbeetle-typescript", "tigerbeetle", true, failures, &run);
            eprintln!("ran tigerbeetle-typescript in {:.1} s", started.elapsed().as_secs_f64());
        }
        Err(why) => skip(&format!("{why}; tigerbeetle-typescript is not run")),
    }
    match python() {
        Ok(py) => {
            let dir = work.join("py/tigerbeetle");
            let clients: Vec<String> = build_all(cases, chobo::target::Target::TigerBeetlePython, &dir).iter().map(|p| p.display().to_string()).collect();
            let run = |only: Only, combo: &str| {
                let mut cmd = Command::new(&py);
                cmd.arg(runner_dir().join("runner.py"));
                run_runner(cmd, &input("tigerbeetle", tb.connection(), combo, cases, &clients, only), work, combo)
            };
            let started = Instant::now();
            run_and_check(cases, "tigerbeetle-python", "tigerbeetle", true, failures, &run);
            eprintln!("ran tigerbeetle-python in {:.1} s", started.elapsed().as_secs_f64());
        }
        Err(why) => skip(&format!("{why}; tigerbeetle-python is not run")),
    }
    match go {
        Some(g) => {
            let clients: Vec<String> = cases.iter().map(|c| g.keys[&(c.stem.clone(), "tigerbeetle")].clone()).collect();
            let run = |only: Only, combo: &str| run_runner(Command::new(&g.bin), &input("tigerbeetle", tb.connection(), combo, cases, &clients, only), work, combo);
            let started = Instant::now();
            run_and_check(cases, "tigerbeetle-go", "tigerbeetle", true, failures, &run);
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

/// The MySQL family's side, on one server: the tables of every book in a database of its own, then
/// each of the three clients. `slug` names the server in the combos (`mysql`, `tidb`).
fn mysql_family(my: &MySql, slug: &str, cases: &[Case], work: &std::path::Path, go: Option<&GoRunner>, failures: &mut Vec<String>) {
    let db = match my.database("backends") {
        Ok(db) => db,
        Err(e) => return failures.push(format!("{}: no database: {e}", my.label)),
    };
    eprintln!("the MySQL family on {}", my.label);
    for (c, f) in cases.iter().zip(build_all(cases, chobo::target::Target::MySql, &work.join(format!("{slug}-sql")))) {
        if let Err(e) = my.load(&db, &f) {
            return failures.push(format!("{} on {}: the tables do not load:\n{e}", c.stem, my.label));
        }
    }
    let conn = my.connection(&db, false);
    match node_mysql() {
        Ok(()) => {
            let dir = ts_dir(work).join(slug);
            let clients: Vec<String> = build_all(cases, chobo::target::Target::MySqlTypeScript, &dir).iter().map(|p| p.display().to_string()).collect();
            let run = |only: Only, combo: &str| {
                let mut cmd = Command::new("node");
                cmd.arg("--no-warnings").arg(runner_dir().join("runner.ts"));
                run_runner(cmd, &input("mysql", conn.clone(), combo, cases, &clients, only), work, combo)
            };
            let combo = format!("{slug}-typescript");
            let started = Instant::now();
            run_and_check(cases, &combo, "mysql", true, failures, &run);
            eprintln!("ran {combo} in {:.1} s", started.elapsed().as_secs_f64());
        }
        Err(why) => skip(&format!("{why}; {slug}-typescript is not run")),
    }
    match python_mysql() {
        Ok(py) => {
            let dir = work.join(format!("py/{slug}"));
            let clients: Vec<String> = build_all(cases, chobo::target::Target::MySqlPython, &dir).iter().map(|p| p.display().to_string()).collect();
            let run = |only: Only, combo: &str| {
                let mut cmd = Command::new(&py);
                cmd.arg(runner_dir().join("runner.py"));
                run_runner(cmd, &input("mysql", conn.clone(), combo, cases, &clients, only), work, combo)
            };
            let combo = format!("{slug}-python");
            let started = Instant::now();
            run_and_check(cases, &combo, "mysql", true, failures, &run);
            eprintln!("ran {combo} in {:.1} s", started.elapsed().as_secs_f64());
        }
        Err(why) => skip(&format!("{why}; {slug}-python is not run")),
    }
    match go {
        Some(g) => {
            let clients: Vec<String> = cases.iter().map(|c| g.keys[&(c.stem.clone(), "mysql")].clone()).collect();
            let run = |only: Only, combo: &str| run_runner(Command::new(&g.bin), &input("mysql", conn.clone(), combo, cases, &clients, only), work, combo);
            let combo = format!("{slug}-go");
            let started = Instant::now();
            run_and_check(cases, &combo, "mysql", true, failures, &run);
            eprintln!("ran {combo} in {:.1} s", started.elapsed().as_secs_f64());
        }
        None => skip(&format!("the Go runner is not built; {slug}-go is not run")),
    }
    // the same, a call's statements in a row in one request: what is sent is not the show's (a
    // request names what it would have run after an error), so only the answers are compared
    let batched = my.connection(&db, true);
    if let Ok(()) = node_mysql() {
        let dir = ts_dir(work).join(slug);
        let clients: Vec<String> = cases.iter().map(|c| dir.join(format!("{}.ts", c.copy.name)).display().to_string()).collect();
        let run = |only: Only, combo: &str| {
            let mut cmd = Command::new("node");
            cmd.arg("--no-warnings").arg(runner_dir().join("runner.ts"));
            run_runner(cmd, &input("mysql", batched.clone(), combo, cases, &clients, only), work, combo)
        };
        run_and_check(cases, &format!("{slug}-typescript-batch"), "mysql", false, failures, &run);
    }
    if let Ok(py) = python_mysql() {
        let dir = work.join(format!("py/{slug}"));
        let clients: Vec<String> = cases.iter().map(|c| dir.join(format!("{}.py", c.copy.name)).display().to_string()).collect();
        let run = |only: Only, combo: &str| {
            let mut cmd = Command::new(&py);
            cmd.arg(runner_dir().join("runner.py"));
            run_runner(cmd, &input("mysql", batched.clone(), combo, cases, &clients, only), work, combo)
        };
        run_and_check(cases, &format!("{slug}-python-batch"), "mysql", false, failures, &run);
    }
    if let Some(g) = go {
        let clients: Vec<String> = cases.iter().map(|c| g.keys[&(c.stem.clone(), "mysql")].clone()).collect();
        let run = |only: Only, combo: &str| run_runner(Command::new(&g.bin), &input("mysql", batched.clone(), combo, cases, &clients, only), work, combo);
        run_and_check(cases, &format!("{slug}-go-batch"), "mysql", false, failures, &run);
    }
    // every balance is what its entries add up to: an account's row holds the entries that are
    // folded into it, and the others are still to be added
    for c in cases {
        let (a, e) = (chobo::mysql::table(&c.copy, "accounts"), chobo::mysql::table(&c.copy, "entries"));
        let off = my.sql(
            &db,
            &format!(
                "select count(*) from {a} `a` left join (select `tenant`, `account`, sum(`d_posted`) `p`, sum(`d_held_in`) `i`, sum(`d_held_out`) `o` from {e} where `folded` = 1 group by `tenant`, `account`) `e` on `e`.`tenant` = `a`.`tenant` and `e`.`account` = `a`.`id` where `a`.`posted` <> coalesce(`e`.`p`, 0) or `a`.`held_in` <> coalesce(`e`.`i`, 0) or `a`.`held_out` <> coalesce(`e`.`o`, 0)"
            ),
        );
        match off {
            Ok(n) if n.trim() == "0" => {}
            Ok(n) => failures.push(format!("{} on {}: {} account(s) whose row is not what its folded entries add up to", c.stem, my.label, n.trim())),
            Err(e) => failures.push(format!("{} on {}: {e}", c.stem, my.label)),
        }
    }
}

#[test]
fn every_scenario_matches_on_the_mysql_family() {
    let started = Instant::now();
    let cases = cases();
    let work = TempDir::new("backends-mysql");
    let go = match go() {
        Ok(()) => Some(build_go(&cases, work.path()).unwrap_or_else(|e| panic!("{e}"))),
        Err(why) => {
            skip(&format!("{why}; the Go clients are not run"));
            None
        }
    };
    let mut failures: Vec<String> = Vec::new();
    let mut logs = Vec::new();
    if need(Need::MySql) {
        match MySql::start() {
            Ok(my) => {
                mysql_family(&my, "mysql", &cases, work.path(), go.as_ref(), &mut failures);
                logs.push(my.log());
            }
            Err(why) => skip(&format!("{why}; no client of the MySQL family is run on MySQL")),
        }
    }
    if need(Need::TiDb) {
        match MySql::at("TIDB", "TiDB") {
            Ok(my) => mysql_family(&my, "tidb", &cases, work.path(), go.as_ref(), &mut failures),
            Err(why) => skip(&format!("{why}; no client of the MySQL family is run on TiDB")),
        }
    }
    // Vitess with its keyspace in one shard, and in two by the tenant, where a transaction that
    // crosses shards fails
    if need(Need::Vitess) {
        for (shards, slug) in [(1, "vitess"), (2, "vitess2")] {
            match MySql::vitess(shards) {
                Ok(my) => {
                    mysql_family(&my, slug, &cases, work.path(), go.as_ref(), &mut failures);
                    logs.push(my.log());
                }
                Err(why) => skip(&format!("{why}; no client of the MySQL family is run on Vitess")),
            }
        }
    }
    eprintln!("the MySQL family took {:.1} s", started.elapsed().as_secs_f64());
    if !failures.is_empty() {
        for log in &logs {
            let errors: Vec<&str> = log.lines().filter(|l| l.contains("[ERROR]") || l.contains("\"level\":\"ERROR\"")).take(20).collect();
            if !errors.is_empty() {
                eprintln!("MySQL's log, its errors:\n{}", errors.join("\n"));
            }
        }
        let shown: Vec<String> = failures.iter().take(15).cloned().collect();
        panic!("{} disagreement(s):\n{}", failures.len(), shown.join("\n"));
    }
}
