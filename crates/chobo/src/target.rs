//! `chobo build --target …`: the seven targets (DESIGN 4.3) and the files each one writes.

use crate::client;
use crate::diag::Diag;
use crate::model::Book;
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Postgres,
    TigerBeetle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Sql,
    TypeScript,
    Python,
    Go,
}

impl Target {
    pub const ALL: [Target; 7] = [
        Target::Postgres,
        Target::PostgresTypeScript,
        Target::PostgresPython,
        Target::PostgresGo,
        Target::TigerBeetleTypeScript,
        Target::TigerBeetlePython,
        Target::TigerBeetleGo,
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
        }
    }

    pub fn parse(s: &str) -> Option<Target> {
        Target::ALL.into_iter().find(|t| t.name() == s)
    }

    pub fn backend(self) -> Backend {
        match self {
            Target::Postgres | Target::PostgresTypeScript | Target::PostgresPython | Target::PostgresGo => Backend::Postgres,
            _ => Backend::TigerBeetle,
        }
    }

    pub fn lang(self) -> Lang {
        match self {
            Target::Postgres => Lang::Sql,
            Target::PostgresTypeScript | Target::TigerBeetleTypeScript => Lang::TypeScript,
            Target::PostgresPython | Target::TigerBeetlePython => Lang::Python,
            Target::PostgresGo | Target::TigerBeetleGo => Lang::Go,
        }
    }
}

/// What a target needs of the book before it writes anything: the names PostgreSQL would cut
/// short (E061), or an operation that does not fit one TigerBeetle request (E060).
pub fn check(book: &Book, target: Target) -> Vec<Diag> {
    match target.backend() {
        Backend::Postgres => postgres::check_names(book),
        Backend::TigerBeetle => client::check_requests(book),
    }
}

/// The files a target writes, by their path under `--out`, or the diagnostics that stop it.
/// `stem` is the book's file name without `.book` (the Go package falls back on it).
pub fn build(book: &Book, stem: &str, target: Target) -> Result<Vec<(String, String)>, Vec<Diag>> {
    let d = check(book, target);
    if !d.is_empty() {
        return Err(d);
    }
    Ok(match target {
        Target::Postgres => vec![(format!("{}.sql", book.name), postgres::build(book)?)],
        Target::PostgresTypeScript => vec![(format!("{}.ts", book.name), client::typescript::postgres(book))],
        Target::TigerBeetleTypeScript => vec![(format!("{}.ts", book.name), client::typescript::tigerbeetle(book))],
        Target::PostgresPython => vec![(format!("{}.py", book.name), client::python::postgres(book))],
        Target::TigerBeetlePython => vec![(format!("{}.py", book.name), client::python::tigerbeetle(book))],
        Target::PostgresGo => client::go::postgres(book, stem),
        Target::TigerBeetleGo => client::go::tigerbeetle(book, stem),
    })
}
