// ── What follows is the same in every book chobo writes for MySQL, TiDB and Vitess ──
//
// A call is one transaction of the database's. The accounts it moves that have bounds are locked
// in the order of their IDs and read, and the read says, row by row, the first move whose bound it
// would break; the writes work the new balances out themselves. The SQL is the book's, written by
// chobo. This part fills in the values (IDs as hex, amounts as digits, the caller's strings as the
// hex of their UTF-8) and picks the next statement from what the last one answered.

use ::sqlx::Row as _;
use ::std::future::Future;

/// The text of each column of a row, None for null.
pub type Values = Vec<Option<String>>;

/// What a client needs of a database of the MySQL family: a connection for each call. sqlx's
/// `MySqlPool` gives one of its connections for each try of a call; a `tokio::sync::Mutex` of a
/// connection of sqlx's (a `&mut MySqlConnection`, a `Transaction`, a `PoolConnection`) gives that
/// connection, to one call at a time; a reference to, or an `Arc` of, any of these the same.
pub trait Database: Send + Sync {
    type Conn<'c>: Connection
    where
        Self: 'c;
    fn connection(&self) -> impl Future<Output = Result<Self::Conn<'_>, ::sqlx::Error>> + Send;
}

/// A connection the statements of a call are sent on.
pub trait Connection: Send {
    /// Run `sql`, one statement or several joined by `; ` in one request, and answer the rows of
    /// each statement, the text of each column.
    fn exec<'a>(&'a mut self, sql: &'a str) -> impl Future<Output = Result<Vec<Vec<Values>>, ::sqlx::Error>> + Send + 'a;
    /// Whether the connection is in a transaction of the caller's, which a call is then part of.
    fn in_transaction(&self) -> bool;
}

/// What a connection of sqlx's answers `sql`: the text protocol, which takes several statements in
/// one request, and gives each column as text.
async fn exec_on(conn: &mut ::sqlx::MySqlConnection, sql: &str) -> Result<Vec<Vec<Values>>, ::sqlx::Error> {
    let mut out: Vec<Vec<Values>> = vec![Vec::new()];
    let mut stream = ::sqlx::raw_sql(::sqlx::AssertSqlSafe(sql.to_string())).fetch_many(conn);
    while let Some(item) = ::std::future::poll_fn(|cx| stream.as_mut().poll_next(cx)).await {
        match item? {
            // a statement has ended: what follows is the next one's
            ::sqlx::Either::Left(_) => out.push(Vec::new()),
            ::sqlx::Either::Right(row) => {
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

impl Database for ::sqlx::MySqlPool {
    type Conn<'c> = ::sqlx::pool::PoolConnection<::sqlx::MySql>;
    fn connection(&self) -> impl Future<Output = Result<Self::Conn<'_>, ::sqlx::Error>> + Send {
        self.acquire()
    }
}

impl Connection for ::sqlx::pool::PoolConnection<::sqlx::MySql> {
    fn exec<'a>(&'a mut self, sql: &'a str) -> impl Future<Output = Result<Vec<Vec<Values>>, ::sqlx::Error>> + Send + 'a {
        exec_on(self, sql)
    }
    fn in_transaction(&self) -> bool {
        ::sqlx::Connection::is_in_transaction(&**self)
    }
}

impl<C> Database for ::tokio::sync::Mutex<C>
where
    C: ::std::ops::DerefMut<Target = ::sqlx::MySqlConnection> + Send,
{
    type Conn<'c>
        = ::tokio::sync::MutexGuard<'c, C>
    where
        Self: 'c;
    async fn connection(&self) -> Result<Self::Conn<'_>, ::sqlx::Error> {
        Ok(self.lock().await)
    }
}

impl<C> Connection for ::tokio::sync::MutexGuard<'_, C>
where
    C: ::std::ops::DerefMut<Target = ::sqlx::MySqlConnection> + Send,
{
    fn exec<'a>(&'a mut self, sql: &'a str) -> impl Future<Output = Result<Vec<Vec<Values>>, ::sqlx::Error>> + Send + 'a {
        exec_on(self, sql)
    }
    fn in_transaction(&self) -> bool {
        ::sqlx::Connection::is_in_transaction(&***self)
    }
}

impl<T: Database> Database for &T {
    type Conn<'c>
        = T::Conn<'c>
    where
        Self: 'c;
    fn connection(&self) -> impl Future<Output = Result<Self::Conn<'_>, ::sqlx::Error>> + Send {
        (**self).connection()
    }
}

impl<T: Database> Database for ::std::sync::Arc<T> {
    type Conn<'c>
        = T::Conn<'c>
    where
        Self: 'c;
    fn connection(&self) -> impl Future<Output = Result<Self::Conn<'_>, ::sqlx::Error>> + Send {
        (**self).connection()
    }
}

/// What a call answers. A refusal is an answer, not an error: nothing has changed, and the reason
/// says why.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Outcome {
    Done,
    DoneBefore,
    Refused(Reason),
}

impl Outcome {
    /// `done`, `done_before` or `refused`, as chobo writes a result.
    pub fn result(self) -> &'static str {
        match self {
            Outcome::Done => "done",
            Outcome::DoneBefore => "done_before",
            Outcome::Refused(_) => "refused",
        }
    }

    /// Why the call was refused; None when it was not.
    pub fn reason(self) -> Option<Reason> {
        match self {
            Outcome::Refused(r) => Some(r),
            _ => None,
        }
    }
}

impl ::std::fmt::Display for Outcome {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Outcome::Refused(r) => write!(f, "refused: {r}"),
            other => f.write_str(other.result()),
        }
    }
}

/// Where a hold is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HoldState {
    Held,
    Posted,
    Voided,
    Expired,
}

impl HoldState {
    /// `held`, `posted`, `voided` or `expired`.
    pub fn as_str(self) -> &'static str {
        match self {
            HoldState::Held => "held",
            HoldState::Posted => "posted",
            HoldState::Voided => "voided",
            HoldState::Expired => "expired",
        }
    }
}

impl ::std::fmt::Display for HoldState {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An account's balance: what is posted, what holds put in, and what holds take out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Balance {
    pub posted: i64,
    pub held_in: i64,
    pub held_out: i64,
}

/// Why a call has no answer: an argument the book does not take, the database's own error, a book
/// the database does not hold as written (`code` is CB001 when an account's row was made with
/// other bounds than the book's), or an answer the client does not expect.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    Argument(String),
    Database(::sqlx::Error),
    Book { code: &'static str, message: String },
    Answer(String),
}

impl Error {
    /// The error number of the database's error (1062, 1213 …).
    pub fn number(&self) -> Option<u16> {
        match self {
            Error::Database(::sqlx::Error::Database(d)) => d.try_downcast_ref::<::sqlx::mysql::MySqlDatabaseError>().map(|m| m.number()),
            _ => None,
        }
    }
}

impl ::std::fmt::Display for Error {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Error::Argument(m) | Error::Answer(m) | Error::Book { message: m, .. } => f.write_str(m),
            Error::Database(e) => write!(f, "{e}"),
        }
    }
}

impl ::std::error::Error for Error {
    fn source(&self) -> Option<&(dyn ::std::error::Error + 'static)> {
        match self {
            Error::Database(e) => Some(e),
            _ => None,
        }
    }
}

impl From<::sqlx::Error> for Error {
    fn from(e: ::sqlx::Error) -> Error {
        Error::Database(e)
    }
}

// The book's definitions, which chobo writes as constants above.

struct AccountDef {
    name: &'static str,
    bounded: bool,
    lower: Option<i64>,
    upper: Option<i64>,
    /// a row of `make_accounts`
    row: &'static str,
}

enum ArgDef {
    Param(usize),
    Lit(&'static str),
}

struct RefDef {
    kind: usize,
    args: &'static [ArgDef],
}

enum AmountDef {
    Param(usize),
    Lit(i64),
}

struct MoveDef {
    amount: AmountDef,
    from: RefDef,
    to: RefDef,
}

struct ParamDef {
    name: &'static str,
    amount: bool,
}

/// What a transfer kind's calls send; `lock` and `apply` are None when no move touches an account
/// with a bound. The end of a hold writes the hold's row, then the accounts' rows.
struct TransferSql {
    key: &'static str,
    lock: Option<&'static str>,
    checks: &'static [(usize, Reason, &'static str)],
    refuse: &'static str,
    entries: &'static str,
    apply: Option<&'static str>,
    post_apply: &'static [&'static str],
    post_entries: &'static str,
    void_apply: &'static [&'static str],
    void_entries: &'static str,
    expire_apply: &'static [&'static str],
    expire_entries: &'static str,
}

struct TransferDef {
    name: &'static str,
    params: &'static [ParamDef],
    key: &'static [usize],
    definition: &'static str,
    moves: &'static [MoveDef],
    sql: TransferSql,
}

struct BookSql {
    begin: &'static str,
    read_accounts: &'static str,
    make_accounts: &'static str,
    read_call: &'static str,
    hold_lock: &'static str,
    status: &'static str,
    balance: &'static str,
    due: &'static str,
    fold_list: &'static str,
    fold_lock: &'static str,
    fold_read: &'static str,
    fold_apply: &'static str,
    fold_mark: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Op {
    Do,
    Hold,
    Post,
    Void,
}

impl Op {
    fn name(self) -> &'static str {
        match self {
            Op::Do => "do",
            Op::Hold => "hold",
            Op::Post => "post",
            Op::Void => "void",
        }
    }
}

/// An argument of a call, in the order of the parameters.
#[derive(Clone, Copy)]
enum Value<'a> {
    Str(&'a str),
    Amt(i64),
}

impl Value<'_> {
    fn text(self) -> String {
        match self {
            Value::Str(s) => s.to_string(),
            Value::Amt(a) => a.to_string(),
        }
    }
}

/// How many times a call of its own transaction is made when the database gives up on it: about
/// 1.5 seconds of waits between them (DESIGN 4.5).
const ATTEMPTS: u32 = 30;

/// The errors after which such a call is made again with the same arguments (the key keeps it from
/// moving twice): a deadlock, a lock not got in time, and TiDB's write conflicts.
const AGAIN: [u16; 5] = [1213, 1205, 9007, 8022, 8028];

const DUPLICATE: u16 = 1062;

/// A wait before the next try: up to 1, 2, 4 … 128 ms, at random, so that the calls that failed
/// together do not meet again.
async fn backoff(attempt: u32) {
    use ::std::hash::{BuildHasher, Hasher};
    let most = 1_000_000u64 << (attempt - 1).min(7);
    let mut h = ::std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u32(attempt);
    ::tokio::time::sleep(::std::time::Duration::from_nanos(h.finish() % most)).await;
}

/// An ID: the first 16 bytes of a SHA-256 over the parts, each with its UTF-8 length in front, as
/// 32 hex digits; 0 and 2^128 - 1 are not IDs.
fn ch_id(parts: &[&str]) -> String {
    let mut bytes = Vec::new();
    for p in ::std::iter::once("chobo/1").chain(parts.iter().copied()) {
        bytes.extend_from_slice(&(p.len() as u32).to_be_bytes());
        bytes.extend_from_slice(p.as_bytes());
    }
    let d = sha256(&bytes);
    let mut first = [0u8; 16];
    first.copy_from_slice(&d[..16]);
    let v = match u128::from_be_bytes(first) {
        0 => 1,
        u128::MAX => u128::MAX - 1,
        v => v,
    };
    format!("{v:032x}")
}

/// SHA-256 (FIPS 180-4).
fn sha256(bytes: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74,
        0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d,
        0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e,
        0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
        0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut message = bytes.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&((bytes.len() as u64).wrapping_mul(8)).to_be_bytes());
    for chunk in message.chunks(64) {
        let mut w = [0u32; 64];
        for (word, four) in w.iter_mut().zip(chunk.chunks(4)) {
            *word = u32::from_be_bytes([four[0], four[1], four[2], four[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for (k, wi) in K.iter().zip(w.iter()) {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(*k).wrapping_add(*wi);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 32];
    for (four, word) in out.chunks_mut(4).zip(h) {
        four.copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// An ID, as 32 hex digits, as SQL.
fn id_lit(h: &str) -> String {
    format!("X'{h}'")
}

/// A string of the caller's as SQL: the hex of its UTF-8, read as utf8mb4, so that nothing in it
/// is ever read as SQL.
fn str_lit(s: &str) -> String {
    let hex: String = s.bytes().map(|b| format!("{b:02x}")).collect();
    format!("_utf8mb4 X'{hex}'")
}

/// IDs as an SQL list: each once, in the order of their hex.
fn id_list(ids: &[String]) -> String {
    let mut u: Vec<&String> = ids.iter().collect();
    u.sort();
    u.dedup();
    u.iter().map(|id| id_lit(id)).collect::<Vec<_>>().join(", ")
}

/// A string as JavaScript's JSON.stringify writes it (PLAN 0.3).
fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_strings(vs: &[String]) -> String {
    format!("[{}]", vs.iter().map(|v| json_string(v)).collect::<Vec<_>>().join(","))
}

/// The values put in a statement, by name.
type Vals = ::std::collections::HashMap<String, String>;

/// Put the values in a statement: each {NAME} by the value of NAME, in one pass.
fn fill(template: &str, values: &Vals) -> String {
    let mut out = String::with_capacity(template.len() + 64);
    let mut rest = template;
    while let Some(at) = rest.find('{') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let name_len = after.find('}').filter(|n| *n > 0 && after[..*n].chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'));
        match name_len.and_then(|n| values.get(&after[..n]).map(|v| (n, v))) {
            Some((n, v)) => {
                out.push_str(v);
                rest = &after[n + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn fill_all(templates: &[&str], values: &Vals) -> Vec<String> {
    templates.iter().map(|t| fill(t, values)).collect()
}

fn with(mut base: Vals, more: &[(&str, String)]) -> Vals {
    for (k, v) in more {
        base.insert(k.to_string(), v.clone());
    }
    base
}

/// What a do or hold is called with, as the key compares it: the arguments in the order of the
/// parameters.
fn content(t: &TransferDef, v: &[Value<'_>]) -> String {
    let fields: Vec<String> = t
        .params
        .iter()
        .zip(v)
        .map(|(p, x)| match x {
            Value::Amt(a) if p.amount => format!("{}:{a}", json_string(p.name)),
            other => format!("{}:{}", json_string(p.name), json_string(&other.text())),
        })
        .collect();
    format!("{{{}}}", fields.join(","))
}

/// One move of a hold, as the database keeps it.
struct HeldMove {
    from: String,
    to: String,
    amount: String,
}

/// The moves of a hold, from the JSON of its row: an array of objects whose values are strings.
fn held_moves(json: &str) -> Option<Vec<HeldMove>> {
    let cs: Vec<char> = json.chars().collect();
    let mut i = 0;
    let skip = |i: &mut usize| {
        while *i < cs.len() && cs[*i].is_whitespace() {
            *i += 1;
        }
    };
    let string = |i: &mut usize| -> Option<String> {
        if cs.get(*i) != Some(&'"') {
            return None;
        }
        *i += 1;
        let mut s = String::new();
        loop {
            match cs.get(*i)? {
                '"' => {
                    *i += 1;
                    return Some(s);
                }
                '\\' => {
                    *i += 1;
                    s.push(match cs.get(*i)? {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        c => *c,
                    });
                }
                c => s.push(*c),
            }
            *i += 1;
        }
    };
    let mut out = Vec::new();
    skip(&mut i);
    if cs.get(i) != Some(&'[') {
        return None;
    }
    i += 1;
    loop {
        skip(&mut i);
        match cs.get(i)? {
            ']' => return Some(out),
            ',' => i += 1,
            '{' => {
                i += 1;
                let (mut from, mut to, mut amount) = (None, None, None);
                loop {
                    skip(&mut i);
                    match cs.get(i)? {
                        '}' => {
                            i += 1;
                            break;
                        }
                        ',' => i += 1,
                        _ => {
                            let k = string(&mut i)?;
                            skip(&mut i);
                            if cs.get(i) != Some(&':') {
                                return None;
                            }
                            i += 1;
                            skip(&mut i);
                            let v = string(&mut i)?;
                            match k.as_str() {
                                "from" => from = Some(v),
                                "to" => to = Some(v),
                                "amount" => amount = Some(v),
                                _ => {}
                            }
                        }
                    }
                }
                out.push(HeldMove { from: from?, to: to?, amount: amount? });
            }
            _ => return None,
        }
    }
}

/// The statements of one call: in a transaction of the call's own (`own`), or in the caller's.
struct Tx<'c, K> {
    c: &'c mut K,
    own: bool,
    batch: bool,
}

impl<K: Connection> Tx<'_, K> {
    /// Run `stmts` in order and answer the rows of each: in one request (`batch`), or one request
    /// a statement, stopping at the first error.
    async fn exec(&mut self, stmts: &[String]) -> Result<Vec<Vec<Values>>, ::sqlx::Error> {
        if stmts.is_empty() {
            return Ok(Vec::new());
        }
        if self.batch && stmts.len() > 1 {
            return self.c.exec(&stmts.join("; ")).await;
        }
        let mut out = Vec::with_capacity(stmts.len());
        for s in stmts {
            let mut sets = self.c.exec(s).await?;
            out.push(if sets.is_empty() { Vec::new() } else { sets.swap_remove(0) });
        }
        Ok(out)
    }

    /// Statements outside the transaction: when the call has its own, before it starts.
    async fn plain(&mut self, stmts: &[String]) -> Result<Vec<Vec<Values>>, ::sqlx::Error> {
        self.exec(stmts).await
    }

    /// Statements of the transaction, which starts with the first of them when the call has its
    /// own, and ends after the last with commit when `commit` is set.
    async fn run(&mut self, begin: bool, commit: bool, stmts: Vec<String>) -> Result<Vec<Vec<Values>>, ::sqlx::Error> {
        let n = stmts.len();
        let first = usize::from(begin && self.own);
        let mut all = Vec::with_capacity(n + 2);
        if begin && self.own {
            all.push(SQL.begin.to_string());
        }
        all.extend(stmts);
        if commit && self.own {
            all.push("commit".to_string());
        }
        let mut rows = self.exec(&all).await?;
        rows.truncate(first + n);
        Ok(rows.split_off(first.min(rows.len())))
    }

    /// The end of a call that writes nothing more: its own transaction rolled back.
    async fn quit(&mut self) -> Result<(), ::sqlx::Error> {
        if self.own {
            self.exec(&["rollback".to_string()]).await?;
        }
        Ok(())
    }
}

/// The text of a column of a row: "" for null, or for a column the row does not have.
fn col(r: &Values, i: usize) -> &str {
    r.get(i).and_then(|v| v.as_deref()).unwrap_or("")
}

/// Whether a column of a row is there and not null.
fn has(r: &Values, i: usize) -> bool {
    r.get(i).is_some_and(|v| v.is_some())
}

fn int(s: &str) -> Result<i64, Error> {
    s.parse().map_err(|_| Error::Answer(format!("chobo: the database answered {s:?} for a number")))
}

/// An account a call names: its ID, its kind and its arguments.
struct Placed {
    id: String,
    kind: usize,
    args: Vec<String>,
}

struct Runtime<D> {
    db: D,
    tenant: String,
    batch: bool,
    base: Vals,
    /// the accounts this value has made sure of in a transaction of a call's own, where their rows
    /// were committed at once: rows are never removed, nor their bounds changed
    sure: ::std::sync::Mutex<::std::collections::HashSet<String>>,
}

impl<D: Database> Runtime<D> {
    fn new(db: D, tenant: &str, batch: bool) -> Runtime<D> {
        let mut base = Vals::new();
        base.insert("TENANT".into(), id_lit(&ch_id(&["tenant", BOOK, tenant])));
        base.insert("TENANT_NAME".into(), str_lit(tenant));
        Runtime { db, tenant: tenant.to_string(), batch, base, sure: ::std::sync::Mutex::new(::std::collections::HashSet::new()) }
    }

    fn sure(&self) -> ::std::sync::MutexGuard<'_, ::std::collections::HashSet<String>> {
        self.sure.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// What a try of a call came to: its answer, or None when it is to be made again on a new try
    /// (its own transaction, given up on by the database). A try that failed in a transaction of
    /// its own rolls it back.
    async fn settle<T, K: Connection>(&self, x: &mut Tx<'_, K>, got: Result<T, Error>, attempt: &mut u32) -> Result<Option<T>, Error> {
        let e = match got {
            Ok(v) => return Ok(Some(v)),
            Err(e) => e,
        };
        if !x.own {
            return Err(e);
        }
        let _ = x.exec(&["rollback".to_string()]).await;
        if e.number().is_some_and(|n| AGAIN.contains(&n)) && *attempt < ATTEMPTS {
            backoff(*attempt).await;
            *attempt += 1;
            return Ok(None);
        }
        Err(e)
    }

    /// One statement outside any transaction of the call's: its rows.
    async fn read(&self, stmt: String) -> Result<Vec<Values>, Error> {
        let mut attempt = 1;
        loop {
            let mut conn = self.db.connection().await?;
            let own = !conn.in_transaction();
            let mut x = Tx { c: &mut conn, own, batch: self.batch };
            let got = x.plain(::std::slice::from_ref(&stmt)).await.map(|mut sets| sets.pop().unwrap_or_default()).map_err(Error::Database);
            if let Some(rows) = self.settle(&mut x, got, &mut attempt).await? {
                return Ok(rows);
            }
        }
    }

    fn account(&self, r: &RefDef, v: &[Value<'_>]) -> Placed {
        let args: Vec<String> = r
            .args
            .iter()
            .map(|a| match a {
                ArgDef::Param(i) => v[*i].text(),
                ArgDef::Lit(s) => s.to_string(),
            })
            .collect();
        let mut parts: Vec<&str> = vec!["account", BOOK, &self.tenant, ACCOUNTS[r.kind].name];
        parts.extend(args.iter().map(String::as_str));
        Placed { id: ch_id(&parts), kind: r.kind, args }
    }

    fn call_id(&self, t: &TransferDef, op: Op, key: &[String]) -> String {
        let mut parts: Vec<&str> = vec!["transfer", BOOK, &self.tenant, t.name, op.name()];
        parts.extend(key.iter().map(String::as_str));
        parts.push("0");
        ch_id(&parts)
    }

    /// Make sure every account is there, with the book's bounds, before a call locks the ones with
    /// bounds: read them, and make the missing ones. In the caller's transaction, a row made goes
    /// with the caller's rollback, so every call there reads them all, and what it makes is not
    /// taken as sure.
    async fn make_sure<K: Connection>(&self, x: &mut Tx<'_, K>, accounts: &[&Placed]) -> Result<(), Error> {
        let mut want: Vec<&Placed> = Vec::new();
        {
            let sure = self.sure();
            for a in accounts {
                if (!x.own || !sure.contains(&a.id)) && !want.iter().any(|w| w.id == a.id) {
                    want.push(a);
                }
            }
        }
        if want.is_empty() {
            return Ok(());
        }
        let ids: Vec<String> = want.iter().map(|a| a.id.clone()).collect();
        let found = x.plain(&[fill(SQL.read_accounts, &with(self.base.clone(), &[("IDS", id_list(&ids))]))]).await?;
        let mut there: Vec<String> = Vec::new();
        for f in found.first().map(Vec::as_slice).unwrap_or(&[]) {
            let id = col(f, 0).to_ascii_lowercase();
            let Some(a) = want.iter().find(|a| a.id == id) else { continue };
            let bound = |i: usize| -> Result<Option<i64>, Error> { if has(f, i) { int(col(f, i)).map(Some) } else { Ok(None) } };
            let d = &ACCOUNTS[a.kind];
            if bound(1)? != d.lower || bound(2)? != d.upper {
                return Err(Error::Book { code: "CB001", message: format!("chobo: the account {}({}) of {BOOK} has other bounds in the database than in the book", d.name, a.args.join(", ")) });
            }
            there.push(id);
        }
        let mut missing: Vec<&&Placed> = want.iter().filter(|a| !there.contains(&a.id)).collect();
        missing.sort_by(|a, b| a.id.cmp(&b.id));
        if !missing.is_empty() {
            let rows: Vec<String> = missing.iter().map(|a| fill(ACCOUNTS[a.kind].row, &with(self.base.clone(), &[("ID", id_lit(&a.id)), ("ARGS", str_lit(&json_strings(&a.args)))]))).collect();
            let mut vals = Vals::new();
            vals.insert("ROWS".into(), rows.join(", "));
            x.plain(&[fill(SQL.make_accounts, &vals)]).await?;
        }
        if x.own {
            self.sure().extend(want.iter().map(|a| a.id.clone()));
        }
        Ok(())
    }

    /// do and hold: `v` has every argument, in the order of the parameters.
    async fn move_(&self, t: &TransferDef, op: Op, v: &[Value<'_>]) -> Result<Outcome, Error> {
        for x in v {
            match x {
                Value::Amt(a) if *a < 0 => return Err(Error::Argument(format!("chobo: an amount is {a}, and an amount is from 0 to 2^63 - 1"))),
                _ => {}
            }
        }
        let mut places: Vec<(Placed, Placed)> = Vec::new();
        for m in t.moves {
            let (from, to) = (self.account(&m.from, v), self.account(&m.to, v));
            // a move from an account to itself: refused before anything is sent, and the key is not used
            if from.id == to.id {
                return Ok(Outcome::Refused(Reason::SameAccount));
            }
            places.push((from, to));
        }
        let key: Vec<String> = t.key.iter().map(|k| v[*k].text()).collect();
        let was = content(t, v);
        let mut vals = with(self.base.clone(), &[("CALL", id_lit(&self.call_id(t, op, &key))), ("KEY", str_lit(&json_strings(&key))), ("CONTENT", str_lit(&was))]);
        let mut bounded: Vec<String> = Vec::new();
        for (i, (m, (from, to))) in t.moves.iter().zip(&places).enumerate() {
            let k = i + 1;
            vals.insert(format!("FROM{k}"), id_lit(&from.id));
            vals.insert(format!("TO{k}"), id_lit(&to.id));
            vals.insert(format!("FROMX{k}"), from.id.clone());
            vals.insert(format!("TOX{k}"), to.id.clone());
            let amount = match m.amount {
                AmountDef::Param(p) => v[p].text(),
                AmountDef::Lit(a) => a.to_string(),
            };
            vals.insert(format!("AMOUNT{k}"), amount);
            if ACCOUNTS[from.kind].bounded {
                bounded.push(from.id.clone());
            }
            if ACCOUNTS[to.kind].bounded {
                bounded.push(to.id.clone());
            }
        }
        vals.insert("BOUNDED".into(), id_list(&bounded));
        let mut locked = bounded.clone();
        locked.sort();
        locked.dedup();
        let all: Vec<&Placed> = places.iter().flat_map(|(f, to)| [f, to]).collect();
        let mut attempt = 1;
        loop {
            let mut conn = self.db.connection().await?;
            let own = !conn.in_transaction();
            let mut x = Tx { c: &mut conn, own, batch: self.batch };
            let got = self.move_in(&mut x, t, op, &vals, &was, &all, &locked).await;
            if let Some(o) = self.settle(&mut x, got, &mut attempt).await? {
                return Ok(o);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn move_in<K: Connection>(&self, x: &mut Tx<'_, K>, t: &TransferDef, op: Op, vals: &Vals, was: &str, all: &[&Placed], locked: &[String]) -> Result<Outcome, Error> {
        self.make_sure(x, all).await?;
        let mut first = vec![fill(t.sql.key, vals)];
        if let Some(lock) = t.sql.lock {
            first.push(fill(lock, vals));
        }
        let rows = match x.run(true, false, first).await {
            Ok(rows) => rows,
            Err(e) => {
                let e = Error::Database(e);
                if e.number() != Some(DUPLICATE) {
                    return Err(e);
                }
                // the key was used: by this call made before, or by another
                let used = x.run(false, false, vec![fill(SQL.read_call, vals)]).await?;
                x.quit().await?;
                let Some(u) = used.first().and_then(|r| r.first()) else {
                    return Err(Error::Answer(format!("chobo: the key of {}.{} is used, and its call is not there", t.name, op.name())));
                };
                return Ok(if col(u, 0) == "refused" {
                    Outcome::Refused(Reason::AlreadyRefused)
                } else if col(u, 2) == was && col(u, 3) == t.definition {
                    Outcome::DoneBefore
                } else {
                    Outcome::Refused(Reason::KeyConflict)
                });
            }
        };
        if let Some(lock) = t.sql.lock {
            let mut lock_rows = rows.get(1).cloned().unwrap_or_default();
            if lock_rows.len() != locked.len() {
                // an account this value made sure of is not there: its row was taken away outside
                // the clients; make it again, and lock again
                {
                    let mut sure = self.sure();
                    for id in locked {
                        sure.remove(id);
                    }
                }
                self.make_sure(x, all).await?;
                let relocked = x.run(false, false, vec![fill(lock, vals)]).await?;
                lock_rows = relocked.into_iter().next().unwrap_or_default();
                if lock_rows.len() != locked.len() {
                    return Err(Error::Answer(format!("chobo: {} of the {} accounts of {}.{} with bounds are in the database", lock_rows.len(), locked.len(), t.name, op.name())));
                }
            }
            let mut first = 0;
            for lr in &lock_rows {
                let n: usize = col(lr, 1).parse().map_err(|_| Error::Answer(format!("chobo: the check of {}.{} answered {:?}", t.name, op.name(), col(lr, 1))))?;
                if n > 0 && (first == 0 || n < first) {
                    first = n;
                }
            }
            if first > 0 {
                let Some((_, reason, sql)) = t.sql.checks.iter().find(|(n, _, _)| *n == first) else {
                    return Err(Error::Answer(format!("chobo: no check {first} of {}.{}", t.name, op.name())));
                };
                x.run(false, true, vec![fill(t.sql.refuse, &with(vals.clone(), &[("REASON", sql.to_string())]))]).await?;
                return Ok(Outcome::Refused(*reason));
            }
        }
        let mut writes = vec![fill(t.sql.entries, vals)];
        if let Some(apply) = t.sql.apply {
            writes.push(fill(apply, vals));
        }
        x.run(false, true, writes).await?;
        Ok(Outcome::Done)
    }

    /// The values of the end of a hold, from its moves as the database has them.
    fn ending(&self, t: &TransferDef, moves: &[HeldMove], posted: &[i64]) -> Vals {
        let mut vals = Vals::new();
        let mut bounded: Vec<String> = Vec::new();
        for (i, m) in moves.iter().enumerate() {
            let k = i + 1;
            vals.insert(format!("FROM{k}"), id_lit(&m.from));
            vals.insert(format!("TO{k}"), id_lit(&m.to));
            vals.insert(format!("HELD{k}"), m.amount.clone());
            vals.insert(format!("POSTED{k}"), posted.get(i).copied().unwrap_or(0).to_string());
            if let Some(d) = t.moves.get(i) {
                if ACCOUNTS[d.from.kind].bounded {
                    bounded.push(m.from.clone());
                }
                if ACCOUNTS[d.to.kind].bounded {
                    bounded.push(m.to.clone());
                }
            }
        }
        vals.insert("BOUNDED".into(), id_list(&bounded));
        vals
    }

    fn hold_vals(&self, t: &TransferDef, key: &[String]) -> Vals {
        with(self.base.clone(), &[("HOLD", id_lit(&self.call_id(t, Op::Hold, key)))])
    }

    /// post and void: `key` in the order of the key; `amounts` in the order of the amounts, or None
    /// for all of it.
    async fn end(&self, t: &TransferDef, op: Op, key: &[&str], amounts: Option<&[i64]>) -> Result<Outcome, Error> {
        for a in amounts.unwrap_or(&[]) {
            if *a < 0 {
                return Err(Error::Argument(format!("chobo: an amount is {a}, and an amount is from 0 to 2^63 - 1")));
            }
        }
        let key: Vec<String> = key.iter().map(|k| k.to_string()).collect();
        let hold = self.hold_vals(t, &key);
        let mut attempt = 1;
        loop {
            let mut conn = self.db.connection().await?;
            let own = !conn.in_transaction();
            let mut x = Tx { c: &mut conn, own, batch: self.batch };
            let got = self.end_in(&mut x, t, op, &key, amounts, &hold).await;
            if let Some(o) = self.settle(&mut x, got, &mut attempt).await? {
                return Ok(o);
            }
        }
    }

    async fn end_in<K: Connection>(&self, x: &mut Tx<'_, K>, t: &TransferDef, op: Op, key: &[String], amounts: Option<&[i64]>, hold: &Vals) -> Result<Outcome, Error> {
        let found = x.run(true, false, vec![fill(SQL.hold_lock, hold)]).await?;
        let Some(h) = found.first().and_then(|r| r.first()).filter(|h| col(h, 0) != "refused").cloned() else {
            x.quit().await?;
            return Ok(Outcome::Refused(Reason::NoSuchHold));
        };
        let (state, posted_json, ended, past) = (col(&h, 1), col(&h, 3), col(&h, 4), col(&h, 5));
        let moves = held_moves(col(&h, 2)).ok_or_else(|| Error::Answer(format!("chobo: the moves of a hold of {} are {:?}", t.name, col(&h, 2))))?;
        let holding: Vec<i64> = moves.iter().map(|m| int(&m.amount)).collect::<Result<_, _>>()?;
        let mut posted = vec![0i64; moves.len()];
        if op == Op::Post {
            match amounts {
                None => posted.copy_from_slice(&holding),
                Some(amounts) => {
                    let amount_params: Vec<usize> = (0..t.params.len()).filter(|i| t.params[*i].amount).collect();
                    for (p, m) in posted.iter_mut().zip(t.moves) {
                        *p = match m.amount {
                            AmountDef::Param(i) => amount_params.iter().position(|x| *x == i).map(|j| amounts[j]).unwrap_or(0),
                            AmountDef::Lit(a) => a,
                        };
                    }
                }
            }
        }
        let posted_text = format!("[{}]", posted.iter().map(|p| format!("\"{p}\"")).collect::<Vec<_>>().join(","));
        let answer = match (state, op) {
            ("posted", Op::Void) => Some(Outcome::Refused(Reason::AlreadyPosted)),
            ("posted", _) if has(&h, 3) && posted_json == posted_text && has(&h, 4) && ended == t.definition => Some(Outcome::DoneBefore),
            ("posted", _) => Some(Outcome::Refused(Reason::KeyConflict)),
            ("voided", Op::Post) => Some(Outcome::Refused(Reason::AlreadyVoided)),
            ("voided", _) if has(&h, 4) && ended == t.definition => Some(Outcome::DoneBefore),
            ("voided", _) => Some(Outcome::Refused(Reason::KeyConflict)),
            _ if state == "expired" || past == "1" => Some(Outcome::Refused(Reason::Expired)),
            _ if op == Op::Post && posted.iter().zip(&holding).any(|(p, h)| p > h) => Some(Outcome::Refused(Reason::OverHold)),
            _ => None,
        };
        if let Some(a) = answer {
            x.quit().await?;
            return Ok(a);
        }
        let mut vals = hold.clone();
        vals.extend(self.ending(t, &moves, &posted));
        vals.insert("CALL".into(), id_lit(&self.call_id(t, op, key)));
        vals.insert("KEY".into(), str_lit(&json_strings(key)));
        let (apply, entries) = if op == Op::Post { (t.sql.post_apply, t.sql.post_entries) } else { (t.sql.void_apply, t.sql.void_entries) };
        let mut writes = fill_all(apply, &vals);
        writes.push(fill(entries, &vals));
        x.run(false, true, writes).await?;
        Ok(Outcome::Done)
    }

    async fn status(&self, t: &TransferDef, key: &[&str]) -> Result<Option<HoldState>, Error> {
        let key: Vec<String> = key.iter().map(|k| k.to_string()).collect();
        let rows = self.read(fill(SQL.status, &self.hold_vals(t, &key))).await?;
        let Some(r) = rows.first() else { return Ok(None) };
        if col(r, 0) == "refused" || !has(r, 1) {
            return Ok(None);
        }
        Ok(match col(r, 1) {
            "held" if col(r, 2) == "1" => Some(HoldState::Expired),
            "held" => Some(HoldState::Held),
            "posted" => Some(HoldState::Posted),
            "voided" => Some(HoldState::Voided),
            "expired" => Some(HoldState::Expired),
            other => return Err(Error::Answer(format!("chobo: a hold of {} is {other:?}", t.name))),
        })
    }

    async fn balance(&self, kind: usize, args: &[&str]) -> Result<Balance, Error> {
        let mut parts: Vec<&str> = vec!["account", BOOK, &self.tenant, ACCOUNTS[kind].name];
        parts.extend(args.iter().copied());
        let rows = self.read(fill(SQL.balance, &with(self.base.clone(), &[("ID", id_lit(&ch_id(&parts)))]))).await?;
        let Some(r) = rows.first() else { return Ok(Balance::default()) };
        let past = || Error::Answer(format!("chobo: the balance of {}({}) is past a 64-bit integer", ACCOUNTS[kind].name, args.join(", ")));
        let num = |i: usize| col(r, i).parse::<i64>().map_err(|_| past());
        Ok(Balance { posted: num(0)?, held_in: num(1)?, held_out: num(2)? })
    }

    /// Give back what the holds past their expiry hold, each in a transaction of its own, and add
    /// the entries of the accounts without bounds to their rows; answers how many holds it ended.
    async fn expire(&self) -> Result<u64, Error> {
        let due = self.read(SQL.due.to_string()).await?;
        let mut ended = 0;
        for d in &due {
            let Some(t) = TRANSFERS.iter().find(|t| t.name == col(d, 2)) else { continue };
            let id = col(d, 1).to_ascii_lowercase();
            let mut hold = Vals::new();
            hold.insert("TENANT".into(), id_lit(&col(d, 0).to_ascii_lowercase()));
            hold.insert("HOLD".into(), id_lit(&id));
            let mut attempt = 1;
            loop {
                let mut conn = self.db.connection().await?;
                let own = !conn.in_transaction();
                let mut x = Tx { c: &mut conn, own, batch: self.batch };
                let got = self.expire_one(&mut x, t, &hold, &id, col(d, 3)).await;
                if let Some(done) = self.settle(&mut x, got, &mut attempt).await? {
                    if done {
                        ended += 1;
                    }
                    break;
                }
            }
        }
        let accounts = self.read(SQL.fold_list.to_string()).await?;
        for a in &accounts {
            let mut at = Vals::new();
            at.insert("TENANT".into(), id_lit(&col(a, 0).to_ascii_lowercase()));
            at.insert("ID".into(), id_lit(&col(a, 1).to_ascii_lowercase()));
            let mut attempt = 1;
            loop {
                let mut conn = self.db.connection().await?;
                let own = !conn.in_transaction();
                let mut x = Tx { c: &mut conn, own, batch: self.batch };
                let got = self.fold(&mut x, &at).await;
                if self.settle(&mut x, got, &mut attempt).await?.is_some() {
                    break;
                }
            }
        }
        Ok(ended)
    }

    async fn expire_one<K: Connection>(&self, x: &mut Tx<'_, K>, t: &TransferDef, hold: &Vals, id: &str, key: &str) -> Result<bool, Error> {
        let found = x.run(true, false, vec![fill(SQL.hold_lock, hold)]).await?;
        let Some(h) = found.first().and_then(|r| r.first()).filter(|h| col(h, 1) == "held" && col(h, 5) == "1").cloned() else {
            x.quit().await?;
            return Ok(false);
        };
        let moves = held_moves(col(&h, 2)).ok_or_else(|| Error::Answer(format!("chobo: the moves of a hold of {} are {:?}", t.name, col(&h, 2))))?;
        let mut vals = hold.clone();
        vals.extend(self.ending(t, &moves, &[]));
        vals.insert("CALL".into(), id_lit(&ch_id(&["expire", id])));
        vals.insert("KEY".into(), str_lit(key));
        let mut writes = fill_all(t.sql.expire_apply, &vals);
        writes.push(fill(t.sql.expire_entries, &vals));
        x.run(false, true, writes).await?;
        Ok(true)
    }

    async fn fold<K: Connection>(&self, x: &mut Tx<'_, K>, at: &Vals) -> Result<(), Error> {
        let got = x.run(true, false, vec![fill(SQL.fold_lock, at), fill(SQL.fold_read, at)]).await?;
        let entries = got.get(1).cloned().unwrap_or_default();
        if entries.is_empty() {
            x.quit().await?;
            return Ok(());
        }
        let mut sums = [0i128; 3];
        let mut marks = Vec::with_capacity(entries.len());
        for e in &entries {
            for (j, sum) in sums.iter_mut().enumerate() {
                let v: i128 = col(e, 2 + j).parse().map_err(|_| Error::Answer(format!("chobo: an entry is {:?}", col(e, 2 + j))))?;
                *sum += v;
            }
            marks.push(format!("(X'{}', {})", col(e, 0).to_ascii_lowercase(), col(e, 1)));
        }
        let mut apply = at.clone();
        apply.insert("POSTED".into(), sums[0].to_string());
        apply.insert("HELD_IN".into(), sums[1].to_string());
        apply.insert("HELD_OUT".into(), sums[2].to_string());
        let mut mark = at.clone();
        mark.insert("ROWS".into(), marks.join(", "));
        x.run(false, true, vec![fill(SQL.fold_apply, &apply), fill(SQL.fold_mark, &mark)]).await?;
        Ok(())
    }
}
