//! What only the MySQL family has (DESIGN 4.5), on a throwaway MySQL server and on a TiDB
//! cluster of the machine's (`RITSU_TIDB`): two calls that touch the same two accounts in the
//! opposite order never deadlock, a write that does not go through the clients is stopped by the
//! checks, a row made under other bounds is an error, expire() gives back what is past its expiry
//! and adds up the entries of the accounts without bounds, many callers after the last of
//! something each get done or refused, a call runs in the caller's transaction and goes with it,
//! a call is a transaction of its own on a connection that is in none, in each of the three
//! languages, a call's statements go in one request with `batch`, a key and a tenant are their
//! bytes, the statements read the same under any `sql_mode`, and a name MySQL cannot hold stops
//! the build (E062).
//!
//! The steps run the TypeScript client, through mysql2, and the step of whose transaction a call is
//! the Python and the Go clients too; the three clients run every scenario on both servers in
//! tests/backends.rs.

mod common;
use chobo::target::Target;
use common::runners::*;
use common::servers::{My, MySql};
use common::*;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// The servers of the family this machine has, each with the slug its steps are named by.
fn servers(what: &str) -> Vec<(MySql, &'static str)> {
    let mut out = Vec::new();
    if need(Need::MySql) {
        match MySql::start() {
            Ok(my) => out.push((my, "mysql")),
            Err(why) => skip(&format!("{why}; {what} is not tried on MySQL")),
        }
    }
    if need(Need::TiDb) {
        match MySql::at("TIDB", "TiDB") {
            Ok(my) => out.push((my, "tidb")),
            Err(why) => skip(&format!("{why}; {what} is not tried on TiDB")),
        }
    }
    // Vitess, its keyspace in two shards by the tenant, where a transaction that crosses shards fails
    if need(Need::Vitess) {
        match MySql::vitess(2) {
            Ok(my) => out.push((my, "vitess2")),
            Err(why) => skip(&format!("{why}; {what} is not tried on Vitess")),
        }
    }
    out
}

/// The ID of a tenant of a book of tests/books, as SQL: what a statement of the tests' own names
/// the tenant's rows by, so that on Vitess it goes to the tenant's shard.
fn tenant_of(book: &str, tenant: &str) -> String {
    let c = case(&root().join(format!("tests/books/{book}.book")));
    chobo::mysql::id_lit(chobo::mysql::tenant_id(&c.book, tenant))
}

/// A database of a server with the tables of some books, and their TypeScript and Python clients.
struct Place {
    db: String,
    dir: PathBuf,
}

/// The cases of the named books of tests/books.
fn cases_of(names: &[&str]) -> Vec<Case> {
    names.iter().map(|n| case(&root().join(format!("tests/books/{n}.book")))).collect()
}

/// Make a database on `my` with the tables of `books`, and build their clients. On TiDB, with
/// `checks`, the tables are made with tidb_enable_check_constraint ON, which keeps their checks.
fn load(my: &MySql, books: &[&str], work: &Path, name: &str, checks: bool) -> Place {
    let db = my.database(name).unwrap_or_else(|e| panic!("{}: {e}", my.label));
    let cases = cases_of(books);
    let tidb = my.label.starts_with("TiDB");
    if tidb && checks {
        my.sql("", "set global tidb_enable_check_constraint = on").unwrap();
    }
    for (c, f) in cases.iter().zip(build_all(&cases, Target::MySql, &work.join(format!("{db}-sql")))) {
        my.load(&db, &f).unwrap_or_else(|e| panic!("{} on {}: {e}", c.stem, my.label));
    }
    if tidb && checks {
        my.sql("", "set global tidb_enable_check_constraint = off").unwrap();
    }
    // the caller's own rows, which a transaction of the caller's writes with a call: on a sharded
    // Vitess, sharded by the tenant, as a caller's rows have to be to commit with a call's
    my.sql(&db, "create table if not exists `orders` (`tenant` binary(16) not null, `id` varchar(20) not null, primary key (`tenant`, `id`))").unwrap_or_else(|e| panic!("{}: {e}", my.label));
    my.shard_by("orders", "tenant").unwrap_or_else(|e| panic!("{}: {e}", my.label));
    let dir = ts_dir(work).join(&db);
    build_all(&cases, Target::MySqlTypeScript, &dir);
    build_all(&cases, Target::MySqlPython, &dir);
    std::fs::write(dir.join("package.json"), "{\"type\": \"module\"}\n").unwrap();
    let nm = dir.join("node_modules");
    if !nm.exists() {
        std::os::unix::fs::symlink(runner_dir().join("node_modules"), &nm).unwrap();
    }
    Place { db, dir }
}

/// Run TypeScript on the place: `body` has `pool` (mysql2's, multiple statements allowed with
/// `multi`) and the client of each of `books` under the book's name; it prints one JSON value.
fn ts(my: &MySql, place: &Place, books: &[&str], body: &str, multi: bool) -> Value {
    let imports: String = books.iter().map(|b| format!("import {{ mysql as {b} }} from \"./{b}.ts\";\n")).collect();
    let script = format!(
        "import mysql from \"mysql2/promise\";\n{imports}const pool = mysql.createPool({{ host: {:?}, port: {}, user: {:?}, database: {:?}, connectionLimit: 16, multipleStatements: {multi} }});\ntry {{\n{body}\n}} finally {{\n  await pool.end();\n}}\n",
        my.host, my.port, my.user, place.db
    );
    let file = place.dir.join(format!("step-{}.ts", std::process::id() as u64 * 1000 + rand_suffix()));
    std::fs::write(&file, script).unwrap();
    let out = Command::new("node").arg("--no-warnings").arg(&file).output().unwrap();
    assert!(out.status.success(), "{} on {}:\n{}{}{}", file.display(), my.label, String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr), errors_of(my));
    let text = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(text.trim().lines().last().unwrap_or("null")).unwrap_or_else(|e| panic!("{e}: {text}"))
}

/// Run Python on the place, through PyMySQL: `body` has `connect(**options)`, a connection of
/// PyMySQL to the place's database, and the client of each of `books` under the book's name; it
/// prints one JSON value.
fn py(my: &MySql, place: &Place, python: &Path, books: &[&str], body: &str) -> Value {
    let imports: String = books.iter().map(|b| format!("from {b} import mysql as {b}\n")).collect();
    let script = format!(
        "import json\nimport sys\n\nimport pymysql\n\nsys.path.insert(0, {:?})\n{imports}\n\ndef connect(**o):\n    return pymysql.connect(host={:?}, port={}, user={:?}, password=\"\", database={:?}, charset=\"utf8mb4\", **o)\n\n\n{body}\n",
        place.dir.display().to_string(),
        my.host,
        my.port,
        my.user,
        place.db
    );
    let file = place.dir.join(format!("step-{}.py", std::process::id() as u64 * 1000 + rand_suffix()));
    std::fs::write(&file, script).unwrap();
    // -B: nothing is written into the runner's venv, nor beside the clients
    let out = Command::new(python).arg("-B").arg(&file).output().unwrap();
    assert!(out.status.success(), "{} on {}:\n{}{}", file.display(), my.label, String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(text.trim().lines().last().unwrap_or("null")).unwrap_or_else(|e| panic!("{e}: {text}"))
}

/// Build and run Go on the place: a module of tools/runner/go's go.mod and go.sum, with the Go
/// client of `book` as the package `book`, and `main` as main.go's func main, which is given the
/// DSN of the place's database as `os.Args[1]` and prints one JSON value.
fn go_main(my: &MySql, place: &Place, work: &Path, book: &str, main: &str) -> Value {
    let module = work.join(format!("go-{}", place.db));
    std::fs::create_dir_all(&module).unwrap();
    for f in ["go.mod", "go.sum"] {
        std::fs::copy(runner_dir().join("go").join(f), module.join(f)).unwrap();
    }
    let c = case(&root().join(format!("tests/books/{book}.book")));
    for (rel, text) in chobo::target::build(&c.copy, &c.stem, Target::MySqlGo, &c.origin).unwrap_or_else(|d| panic!("{book}: {}", d[0].message.en)) {
        let at = module.join("books/b").join(Path::new(&rel).file_name().unwrap());
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, text).unwrap();
    }
    std::fs::write(
        module.join("main.go"),
        format!(
            "package main\n\nimport (\n\t\"context\"\n\t\"database/sql\"\n\t\"encoding/json\"\n\t\"os\"\n\n\t_ \"github.com/go-sql-driver/mysql\"\n\n\tbook \"chobo-runner/books/b\"\n)\n\nfunc must(err error) {{\n\tif err != nil {{\n\t\tpanic(err)\n\t}}\n}}\n\nfunc main() {{\n\tctx := context.Background()\n\tdb, err := sql.Open(\"mysql\", os.Args[1])\n\tmust(err)\n\tdefer db.Close()\n\tout := map[string]any{{}}\n{main}\n\tmust(json.NewEncoder(os.Stdout).Encode(out))\n}}\n"
        ),
    )
    .unwrap();
    let bin = module.join("steps");
    let built = go_in(&module).args(["build", "-mod=readonly", "-o"]).arg(&bin).arg(".").output().unwrap();
    assert!(built.status.success(), "the Go of the steps did not build:\n{}{}", String::from_utf8_lossy(&built.stdout), String::from_utf8_lossy(&built.stderr));
    let out = Command::new(&bin).arg(format!("{}@tcp({}:{})/{}", my.user, my.host, my.port, place.db)).output().unwrap();
    assert!(out.status.success(), "the Go of the steps on {}:\n{}{}", my.label, String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(text.trim().lines().last().unwrap_or("null")).unwrap_or_else(|e| panic!("{e}: {text}"))
}

/// What the server's log says went wrong, for a failure to show: the lines of errors, the last 30.
fn errors_of(my: &MySql) -> String {
    let log = my.log();
    let lines: Vec<&str> = log.lines().filter(|l| l.contains("[ERROR]") || l.contains("\"level\":\"ERROR\"") || l.contains("\"level\":\"WARN\"")).collect();
    if lines.is_empty() {
        return String::new();
    }
    format!("\n--- the server's log, its errors\n{}", lines[lines.len().saturating_sub(30)..].join("\n"))
}

fn rand_suffix() -> u64 {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

/// A pool that counts the errors after which a call of its own is made again.
const COUNTING: &str = "const counted = { again: 0 };\nconst counting = { getConnection: async () => { const c = await pool.getConnection(); return { query: async (o: any) => { try { return await c.query(o); } catch (e: any) { if ([1213, 1205, 9007].includes(e.errno)) counted.again++; throw e; } }, release: () => c.release() }; } };\n";

/// move_stock between two warehouses, both ways at once, a hundred times each way in each of four
/// callers: the rows are locked in the order of their IDs, so no two calls wait on each other in a
/// circle, and no call is made again.
fn no_deadlock(my: &MySql, place: &Place, book: &str, names: [&str; 8]) {
    let [stock_in, move_stock, warehouse, slip, from_place, to_place, at, qty] = names;
    let v = ts(
        my,
        place,
        &[book],
        &format!(
            "{COUNTING}const b = {book}(counting, {{ tenant: \"deadlock\" }});
await b.{stock_in}.do({{ {slip}: \"in-a\", {at}: \"east\", sku: \"s\", {qty}: 1000n }} as any);
await b.{stock_in}.do({{ {slip}: \"in-b\", {at}: \"west\", sku: \"s\", {qty}: 1000n }} as any);
const started = Date.now();
const results = await Promise.all([0, 1, 2, 3].map(async (w) => {{
  const out: string[] = [];
  for (let i = 0; i < 100; i++) {{
    const [from, to] = w % 2 === 0 ? [\"east\", \"west\"] : [\"west\", \"east\"];
    out.push((await b.{move_stock}.do({{ {slip}: `w${{w}}-${{i}}`, {from_place}: from, {to_place}: to, sku: \"s\", {qty}: 1n }} as any)).result);
  }}
  return out;
}}));
const east = await b.balance.{warehouse}({{ {at}: \"east\", sku: \"s\" }} as any);
const west = await b.balance.{warehouse}({{ {at}: \"west\", sku: \"s\" }} as any);
console.log(JSON.stringify({{ done: results.flat().filter((r) => r === \"done\").length, again: counted.again, east: String(east.posted), west: String(west.posted), seconds: (Date.now() - started) / 1000 }}));"
        ),
        false,
    );
    assert_eq!(v["done"], 400, "{} {v}", my.label);
    assert_eq!((v["east"].as_str(), v["west"].as_str()), (Some("1000"), Some("1000")), "{} {v}", my.label);
    assert_eq!(v["again"], 0, "{}: a call met a deadlock or a lock it waited too long for: {v}", my.label);
    eprintln!("no deadlock on {}: 400 moves between two accounts both ways at once, in {} s, none made again", my.label, v["seconds"]);
}

/// A write that does not go through the clients is stopped by the rows' checks.
fn checks_stop_writes(my: &MySql, place: &Place, stock: [&str; 5], wallet: [&str; 6]) {
    let [sbook, receive, delivery, skind, qty] = stock;
    let [wbook, deposit, deposit_id, wkind, amount, member] = wallet;
    ts(
        my,
        place,
        &[sbook, wbook],
        &format!(
            "await {sbook}(pool, {{ tenant: \"check\" }}).{receive}.do({{ {delivery}: \"n-1\", sku: \"A\", {qty}: 3n }} as any);
await {wbook}(pool, {{ tenant: \"check\" }}).{deposit}.do({{ {deposit_id}: \"p-1\", {member}: \"m\", {amount}: 5n }} as any);
console.log(\"null\");"
        ),
        false,
    );
    let accounts = chobo::mysql::ident(&format!("{sbook}_accounts"));
    let err = my.sql(&place.db, &format!("update {accounts} set `held_out` = `posted` + 1 where `tenant` = {} and `kind` = '{skind}'", tenant_of(sbook, "check"))).unwrap_err();
    assert!(err.contains("3819") && err.contains(&format!("{sbook}_within_lower")), "{}: {err}", my.label);
    let accounts = chobo::mysql::ident(&format!("{wbook}_accounts"));
    let err = my.sql(&place.db, &format!("update {accounts} set `posted` = 100001 where `tenant` = {} and `kind` = '{wkind}'", tenant_of(wbook, "check"))).unwrap_err();
    assert!(err.contains("3819") && err.contains(&format!("{wbook}_within_upper")), "{}: {err}", my.label);
    eprintln!("checks on {}: a write past a bound that does not go through the clients is stopped", my.label);
}

/// On TiDB, tables made with tidb_enable_check_constraint OFF keep no check: the bounds are kept by
/// the clients alone, as the head of the SQL says.
fn no_checks_without_the_variable(my: &MySql, place: &Place, sbook: &str, receive: &str, delivery: &str, skind: &str, qty: &str) {
    ts(my, place, &[sbook], &format!("await {sbook}(pool, {{ tenant: \"nocheck\" }}).{receive}.do({{ {delivery}: \"n-1\", sku: \"A\", {qty}: 3n }} as any);\nconsole.log(\"null\");"), false);
    let accounts = chobo::mysql::ident(&format!("{sbook}_accounts"));
    my.sql(&place.db, &format!("update {accounts} set `held_out` = `posted` + 1 where `tenant` = {} and `kind` = '{skind}'", tenant_of(sbook, "nocheck"))).unwrap_or_else(|e| panic!("{}: {e}", my.label));
    eprintln!("checks on {}: made without tidb_enable_check_constraint, the tables keep none", my.label);
}

/// A row made under other bounds is an error (CB001), not a refusal: the book changed under it.
fn other_bounds(my: &MySql, place: &Place, names: [&str; 7]) {
    let [book, receive, delivery, kind, qty, reserve, order] = names;
    ts(my, place, &[book], &format!("await {book}(pool, {{ tenant: \"bounds\" }}).{receive}.do({{ {delivery}: \"n-1\", sku: \"A\", {qty}: 3n }} as any);\nconsole.log(\"null\");"), false);
    let accounts = chobo::mysql::ident(&format!("{book}_accounts"));
    my.sql(&place.db, &format!("update {accounts} set `lower_bound` = 2 where `tenant` = {} and `kind` = '{kind}'", tenant_of(book, "bounds"))).unwrap();
    let v = ts(
        my,
        place,
        &[book],
        &format!("try {{ await {book}(pool, {{ tenant: \"bounds\" }}).{reserve}.hold({{ {order}: \"o-1\", sku: \"A\", {qty}: 1n }} as any); console.log(JSON.stringify({{ code: null }})); }} catch (e: any) {{ console.log(JSON.stringify({{ code: e.code ?? null }})); }}"),
        false,
    );
    assert_eq!(v["code"], "CB001", "{} {v}", my.label);
    eprintln!("bounds on {}: a row made under other bounds stops the call with CB001", my.label);
}

/// expire() gives back what the holds past their expiry hold, says how many it ended, adds the
/// entries of the accounts without bounds to their rows, and every row is what its folded entries
/// add up to.
fn expire(my: &MySql, place: &Place, names: [&str; 9]) {
    let [book, receive, delivery, qty, reserve, order, stock, customers, kind] = names;
    let v = ts(
        my,
        place,
        &[book],
        &format!(
            "const b = {book}(pool, {{ tenant: \"expire\" }});
await b.{receive}.do({{ {delivery}: \"n-1\", sku: \"A\", {qty}: 5n }} as any);
for (const [o, n] of [[\"o-1\", 2n], [\"o-2\", 1n], [\"o-3\", 1n]] as const) await b.{reserve}.hold({{ {order}: o, sku: \"A\", {qty}: n }} as any);
await b.{reserve}.void({{ {order}: \"o-3\", sku: \"A\" }} as any);
const before = await b.balance.{stock}({{ sku: \"A\" }});
const held = await b.balance.{customers}();
await new Promise((r) => setTimeout(r, {wait}));
const status = await b.{reserve}.status({{ {order}: \"o-1\", sku: \"A\" }} as any);
const still = await b.balance.{stock}({{ sku: \"A\" }});
const n = await b.expire();
const after = await b.balance.{stock}({{ sku: \"A\" }});
const back = await b.balance.{customers}();
const again = await b.expire();
console.log(JSON.stringify({{ before: String(before.held_out), held: String(held.held_in), status, still: String(still.held_out), n, after: [after.posted, after.held_in, after.held_out].map(String), back: [back.posted, back.held_in, back.held_out].map(String), again }}));",
            wait = EXPIRY * 1000 + 300
        ),
        false,
    );
    assert_eq!(v["before"], "3", "{} {v}", my.label);
    assert_eq!(v["held"], "3", "{}: an account without bounds holds what its entries say: {v}", my.label);
    assert_eq!(v["status"], "expired", "{}: past its expiry, a hold is expired before expire() gives it back: {v}", my.label);
    assert_eq!(v["still"], "3", "{}: the database has no clock of its own that acts: the hold is still counted: {v}", my.label);
    assert!(v["n"].as_u64().unwrap_or(0) >= 2, "{}: expire() ended {}", my.label, v["n"]);
    assert_eq!(v["after"], json!(["5", "0", "0"]), "{} {v}", my.label);
    assert_eq!(v["back"], json!(["0", "0", "0"]), "{} {v}", my.label);
    assert_eq!(v["again"], 0, "{} {v}", my.label);
    let calls = chobo::mysql::ident(&format!("{book}_calls"));
    let tenant = tenant_of(book, "expire");
    let states = my.sql(&place.db, &format!("select `state` from {calls} where `tenant` = {tenant} and `op` = 'hold' order by `key`")).unwrap();
    assert_eq!(states.split_whitespace().collect::<Vec<_>>(), ["expired", "expired", "voided"], "{}", my.label);
    let (a, e) = (chobo::mysql::ident(&format!("{book}_accounts")), chobo::mysql::ident(&format!("{book}_entries")));
    let unfolded = my.sql(&place.db, &format!("select count(*) from {e} `e` where `e`.`tenant` = {tenant} and `e`.`folded` = 0")).unwrap();
    assert_eq!(unfolded.trim(), "0", "{}: expire() leaves no entry to add up", my.label);
    let off = my
        .sql(
            &place.db,
            &format!(
                "select count(*) from {a} `a` left join (select `tenant`, `account`, sum(`d_posted`) `p`, sum(`d_held_in`) `i`, sum(`d_held_out`) `o` from {e} where `tenant` = {tenant} group by `tenant`, `account`) `x` on `x`.`tenant` = `a`.`tenant` and `x`.`account` = `a`.`id` where `a`.`tenant` = {tenant} and (`a`.`posted` <> coalesce(`x`.`p`, 0) or `a`.`held_in` <> coalesce(`x`.`i`, 0) or `a`.`held_out` <> coalesce(`x`.`o`, 0))"
            ),
        )
        .unwrap();
    assert_eq!(off.trim(), "0", "{}", my.label);
    let _ = kind;
    eprintln!("expire() on {}: {} holds given back, the entries added up, the rows and the entries agree", my.label, v["n"]);
}

/// Eight callers after the last 100 of one account, fifty holds of 1 each: exactly 100 are done and
/// 300 refused, one statement at a time and in one request a call. How long it takes is printed.
fn contention(my: &MySql, place: &Place, names: [&str; 6]) {
    let [book, receive, delivery, qty, reserve, order] = names;
    for batch in [false, true] {
        let v = ts(
            my,
            place,
            &[book],
            &format!(
                "{COUNTING}const t = \"last-{batch}\";
await {book}(pool, {{ tenant: t }}).{receive}.do({{ {delivery}: \"n-1\", sku: \"A\", {qty}: 100n }} as any);
const b = {book}(counting, {{ tenant: t, batch: {batch} }});
await b.{reserve}.hold({{ {order}: \"warm\", sku: \"B\", {qty}: 0n }} as any);
const started = Date.now();
const results = await Promise.all(Array.from({{ length: 8 }}, async (_, w) => {{
  const out: string[] = [];
  for (let i = 0; i < 50; i++) {{ const r = await b.{reserve}.hold({{ {order}: `w${{w}}-${{i}}`, sku: \"A\", {qty}: 1n }} as any); out.push(r.result === \"refused\" ? r.reason : r.result); }}
  return out;
}}));
const seconds = (Date.now() - started) / 1000;
const flat = results.flat();
const stock = await b.balance.stock({{ sku: \"A\" }} as any);
console.log(JSON.stringify({{ done: flat.filter((r) => r === \"done\").length, refused: flat.filter((r) => r === \"out_of_stock\").length, held: String(stock.held_out), again: counted.again, seconds }}));"
            ),
            batch,
        );
        assert_eq!((v["done"].as_u64(), v["refused"].as_u64(), v["held"].as_str()), (Some(100), Some(300), Some("100")), "{} batch {batch}: {v}", my.label);
        let seconds = v["seconds"].as_f64().unwrap_or(0.0);
        eprintln!(
            "contention on {}{}: 400 holds of the last 100 of one account by 8 callers: 100 done, 300 refused, in {seconds:.2} s ({:.0} calls a second, {} made again)",
            my.label,
            if batch { ", in one request a call" } else { "" },
            400.0 / seconds.max(0.001),
            v["again"]
        );
    }
}

/// A call given a connection runs in the caller's transaction: it commits with the caller's own
/// rows, and goes with the caller's rollback, account rows and all, the key too.
fn caller_transaction(my: &MySql, place: &Place, names: [&str; 4]) {
    let [book, receive, delivery, qty] = names;
    let t = tenant_of(book, "caller");
    let v = ts(
        my,
        place,
        &[book],
        &format!(
            "const c = await pool.getConnection();
const b = {book}(c, {{ tenant: \"caller\" }});
await c.query(\"start transaction\");
await c.query(\"insert into `orders` (`tenant`, `id`) values ({t}, 'o-1')\");
const first = await b.{receive}.do({{ {delivery}: \"d-1\", sku: \"A\", {qty}: 3n }} as any);
await c.query(\"commit\");
await c.query(\"start transaction\");
const second = await b.{receive}.do({{ {delivery}: \"d-2\", sku: \"B\", {qty}: 5n }} as any);
await c.query(\"rollback\");
await c.query(\"start transaction\");
const third = await b.{receive}.do({{ {delivery}: \"d-3\", sku: \"B\", {qty}: 7n }} as any);
await c.query(\"commit\");
c.release();
const own = {book}(pool, {{ tenant: \"caller\" }});
const a = await own.balance.stock({{ sku: \"A\" }} as any);
const bb = await own.balance.stock({{ sku: \"B\" }} as any);
const retried = await own.{receive}.do({{ {delivery}: \"d-2\", sku: \"B\", {qty}: 5n }} as any);
const [orders] = await pool.query(\"select count(*) as n from `orders` where `tenant` = {t}\");
console.log(JSON.stringify({{ first: first.result, second: second.result, third: third.result, a: String(a.posted), b: String(bb.posted), retried: retried.result, orders: (orders as any)[0].n }}));"
        ),
        false,
    );
    assert_eq!(
        (v["first"].as_str(), v["second"].as_str(), v["third"].as_str(), v["a"].as_str(), v["b"].as_str(), v["retried"].as_str()),
        (Some("done"), Some("done"), Some("done"), Some("3"), Some("7"), Some("done")),
        "{} {v}",
        my.label
    );
    assert_eq!(v["orders"].as_u64(), Some(1), "{} {v}", my.label);
    eprintln!("caller's transaction on {}: a call commits with the caller's rows and goes with its rollback, the account it made too", my.label);
}

/// Whose transaction a call is, on a connection the caller gives. TypeScript: mysql2's connection
/// is asked first (`set @chobo = 0`); with autocommit on and no transaction open, the call is a transaction
/// of its own on it, `begin` to `commit`; in a transaction the caller began, or with autocommit
/// off, it is part of it and goes with its rollback. Python: the same, from what PyMySQL keeps of
/// the server's status, `begin()` with autocommit on among them. Go: a `*sql.Conn` is a
/// transaction of its own, a `*sql.Tx` the caller's. An account made in a transaction rolled
/// back is made again by the next call of its own, so that its balance is not lost.
fn whose_transaction(my: &MySql, place: &Place, work: &Path) {
    let v = ts(
        my,
        place,
        &["stock_reservation"],
        &"const BEGIN = \"begin /*T! pessimistic */\";
const c = await pool.getConnection();
const sent: string[] = [];
const rec = { query: (o: any) => { sent.push(o.sql); return c.query(o); } };
const b = stock_reservation(rec as any, { tenant: \"whose-ts\" });
const by = async (f: () => Promise<any>) => { sent.length = 0; const r = await f(); return { result: r.result, asked: sent[0] === \"set @chobo = 0\", begin: sent.includes(BEGIN), commit: sent.includes(\"commit\") }; };
const own = await by(() => b.receive.do({ delivery: \"t-1\", sku: \"A\", qty: 3n }));
await c.query(\"start transaction\");
await c.query(\"insert into `orders` (`tenant`, `id`) values (@TENANT@, 't-o1')\");
const begun = await by(() => b.receive.do({ delivery: \"t-2\", sku: \"B\", qty: 5n }));
await c.query(\"rollback\");
await c.query(\"set autocommit = 0\");
const off = await by(() => b.receive.do({ delivery: \"t-3\", sku: \"A\", qty: 7n }));
await c.query(\"rollback\");
await c.query(\"set autocommit = 1\");
const after = await by(() => b.receive.do({ delivery: \"t-4\", sku: \"B\", qty: 2n }));
c.release();
const other = stock_reservation(pool, { tenant: \"whose-ts\" });
const a = await other.balance.stock({ sku: \"A\" });
const bb = await other.balance.stock({ sku: \"B\" });
const again = await other.receive.do({ delivery: \"t-2\", sku: \"B\", qty: 5n });
const [orders] = await pool.query(\"select count(*) as n from `orders` where `tenant` = @TENANT@ and `id` = 't-o1'\");
console.log(JSON.stringify({ own, begun, off, after, a: String(a.posted), b: String(bb.posted), again: again.result, orders: (orders as any)[0].n }));"
            .replace("@TENANT@", &tenant_of("stock_reservation", "whose-ts")),
        false,
    );
    let own = json!({"result": "done", "asked": true, "begin": true, "commit": true});
    let theirs = json!({"result": "done", "asked": true, "begin": false, "commit": false});
    assert_eq!((&v["own"], &v["begun"], &v["off"], &v["after"]), (&own, &theirs, &theirs, &own), "{} TypeScript: {v}", my.label);
    assert_eq!((v["a"].as_str(), v["b"].as_str(), v["again"].as_str(), v["orders"].as_u64()), (Some("3"), Some("2"), Some("done"), Some(0)), "{} TypeScript: {v}", my.label);
    eprintln!("whose transaction on {}, TypeScript: on a connection in none, a call is its own; in the caller's, or with autocommit off, part of it", my.label);

    match python_mysql() {
        Ok(python) => {
            let v = py(
                my,
                place,
                &python,
                &["stock_reservation"],
                &"BEGIN = \"begin /*T! pessimistic */\"


class Rec:
    \"\"\"A connection that writes down every statement sent through it.\"\"\"

    def __init__(self, conn):
        self.conn = conn
        self.sent = []

    def __getattr__(self, name):
        return getattr(self.conn, name)

    def cursor(self):
        return Cur(self, self.conn.cursor())


class Cur:
    def __init__(self, rec, cur):
        self.rec, self.cur = rec, cur

    def __getattr__(self, name):
        return getattr(self.cur, name)

    def execute(self, sql, *args):
        self.rec.sent.append(sql)
        return self.cur.execute(sql, *args)


conn = connect(autocommit=True)
cur = conn.cursor()
rec = Rec(conn)
b = stock_reservation(rec, tenant=\"whose-py\")


def by(f):
    rec.sent.clear()
    r = f()
    return {\"result\": r.result, \"begin\": BEGIN in rec.sent, \"commit\": \"commit\" in rec.sent}


own = by(lambda: b.receive.do(delivery=\"p-1\", sku=\"A\", qty=3))
conn.begin()
cur.execute(\"insert into `orders` (`tenant`, `id`) values (@TENANT@, 'p-o1')\")
begun = by(lambda: b.receive.do(delivery=\"p-2\", sku=\"B\", qty=5))
conn.rollback()
conn.autocommit(False)
off = by(lambda: b.receive.do(delivery=\"p-3\", sku=\"A\", qty=7))
conn.rollback()
conn.autocommit(True)
after = by(lambda: b.receive.do(delivery=\"p-4\", sku=\"B\", qty=2))
other = stock_reservation(connect(autocommit=True), tenant=\"whose-py\")
a = other.balance.stock(sku=\"A\")
bb = other.balance.stock(sku=\"B\")
again = other.receive.do(delivery=\"p-2\", sku=\"B\", qty=5)
cur.execute(\"select count(*) from `orders` where `tenant` = @TENANT@ and `id` = 'p-o1'\")
print(json.dumps({\"own\": own, \"begun\": begun, \"off\": off, \"after\": after, \"a\": str(a.posted), \"b\": str(bb.posted), \"again\": again.result, \"orders\": cur.fetchone()[0]}))"
                    .replace("@TENANT@", &tenant_of("stock_reservation", "whose-py")),
            );
            let own = json!({"result": "done", "begin": true, "commit": true});
            let theirs = json!({"result": "done", "begin": false, "commit": false});
            assert_eq!((&v["own"], &v["begun"], &v["off"], &v["after"]), (&own, &theirs, &theirs, &own), "{} Python: {v}", my.label);
            assert_eq!((v["a"].as_str(), v["b"].as_str(), v["again"].as_str(), v["orders"].as_u64()), (Some("3"), Some("2"), Some("done"), Some(0)), "{} Python: {v}", my.label);
            eprintln!("whose transaction on {}, Python: with autocommit on, a call is its own; after begin(), or with autocommit off, part of the caller's", my.label);
        }
        Err(why) => skip(&format!("{why}; whose transaction a call of the Python client is, is not tried on {}", my.label)),
    }

    match go() {
        Ok(()) => {
            let v = go_main(
                my,
                place,
                work,
                "stock_reservation",
                &"\tconn, err := db.Conn(ctx)
\tmust(err)
\tr, err := book.MySQL(conn, \"whose-go\", false).Receive.Do(ctx, book.ReceiveArgs{Delivery: \"g-1\", Sku: \"A\", Qty: 3})
\tmust(err)
\tout[\"conn\"] = r.Outcome
\tmust(conn.Close())
\tother := book.MySQL(db, \"whose-go\", false)
\ta, err := other.Balance.Stock(ctx, book.StockAccount{Sku: \"A\"})
\tmust(err)
\tout[\"seen\"] = a.Posted
\ttx, err := db.BeginTx(ctx, nil)
\tmust(err)
\t_, err = tx.ExecContext(ctx, \"insert into `orders` (`tenant`, `id`) values (@TENANT@, 'g-o1')\")
\tmust(err)
\tr, err = book.MySQL(tx, \"whose-go\", false).Receive.Do(ctx, book.ReceiveArgs{Delivery: \"g-2\", Sku: \"B\", Qty: 5})
\tmust(err)
\tout[\"tx\"] = r.Outcome
\tbb, err := other.Balance.Stock(ctx, book.StockAccount{Sku: \"B\"})
\tmust(err)
\tout[\"unseen\"] = bb.Posted
\tmust(tx.Rollback())
\tr, err = other.Receive.Do(ctx, book.ReceiveArgs{Delivery: \"g-2\", Sku: \"B\", Qty: 5})
\tmust(err)
\tout[\"again\"] = r.Outcome
\tbb, err = other.Balance.Stock(ctx, book.StockAccount{Sku: \"B\"})
\tmust(err)
\tout[\"b\"] = bb.Posted
\tvar n int
\tmust(db.QueryRowContext(ctx, \"select count(*) from `orders` where `tenant` = @TENANT@ and `id` = 'g-o1'\").Scan(&n))
\tout[\"orders\"] = n"
                    .replace("@TENANT@", &tenant_of("stock_reservation", "whose-go")),
            );
            assert_eq!(v, json!({"conn": "done", "seen": 3, "tx": "done", "unseen": 0, "again": "done", "b": 5, "orders": 0}), "{} Go", my.label);
            eprintln!("whose transaction on {}, Go: on a *sql.Conn, a call is its own; on a *sql.Tx, part of the caller's", my.label);
        }
        Err(why) => skip(&format!("{why}; whose transaction a call of the Go client is, is not tried on {}", my.label)),
    }

    if need(Need::Rustc) {
        match rust() {
            Ok(()) => {
                let bin = whose_rust(work, &place.db, &my.host, my.port, &my.user, &tenant_of("stock_reservation", "whose-rust"));
                let out = Command::new(&bin).output().unwrap();
                assert!(out.status.success(), "the Rust of the steps on {}:\n{}{}", my.label, String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
                let v: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stdout)));
                assert_eq!(v, json!({"conn": "done", "seen": 3, "tx": "done", "unseen": 0, "again": "done", "b": 5, "orders": 0}), "{} Rust", my.label);
                eprintln!("whose transaction on {}, Rust: on a connection in no transaction, a call is its own; on a Transaction of sqlx's, part of the caller's", my.label);
            }
            Err(why) => skip(&format!("{why}; whose transaction a call of the Rust client is, is not tried on {}", my.label)),
        }
    }
}

/// Build the Rust of the steps of whose transaction a call is, on `db` of the server at
/// `host:port`: a call on a connection of the pool that is in no transaction is a transaction of
/// its own, seen at once from another connection; a call on a `Transaction` of sqlx's is part of
/// it, unseen until it commits, and gone with its rollback, the caller's own row too. `tenant` is
/// the tenant's ID as SQL. The binary prints one JSON object.
fn whose_rust(work: &Path, db: &str, host: &str, port: u16, user: &str, tenant: &str) -> PathBuf {
    let c = case(&root().join("tests/books/stock_reservation.book"));
    let module = chobo::target::build(&c.copy, &c.stem, Target::MySqlRust, &c.origin).unwrap_or_else(|d| panic!("{}", d[0].message.en)).remove(0).1;
    let main = format!(
        r#"#![deny(warnings)]

#[allow(dead_code)]
mod harness;

#[path = "books/book.rs"]
mod book;

fn main() {{
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    rt.block_on(async {{
        let options = sqlx::mysql::MySqlConnectOptions::new().host({host:?}).port({port}).username({user:?}).database({db:?}).pipes_as_concat(false);
        let pool = sqlx::mysql::MySqlPoolOptions::new().max_connections(4).connect_with(options).await.unwrap();
        let mut conn = pool.acquire().await.unwrap();
        let r = book::mysql(tokio::sync::Mutex::new(&mut *conn), "whose-rust", false).receive().r#do(book::ReceiveArgs {{ delivery: "r-1", sku: "A", qty: 3 }}).await.unwrap();
        drop(conn);
        let other = book::mysql(pool.clone(), "whose-rust", false);
        let seen = other.balance().stock(book::StockAccount {{ sku: "A" }}).await.unwrap().posted;
        let mut tx = pool.begin().await.unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe("insert into `orders` (`tenant`, `id`) values ({tenant}, 'r-o1')")).execute(&mut *tx).await.unwrap();
        let in_tx = book::mysql(tokio::sync::Mutex::new(&mut *tx), "whose-rust", false).receive().r#do(book::ReceiveArgs {{ delivery: "r-2", sku: "B", qty: 5 }}).await.unwrap();
        let unseen = other.balance().stock(book::StockAccount {{ sku: "B" }}).await.unwrap().posted;
        tx.rollback().await.unwrap();
        let again = other.receive().r#do(book::ReceiveArgs {{ delivery: "r-2", sku: "B", qty: 5 }}).await.unwrap();
        let b = other.balance().stock(book::StockAccount {{ sku: "B" }}).await.unwrap().posted;
        let orders: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe("select count(*) from `orders` where `tenant` = {tenant} and `id` = 'r-o1'")).fetch_one(&pool).await.unwrap();
        println!("{{}}", serde_json::json!({{"conn": r.result(), "seen": seen, "tx": in_tx.result(), "unseen": unseen, "again": again.result(), "b": b, "orders": orders}}));
        pool.close().await;
    }});
}}
"#
    );
    build_rust_with(work, "whose", &[("book".to_string(), module)], &main).unwrap_or_else(|e| panic!("{e}"))
}

/// The Rust of whose transaction a call is builds, held to no warning, when there is no server of
/// the MySQL family to run it on.
#[test]
fn the_rust_of_whose_transaction_builds() {
    if !need(Need::Rustc) {
        return;
    }
    if let Err(why) = rust() {
        skip(&format!("{why}; the Rust of whose transaction a call is, is not built"));
        return;
    }
    let work = TempDir::new("whose-rust");
    let bin = whose_rust(work.path(), "chobo", "127.0.0.1", 3306, "root", "X'00000000000000000000000000000001'");
    assert!(bin.is_file());
}

/// With `batch`, a call sends what it sends in a row in one request: two requests a call, against
/// up to six one statement at a time. The answers are the same.
fn batch(my: &MySql, place: &Place, names: [&str; 6]) {
    let [book, receive, delivery, qty, reserve, order] = names;
    let script = |batch: bool| {
        format!(
            "const counted = {{ requests: 0 }};
const counting = {{ getConnection: async () => {{ const c = await pool.getConnection(); return {{ query: (o: any) => {{ counted.requests++; return c.query(o); }}, release: () => c.release() }}; }} }};
const b = {book}(counting, {{ tenant: \"batch-{batch}\", batch: {batch} }});
const out: string[] = [];
const step = async (f: () => Promise<any>) => {{ const before = counted.requests; const r = await f(); out.push(`${{r.result}}${{r.reason ? \" \" + r.reason : \"\"}} (${{counted.requests - before}})`); }};
await step(() => b.{receive}.do({{ {delivery}: \"n-1\", sku: \"A\", {qty}: 3n }} as any));
await step(() => b.{reserve}.hold({{ {order}: \"o-1\", sku: \"A\", {qty}: 2n }} as any));
await step(() => b.{reserve}.hold({{ {order}: \"o-2\", sku: \"A\", {qty}: 2n }} as any));
await step(() => b.{reserve}.hold({{ {order}: \"o-1\", sku: \"A\", {qty}: 2n }} as any));
await step(() => b.{reserve}.post({{ {order}: \"o-1\", sku: \"A\" }} as any, {{ {qty}: 1n }} as any));
await step(() => b.{reserve}.void({{ {order}: \"o-1\", sku: \"A\" }} as any));
console.log(JSON.stringify(out));"
        )
    };
    let one = ts(my, place, &[book], &script(false), false);
    let batched = ts(my, place, &[book], &script(true), true);
    let answers = |v: &Value| v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().split(" (").next().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(answers(&one), ["done", "done", "refused out_of_stock", "done_before", "done", "refused already_posted"], "{} {one}", my.label);
    assert_eq!(answers(&one), answers(&batched), "{}: {one} {batched}", my.label);
    eprintln!("batch on {}: one statement a request {one}; in one request a call {batched}", my.label);
}

/// A key and a tenant are the bytes of their IDs, never text the collation compares: keys apart by
/// a capital letter, a space at the end, or a voiced mark are three calls, and tenants apart by a
/// capital letter are two sets of balances.
fn keys_are_bytes(my: &MySql, place: &Place, names: [&str; 4]) {
    let [book, receive, delivery, qty] = names;
    let v = ts(
        my,
        place,
        &[book],
        &format!(
            "const b = {book}(pool, {{ tenant: \"shop\" }});
const B = {book}(pool, {{ tenant: \"Shop\" }});
const rs: string[] = [];
for (const d of [\"d-1\", \"D-1\", \"d-1 \", \"は\", \"ば\", \"ぱ\", \"ハ\"]) rs.push((await b.{receive}.do({{ {delivery}: d, sku: \"A\", {qty}: 1n }} as any)).result);
rs.push((await B.{receive}.do({{ {delivery}: \"d-1\", sku: \"A\", {qty}: 10n }} as any)).result);
const small = await b.balance.stock({{ sku: \"A\" }} as any);
const big = await B.balance.stock({{ sku: \"A\" }} as any);
console.log(JSON.stringify({{ rs, small: String(small.posted), big: String(big.posted) }}));"
        ),
        false,
    );
    assert_eq!(v["rs"], json!(["done", "done", "done", "done", "done", "done", "done", "done"]), "{} {v}", my.label);
    assert_eq!((v["small"].as_str(), v["big"].as_str()), (Some("7"), Some("10")), "{} {v}", my.label);
    eprintln!("keys on {}: keys apart by a capital, a trailing space or a voiced mark are three calls and more; tenants apart by a capital, two sets of balances", my.label);
}

/// The statements read the same whatever the session's sql_mode: identifiers in backquotes, the
/// caller's strings in hex. A key with a quote and a backslash goes through, and again.
fn sql_mode(my: &MySql, place: &Place, names: [&str; 4]) {
    let [book, receive, delivery, qty] = names;
    // Vitess's vtgate reads every statement itself, and lets no session set a mode that changes how
    // one is read (ANSI_QUOTES, NO_BACKSLASH_ESCAPES, PIPES_AS_CONCAT): there the keys are tried
    // under the mode it keeps
    let modes = if my.is_vitess() { "" } else { "ANSI_QUOTES,NO_BACKSLASH_ESCAPES,PIPES_AS_CONCAT" };
    let set = if modes.is_empty() { String::new() } else { format!("(pool as any).pool.on(\"connection\", (c: any) => {{ c.query(\"set session sql_mode = '{modes}'\"); }});\n") };
    let v = ts(
        my,
        place,
        &[book],
        &format!(
            "{set}const b = {book}(pool, {{ tenant: \"o'mode\\\\x\" }});
const rs: string[] = [];
for (const d of [\"it's\", \"back\\\\slash\", \"it's\"]) rs.push((await b.{receive}.do({{ {delivery}: d, sku: \"A\\\"B\", {qty}: 2n }} as any)).result);
const s = await b.balance.stock({{ sku: \"A\\\"B\" }} as any);
console.log(JSON.stringify({{ rs, posted: String(s.posted) }}));"
        ),
        false,
    );
    assert_eq!(v["rs"], json!(["done", "done", "done_before"]), "{} {v}", my.label);
    assert_eq!(v["posted"], "4", "{} {v}", my.label);
    let under = if modes.is_empty() { "the sql_mode it keeps".to_string() } else { modes.to_string() };
    eprintln!("sql_mode on {}: under {under}, quotes and backslashes in keys go through", my.label);
}

#[test]
fn what_only_the_mysql_family_has() {
    let all = servers("what only the MySQL family has");
    if all.is_empty() {
        return;
    }
    if node_mysql().is_err() {
        skip("node or mysql2 in tools/runner/node_modules is missing; what only the MySQL family has is not tried");
        return;
    }
    let work = TempDir::new("mysql-only");
    let books = ["stock_reservation", "to_itself", "wallet"];
    for (my, slug) in &all {
        let place = load(my, &books, work.path(), slug, true);
        no_deadlock(my, &place, "to_itself", ["stock_in", "move_stock", "warehouse", "slip", "from_place", "to_place", "place", "qty"]);
        checks_stop_writes(my, &place, ["stock_reservation", "receive", "delivery", "stock", "qty"], ["wallet", "deposit", "deposit_id", "wallet", "amount", "member"]);
        if my.label.starts_with("TiDB") {
            let bare = load(my, &["stock_reservation"], work.path(), &format!("{slug}-nocheck"), false);
            no_checks_without_the_variable(my, &bare, "stock_reservation", "receive", "delivery", "stock", "qty");
        }
        other_bounds(my, &place, ["stock_reservation", "receive", "delivery", "stock", "qty", "reserve", "order"]);
        expire(my, &place, ["stock_reservation", "receive", "delivery", "qty", "reserve", "order", "stock", "customers", "stock"]);
        contention(my, &place, ["stock_reservation", "receive", "delivery", "qty", "reserve", "order"]);
        caller_transaction(my, &place, ["stock_reservation", "receive", "delivery", "qty"]);
        whose_transaction(my, &place, work.path());
        batch(my, &place, ["stock_reservation", "receive", "delivery", "qty", "reserve", "order"]);
        keys_are_bytes(my, &place, ["stock_reservation", "receive", "delivery", "qty"]);
        sql_mode(my, &place, ["stock_reservation", "receive", "delivery", "qty"]);
    }
}

/// The twin of `what_only_the_mysql_family_has` with the Japanese books, whose tables are named in
/// Japanese: the steps that touch the names.
#[test]
fn what_only_the_mysql_family_has_in_japanese() {
    let all = servers("what only the MySQL family has, with the Japanese books,");
    if all.is_empty() {
        return;
    }
    if node_mysql().is_err() {
        skip("node or mysql2 in tools/runner/node_modules is missing; what only the MySQL family has is not tried with the Japanese books");
        return;
    }
    let work = TempDir::new("mysql-only-ja");
    let books = ["在庫", "自分あて", "ウォレット"];
    for (my, slug) in &all {
        let place = load(my, &books, work.path(), &format!("{slug}-ja"), true);
        no_deadlock(my, &place, "自分あて", ["入庫", "移し替え", "倉庫", "伝票", "元", "先", "場所", "数"]);
        checks_stop_writes(my, &place, ["在庫", "入荷", "納品書", "在庫", "数"], ["ウォレット", "入金", "入金番号", "ウォレット", "額", "会員"]);
        other_bounds(my, &place, ["在庫", "入荷", "納品書", "在庫", "数", "引当", "注文"]);
        expire(my, &place, ["在庫", "入荷", "納品書", "数", "引当", "注文", "在庫", "客", "在庫"]);
    }
}

/// A name MySQL cannot hold stops the build (E062): one longer than 64 characters, and one with a
/// character beyond the Basic Multilingual Plane, which MySQL would write as `?`.
#[test]
fn a_name_mysql_cannot_hold() {
    for (fixture, says) in [("tests/fixtures/mysql_name_length.book", "is 69 characters long"), ("tests/fixtures/名前の文字.book", "U+20BB7")] {
        let work = TempDir::new("e062");
        let out = chobo().args(["build", root().join(fixture).to_str().unwrap(), "--target", "mysql", "--out", work.path().to_str().unwrap()]).output().unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(1), "{fixture}: {err}");
        assert!(err.contains("error[E062]") && err.contains(says), "{fixture}: {err}");
        assert_eq!(std::fs::read_dir(work.path()).unwrap().count(), 0, "{fixture}: nothing is written");
    }
}

/// No comment of the SQL holds a `;`: vtgate cuts the query it is sent at every one, inside a
/// comment too, and answers the comment alone with "Query was empty" (1065).
#[test]
fn the_sql_holds_no_semicolon_in_a_comment() {
    for c in cases() {
        for (rel, text) in chobo::target::build(&c.book, &c.stem, Target::MySql, &c.origin).unwrap_or_else(|d| panic!("{}: {}", c.stem, d[0].message.en)) {
            for (n, line) in text.lines().enumerate() {
                assert!(!(line.trim_start().starts_with("--") && line.contains(';')), "{rel}:{}: a comment with a `;`: {line}", n + 1);
            }
        }
    }
}

#[allow(dead_code)]
fn _unused(_: Duration) {}
