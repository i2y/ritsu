//! `chobo build --target …`: the fourteen targets (DESIGN 4.3, 4.5, 14) and the files each one writes.

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
    PostgresRust,
    TigerBeetleTypeScript,
    TigerBeetlePython,
    TigerBeetleGo,
    TigerBeetleRust,
    MySql,
    MySqlTypeScript,
    MySqlPython,
    MySqlGo,
    MySqlRust,
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
    Rust,
}

impl Target {
    pub const ALL: [Target; 14] = [
        Target::Postgres,
        Target::PostgresTypeScript,
        Target::PostgresPython,
        Target::PostgresGo,
        Target::PostgresRust,
        Target::TigerBeetleTypeScript,
        Target::TigerBeetlePython,
        Target::TigerBeetleGo,
        Target::TigerBeetleRust,
        Target::MySql,
        Target::MySqlTypeScript,
        Target::MySqlPython,
        Target::MySqlGo,
        Target::MySqlRust,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Target::Postgres => "postgres",
            Target::PostgresTypeScript => "postgres-typescript",
            Target::PostgresPython => "postgres-python",
            Target::PostgresGo => "postgres-go",
            Target::PostgresRust => "postgres-rust",
            Target::TigerBeetleTypeScript => "tigerbeetle-typescript",
            Target::TigerBeetlePython => "tigerbeetle-python",
            Target::TigerBeetleGo => "tigerbeetle-go",
            Target::TigerBeetleRust => "tigerbeetle-rust",
            Target::MySql => "mysql",
            Target::MySqlTypeScript => "mysql-typescript",
            Target::MySqlPython => "mysql-python",
            Target::MySqlGo => "mysql-go",
            Target::MySqlRust => "mysql-rust",
        }
    }

    pub fn parse(s: &str) -> Option<Target> {
        Target::ALL.into_iter().find(|t| t.name() == s)
    }

    pub fn backend(self) -> Backend {
        match self {
            Target::Postgres | Target::PostgresTypeScript | Target::PostgresPython | Target::PostgresGo | Target::PostgresRust => Backend::Postgres,
            Target::MySql | Target::MySqlTypeScript | Target::MySqlPython | Target::MySqlGo | Target::MySqlRust => Backend::MySql,
            _ => Backend::TigerBeetle,
        }
    }

    pub fn lang(self) -> Lang {
        match self {
            Target::Postgres | Target::MySql => Lang::Sql,
            Target::PostgresTypeScript | Target::TigerBeetleTypeScript | Target::MySqlTypeScript => Lang::TypeScript,
            Target::PostgresPython | Target::TigerBeetlePython | Target::MySqlPython => Lang::Python,
            Target::PostgresGo | Target::TigerBeetleGo | Target::MySqlGo => Lang::Go,
            Target::PostgresRust | Target::TigerBeetleRust | Target::MySqlRust => Lang::Rust,
        }
    }
}

/// What a target needs of the book before it writes anything: the names PostgreSQL would cut
/// short (E061), an operation that does not fit one TigerBeetle request (E060), the names MySQL
/// cannot hold as written (E062), and for a client in Rust, the names Rust takes as no identifier
/// (E063).
pub fn check(book: &Book, target: Target) -> Vec<Diag> {
    let mut d = match target.backend() {
        Backend::Postgres => postgres::check_names(book),
        Backend::TigerBeetle => client::check_requests(book),
        Backend::MySql => mysql::check_names(book),
    };
    if target.lang() == Lang::Rust {
        d.extend(client::rust::check_names(book));
        crate::model::sort(&mut d);
    }
    d
}

/// The files a target writes, by their path under `--out`, or the diagnostics that stop it.
/// `stem` is the book's file name without `.book` (the Go package falls back on it); `origin` is
/// the `.book` as the head of each file names it (ritsu's DESIGN 9.2).
pub fn build(book: &Book, stem: &str, target: Target, origin: &Origin) -> Result<Vec<(String, String)>, Vec<Diag>> {
    build_in(book, stem, target, origin, None)
}

/// [`build`], for a client written into a package of ritsu's when `in_pkg` says so (DESIGN 4.3,
/// ritsu's DESIGN 9.3): its amounts of money and of the quantities are then the package's types of
/// their units. The SQL is the same either way.
pub fn build_in(book: &Book, stem: &str, target: Target, origin: &Origin, in_pkg: Option<&client::InPackage>) -> Result<Vec<(String, String)>, Vec<Diag>> {
    let d = check(book, target);
    if !d.is_empty() {
        return Err(d);
    }
    Ok(match target {
        Target::Postgres => vec![(format!("{}.sql", book.name), postgres::build(book, origin)?)],
        Target::PostgresTypeScript => vec![(format!("{}.ts", book.name), client::typescript::postgres(book, origin, in_pkg))],
        Target::TigerBeetleTypeScript => vec![(format!("{}.ts", book.name), client::typescript::tigerbeetle(book, origin, in_pkg))],
        Target::PostgresPython => vec![(format!("{}.py", book.name), client::python::postgres(book, origin, in_pkg))],
        Target::TigerBeetlePython => vec![(format!("{}.py", book.name), client::python::tigerbeetle(book, origin, in_pkg))],
        Target::PostgresGo => client::go::postgres(book, stem, origin, in_pkg),
        Target::TigerBeetleGo => client::go::tigerbeetle(book, stem, origin, in_pkg),
        Target::MySql => vec![(format!("{}.sql", book.name), mysql::build(book, origin)?)],
        Target::MySqlTypeScript => vec![(format!("{}.ts", book.name), client::typescript::mysql(book, origin, in_pkg))],
        Target::MySqlPython => vec![(format!("{}.py", book.name), client::python::mysql(book, origin, in_pkg))],
        Target::MySqlGo => client::go::mysql(book, stem, origin, in_pkg),
        Target::PostgresRust => client::rust::postgres(book, stem, origin, in_pkg),
        Target::TigerBeetleRust => client::rust::tigerbeetle(book, stem, origin, in_pkg),
        Target::MySqlRust => client::rust::mysql(book, stem, origin, in_pkg),
    })
}

pub use ritsu_emit::header::Origin;
