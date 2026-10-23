// ── What follows is the same in every book chobo writes for PostgreSQL ───────
//
// A call is one SQL function, which does it in one transaction. The function answers a refusal as
// a row; an error is a mistake in the call, or the database's own.

use ::sqlx::Row as _;
use ::std::future::Future;

/// A value a call passes a function: a string, an amount, or the null of an amount left out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Param {
    Text(String),
    Int(i64),
    Null,
}

/// What a client needs of PostgreSQL: the one row a statement answers, with its parameters bound
/// in order. sqlx's `PgPool` has it, each call on a connection of the pool; so does a
/// `tokio::sync::Mutex` of a connection of sqlx's (a `&mut PgConnection`, a `Transaction`, a
/// `PoolConnection`), each call on that connection, in the transaction it is in, one at a time;
/// and a reference to, or an `Arc` of, any of these.
pub trait Database: Send + Sync {
    fn row<'a>(&'a self, sql: &'static str, params: &'a [Param]) -> impl Future<Output = Result<::sqlx::postgres::PgRow, ::sqlx::Error>> + Send + 'a;
}

/// The statement, its parameters bound.
fn bind<'q>(sql: &'static str, params: &'q [Param]) -> ::sqlx::query::Query<'q, ::sqlx::Postgres, ::sqlx::postgres::PgArguments> {
    let mut q = ::sqlx::query(sql);
    for p in params {
        q = match p {
            Param::Text(s) => q.bind(s.as_str()),
            Param::Int(n) => q.bind(*n),
            Param::Null => q.bind(None::<i64>),
        };
    }
    q
}

impl Database for ::sqlx::PgPool {
    fn row<'a>(&'a self, sql: &'static str, params: &'a [Param]) -> impl Future<Output = Result<::sqlx::postgres::PgRow, ::sqlx::Error>> + Send + 'a {
        bind(sql, params).fetch_one(self)
    }
}

impl<C> Database for ::tokio::sync::Mutex<C>
where
    C: ::std::ops::DerefMut<Target = ::sqlx::PgConnection> + Send,
{
    async fn row<'a>(&'a self, sql: &'static str, params: &'a [Param]) -> Result<::sqlx::postgres::PgRow, ::sqlx::Error> {
        let mut c = self.lock().await;
        bind(sql, params).fetch_one(&mut **c).await
    }
}

impl<T: Database> Database for &T {
    fn row<'a>(&'a self, sql: &'static str, params: &'a [Param]) -> impl Future<Output = Result<::sqlx::postgres::PgRow, ::sqlx::Error>> + Send + 'a {
        (**self).row(sql, params)
    }
}

impl<T: Database> Database for ::std::sync::Arc<T> {
    fn row<'a>(&'a self, sql: &'static str, params: &'a [Param]) -> impl Future<Output = Result<::sqlx::postgres::PgRow, ::sqlx::Error>> + Send + 'a {
        (**self).row(sql, params)
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

/// Why a call has no answer: an argument the book does not take, the database's own error (a row
/// made under other bounds than the book's is CB001), or an answer chobo's SQL does not give.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    Argument(String),
    Database(::sqlx::Error),
    Answer(String),
}

impl Error {
    /// The SQLSTATE of the database's error.
    pub fn code(&self) -> Option<String> {
        match self {
            Error::Database(e) => sqlstate(e),
            _ => None,
        }
    }
}

impl ::std::fmt::Display for Error {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match self {
            Error::Argument(m) | Error::Answer(m) => f.write_str(m),
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

/// How many times a call is made when PostgreSQL answers a serialization failure or a deadlock:
/// about 1.5 seconds of waits between them (DESIGN 4.1).
const ATTEMPTS: u32 = 30;

/// A wait before the next try: up to 1, 2, 4 … 128 ms, at random, so that the calls that failed
/// together do not meet again.
async fn backoff(attempt: u32) {
    use ::std::hash::{BuildHasher, Hasher};
    let most = 1_000_000u64 << (attempt - 1).min(7);
    let mut h = ::std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u32(attempt);
    ::tokio::time::sleep(::std::time::Duration::from_nanos(h.finish() % most)).await;
}

fn sqlstate(e: &::sqlx::Error) -> Option<String> {
    e.as_database_error().and_then(|d| d.code()).map(|c| c.into_owned())
}

struct Runtime<D> {
    db: D,
    tenant: String,
}

impl<D: Database> Runtime<D> {
    fn new(db: D, tenant: &str) -> Runtime<D> {
        Runtime { db, tenant: tenant.to_string() }
    }

    /// The tenant, then `rest`: what a function is called with.
    fn params(&self, rest: Vec<Param>) -> Vec<Param> {
        let mut out = vec![Param::Text(self.tenant.clone())];
        out.extend(rest);
        out
    }

    /// The one row of `sql`. A serialization failure (40001) or a deadlock (40P01) is tried again
    /// with the same arguments: the key keeps the call from moving twice. Inside a transaction of
    /// the caller's, the second try finds the transaction aborted (25P02), and the first error is
    /// returned.
    async fn row(&self, sql: &'static str, params: &[Param]) -> Result<::sqlx::postgres::PgRow, Error> {
        let mut first: Option<::sqlx::Error> = None;
        let mut attempt = 1;
        loop {
            let e = match self.db.row(sql, params).await {
                Ok(row) => return Ok(row),
                Err(e) => e,
            };
            let code = sqlstate(&e);
            if first.is_some() && code.as_deref() == Some("25P02") {
                return Err(Error::Database(first.unwrap_or(e)));
            }
            if matches!(code.as_deref(), Some("40001" | "40P01")) && attempt < ATTEMPTS {
                first.get_or_insert(e);
                backoff(attempt).await;
                attempt += 1;
                continue;
            }
            return Err(Error::Database(e));
        }
    }

    async fn call(&self, sql: &'static str, rest: Vec<Param>) -> Result<Outcome, Error> {
        for p in &rest {
            match p {
                Param::Int(a) if *a < 0 => return Err(Error::Argument(format!("chobo: an amount is {a}, and an amount is from 0 to 2^63 - 1"))),
                _ => {}
            }
        }
        let row = self.row(sql, &self.params(rest)).await?;
        let result: String = row.try_get(0)?;
        let reason: Option<String> = row.try_get(1)?;
        match (result.as_str(), reason.as_deref().and_then(Reason::from_name)) {
            ("done", _) => Ok(Outcome::Done),
            ("done_before", _) => Ok(Outcome::DoneBefore),
            ("refused", Some(r)) => Ok(Outcome::Refused(r)),
            _ => Err(Error::Answer(format!("chobo: the function answered {result:?}, {reason:?}"))),
        }
    }

    async fn status(&self, sql: &'static str, rest: Vec<Param>) -> Result<Option<HoldState>, Error> {
        let row = self.row(sql, &self.params(rest)).await?;
        let state: Option<String> = row.try_get(0)?;
        match state.as_deref() {
            None => Ok(None),
            Some("held") => Ok(Some(HoldState::Held)),
            Some("posted") => Ok(Some(HoldState::Posted)),
            Some("voided") => Ok(Some(HoldState::Voided)),
            Some("expired") => Ok(Some(HoldState::Expired)),
            Some(other) => Err(Error::Answer(format!("chobo: the status function answered {other:?}"))),
        }
    }

    async fn balance(&self, sql: &'static str, rest: Vec<Param>) -> Result<Balance, Error> {
        let row = self.row(sql, &self.params(rest)).await?;
        Ok(Balance { posted: row.try_get(0)?, held_in: row.try_get(1)?, held_out: row.try_get(2)? })
    }

    /// Give back what the holds past their expiry hold; answers how many holds it ended.
    async fn expire(&self, sql: &'static str) -> Result<u64, Error> {
        let row = self.row(sql, &[]).await?;
        let n: i32 = row.try_get(0)?;
        u64::try_from(n).map_err(|_| Error::Answer(format!("chobo: expire() answered {n}")))
    }
}
