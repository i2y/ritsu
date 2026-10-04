//! What only PostgreSQL has (PLAN C6, DESIGN 6): two calls that touch the same two accounts in
//! the opposite order never deadlock, a client under REPEATABLE READ tries a serialization
//! failure again and ends where READ COMMITTED would, a write that does not go through the
//! functions is stopped by the checks, a row made under other bounds is an error, expire()
//! gives back what is past its expiry, chobo_id makes the IDs of PLAN 0.3, and a name too long
//! for PostgreSQL stops the build (E061).
//!
//! One throwaway cluster serves the whole test; it is stopped when the test ends, failing too.

mod common;
use chobo::target::Target;
use common::runners::*;
use common::servers::{Pg, Postgres};
use common::*;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

fn load_books(pg: &Postgres, work: &Path) -> Vec<Case> {
    let cases = cases();
    for (c, f) in cases.iter().zip(build_all(&cases, Target::Postgres, &work.join("sql"))) {
        let out = pg.db().arg("-f").arg(&f).output().unwrap();
        assert!(out.status.success(), "{}: {}", c.stem, String::from_utf8_lossy(&out.stderr));
    }
    cases
}

/// The IDs of PLAN 0.3, as chobo_id makes them in SQL.
fn ids(pg: &Postgres) {
    let cases: &[(&[&str], &str)] = &[
        (&["account", "在庫", "", "在庫", "A-1"], "396213c27529b522708ee6da1d0d03f7"),
        (&["account", "inventory", "", "stock", "A-1"], "f0857e5a2049d01a7f4989e04ee4e37f"),
        (&["account", "inventory", "t-1", "stock", "A-1"], "9a98b9b990f5c3ea4640cd5755b4ce17"),
        (&["account", "inventory", "", "supplier"], "cafb18eaf6485ea880c3ef6350f4cdeb"),
        (&["transfer", "在庫", "", "引当", "hold", "o-1", "A-1", "0"], "594bbb54c47e8b03c55fac1644fbcffa"),
        (&["transfer", "在庫", "", "引当", "post", "o-1", "A-1", "0"], "0a75517949dcb9dec36a9a9d9b04a50c"),
        (&["sink", "在庫", "", "個"], "e132a4987f7c612337996fa93bc06f65"),
        (&["room", "f0857e5a2049d01a7f4989e04ee4e37f"], "2bd360017fee460bbca3ca67451e63dc"),
        (&["opening", "2bd360017fee460bbca3ca67451e63dc"], "c5af9cb7947230b39cfaa34fff20953b"),
        (&["account", "b", "", "a:b", "c"], "464fb5f164fc1b9fd9fc7ea1157c5aaf"),
        (&["account", "b", "", "a", "b:c"], "4f01a8c48b5a8baed7db4f9df67b300b"),
    ];
    for (parts, want) in cases {
        let arr: Vec<String> = parts.iter().map(|p| chobo::postgres::lit(p)).collect();
        let got = pg.exec(&format!("select \"在庫\".chobo_id(array[{}]::text[])", arr.join(", ")));
        assert_eq!(got.trim().replace('-', ""), *want, "{parts:?}");
    }
    eprintln!("chobo_id: {} IDs as PLAN 0.3 has them", cases.len());
}

/// Run each script in a psql of its own, all at once; what each printed and its errors.
fn at_once(pg: &Postgres, scripts: &[String]) -> Vec<(String, String)> {
    std::thread::scope(|s| {
        let hs: Vec<_> = scripts
            .iter()
            .map(|sql| {
                s.spawn(move || {
                    let out = pg.db().args(["-A", "-t", "-v", "ON_ERROR_STOP=0", "-v", "VERBOSITY=verbose", "-c", sql]).output().unwrap();
                    (String::from_utf8_lossy(&out.stdout).to_string(), String::from_utf8_lossy(&out.stderr).to_string())
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    })
}

/// 移し替え between two warehouses, both ways at once, a hundred times each way in each of four
/// sessions: the rows are locked in the order of their IDs, so no two calls wait on each other in
/// a circle.
fn no_deadlock(pg: &Postgres) {
    let t = "'deadlock'";
    pg.exec(&format!("select * from \"自分あて\".\"入庫_do\"({t}, 'in-a', '東', 's', 1000); select * from \"自分あて\".\"入庫_do\"({t}, 'in-b', '西', 's', 1000);"));
    let scripts: Vec<String> = (0..4)
        .map(|w| {
            (0..100)
                .map(|i| {
                    let (from, to) = if w % 2 == 0 { ("東", "西") } else { ("西", "東") };
                    format!("select result from \"自分あて\".\"移し替え_do\"({t}, 'w{w}-{i}', '{from}', '{to}', 's', 1);")
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect();
    let started = Instant::now();
    for (out, err) in at_once(pg, &scripts) {
        assert!(err.is_empty(), "a session failed:\n{err}");
        assert_eq!(out.lines().filter(|l| *l == "done").count(), 100, "{out}");
    }
    let east = pg.exec(&format!("select posted from \"自分あて\".\"balance_倉庫\"({t}, '東', 's')"));
    let west = pg.exec(&format!("select posted from \"自分あて\".\"balance_倉庫\"({t}, '西', 's')"));
    assert_eq!((east.trim(), west.trim()), ("1000", "1000"));
    eprintln!("no deadlock: 400 moves between two accounts both ways at once, in {:.1} s", started.elapsed().as_secs_f64());
}

/// A write that does not go through the functions is stopped by the rows' checks.
fn checks_stop_writes(pg: &Postgres) {
    let t = "'check'";
    pg.exec(&format!("select * from \"在庫\".\"入荷_do\"({t}, 'n-1', 'A', 3)"));
    let out = pg.db().args(["-c", &format!("update \"在庫\".accounts set held_out = posted + 1 where tenant = {t} and kind = '在庫'")]).output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success() && err.contains("within_lower"), "{err}");
    pg.exec("select * from \"ウォレット\".\"入金_do\"('check', 'p-1', 'm', 5)");
    let out = pg.db().args(["-c", "update \"ウォレット\".accounts set posted = 100001 where tenant = 'check' and kind = 'ウォレット'"]).output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success() && err.contains("within_upper"), "{err}");
    eprintln!("checks: a write past a bound that does not go through the functions is stopped");
}

/// A row made under other bounds is an error (CB001), not a refusal: the book changed under it.
fn other_bounds(pg: &Postgres) {
    let t = "'bounds'";
    pg.exec(&format!("select * from \"在庫\".\"入荷_do\"({t}, 'n-1', 'A', 3)"));
    pg.exec(&format!("update \"在庫\".accounts set lower_bound = 2 where tenant = {t} and kind = '在庫'"));
    let out = pg.db().args(["-v", "VERBOSITY=verbose", "-c", &format!("select * from \"在庫\".\"引当_hold\"({t}, 'o-1', 'A', 1)")]).output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success() && err.contains("CB001"), "{err}");
    eprintln!("bounds: a row made under other bounds stops the call with CB001");
}

/// expire() gives back what the holds past their expiry hold, says how many it ended, and the
/// entries still add up to the balances.
fn expire(pg: &Postgres) {
    let t = "'expire'";
    pg.exec(&format!(
        "select * from \"在庫\".\"入荷_do\"({t}, 'n-1', 'A', 5); select * from \"在庫\".\"引当_hold\"({t}, 'o-1', 'A', 2); select * from \"在庫\".\"引当_hold\"({t}, 'o-2', 'A', 1); select * from \"在庫\".\"引当_hold\"({t}, 'o-3', 'A', 1); select * from \"在庫\".\"引当_void\"({t}, 'o-3', 'A');"
    ));
    let before = pg.exec(&format!("select held_out from \"在庫\".\"balance_在庫\"({t}, 'A')"));
    assert_eq!(before.trim(), "3");
    std::thread::sleep(Duration::from_secs(EXPIRY) + Duration::from_millis(300));
    let status = pg.exec(&format!("select \"在庫\".\"引当_status\"({t}, 'o-1', 'A')"));
    assert_eq!(status.trim(), "expired", "past its expiry, a hold is expired before expire() gives it back");
    let still = pg.exec(&format!("select held_out from \"在庫\".\"balance_在庫\"({t}, 'A')"));
    assert_eq!(still.trim(), "3", "PostgreSQL has no clock of its own: the hold is still counted");
    let n: i64 = pg.exec("select \"在庫\".expire()").trim().parse().unwrap();
    assert!(n >= 2, "expire() ended {n} holds");
    let after = pg.exec(&format!("select posted, held_in, held_out from \"在庫\".\"balance_在庫\"({t}, 'A')"));
    assert_eq!(after.trim(), "5|0|0");
    let states = pg.exec(&format!("select state from \"在庫\".holds where tenant = {t} order by key->>0"));
    assert_eq!(states.split_whitespace().collect::<Vec<_>>(), ["expired", "expired", "voided"]);
    assert_eq!(pg.exec("select \"在庫\".expire()").trim(), "0");
    let off = pg.exec(&format!(
        "select count(*) from \"在庫\".accounts a, lateral (select coalesce(sum(d_posted), 0) p, coalesce(sum(d_held_in), 0) i, coalesce(sum(d_held_out), 0) o from \"在庫\".entries e where e.tenant = a.tenant and e.account = a.id) x where a.tenant = {t} and (a.posted, a.held_in, a.held_out) is distinct from (x.p, x.i, x.o)"
    ));
    assert_eq!(off.trim(), "0");
    eprintln!("expire(): {n} holds given back, the balances and the entries agree");
}

/// A client under REPEATABLE READ: four callers at once, fifty calls each, on the same two rows.
/// PostgreSQL answers some with a serialization failure; the client tries them again, and every
/// call ends done, as under READ COMMITTED.
fn repeatable_read(pg: &Postgres, work: &Path) {
    const WORKERS: usize = 4;
    const CALLS: usize = 50;
    let check = |tenant: &str, out: &str| {
        let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap_or_else(|e| panic!("{tenant}: {e}: {out}"));
        let results = v["results"].as_array().unwrap();
        assert_eq!(results.len(), WORKERS * CALLS, "{tenant}");
        assert!(results.iter().all(|r| r == "done"), "{tenant}: {results:?}");
        let conflicts = v["conflicts"].as_u64().unwrap();
        let posted = pg.exec(&format!("select posted from \"在庫\".\"balance_在庫\"('{tenant}', 'A')"));
        assert_eq!(posted.trim(), (WORKERS * CALLS).to_string(), "{tenant}");
        eprintln!("repeatable read: {tenant}: {} calls done, {conflicts} serialization failure(s) tried again", WORKERS * CALLS);
        conflicts
    };
    let conn = pg.connection();
    let (host, port, user) = (conn["host"].as_str().unwrap().to_string(), conn["port"].to_string(), conn["user"].as_str().unwrap().to_string());
    let mut conflicts = 0;

    match node() {
        Ok(()) => {
            let dir = ts_dir(work).join("rr");
            build_all(&cases_of(&["在庫"]), Target::PostgresTypeScript, &dir);
            let script = dir.join("rr.ts");
            std::fs::write(
                &script,
                format!(
                    r#"import pg from "pg";
import {{ postgres }} from "./在庫.ts";
const pool = new pg.Pool({{ host: {host:?}, port: {port}, user: {user:?}, database: "postgres", max: {WORKERS} }});
pool.on("connect", (c) => {{ c.query("set default_transaction_isolation = 'repeatable read'"); }});
let conflicts = 0;
const db = {{ query: async (text: string, values?: unknown[]) => {{ try {{ return await pool.query(text, values); }} catch (e) {{ if ((e as {{ code?: string }}).code === "40001") conflicts++; throw e; }} }} }};
const book = postgres(db, {{ tenant: "rr-typescript" }});
const results = await Promise.all(Array.from({{ length: {WORKERS} }}, async (_, w) => {{
  const out: string[] = [];
  for (let i = 0; i < {CALLS}; i++) out.push((await book.入荷.do({{ 納品書: `w${{w}}-${{i}}`, sku: "A", 数: 1n }})).result);
  return out;
}}));
console.log(JSON.stringify({{ results: results.flat(), conflicts }}));
await pool.end();
"#
                ),
            )
            .unwrap();
            let out = Command::new("node").arg("--no-warnings").arg(&script).output().unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            conflicts += check("rr-typescript", &String::from_utf8_lossy(&out.stdout));
        }
        Err(why) => skip(&format!("{why}; the TypeScript client is not tried under REPEATABLE READ")),
    }
    match python() {
        Ok(py) => {
            let dir = work.join("rr-py");
            let file = build_all(&cases_of(&["在庫"]), Target::PostgresPython, &dir).remove(0);
            let script = dir.join("rr.py");
            std::fs::write(
                &script,
                format!(
                    r#"import importlib.util, json, sys, threading
import psycopg
spec = importlib.util.spec_from_file_location("book", {file:?})
mod = importlib.util.module_from_spec(spec)
sys.modules["book"] = mod
spec.loader.exec_module(mod)
conflicts = 0
lock = threading.Lock()

class Counting:
    def __init__(self, conn):
        self.conn = conn
    def cursor(self):
        return CountingCursor(self.conn.cursor())

class CountingCursor:
    def __init__(self, cur):
        self.cur = cur
    def __enter__(self):
        return self
    def __exit__(self, *exc):
        self.cur.close()
    def execute(self, sql, params):
        global conflicts
        try:
            self.cur.execute(sql, params)
        except psycopg.errors.SerializationFailure:
            with lock:
                conflicts += 1
            raise
    def fetchone(self):
        return self.cur.fetchone()

results = []
def work(w):
    conn = psycopg.connect(host={host:?}, port={port}, user={user:?}, dbname="postgres", autocommit=True, options="-c default_transaction_isolation=repeatable\\ read")
    book = mod.postgres(Counting(conn), tenant="rr-python")
    out = [book.入荷.do(納品書=f"w{{w}}-{{i}}", sku="A", 数=1).result for i in range({CALLS})]
    with lock:
        results.extend(out)
    conn.close()

threads = [threading.Thread(target=work, args=(w,)) for w in range({WORKERS})]
for t in threads: t.start()
for t in threads: t.join()
print(json.dumps({{"results": results, "conflicts": conflicts}}))
"#,
                    file = file.display().to_string()
                ),
            )
            .unwrap();
            let out = Command::new(py).arg(&script).output().unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            conflicts += check("rr-python", &String::from_utf8_lossy(&out.stdout));
        }
        Err(why) => skip(&format!("{why}; the Python client is not tried under REPEATABLE READ")),
    }
    match go() {
        Ok(()) => {
            let module = work.join("rr-go");
            for f in ["go.mod", "go.sum"] {
                std::fs::create_dir_all(&module).unwrap();
                std::fs::copy(runner_dir().join("go").join(f), module.join(f)).unwrap();
            }
            let c = &cases_of(&["在庫"])[0];
            for (rel, text) in chobo::target::build(&c.copy, &c.stem, Target::PostgresGo).unwrap() {
                let p = module.join("book").join(Path::new(&rel).file_name().unwrap());
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(p, text).unwrap();
            }
            std::fs::write(
                module.join("main.go"),
                format!(
                    r#"package main

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"sync"
	"sync/atomic"

	"chobo-runner/book"

	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgconn"
	"github.com/jackc/pgx/v5/pgxpool"
)

var conflicts atomic.Int64

type counting struct{{ p *pgxpool.Pool }}

type row struct{{ r pgx.Row }}

func (c counting) QueryRow(ctx context.Context, sql string, args ...any) pgx.Row {{
	return row{{c.p.QueryRow(ctx, sql, args...)}}
}}

func (r row) Scan(dest ...any) error {{
	err := r.r.Scan(dest...)
	var e *pgconn.PgError
	if errors.As(err, &e) && e.Code == "40001" {{
		conflicts.Add(1)
	}}
	return err
}}

func main() {{
	ctx := context.Background()
	cfg, err := pgxpool.ParseConfig("host={host} port={port} user={user} dbname=postgres pool_max_conns={WORKERS}")
	if err != nil {{
		panic(err)
	}}
	cfg.ConnConfig.RuntimeParams["default_transaction_isolation"] = "repeatable read"
	pool, err := pgxpool.NewWithConfig(ctx, cfg)
	if err != nil {{
		panic(err)
	}}
	defer pool.Close()
	b := book.Postgres(counting{{pool}}, "rr-go")
	var mu sync.Mutex
	results := []string{{}}
	var wg sync.WaitGroup
	for w := range {WORKERS} {{
		wg.Add(1)
		go func() {{
			defer wg.Done()
			for i := range {CALLS} {{
				r, err := b.X入荷.Do(ctx, book.X入荷Args{{X納品書: fmt.Sprintf("w%d-%d", w, i), Sku: "A", X数: 1}})
				if err != nil {{
					panic(err)
				}}
				mu.Lock()
				results = append(results, r.Outcome)
				mu.Unlock()
			}}
		}}()
	}}
	wg.Wait()
	out, _ := json.Marshal(map[string]any{{"results": results, "conflicts": conflicts.Load()}})
	fmt.Println(string(out))
}}
"#
                ),
            )
            .unwrap();
            let out = go_in(&module).args(["run", "-mod=readonly", "."]).output().unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            conflicts += check("rr-go", &String::from_utf8_lossy(&out.stdout));
        }
        Err(why) => skip(&format!("{why}; the Go client is not tried under REPEATABLE READ")),
    }
    if node().is_ok() || python().is_ok() || go().is_ok() {
        assert!(conflicts > 0, "no call met a serialization failure: the clients' tries again were not tried");
    }
}

/// The cases of the named books of tests/books.
fn cases_of(names: &[&str]) -> Vec<Case> {
    names.iter().map(|n| case(&root().join(format!("tests/books/{n}.book")))).collect()
}

fn too_long() {
    let work = TempDir::new("e061");
    let out = chobo().args(["build", root().join("tests/fixtures/名前の長さ.book").to_str().unwrap(), "--target", "postgres", "--out", work.path().to_str().unwrap()]).output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("error[E061]"), "{err}");
    assert_eq!(std::fs::read_dir(work.path()).unwrap().count(), 0, "nothing is written");
    eprintln!("E061: a name too long for PostgreSQL stops the build");
}

#[test]
fn what_only_postgres_has() {
    if !need(Need::Postgres) {
        return;
    }
    too_long();
    let pg = match Postgres::start() {
        Ok(pg) => pg,
        Err(why) => {
            skip(&format!("{why}; what only PostgreSQL has is not tried"));
            return;
        }
    };
    let work = TempDir::new("postgres-only");
    load_books(&pg, work.path());
    ids(&pg);
    no_deadlock(&pg);
    checks_stop_writes(&pg);
    other_bounds(&pg);
    expire(&pg);
    repeatable_read(&pg, work.path());
}

/// What `too_long` checks in `what_only_postgres_has`, with the English book: a name of more than 63
/// ASCII letters stops the build.
#[test]
fn a_name_too_long_for_postgres_in_english() {
    if !need(Need::Postgres) {
        return;
    }
    let work = TempDir::new("e061-en");
    let out = chobo().args(["build", root().join("tests/fixtures/name_length.book").to_str().unwrap(), "--target", "postgres", "--out", work.path().to_str().unwrap()]).output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("error[E061]") && err.contains("is 67 bytes long"), "{err}");
    assert_eq!(std::fs::read_dir(work.path()).unwrap().count(), 0, "nothing is written");
}
