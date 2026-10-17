//! `chobo build --target …`: the eleven targets (DESIGN 4.3, 4.5) and the files each one writes.

use crate::client;
use crate::diag::Diag;
use crate::model::Book;
use crate::mysql;
use crate::postgres;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Target {
    Postgres,
    PostgresTypeScript,
    PostgresPython,
    PostgresGo,
    TigerBeetleTypeScript,
    TigerBeetlePython,
    TigerBeetleGo,
    MySql,
    MySqlTypeScript,
    MySqlPython,
    MySqlGo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Postgres,
    TigerBeetle,
    /// MySQL, TiDB and Vitess (DESIGN 4.5)
    MySql,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Sql,
    TypeScript,
    Python,
    Go,
}

impl Target {
    pub const ALL: [Target; 11] = [
        Target::Postgres,
        Target::PostgresTypeScript,
        Target::PostgresPython,
        Target::PostgresGo,
        Target::TigerBeetleTypeScript,
        Target::TigerBeetlePython,
        Target::TigerBeetleGo,
        Target::MySql,
        Target::MySqlTypeScript,
        Target::MySqlPython,
        Target::MySqlGo,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Target::Postgres => "postgres",
            Target::PostgresTypeScript => "postgres-typescript",
            Target::PostgresPython => "postgres-python",
            Target::PostgresGo => "postgres-go",
            Target::TigerBeetleTypeScript => "tigerbeetle-typescript",
            Target::TigerBeetlePython => "tigerbeetle-python",
            Target::TigerBeetleGo => "tigerbeetle-go",
            Target::MySql => "mysql",
            Target::MySqlTypeScript => "mysql-typescript",
            Target::MySqlPython => "mysql-python",
            Target::MySqlGo => "mysql-go",
        }
    }

    pub fn parse(s: &str) -> Option<Target> {
        Target::ALL.into_iter().find(|t| t.name() == s)
    }

    pub fn backend(self) -> Backend {
        match self {
            Target::Postgres | Target::PostgresTypeScript | Target::PostgresPython | Target::PostgresGo => Backend::Postgres,
            Target::MySql | Target::MySqlTypeScript | Target::MySqlPython | Target::MySqlGo => Backend::MySql,
            _ => Backend::TigerBeetle,
        }
    }

    pub fn lang(self) -> Lang {
        match self {
            Target::Postgres | Target::MySql => Lang::Sql,
            Target::PostgresTypeScript | Target::TigerBeetleTypeScript | Target::MySqlTypeScript => Lang::TypeScript,
            Target::PostgresPython | Target::TigerBeetlePython | Target::MySqlPython => Lang::Python,
            Target::PostgresGo | Target::TigerBeetleGo | Target::MySqlGo => Lang::Go,
        }
    }
}

/// What a target needs of the book before it writes anything: the names PostgreSQL would cut
/// short (E061), an operation that does not fit one TigerBeetle request (E060), or the names MySQL
/// cannot hold as written (E062).
pub fn check(book: &Book, target: Target) -> Vec<Diag> {
    match target.backend() {
        Backend::Postgres => postgres::check_names(book),
        Backend::TigerBeetle => client::check_requests(book),
        Backend::MySql => mysql::check_names(book),
    }
}

/// The files a target writes, by their path under `--out`, or the diagnostics that stop it.
/// `stem` is the book's file name without `.book` (the Go package falls back on it); `origin` is
/// the `.book` as the head of each file names it (ritsu's DESIGN 9.2).
pub fn build(book: &Book, stem: &str, target: Target, origin: &Origin) -> Result<Vec<(String, String)>, Vec<Diag>> {
    let d = check(book, target);
    if !d.is_empty() {
        return Err(d);
    }
    Ok(match target {
        Target::Postgres => vec![(format!("{}.sql", book.name), postgres::build(book, origin)?)],
        Target::PostgresTypeScript => vec![(format!("{}.ts", book.name), client::typescript::postgres(book, origin))],
        Target::TigerBeetleTypeScript => vec![(format!("{}.ts", book.name), client::typescript::tigerbeetle(book, origin))],
        Target::PostgresPython => vec![(format!("{}.py", book.name), client::python::postgres(book, origin))],
        Target::TigerBeetlePython => vec![(format!("{}.py", book.name), client::python::tigerbeetle(book, origin))],
        Target::PostgresGo => client::go::postgres(book, stem, origin),
        Target::TigerBeetleGo => client::go::tigerbeetle(book, stem, origin),
        Target::MySql => vec![(format!("{}.sql", book.name), mysql::build(book, origin)?)],
        Target::MySqlTypeScript => vec![(format!("{}.ts", book.name), client::typescript::mysql(book, origin))],
        Target::MySqlPython => vec![(format!("{}.py", book.name), client::python::mysql(book, origin))],
        Target::MySqlGo => client::go::mysql(book, stem, origin),
    })
}

pub use ritsu_emit::header::Origin;
