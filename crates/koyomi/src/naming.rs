//! What each target calls things (PLAN B.10): the module, the functions, their signatures.
//! `api` prints these, and the generators of stage C use the same functions, so the two
//! cannot name a function differently.

use crate::ast::Ty;

/// The names the generated code defines or leans on beside the dates' functions, spelled as
/// aliases. An alias equal to one of them would collide with it in some target (E009): `date`
/// is Go's `Date` type, `koyomi_error` its `KoyomiError`, `parse_date` its `ParseDate`; `err`
/// is the error a Go function carries; and the Python calls the builtins `range`, `str`,
/// `int`, `bool`, `tuple`, `isinstance`, `frozenset` and `super`, which a function of the same
/// name would hide. The helpers the generators write start with `_`, which no alias can, and
/// the one local of every function is `day`, a word of the language, so neither needs an entry.
pub const GENERATED: &[&str] = &[
    "is_open", "date", "koyomi_error", "parse_date", "datetime", "timedelta", "err", "range", "str", "int", "bool", "tuple",
    "isinstance", "frozenset", "super",
];

/// The names a file's alias cannot take, because the alias names a Python module, a Rust
/// module and a PostgreSQL schema beside ones the generated code reads (E009): Python's
/// `datetime`, `json`, `sys` and `typing` (a module of that name beside the runner would be
/// imported in their place), Rust's `std`, `core` and `alloc`, and PostgreSQL's own schemas.
pub const MODULES: &[&str] = &["datetime", "json", "sys", "typing", "std", "core", "alloc", "pg_catalog", "pg_temp", "pg_toast", "information_schema"];

/// `payment_at` → `PaymentAt`: Go's exported name for an alias.
pub fn pascal(alias: &str) -> String {
    alias
        .split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut cs = p.chars();
            match cs.next() {
                Some(c) => c.to_ascii_uppercase().to_string() + cs.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// `payment_terms` → `paymentterms`: a Go package name.
pub fn go_package(alias: &str) -> String {
    alias.replace('_', "")
}

/// The function that gives a date's time (`at`).
pub fn at_alias(alias: &str) -> String {
    format!("{alias}_at")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    TypeScript,
    Python,
    Go,
    Rust,
    Sql,
}

pub const TARGETS: [Target; 5] = [Target::TypeScript, Target::Python, Target::Go, Target::Rust, Target::Sql];

impl Target {
    /// The key in `api`'s JSON and the value of `gen --target`.
    pub fn key(self) -> &'static str {
        match self {
            Target::TypeScript => "typescript",
            Target::Python => "python",
            Target::Go => "go",
            Target::Rust => "rust",
            Target::Sql => "sql",
        }
    }

    /// The file `gen` writes for a `.cal` with this alias, under the output directory.
    pub fn file(self, alias: &str) -> String {
        match self {
            Target::TypeScript => format!("typescript/{alias}.ts"),
            Target::Python => format!("python/{alias}.py"),
            Target::Go => format!("go/{0}/{0}.go", go_package(alias)),
            Target::Rust => format!("rust/{alias}.rs"),
            Target::Sql => format!("sql/{alias}.sql"),
        }
    }

    /// What the generated code is imported or called by: the module, package or schema.
    pub fn module(self, alias: &str) -> String {
        match self {
            Target::Go => go_package(alias),
            _ => alias.to_string(),
        }
    }

    pub fn date_type(self) -> &'static str {
        match self {
            Target::TypeScript => "string",
            Target::Python => "datetime.date",
            Target::Go | Target::Rust => "Date",
            Target::Sql => "date",
        }
    }

    pub fn errors(self) -> &'static str {
        match self {
            Target::TypeScript | Target::Python | Target::Rust => "KoyomiError",
            Target::Go => "*KoyomiError",
            Target::Sql => "SQLSTATE 22023",
        }
    }

    pub fn ty(self, t: Ty) -> &'static str {
        match (self, t) {
            (Target::TypeScript, Ty::Date) => "string",
            (Target::TypeScript, Ty::Int) => "number",
            (Target::Python, Ty::Date) => "date",
            (Target::Python, Ty::Int) => "int",
            (Target::Go, Ty::Date) => "Date",
            (Target::Go, Ty::Int) => "int",
            (Target::Rust, Ty::Date) => "Date",
            (Target::Rust, Ty::Int) => "i64",
            (Target::Sql, Ty::Date) => "date",
            (Target::Sql, Ty::Int) => "integer",
        }
    }

    /// The name of a date's function (or its `_at` function) in this target.
    pub fn function(self, alias: &str) -> String {
        match self {
            Target::Go => pascal(alias),
            _ => alias.to_string(),
        }
    }

    /// The signature of a function from `params` (alias, type) to a date, or to a time when
    /// `at` (RFC 3339, in UTC).
    pub fn signature(self, module: &str, alias: &str, params: &[(String, Ty)], at: bool) -> String {
        let f = self.function(alias);
        match self {
            Target::TypeScript => {
                let ps: Vec<String> = params.iter().map(|(a, t)| format!("{a}: {}", self.ty(*t))).collect();
                format!("export function {f}({}): string", ps.join(", "))
            }
            Target::Python => {
                let ps: Vec<String> = params.iter().map(|(a, t)| format!("{a}: {}", self.ty(*t))).collect();
                format!("def {f}({}) -> {}", ps.join(", "), if at { "str" } else { "date" })
            }
            Target::Go => {
                let ps: Vec<String> = params.iter().map(|(a, t)| format!("{a} {}", self.ty(*t))).collect();
                format!("func {f}({}) ({}, error)", ps.join(", "), if at { "string" } else { "Date" })
            }
            Target::Rust => {
                let ps: Vec<String> = params.iter().map(|(a, t)| format!("{a}: {}", self.ty(*t))).collect();
                format!("pub fn {f}({}) -> Result<{}, KoyomiError>", ps.join(", "), if at { "String" } else { "Date" })
            }
            Target::Sql => {
                let ps: Vec<String> = params.iter().map(|(a, t)| format!("{a} {}", self.ty(*t))).collect();
                format!("{module}.{f}({}) RETURNS {}", ps.join(", "), if at { "timestamptz" } else { "date" })
            }
        }
    }

    /// The signature of `is_open`.
    pub fn is_open(self, module: &str) -> String {
        match self {
            Target::TypeScript => "export function is_open(day: string): boolean".into(),
            Target::Python => "def is_open(day: date) -> bool".into(),
            Target::Go => "func IsOpen(day Date) (bool, error)".into(),
            Target::Rust => "pub fn is_open(day: Date) -> Result<bool, KoyomiError>".into(),
            Target::Sql => format!("{module}.is_open(day date) RETURNS boolean"),
        }
    }
}
