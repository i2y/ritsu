//! `chobo gen --target <language> --books <database>`: the fourteen pairs of a language and the
//! database of the books (DESIGN 4.3, 4.5, 14, 17), and the files each one writes. A pair is what a
//! check and a build are of; the command line names its two halves, `--target` the language (any
//! number of them) and `--books` the database (one).

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

/// The values of `--target`: the languages, in the order `gen` writes them, whatever the order of
/// the flags (DESIGN 17).
pub const LANGS: [&str; 5] = ["sql", "typescript", "python", "go", "rust"];

/// The values of `--books`: the databases, `postgres` when it is not given (as `ritsu gen`'s).
pub const BOOKS: [&str; 3] = ["postgres", "tigerbeetle", "mysql"];

/// The pairs `--target` took up to 0.40.0, each with the flags that say it now (DESIGN 17): given
/// one, `gen` stops and says what to write. `postgres` and `mysql` were the SQL.
pub const RENAMED: [(&str, &str, &str); 14] = [
    ("postgres", "--target sql", "0.41.0"),
    ("postgres-typescript", "--target typescript --books postgres", "0.41.0"),
    ("postgres-python", "--target python --books postgres", "0.41.0"),
    ("postgres-go", "--target go --books postgres", "0.41.0"),
    ("postgres-rust", "--target rust --books postgres", "0.41.0"),
    ("tigerbeetle-typescript", "--target typescript --books tigerbeetle", "0.41.0"),
    ("tigerbeetle-python", "--target python --books tigerbeetle", "0.41.0"),
    ("tigerbeetle-go", "--target go --books tigerbeetle", "0.41.0"),
    ("tigerbeetle-rust", "--target rust --books tigerbeetle", "0.41.0"),
    ("mysql", "--target sql --books mysql", "0.41.0"),
    ("mysql-typescript", "--target typescript --books mysql", "0.41.0"),
    ("mysql-python", "--target python --books mysql", "0.41.0"),
    ("mysql-go", "--target go --books mysql", "0.41.0"),
    ("mysql-rust", "--target rust --books mysql", "0.41.0"),
];

impl Lang {
    pub const ALL: [Lang; 5] = [Lang::Sql, Lang::TypeScript, Lang::Python, Lang::Go, Lang::Rust];

    /// Its value of `--target`, which is also the directory under `--out` its files go in.
    pub fn key(self) -> &'static str {
        match self {
            Lang::Sql => "sql",
            Lang::TypeScript => "typescript",
            Lang::Python => "python",
            Lang::Go => "go",
            Lang::Rust => "rust",
        }
    }

    pub fn parse(s: &str) -> Option<Lang> {
        Lang::ALL.into_iter().find(|l| l.key() == s)
    }
}

impl Backend {
    pub const ALL: [Backend; 3] = [Backend::Postgres, Backend::TigerBeetle, Backend::MySql];

    /// Its value of `--books`.
    pub fn key(self) -> &'static str {
        match self {
            Backend::Postgres => "postgres",
            Backend::TigerBeetle => "tigerbeetle",
            Backend::MySql => "mysql",
        }
    }

    pub fn parse(s: &str) -> Option<Backend> {
        Backend::ALL.into_iter().find(|b| b.key() == s)
    }
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

    /// The pair's name: the playground's list of the fourteen, and the tests' names of them. What
    /// a person reads names the pair by its flags ([`Target::flags`]).
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

    /// The pair of a language and a database: none for SQL on TigerBeetle, which takes no SQL.
    pub fn of(lang: Lang, books: Backend) -> Option<Target> {
        Target::ALL.into_iter().find(|t| t.lang() == lang && t.backend() == books)
    }

    /// The flags of `gen` that write it: `--target go --books postgres`.
    pub fn flags(self) -> String {
        format!("--target {} --books {}", self.lang().key(), self.backend().key())
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
/// cannot hold as written (E062), for a client in Rust, the names Rust takes as no identifier
/// (E063), and the names the target spells as one identifier or cannot take as they are (E064,
/// what `check` warns of with W108).
pub fn check(book: &Book, target: Target) -> Vec<Diag> {
    check_in(book, target, false)
}

/// [`check`], for the client a package of ritsu's holds when `in_package` (ritsu's DESIGN 9.3),
/// where the Python module's name is the package's (`<package>.books.<name>`).
pub fn check_in(book: &Book, target: Target, in_package: bool) -> Vec<Diag> {
    let mut d = match target.backend() {
        Backend::Postgres => postgres::check_names(book),
        Backend::TigerBeetle => client::check_requests(book),
        Backend::MySql => mysql::check_names(book),
    };
    let names = crate::spellings::check_target(book, target, in_package);
    if target.lang() == Lang::Rust || !names.is_empty() {
        if target.lang() == Lang::Rust {
            d.extend(client::rust::check_names(book));
        }
        d.extend(names);
        crate::model::sort(&mut d);
    }
    d
}

/// The files a target writes, by their path under its directory (`<out>/<language>/`), or the
/// diagnostics that stop it.
/// `stem` is the book's file name without `.book` (the Go package falls back on it); `origin` is
/// the `.book` as the head of each file names it (ritsu's DESIGN 9.2).
pub fn build(book: &Book, stem: &str, target: Target, origin: &Origin) -> Result<Vec<(String, String)>, Vec<Diag>> {
    build_in(book, stem, target, origin, None)
}

/// [`build`], for a client written into a package of ritsu's when `in_pkg` says so (DESIGN 4.3,
/// ritsu's DESIGN 9.3): its amounts of money and of the quantities are then the package's types of
/// their units. The SQL is the same either way.
pub fn build_in(book: &Book, stem: &str, target: Target, origin: &Origin, in_pkg: Option<&client::InPackage>) -> Result<Vec<(String, String)>, Vec<Diag>> {
    let d = check_in(book, target, in_pkg.is_some());
    if !d.is_empty() {
        return Err(d);
    }
    unchecked(book, stem, target, origin, in_pkg)
}

/// The files of [`build_in`] without [`check`]: what the target would write of a book its check
/// stops. The tests hold the code of a book `check` warns of (W108) to the language's own tools
/// with it, which is how they know that the target that warning names is the one that breaks.
pub fn unchecked(book: &Book, stem: &str, target: Target, origin: &Origin, in_pkg: Option<&client::InPackage>) -> Result<Vec<(String, String)>, Vec<Diag>> {
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
