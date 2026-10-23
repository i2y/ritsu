//! Runs scenarios through the Rust clients chobo writes, for tests/backends.rs and the tests of
//! what only one database has. Its input and output are those of ../../runner.ts (see there).
//!
//! The test builds one binary of this crate and every generated module it runs, with a main.rs of
//! its own: it implements each module's traits (`Database`, `Connection`, `TigerBeetleClient`) for
//! the values here that write down what is sent (`PgRec`, `MyRec` and `MyConn`, `TbRec`), adapts
//! each module's typed calls to [`Book`], and calls [`main`].

use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tigerbeetle as tb;

/// What an adapter's call answers.
pub type Fut<'a, T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send + 'a>>;

/// A JSON object of the scenario: the arguments of a call, of an account.
pub type Args = Map<String, Value>;

/// A generated module's book value, called by the names of the book.
pub trait Book: Send + Sync {
    /// The call's result and reason ("" when it is not refused).
    fn call<'a>(&'a self, kind: &'a str, op: &'a str, args: &'a Args, amounts: Option<&'a Args>) -> Fut<'a, (String, String)>;
    fn balance<'a>(&'a self, account: &'a str, args: &'a Args) -> Fut<'a, (i64, i64, i64)>;
    /// Where the hold is; "" when there is no such hold.
    fn status<'a>(&'a self, kind: &'a str, args: &'a Args) -> Fut<'a, String>;
    fn expire(&self) -> Fut<'_, ()>;
}

/// How main.rs makes a book value of one generated module.
pub enum Entry {
    Postgres(fn(PgRec, &str) -> Box<dyn Book>),
    TigerBeetle(fn(TbRec, &str) -> Box<dyn Book>),
    MySql(fn(MyRec, &str, bool) -> Box<dyn Book>),
}

/// A string argument of the scenario.
pub fn s<'a>(m: &'a Args, k: &str) -> &'a str {
    m.get(k).and_then(Value::as_str).unwrap_or("")
}

/// An amount of the scenario.
pub fn a(m: &Args, k: &str) -> i64 {
    m.get(k).and_then(Value::as_i64).unwrap_or(0)
}

/// The requests of one operation, written down as the client sends them.
#[derive(Clone, Default)]
pub struct Recorder(Arc<Mutex<Option<Vec<Value>>>>);

impl Recorder {
    fn note(&self, v: Value) {
        if let Some(c) = self.0.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            c.push(v);
        }
    }

    fn start(&self) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(Vec::new());
    }

    fn take(&self) -> Vec<Value> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).take().unwrap_or_default()
    }
}

// ── PostgreSQL ──────────────────────────────────────────────────────────

/// A value a PostgreSQL client passes, as the harness sees it.
pub enum P {
    Text(String),
    Int(i64),
    Null,
}

/// The pool of the run, through which a book value's calls go, written down.
pub struct PgRec {
    pub pool: sqlx::PgPool,
    pub rec: Recorder,
}

impl PgRec {
    pub fn row<'a>(&'a self, sql: &'static str, ps: Vec<P>) -> impl Future<Output = Result<sqlx::postgres::PgRow, sqlx::Error>> + Send + 'a {
        let shown: Vec<Value> = ps
            .iter()
            .map(|p| match p {
                P::Text(s) => json!(s),
                P::Int(n) => json!(n.to_string()),
                P::Null => Value::Null,
            })
            .collect();
        self.rec.note(json!({"sql": sql, "params": shown}));
        async move {
            let mut q = sqlx::query(sql);
            for p in &ps {
                q = match p {
                    P::Text(s) => q.bind(s.as_str()),
                    P::Int(n) => q.bind(*n),
                    P::Null => q.bind(None::<i64>),
                };
            }
            q.fetch_one(&self.pool).await
        }
    }
}

// ── TigerBeetle ─────────────────────────────────────────────────────────

const ACCOUNT_FLAGS: [&str; 6] = ["linked", "debits_must_not_exceed_credits", "credits_must_not_exceed_debits", "history", "imported", "closed"];
const TRANSFER_FLAGS: [&str; 9] = ["linked", "pending", "post_pending_transfer", "void_pending_transfer", "balancing_debit", "balancing_credit", "closing_debit", "closing_credit", "imported"];

fn flag_names(names: &[&'static str], bits: u16) -> Vec<&'static str> {
    names.iter().enumerate().filter(|(i, _)| bits & (1 << i) != 0).map(|(_, n)| *n).collect()
}

fn hex(v: u128) -> String {
    format!("{v:032x}")
}

fn account_json(a: &tb::Account) -> Value {
    json!({"id": hex(a.id), "ledger": a.ledger, "code": a.code, "flags": flag_names(&ACCOUNT_FLAGS, a.flags.0), "user_data_128": hex(a.user_data_128), "user_data_64": a.user_data_64, "user_data_32": a.user_data_32})
}

fn transfer_json(x: &tb::Transfer) -> Value {
    json!({
        "id": hex(x.id), "debit_account_id": hex(x.debit_account_id), "credit_account_id": hex(x.credit_account_id), "amount": x.amount.to_string(),
        "pending_id": hex(x.pending_id), "user_data_128": hex(x.user_data_128), "user_data_64": x.user_data_64, "user_data_32": x.user_data_32,
        "timeout": x.timeout, "ledger": x.ledger, "code": x.code, "flags": flag_names(&TRANSFER_FLAGS, x.flags.0),
    })
}

/// A client of the run, through which a book value's requests go, written down.
pub struct TbRec {
    pub client: Arc<tb::Client>,
    pub rec: Recorder,
}

impl TbRec {
    pub fn create_accounts(&self, accounts: &[tb::Account]) -> impl Future<Output = Result<Vec<tb::CreateAccountResult>, tb::Error>> + Send {
        self.rec.note(json!({"create_accounts": accounts.iter().map(account_json).collect::<Vec<_>>()}));
        self.client.create_accounts(accounts)
    }

    pub fn create_transfers(&self, transfers: &[tb::Transfer]) -> impl Future<Output = Result<Vec<tb::CreateTransferResult>, tb::Error>> + Send {
        self.rec.note(json!({"create_transfers": transfers.iter().map(transfer_json).collect::<Vec<_>>()}));
        self.client.create_transfers(transfers)
    }

    pub fn lookup_accounts(&self, ids: &[u128]) -> impl Future<Output = Result<Vec<tb::Account>, tb::Error>> + Send {
        self.rec.note(json!({"lookup_accounts": ids.iter().map(|i| hex(*i)).collect::<Vec<_>>()}));
        self.client.lookup_accounts(ids)
    }

    pub fn lookup_transfers(&self, ids: &[u128]) -> impl Future<Output = Result<Vec<tb::Transfer>, tb::Error>> + Send {
        self.rec.note(json!({"lookup_transfers": ids.iter().map(|i| hex(*i)).collect::<Vec<_>>()}));
        self.client.lookup_transfers(ids)
    }
}

// ── the MySQL family ────────────────────────────────────────────────────

/// The pool of the run, from which a book value's calls take their connections.
pub struct MyRec {
    pub pool: sqlx::MySqlPool,
    pub rec: Recorder,
}

/// A connection of the pool, whose statements are written down as they are sent.
pub struct MyConn {
    pub conn: sqlx::pool::PoolConnection<sqlx::MySql>,
    pub rec: Recorder,
}

impl MyRec {
    pub async fn connection(&self) -> Result<MyConn, sqlx::Error> {
        Ok(MyConn { conn: self.pool.acquire().await?, rec: self.rec.clone() })
    }
}

impl MyConn {
    pub async fn exec(&mut self, sql: &str) -> Result<Vec<Vec<Vec<Option<String>>>>, sqlx::Error> {
        use sqlx::Row as _;
        self.rec.note(json!({"sql": sql}));
        let mut out: Vec<Vec<Vec<Option<String>>>> = vec![Vec::new()];
        let mut stream = sqlx::raw_sql(sqlx::AssertSqlSafe(sql.to_string())).fetch_many(&mut *self.conn);
        while let Some(item) = std::future::poll_fn(|cx| stream.as_mut().poll_next(cx)).await {
            match item? {
                sqlx::Either::Left(_) => out.push(Vec::new()),
                sqlx::Either::Right(row) => {
                    let mut values = Vec::with_capacity(row.len());
                    for i in 0..row.len() {
                        values.push(row.try_get_unchecked::<Option<String>, usize>(i)?);
                    }
                    if let Some(last) = out.last_mut() {
                        last.push(values);
                    }
                }
            }
        }
        out.pop();
        Ok(out)
    }

    pub fn in_transaction(&self) -> bool {
        sqlx::Connection::is_in_transaction(&*self.conn)
    }
}

// ── the input ───────────────────────────────────────────────────────────

struct BookIn {
    name: String,
    client: String,
    expiry: f64,
    keys: BTreeMap<String, Vec<String>>,
    scenarios: Vec<Value>,
}

/// What the run connects to.
enum Db {
    Postgres(sqlx::PgPool),
    TigerBeetle(Vec<Arc<tb::Client>>),
    MySql(sqlx::MySqlPool, bool),
}

struct Runner {
    db: Db,
    next: std::sync::atomic::AtomicUsize,
    /// one expire() at a time, as one job would call it: an expire() skips the holds another is
    /// giving back, and returns before that one commits
    expiring: tokio::sync::Mutex<()>,
}

impl Runner {
    fn book(&self, entry: &Entry, rec: Recorder, tenant: &str) -> Result<Box<dyn Book>, String> {
        match (entry, &self.db) {
            (Entry::Postgres(f), Db::Postgres(pool)) => Ok(f(PgRec { pool: pool.clone(), rec }, tenant)),
            (Entry::MySql(f), Db::MySql(pool, batch)) => Ok(f(MyRec { pool: pool.clone(), rec }, tenant, *batch)),
            (Entry::TigerBeetle(f), Db::TigerBeetle(clients)) => {
                let n = self.next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(f(TbRec { client: clients[n % clients.len()].clone(), rec }, tenant))
            }
            _ => Err("the client is not one of this database's".into()),
        }
    }
}

/// What a hold of a scenario still holds on TigerBeetle.
struct Pending {
    debit: String,
    amount: u128,
    timeout: u64,
}

struct Caller {
    rec: Recorder,
    book: Box<dyn Book>,
}

/// What one scenario's run comes to, as the runners write it.
#[derive(Default)]
struct Run {
    sent: Vec<Value>,
    done_holds: Vec<Instant>,
    /// when the first hold since the last `pass` was asked for (runner.ts says why)
    held_since: Option<Instant>,
    late: bool,
    pending: BTreeMap<String, Vec<Pending>>,
    debited: BTreeSet<String>,
}

fn hold_key(keys: &BTreeMap<String, Vec<String>>, kind: &str, args: &Args) -> String {
    let vals: Vec<Value> = keys.get(kind).map(|ks| ks.iter().map(|k| args.get(k).cloned().unwrap_or(Value::Null)).collect()).unwrap_or_default();
    format!("{kind}{}", Value::Array(vals))
}

async fn call(c: &Caller, run: &Mutex<Run>, keys: &BTreeMap<String, Vec<String>>, op: &Value, step: usize, caller_no: usize) -> Result<Value, String> {
    c.rec.start();
    let asked = Instant::now();
    let kind = op["kind"].as_str().unwrap_or("");
    let name = op["op"].as_str().unwrap_or("");
    let empty = Args::new();
    let args = op["args"].as_object().unwrap_or(&empty);
    let amounts = op.get("amounts").and_then(Value::as_object);
    let got = c.book.call(kind, name, args, amounts).await;
    let requests = c.rec.take();
    let (outcome, reason) = got.map_err(|e| format!("step {step}: {kind}.{name}: {e}"))?;
    let mut at = json!({"step": step, "op": name, "kind": kind, "requests": requests});
    if caller_no > 0 {
        at["caller"] = json!(caller_no);
    }
    let mut r = run.lock().unwrap_or_else(|e| e.into_inner());
    if outcome == "done" && name == "hold" {
        r.done_holds.push(Instant::now());
        r.held_since.get_or_insert(asked);
        let chain = requests.iter().filter_map(|q| q.get("create_transfers")).next_back().and_then(Value::as_array).cloned().unwrap_or_default();
        let mut ps = Vec::new();
        for x in chain {
            let flags: Vec<&str> = x["flags"].as_array().map(|fs| fs.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
            if flags.contains(&"pending") && x["user_data_32"] != 2 {
                let debit = x["debit_account_id"].as_str().unwrap_or("").to_string();
                r.debited.insert(debit.clone());
                ps.push(Pending { debit, amount: x["amount"].as_str().unwrap_or("0").parse().unwrap_or(0), timeout: x["timeout"].as_u64().unwrap_or(0) });
            }
        }
        r.pending.insert(hold_key(keys, kind, args), ps);
    }
    if outcome == "done" && (name == "post" || name == "void") {
        r.pending.remove(&hold_key(keys, kind, args));
    }
    r.sent.push(at);
    let mut res = json!({"op": name, "kind": kind, "result": outcome});
    if outcome == "refused" {
        res["reason"] = json!(reason);
    }
    Ok(res)
}

fn outlived(r: &mut Run, expiry: f64) {
    if r.held_since.is_some_and(|t| t.elapsed() >= Duration::from_secs_f64(expiry)) {
        r.late = true;
    }
}

async fn expire_all(runner: &Runner, main: &Caller, run: &Mutex<Run>, expiry: f64) -> Result<(), String> {
    let until = {
        let r = run.lock().unwrap_or_else(|e| e.into_inner());
        r.done_holds.iter().max().copied().unwrap_or_else(Instant::now) + Duration::from_secs_f64(expiry) + Duration::from_millis(300)
    };
    tokio::time::sleep(until.saturating_duration_since(Instant::now())).await;
    let clients = match &runner.db {
        Db::TigerBeetle(clients) => clients,
        _ => {
            let _one = runner.expiring.lock().await;
            return main.book.expire().await;
        }
    };
    let (ids, want) = {
        let mut r = run.lock().unwrap_or_else(|e| e.into_inner());
        let mut pending = std::mem::take(&mut r.pending);
        pending.retain(|_, ps| {
            ps.retain(|p| p.timeout == 0);
            !ps.is_empty()
        });
        let mut want: BTreeMap<String, u128> = r.debited.iter().map(|d| (d.clone(), 0)).collect();
        for p in pending.values().flatten() {
            *want.entry(p.debit.clone()).or_default() += p.amount;
        }
        r.pending = pending;
        let ids: Vec<u128> = r.debited.iter().filter_map(|d| u128::from_str_radix(d, 16).ok()).collect();
        (ids, want)
    };
    for tries in 0.. {
        let found = if ids.is_empty() { Vec::new() } else { clients[0].lookup_accounts(&ids).await.map_err(|e| e.to_string())? };
        if found.iter().all(|a| want.get(&hex(a.id)).copied().unwrap_or(0) == a.debits_pending) {
            return Ok(());
        }
        if tries > 200 {
            return Err("the holds past their expiry were not given back within 10 seconds".into());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Ok(())
}

async fn scenario(runner: Arc<Runner>, entry: &Entry, book: &BookIn, sc: &Value) -> Result<Value, String> {
    let tenant = sc["tenant"].as_str().unwrap_or("").to_string();
    let run = Mutex::new(Run::default());
    let caller = |runner: &Runner| -> Result<Caller, String> {
        let rec = Recorder::default();
        Ok(Caller { book: runner.book(entry, rec.clone(), &tenant)?, rec })
    };
    let main = caller(&runner)?;
    let mut steps = Vec::new();
    for (i, step) in sc["steps"].as_array().cloned().unwrap_or_default().iter().enumerate() {
        match step["op"].as_str() {
            Some("pass") => {
                outlived(&mut run.lock().unwrap_or_else(|e| e.into_inner()), book.expiry);
                expire_all(&runner, &main, &run, book.expiry).await?;
                run.lock().unwrap_or_else(|e| e.into_inner()).held_since = None;
                steps.push(json!({"op": "pass"}));
            }
            Some("together") => {
                let ops: Vec<Vec<Value>> = step["callers"].as_array().map(|cs| cs.iter().map(|c| c.as_array().cloned().unwrap_or_default()).collect()).unwrap_or_default();
                let callers: Vec<Caller> = ops.iter().map(|_| caller(&runner)).collect::<Result<_, _>>()?;
                let runs = ops.iter().zip(&callers).enumerate().map(|(ci, (ops, c))| {
                    let run = &run;
                    async move {
                        let mut rs = Vec::new();
                        for op in ops {
                            rs.push(call(c, run, &book.keys, op, i + 1, ci + 1).await?);
                        }
                        Ok::<Vec<Value>, String>(rs)
                    }
                });
                let outs = join_all(runs.collect()).await;
                let outs: Vec<Vec<Value>> = outs.into_iter().collect::<Result<_, _>>()?;
                steps.push(json!({"op": "together", "callers": outs}));
            }
            _ => steps.push(call(&main, &run, &book.keys, step, i + 1, 0).await?),
        }
    }
    let mut accounts = Vec::new();
    let empty = Args::new();
    for a in sc["accounts"].as_array().cloned().unwrap_or_default() {
        let name = a["account"].as_str().unwrap_or("");
        let params = a["params"].as_object().unwrap_or(&empty);
        let (posted, held_in, held_out) = main.book.balance(name, params).await?;
        accounts.push(json!({"account": name, "args": a["args"], "posted": posted, "held_in": held_in, "held_out": held_out}));
    }
    let mut holds = Vec::new();
    for h in sc["holds"].as_array().cloned().unwrap_or_default() {
        let kind = h["kind"].as_str().unwrap_or("");
        let params = h["args"].as_object().unwrap_or(&empty);
        let state = main.book.status(kind, params).await?;
        if !state.is_empty() {
            holds.push(json!({"kind": kind, "key": h["key"], "state": state}));
        }
    }
    let mut r = run.into_inner().unwrap_or_else(|e| e.into_inner());
    outlived(&mut r, book.expiry);
    Ok(json!({"tenant": tenant, "result": {"steps": steps, "accounts": accounts, "holds": holds}, "sent": r.sent, "late": r.late, "error": null}))
}

/// Run futures at the same time, each to its end; their outputs in order.
async fn join_all<F: Future>(fs: Vec<F>) -> Vec<F::Output> {
    let mut fs: Vec<Pin<Box<F>>> = fs.into_iter().map(Box::pin).collect();
    let mut outs: Vec<Option<F::Output>> = fs.iter().map(|_| None).collect();
    std::future::poll_fn(|cx| {
        let mut all = true;
        for (f, out) in fs.iter_mut().zip(outs.iter_mut()) {
            if out.is_none() {
                match f.as_mut().poll(cx) {
                    std::task::Poll::Ready(v) => *out = Some(v),
                    std::task::Poll::Pending => all = false,
                }
            }
        }
        if all { std::task::Poll::Ready(()) } else { std::task::Poll::Pending }
    })
    .await;
    outs.into_iter().map(|o| o.expect("every future ran to its end")).collect()
}

/// Run the scenarios the input (the first argument) names through the modules of `entries`.
pub fn main(entries: Vec<(&'static str, Entry)>) {
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(8).enable_all().build().expect("a runtime");
    match rt.block_on(run(entries)) {
        Ok(out) => println!("{out}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

async fn run(entries: Vec<(&'static str, Entry)>) -> Result<Value, String> {
    let path = std::env::args().nth(1).ok_or("the input is the first argument")?;
    let input: Value = serde_json::from_str(&std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?).map_err(|e| e.to_string())?;
    let backend = input["backend"].as_str().unwrap_or("");
    let db = match backend {
        "postgres" => {
            let c = &input["postgres"];
            let options = sqlx::postgres::PgConnectOptions::new()
                .socket(c["host"].as_str().unwrap_or(""))
                .port(c["port"].as_u64().unwrap_or(5432) as u16)
                .username(c["user"].as_str().unwrap_or(""))
                .database(c["database"].as_str().unwrap_or(""));
            Db::Postgres(sqlx::postgres::PgPoolOptions::new().max_connections(20).connect_with(options).await.map_err(|e| e.to_string())?)
        }
        "mysql" => {
            let c = &input["mysql"];
            // Vitess's vtgate lets no session set a mode that changes how a statement is read, and
            // the statements of chobo read the same under any
            let options = sqlx::mysql::MySqlConnectOptions::new()
                .host(c["host"].as_str().unwrap_or(""))
                .port(c["port"].as_u64().unwrap_or(3306) as u16)
                .username(c["user"].as_str().unwrap_or(""))
                .database(c["database"].as_str().unwrap_or(""))
                .pipes_as_concat(false);
            let pool = sqlx::mysql::MySqlPoolOptions::new().max_connections(40).connect_with(options).await.map_err(|e| e.to_string())?;
            Db::MySql(pool, c["batch"].as_bool().unwrap_or(false))
        }
        _ => {
            let c = &input["tigerbeetle"];
            let cluster: u128 = c["cluster"].as_str().unwrap_or("0").parse().map_err(|e| format!("the cluster: {e}"))?;
            let addresses: Vec<&str> = c["addresses"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
            let mut clients = Vec::new();
            for _ in 0..4 {
                clients.push(Arc::new(tb::Client::new(cluster, &addresses.join(",")).map_err(|e| e.to_string())?));
            }
            Db::TigerBeetle(clients)
        }
    };
    let runner = Arc::new(Runner { db, next: Default::default(), expiring: tokio::sync::Mutex::new(()) });
    let entries: Arc<BTreeMap<&'static str, Entry>> = Arc::new(entries.into_iter().collect());
    let mut books_in = Vec::new();
    for b in input["books"].as_array().cloned().unwrap_or_default() {
        books_in.push(Arc::new(BookIn {
            name: b["name"].as_str().unwrap_or("").to_string(),
            client: b["client"].as_str().unwrap_or("").to_string(),
            expiry: b["expiry"].as_f64().unwrap_or(0.0),
            keys: serde_json::from_value(b["keys"].clone()).unwrap_or_default(),
            scenarios: b["scenarios"].as_array().cloned().unwrap_or_default(),
        }));
    }
    let mut tasks = Vec::new();
    for book in &books_in {
        if !entries.contains_key(book.client.as_str()) {
            return Err(format!("the runner has no module {:?}", book.client));
        }
        let mut handles = Vec::new();
        for si in 0..book.scenarios.len() {
            let (runner, entries, book) = (runner.clone(), entries.clone(), book.clone());
            handles.push(tokio::spawn(async move {
                let sc = &book.scenarios[si];
                let entry = &entries[book.client.as_str()];
                match scenario(runner, entry, &book, sc).await {
                    Ok(v) => v,
                    Err(e) => json!({"tenant": sc["tenant"], "result": null, "sent": [], "error": e}),
                }
            }));
        }
        tasks.push((book.name.clone(), handles));
    }
    let mut books = Vec::new();
    for (name, handles) in tasks {
        let mut scenarios = Vec::new();
        for h in handles {
            scenarios.push(h.await.map_err(|e| e.to_string())?);
        }
        books.push(json!({"name": name, "scenarios": scenarios}));
    }
    if let Db::Postgres(pool) = &runner.db {
        pool.close().await;
    }
    if let Db::MySql(pool, _) = &runner.db {
        pool.close().await;
    }
    Ok(json!({"books": books}))
}
