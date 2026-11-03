//! The names a file writes that a target spells as one identifier, and the ones a target cannot
//! take (W001 when the file is checked, E016 when `gen` is to write a target one breaks; DESIGN 15,
//! 17). Every alias is an identifier in each target as it is written, but for three spellings: Go
//! names a date's function in PascalCase (`payment_terms` is `PaymentTerms`) and a package by the
//! file's alias without its `_` (`paymentterms`), PostgreSQL keeps the first 63 bytes of a name, and
//! rustc's lint `non_snake_case` warns of a name with two `_` in a row, which the Rust and Wasm code
//! is built to pass with `-D warnings`. So two aliases that differ only in their `_` are one
//! function in Go, two long aliases that share their first 63 bytes are one function or one
//! parameter in PostgreSQL, an alias can be in Go a name the generated code declares itself
//! (`Date`, `TestMain`), a file's alias can make a package Go cannot take (`goto`, `main`), and
//! `a__b` is a function, a parameter or a module rustc warns of. A file's alias also names the
//! Python module, and Python's standard library can have a module of that name (`collections`):
//! Python keeps one module per name; and it names the PostgreSQL schema, which cannot be one of
//! PostgreSQL's own (`pg_…`, `information_schema`). A name can also be one the code of some targets
//! uses as it is: the calendar's `is_open`, a built-in of Python the module calls (`str`), the
//! variable `err` of every Go function, and an input's name, the key of the JSON the Wasm module
//! reads, which a JavaScript object literal cannot write when it is `__proto__`. And the files given
//! together are compared by their headings ([`across`]): two aliases can be one Go package, one
//! package of WIT or one PostgreSQL schema.
//!
//! Each of them breaks some of the targets, so `check` warns (W001), as rulec's W121 does (rulec's
//! DESIGN 15.263), and `gen` stops when it is to write a target one breaks (E016), as chobo's build
//! does (chobo's DESIGN 16). Nothing is renamed. A name the code of every target breaks on is E009,
//! which stops the file before this is looked at: a keyword or a reserved word (resolve.rs), an
//! alias that is another date's `<alias>_at`, and a date named `is_open` that takes the date input
//! alone, beside a calendar's ([`every_target`]).

use crate::ast::{File, Kind, Name, Span, Ty};
use crate::diag::{Diag, Severity};
use crate::naming::{self, Target, go_package, pascal};
use crate::resolve::Model;
use ritsu_base::text::{Text, capitalize};
use ritsu_emit::copies::rulec::PY_STDLIB;
use ritsu_emit::words::go::KEYWORDS as GO_KEYWORDS;

/// PostgreSQL keeps the first 63 bytes of a name (NAMEDATALEN − 1) and cuts a longer one short
/// with only a notice.
pub const PG_NAME_MAX: usize = 63;

/// One W001: the targets it breaks, the diagnostic, and what fixes it.
#[derive(Clone, Debug)]
pub struct Found {
    pub targets: Vec<Target>,
    /// The warning. Its last note is [`leave_or`] of `rename`.
    pub diag: Diag,
    /// What to change, as the words after "For one you do, " say it (`give it another alias
    /// (\`payment(<alias>)\`)`): what E016 asks for.
    pub rename: Text,
    /// Whether the code breaks as well where a package of ritsu's holds it (ritsu's DESIGN 9.3):
    /// every finding but a module named like one of Python's standard library, which a package
    /// holds under its own name (`<package>.dates.collections`).
    pub in_package: bool,
}

/// A finding of `targets`: the warning at `s` of `f`, its reasons, and its last note, [`leave_or`]
/// of `rename`.
fn found(targets: Vec<Target>, f: &File, s: Span, msg: Text, reasons: Vec<Text>, rename: Text) -> Found {
    let mut diag = warning(f, s, msg);
    for r in reasons {
        diag = diag.note(r);
    }
    let diag = diag.note(leave_or(&rename));
    Found { targets, diag, rename, in_package: true }
}

/// The last note of a W001: leave it for a target you do not generate, else `rename`.
fn leave_or(rename: &Text) -> Text {
    tr!(
        "使わない出力先なら、このままで構いません。使うなら、{}。",
        "For a target you do not generate, leave it. For one you do, {}.",
        rename.ja;
        rename.en
    )
}

/// E016: the findings that break target `t`, which `gen` does not write (DESIGN 15); of them, the
/// ones that break a package of ritsu's as well, when `in_package`. Each is its warning made an
/// error, at the same place, with the same headline and reasons, and in place of the last note,
/// what to do: `rename`, or `--target` with the targets used.
pub fn stops(found: &[Found], t: Target, in_package: bool) -> Vec<Diag> {
    found
        .iter()
        .filter(|f| f.targets.contains(&t) && (f.in_package || !in_package))
        .map(|f| {
            let mut d = f.diag.clone();
            d.code = "E016";
            d.severity = Severity::Error;
            d.notes.pop();
            d.note(tr!(
                "{}。使わない出力先なら、`--target` で使う出力先を挙げてください。",
                "{}. For a target you do not use, name the ones you use with `--target`.",
                f.rename.ja;
                capitalize(&f.rename.en)
            ))
        })
        .collect()
}

/// The diagnostics of [`find`].
pub fn check(m: &Model) -> Vec<Diag> {
    find(m).into_iter().map(|f| f.diag).collect()
}

/// Every W001 of a dates file that passes the names and types: the file's alias (as
/// [`file_alias`]), the functions in Go, in Rust and Wasm, and in PostgreSQL, and the parameters in
/// PostgreSQL and in Rust and Wasm.
pub fn find(m: &Model) -> Vec<Found> {
    let mut out = file_alias(&m.file);
    let fs = functions(m);
    out.extend(go_functions(m, &fs));
    out.extend(calendar_function(m, &fs));
    out.extend(python_functions(m, &fs));
    out.extend(rust_functions(m));
    out.extend(pg_functions(m, &fs));
    out.extend(pg_parameters(m));
    out.extend(rust_parameters(m));
    out.extend(go_parameters(m));
    out.extend(wasm_keys(m));
    out
}

/// What a file's alias names where a target cannot take it: the Go package ([`package`]), the Rust
/// module ([`rust_module`]), the Python module ([`python_module`]) and the PostgreSQL schema
/// ([`pg_schema`]).
pub fn file_alias(f: &File) -> Vec<Found> {
    let mut v = package(f);
    v.extend(rust_module(f));
    v.extend(python_module(f));
    v.extend(pg_schema(f));
    v
}

/// The word a heading starts with: `dates` or `calendar`.
fn heading_word(f: &File) -> &'static str {
    match f.kind {
        Kind::Dates => "dates",
        Kind::Calendar => "calendar",
    }
}

/// What to change when the file's alias is what breaks: the alias, in the heading.
fn rename_file(f: &File) -> Text {
    let what = heading_word(f);
    let (n, v) = (&f.name.text, &f.version);
    tr!("ファイルに別の別名を付けてください（`{what} {n}(<別名>) v{v}`）", "give the file another alias (`{what} {n}(<alias>) v{v}`)")
}

/// The package Go names a file's code by, when Go cannot take it: a keyword (`go_to` is `goto`), or
/// `main`, which Go builds as a program that no other package imports. An alias Go refuses as it is
/// written is E009 already.
pub fn package(f: &File) -> Vec<Found> {
    let Some(alias) = f.name.ascii().filter(|a| crate::ast::is_alias(a)) else { return Vec::new() };
    let pkg = go_package(alias);
    let keyword = GO_KEYWORDS.contains(&pkg.as_str());
    let note = if keyword {
        tr!(
            "Go: パッケージの名前は、ファイルの別名から `_` を除いたものなので、`{pkg}` になります。`{pkg}` は Go のキーワードで、パッケージの名前に使えないので、コンパイルできません。",
            "Go: a package is named by the file's alias without its `_`, so it is `{pkg}`, a keyword of Go, which cannot name a package: the code does not compile."
        )
    } else if pkg == "main" {
        tr!(
            "Go: パッケージの名前は、ファイルの別名から `_` を除いたものなので、`main` になります。Go は `main` のパッケージをプログラムとして組み、ほかのパッケージから読み込めないので、生成した関数を呼べません。",
            "Go: a package is named by the file's alias without its `_`, so it is `main`, which Go builds as a program and no other package can import: the generated functions cannot be called."
        )
    } else {
        return Vec::new();
    };
    let msg = if keyword {
        tr!(
            "生成した Go のコードでは、ファイルの別名 `{alias}` がパッケージの名前 `{pkg}` になり、Go はその名前を使えません",
            "In the code generated for Go, the file's alias `{alias}` is the package `{pkg}`, a name Go cannot take"
        )
    } else {
        tr!(
            "生成した Go のコードでは、ファイルの別名 `{alias}` がパッケージの名前 `{pkg}` になり、ほかのパッケージから読み込めません",
            "In the code generated for Go, the file's alias `{alias}` is the package `{pkg}`, which no other package can import"
        )
    };
    vec![found(vec![Target::Go], f, span(&f.name), msg, vec![note], rename_file(f))]
}

/// Whether rustc's lint `non_snake_case` takes an alias as it is. An alias has no upper case
/// letter, so what makes it warn is two `_` in a row, once the `_` at its ends are left out
/// (`payment_` is snake case, `payment__terms` and `payment__at` are not).
pub fn rust_snake(id: &str) -> bool {
    !id.trim_matches('_').contains("__")
}

/// The note every Rust finding ends its reason with: what the lint does, and where the build stops.
fn rust_lint(what: &Text) -> Text {
    tr!(
        "Rust: rustc のリント `non_snake_case` は `_` が二つ続く名前を警告するので、`-D warnings`（警告をエラーにする）でコンパイルすると、{}止まります。",
        "Rust: rustc's lint `non_snake_case` warns of a name with two `_` in a row, so a build with `-D warnings` (warnings as errors) stops at {}.",
        then(&what.ja, "で");
        what.en
    )
}

/// The note of a finding in the Rust module that the Wasm module holds as well.
fn wasm_too() -> Text {
    tr!(
        "Wasm: モジュールは同じ Rust のコードから組むので、同じところで止まります。",
        "Wasm: the module is built from the same Rust code, and stops at the same place."
    )
}

/// Rust: the module a file's code is declared as, by the file's alias, when rustc warns of it
/// ([`rust_snake`]). The warning is at the `mod` that declares it: in the runner `gen` writes
/// beside the code, and in a crate that uses it. The Wasm module declares the code as `mod k`, so
/// only Rust is named.
fn rust_module(f: &File) -> Vec<Found> {
    let Some(alias) = f.name.ascii().filter(|a| crate::ast::is_alias(a)) else { return Vec::new() };
    if rust_snake(alias) {
        return Vec::new();
    }
    let at = tr!(
        "モジュールを宣言する `mod {alias};`（`gen` がコードの横に書くランナーと、コードを使うクレートのもの）",
        "the `mod {alias};` that declares the module (in the runner `gen` writes beside the code, and in a crate that uses it)"
    );
    let msg = tr!(
        "生成した Rust のコードでは、ファイルの別名 `{alias}` がモジュールの名前になり、rustc が警告します",
        "In the code generated for Rust, the file's alias `{alias}` is the module's name, which rustc warns of"
    );
    vec![found(vec![Target::Rust], f, span(&f.name), msg, vec![rust_lint(&at)], rename_file(f))]
}

/// Python: the module a file's code is, by the file's alias, when Python's standard library has a
/// module of that name (`collections`, `site`). Python imports one module of a name, so either the
/// generated functions cannot be called, or the generated module is what the program gets in place
/// of the standard library's. That is so of the ones the generated code imports itself too: the
/// module imports `datetime`, and the runner `json`, `sys` and `typing` (`json` is a word of
/// PostgreSQL, E009). A package of ritsu's holds the module under the package's name
/// (`<package>.dates.collections`), where it breaks nothing, so only `gen`'s code is named.
fn python_module(f: &File) -> Vec<Found> {
    let Some(alias) = f.name.ascii().filter(|a| crate::ast::is_alias(a)) else { return Vec::new() };
    if !PY_STDLIB.contains(&alias) {
        return Vec::new();
    }
    let msg = tr!(
        "生成した Python のコードでは、ファイルの別名 `{alias}` がモジュールの名前になり、Python の標準ライブラリに同じ名前のモジュールがあります",
        "In the code generated for Python, the file's alias `{alias}` is the module's name, which a module of Python's standard library has"
    );
    let why = tr!(
        "Python: Python は同じ名前のモジュールを一つしか読み込まず、`import {alias}` は先に読み込んだほうを返します。標準ライブラリの `{alias}` を読み込んだあとでは、生成した関数を呼べません。先に生成したモジュールを読み込むと、プログラムのどの `import {alias}` も、標準ライブラリのものの代わりに生成したモジュールを返します。",
        "Python: Python keeps one module per name, and `import {alias}` gives the one imported first. Once the standard library's `{alias}` is imported, the generated functions cannot be called; imported before it, the generated module is what every `import {alias}` in the program gets in place of the standard library's."
    );
    let package = tr!(
        "`ritsu gen` のパッケージは、このモジュールを `<パッケージ>.dates.{alias}` として持つので、標準ライブラリのものとぶつかりません。",
        "A package of `ritsu gen` holds the module as `<package>.dates.{alias}`, which does not meet the standard library's."
    );
    let mut reasons = vec![why];
    // the ones the generated code imports itself: what `gen`'s own code does with them
    match alias {
        "datetime" => reasons.push(tr!(
            "生成したモジュールも `from datetime import date` で標準ライブラリの `datetime` を読み込むので、モジュールの横から読み込むと、モジュールが自分を読み込んで止まります（`ImportError`）。",
            "The generated module imports the standard library's `datetime` itself (`from datetime import date`), so imported from beside it, the module imports itself and stops (`ImportError`)."
        )),
        "json" | "sys" | "typing" => reasons.push(tr!(
            "`gen` がモジュールの横に書くランナーも、標準ライブラリの `{alias}` を読み込みます。",
            "The runner `gen` writes beside the module imports the standard library's `{alias}` too."
        )),
        _ => {}
    }
    reasons.push(package);
    let mut v = found(vec![Target::Python], f, span(&f.name), msg, reasons, rename_file(f));
    v.in_package = false;
    vec![v]
}

/// PostgreSQL: the schema a file's code is, by the file's alias, when it is one of PostgreSQL's
/// own: a name that starts with `pg_`, which PostgreSQL keeps for the schemas of the system and will
/// not create (`unacceptable schema name`), or `information_schema`, which is there already, and
/// where only a superuser creates a function (`permission denied for schema information_schema`).
/// The SQL file creates the schema and every function in it, so it does not load. A package of
/// ritsu's holds no SQL.
fn pg_schema(f: &File) -> Vec<Found> {
    let Some(alias) = f.name.ascii().filter(|a| crate::ast::is_alias(a)) else { return Vec::new() };
    let why = if alias.starts_with("pg_") {
        tr!(
            "PostgreSQL: `pg_` で始まる名前は、PostgreSQL がシステムのスキーマのために取っておく名前なので、SQL のファイルの `CREATE SCHEMA` が止まり（`unacceptable schema name`）、ファイルを読み込めません。",
            "PostgreSQL: a name that starts with `pg_` is one PostgreSQL keeps for the schemas of the system, so the `CREATE SCHEMA` of the SQL file stops (`unacceptable schema name`), and the file does not load."
        )
    } else if alias == "information_schema" {
        tr!(
            "PostgreSQL: `information_schema` は PostgreSQL がもとから持つスキーマで、関数を作れるのはスーパーユーザーだけです。ほかのロールでは SQL のファイルの `CREATE FUNCTION` が止まり（`permission denied for schema information_schema`）、ファイルを読み込めません。",
            "PostgreSQL: `information_schema` is a schema PostgreSQL has itself, where only a superuser creates a function: as any other role, the `CREATE FUNCTION` of the SQL file stops (`permission denied for schema information_schema`), and the file does not load."
        )
    } else {
        return Vec::new();
    };
    let msg = tr!(
        "生成した PostgreSQL のコードでは、ファイルの別名 `{alias}` がスキーマの名前になり、PostgreSQL のスキーマの名前と同じです",
        "In the code generated for PostgreSQL, the file's alias `{alias}` is the schema's name, a name of PostgreSQL's own schemas"
    );
    vec![found(vec![Target::Sql], f, span(&f.name), msg, vec![why], rename_file(f))]
}

/// Rust and Wasm: a date whose function rustc warns of ([`rust_snake`]): its own (the alias), or
/// that of its time (`<alias>_at`, which has two `_` in a row when the alias ends with one). A date
/// is told once, of the first of its functions the lint takes.
fn rust_functions(m: &Model) -> Vec<Found> {
    let mut out = Vec::new();
    for (k, d) in m.dates.iter().enumerate() {
        let name = &m.file.dates[k].name;
        let at = naming::at_alias(&d.alias);
        let (id, is_at) = if !rust_snake(&d.alias) {
            (d.alias.clone(), false)
        } else if d.at.is_some() && !rust_snake(&at) {
            (at, true)
        } else {
            continue;
        };
        let w = function_of(&name.text, &id, is_at);
        let t = &name.text;
        let rename = if is_at {
            tr!("`_` で終わらない別名を付けてください（`{t}(<別名>)`）", "give the date an alias that does not end with `_` (`{t}(<alias>)`)")
        } else {
            rename_it(name)
        };
        let msg = tr!(
            "生成した Rust と Wasm のコードでは、`{id}` は rustc が警告する名前になります",
            "In the code generated for Rust and Wasm, `{id}` is a name rustc warns of"
        );
        out.push(found(vec![Target::Rust, Target::Wasm], &m.file, span(name), msg, vec![rust_lint(&w), wasm_too()], rename));
    }
    out
}

/// Rust and Wasm: an input whose parameter rustc warns of ([`rust_snake`]), when a function takes
/// it.
fn rust_parameters(m: &Model) -> Vec<Found> {
    let mut out = Vec::new();
    for (i, a) in m.inputs.iter().enumerate() {
        if rust_snake(&a.alias) {
            continue;
        }
        let Some(k) = m.dates.iter().position(|d| d.params.contains(&i)) else { continue };
        let n = &m.file.inputs[i].name;
        let taker = function_of(&m.dates[k].name, &m.dates[k].alias, false);
        let id = &a.alias;
        let what = tr!(
            "入力「{}」の引数 `{id}`（{}、入力を取るほかの関数のもの）",
            "the parameter `{id}` of the input {} (of {}, and of every other function that takes the input)",
            n.text,
            then(&taker.ja, "と");
            n.text,
            taker.en
        );
        let msg = tr!(
            "生成した Rust と Wasm のコードでは、`{id}` は rustc が警告する名前になります",
            "In the code generated for Rust and Wasm, `{id}` is a name rustc warns of"
        );
        out.push(found(vec![Target::Rust, Target::Wasm], &m.file, span(n), msg, vec![rust_lint(&what), wasm_too()], rename_it(n)));
    }
    out
}

/// A function of the generated code that a date makes: its own, or the one of its time.
struct Function<'a> {
    /// The date it is of, by index.
    date: usize,
    /// Whether it is the function of the date's time (`<alias>_at`).
    at: bool,
    /// Its name in every target but Go: the alias, or `<alias>_at`.
    ident: String,
    name: &'a Name,
}

impl Function<'_> {
    fn what(&self) -> Text {
        function_of(&self.name.text, &self.ident, self.at)
    }
}

/// `日付「支払日」の関数 payment`, as a note names a date's function (or the function of its time):
/// with the function's name when it is not the date's as written.
fn function_of(n: &str, id: &str, at: bool) -> Text {
    match (at, n == id) {
        (false, true) => tr!("日付「{n}」の関数", "the function of the date {n}"),
        (false, false) => tr!("日付「{n}」の関数 `{id}`", "the function of {n} (`{id}`)"),
        (true, _) => tr!("日付「{n}」の時刻の関数 `{id}`", "the function of the time of {n} (`{id}`)"),
    }
}

/// Every function a dates file's code exports for its dates, in the order the code has them: a
/// date's, then its time's when it has `at`.
fn functions(m: &Model) -> Vec<Function<'_>> {
    let mut v = Vec::new();
    for (k, d) in m.dates.iter().enumerate() {
        let name = &m.file.dates[k].name;
        v.push(Function { date: k, at: false, ident: d.alias.clone(), name });
        if d.at.is_some() {
            v.push(Function { date: k, at: true, ident: naming::at_alias(&d.alias), name });
        }
    }
    v
}

/// What the Go code declares itself beside the dates' functions, and what each is: the module's
/// types and functions (`IsOpen` when the file has a calendar), and the runner's `TestMain`.
fn go_own(m: &Model) -> Vec<(&'static str, Text)> {
    let mut v = vec![
        ("Date", tr!("生成したコードは、日付の型 `Date` を宣言します。", "the generated code declares `Date`, the type of a date")),
        ("ParseDate", tr!("生成したコードは、日付を読む関数 `ParseDate` を宣言します。", "the generated code declares `ParseDate`, the function that reads a date")),
        ("KoyomiError", tr!("生成したコードは、エラーの型 `KoyomiError` を宣言します。", "the generated code declares `KoyomiError`, the type of its errors")),
        ("TestMain", tr!("ランナー（`<パッケージ名>_runner_test.go`）は `TestMain` を宣言します。", "the runner (`<package>_runner_test.go`) declares `TestMain`")),
    ];
    if m.cal.is_some() {
        v.push(("IsOpen", tr!("生成したコードは、営業日かを返す関数 `IsOpen` を宣言します。", "the generated code declares `IsOpen`, the function that says whether a day is a business day")));
    }
    v
}

/// Go: two functions whose names are one in PascalCase, and a function named like one of the Go
/// code's own names.
fn go_functions(m: &Model, fs: &[Function]) -> Vec<Found> {
    let mut out = Vec::new();
    for (i, f) in fs.iter().enumerate() {
        let id = pascal(&f.ident);
        if let Some(first) = fs[..i].iter().find(|g| pascal(&g.ident) == id) {
            // One date's own two functions are never one: `<alias>_at` has an `At` the other lacks.
            let (x, y) = (first.what(), f.what());
            let l = span(first.name).line;
            let msg = tr!(
                "`{}`（{l} 行目）と `{}` は、生成した Go のコードで同じ識別子になります",
                "`{}` (line {l}) and `{}` are one identifier in the code generated for Go",
                first.ident,
                f.ident
            );
            let why = tr!(
                "Go: 関数の名前は PascalCase で書くので、{}{}は、どちらも `{id}` です。同じ名前を一つのパッケージに二度宣言することになり、コンパイルできません。",
                "Go: a function is named in PascalCase, so {} and {} are both `{id}`: one package declares the name twice, which does not compile.",
                then(&x.ja, "と"),
                then(&y.ja, "");
                x.en,
                y.en
            );
            out.push(found(vec![Target::Go], &m.file, span(f.name), msg, vec![why], rename_one_of_two(f.name)));
            continue;
        }
        if let Some((_, own)) = go_own(m).into_iter().find(|(n, _)| *n == id) {
            // The calendar's function as it is written, every target's: told once, of them all.
            if id == "IsOpen" && f.ident == IS_OPEN {
                continue;
            }
            let w = f.what();
            let msg = tr!("`{}` は、生成した Go のコードが自分で使う名前と同じになります", "`{}` takes a name the code generated for Go uses itself", f.ident);
            let why = tr!(
                "Go: {}{}も、PascalCase で書くと `{id}` です。同じ名前を一つのパッケージに二度宣言することになり、{}",
                "Go: {}, and in PascalCase {} is `{id}` as well: one package declares the name twice, {}",
                own.ja,
                then(&w.ja, ""),
                if id == "TestMain" { "`go test` と `go vet` が通りません。" } else { "コンパイルできません。" };
                own.en,
                w.en,
                if id == "TestMain" { "so `go test` and `go vet` do not compile it." } else { "which does not compile." }
            );
            out.push(found(vec![Target::Go], &m.file, span(f.name), msg, vec![why], rename_it(f.name)));
        }
    }
    out
}

/// The function every target's code has for a file that uses a calendar.
const IS_OPEN: &str = "is_open";

/// Whether a date's function takes the date input alone: then its types are those of the
/// calendar's `is_open(day date)`.
fn takes_date_alone(m: &Model, k: usize) -> bool {
    m.dates[k].params.iter().map(|i| m.inputs[*i].ty).collect::<Vec<_>>() == [Ty::Date]
}

/// The names the code of every target breaks on, which `check` refuses (E009) before the W001 are
/// looked at: a date named `is_open` in a file that uses a calendar, when it takes the date input
/// alone. Every target's code has the calendar's `is_open` beside the date's function, and
/// PostgreSQL, which holds two functions of one name apart by the types they take, holds these as
/// one: the date's `RETURNS date` stops the calendar's `RETURNS boolean` (`cannot change return type
/// of existing function`). A date that takes an integer input as well is W001
/// ([`calendar_function`]).
pub fn every_target(m: &Model) -> Vec<Diag> {
    let mut out = Vec::new();
    if m.cal.is_none() {
        return out;
    }
    for (k, date) in m.dates.iter().enumerate() {
        if date.alias != IS_OPEN || !takes_date_alone(m, k) {
            continue;
        }
        let n = &m.file.dates[k].name;
        let (s, t) = (span(n), &n.text);
        let d = Diag::error("E009", &m.file.path, s.line, s.col, tr!("別名 is_open は、カレンダーの関数 is_open とぶつかります", "The alias is_open collides with is_open, the function of the calendar"))
            .source(&m.file.src)
            .note(tr!(
                "カレンダーを使うファイルでは、どの出力先のコードも、営業日かを返す関数 `is_open`（Go では `IsOpen`）を置きます。この日付は日付の入力だけを取るので、PostgreSQL でも引数の型が `is_open(day date)` と同じになり、どの出力先でも、同じ名前の関数を二度宣言することになります。",
                "In a file that uses a calendar, the code of every target has `is_open` (`IsOpen` in Go), the function that says whether a day is a business day. This date takes the date input alone, so even in PostgreSQL its types are those of `is_open(day date)`: the code of every target declares the function twice."
            ))
            .note(tr!("別の別名を付けてください（`{t}(<別名>)`）。", "Give it another alias (`{t}(<alias>)`)."));
        out.push(d);
    }
    out
}

/// The calendar's function, `is_open`, as a date's (an alias `is_open` in a file that uses a
/// calendar), when the date takes an integer input as well: TypeScript, Python, Go, Rust and Wasm
/// have the name twice, and PostgreSQL holds the two functions apart by their types. The date that
/// takes the date input alone breaks PostgreSQL too, and is E009 ([`every_target`]).
fn calendar_function(m: &Model, fs: &[Function]) -> Vec<Found> {
    if m.cal.is_none() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for f in fs.iter().filter(|f| !f.at && f.ident == IS_OPEN && !takes_date_alone(m, f.date)) {
        let w = f.what();
        let msg = tr!(
            "`is_open` は、カレンダーを使うファイルで生成したコードが置く関数 `is_open` と同じ名前になります",
            "`is_open` takes the name of `is_open`, the function the generated code has for the calendar"
        );
        let reasons = vec![
            tr!(
                "TypeScript: モジュールが `is_open` を二度宣言することになり、`tsc` が通らず、Node も読み込めません（`Identifier 'is_open' has already been declared`）。",
                "TypeScript: the module declares `is_open` twice, which `tsc` does not take and Node does not load (`Identifier 'is_open' has already been declared`)."
            ),
            tr!(
                "Python: 後の `def is_open` が前の関数を置き換えるので、モジュールの `is_open` はカレンダーの関数になり、{}呼べません。`mypy --strict` も通りません。",
                "Python: the later `def is_open` takes the place of the first, so the module's `is_open` is the calendar's, and {} cannot be called; `mypy --strict` does not pass either.",
                then(&w.ja, "を");
                w.en
            ),
            tr!("Go: どちらも `IsOpen` です。同じ名前を一つのパッケージに二度宣言することになり、コンパイルできません。", "Go: both are `IsOpen`: one package declares the name twice, which does not compile."),
            tr!("Rust: 同じ名前の関数を二度宣言することになり、コンパイルできません。", "Rust: the module declares two functions of the name, which does not compile."),
            wasm_too(),
            tr!(
                "PostgreSQL: {}整数の入力も取り、引数の型がカレンダーの `is_open(day date)` と違うので、PostgreSQL は二つを別の関数として持ちます。",
                "PostgreSQL: {} takes an integer input as well, so its types are not those of the calendar's `is_open(day date)`, and PostgreSQL holds the two apart.",
                then(&w.ja, "は");
                capitalize(&w.en)
            ),
        ];
        out.push(found(vec![Target::TypeScript, Target::Python, Target::Go, Target::Rust, Target::Wasm], &m.file, span(f.name), msg, reasons, rename_it(f.name)));
    }
    out
}

/// Python: a date's function named like a built-in of Python the generated module uses, which the
/// function at the top of the module hides from it: `str` and `tuple`, which the module annotates
/// with, and `isinstance` and `str`, which it calls to check an integer input (when a function takes
/// one). `int`, `bool`, `range` and `super`, which the module uses too, are E009 already (words of
/// koyomi, of Go, or of TypeScript and Rust), and `frozenset` breaks nothing: the module calls it as
/// it is imported, before the date's function is defined, and `mypy --strict` passes it.
fn python_functions(m: &Model, fs: &[Function]) -> Vec<Found> {
    let ints = m.dates.iter().any(|d| d.params.iter().any(|i| m.inputs[*i].ty == Ty::Int));
    let mut out = Vec::new();
    for f in fs {
        let id = f.ident.as_str();
        let w = f.what();
        let why = match id {
            "isinstance" if ints => tr!(
                "Python: 生成したコードは、整数の入力を組み込みの `isinstance` で確かめます。{}モジュールの一番外の同じ名前の関数なので、確かめるところでその関数が呼ばれ、整数の入力を取る関数は、どれも `KoyomiError` でない例外で止まります。`mypy --strict` も通りません。",
                "Python: the generated code checks an integer input with the built-in `isinstance`. {} is a function of that name at the top of the module, so the check calls it, and every function that takes an integer input stops with an exception that is not a `KoyomiError`; `mypy --strict` does not pass either.",
                then(&w.ja, "は");
                capitalize(&w.en)
            ),
            "str" if ints => tr!(
                "Python: 生成したコードは、型の注釈に組み込みの `str` を使い、範囲の外の整数の入力を言う文も `str` で作ります。{}モジュールの一番外の同じ名前の関数なので、`mypy --strict` が通らず（`is not valid as a type`）、整数の入力が範囲の外のとき、関数は `KoyomiError` でない例外で止まります。",
                "Python: the generated code annotates with the built-in `str`, and makes the sentence of an integer input outside its range with it. {} is a function of that name at the top of the module, so `mypy --strict` does not pass (`is not valid as a type`), and for an integer input outside its range a function stops with an exception that is not a `KoyomiError`.",
                then(&w.ja, "は");
                capitalize(&w.en)
            ),
            "str" | "tuple" => tr!(
                "Python: 生成したコードは、組み込みの `{id}` を型の注釈に使います。{}モジュールの一番外の同じ名前の関数なので、`mypy --strict` が通りません（`is not valid as a type`）。動かしたときの答えは変わりません。",
                "Python: the generated code annotates with the built-in `{id}`. {} is a function of that name at the top of the module, so `mypy --strict` does not pass (`is not valid as a type`); what the code answers when run is the same.",
                then(&w.ja, "は");
                capitalize(&w.en)
            ),
            _ => continue,
        };
        let msg = tr!(
            "生成した Python のコードでは、`{id}` が、コードの使う組み込みの `{id}` を隠します",
            "In the code generated for Python, `{id}` hides Python's built-in `{id}`, which the code uses"
        );
        out.push(found(vec![Target::Python], &m.file, span(f.name), msg, vec![why], rename_it(f.name)));
    }
    out
}

/// Go: an input whose parameter is `err`, when a function takes it. Every function of the Go code
/// keeps its errors in `err` (`day, err := …`), which then is the parameter, of another type.
fn go_parameters(m: &Model) -> Vec<Found> {
    let mut out = Vec::new();
    for (i, a) in m.inputs.iter().enumerate() {
        if a.alias != "err" {
            continue;
        }
        let Some(k) = m.dates.iter().position(|d| d.params.contains(&i)) else { continue };
        let n = &m.file.inputs[i].name;
        let taker = function_of(&m.dates[k].name, &m.dates[k].alias, false);
        let msg = tr!(
            "生成した Go のコードでは、入力「{}」の引数 `err` が、関数がエラーを受ける変数 `err` と同じ名前になります",
            "In the code generated for Go, the parameter `err` of the input {} is the variable `err` the functions keep their errors in",
            n.text;
            n.text
        );
        let why = tr!(
            "Go: 関数は `day, err := …` のようにエラーを `err` に受けるので、{}（入力を取るほかの関数も）は `error` を引数の `err` に入れることになり、コンパイルできません（`value of interface type error`）。",
            "Go: a function keeps its errors in `err` (`day, err := …`), so {} (and every other function that takes the input) puts an `error` in the parameter `err`, which does not compile (`value of interface type error`).",
            taker.ja;
            taker.en
        );
        out.push(found(vec![Target::Go], &m.file, span(n), msg, vec![why], rename_it(n)));
    }
    out
}

/// Wasm: an input named `__proto__`, when a function takes it. The module takes the inputs as one
/// JSON object, by their names, and a JavaScript host that writes the object as a literal cannot put
/// `__proto__` in it: `__proto__: …` in an object literal sets the object's prototype, not a
/// property (a string or a number is no prototype, and is dropped), so the JSON `JSON.stringify`
/// writes of it has no key `__proto__`, and the module answers that the input is missing. The other
/// targets take the inputs as parameters, by their aliases, and the runners read the key from the
/// JSON of a line of the vectors, where it is a property as written. The key is the input's name,
/// which another alias does not change.
fn wasm_keys(m: &Model) -> Vec<Found> {
    let mut out = Vec::new();
    for (i, a) in m.inputs.iter().enumerate() {
        if a.name != "__proto__" || !m.dates.iter().any(|d| d.params.contains(&i)) {
            continue;
        }
        let n = &m.file.inputs[i].name;
        let msg = tr!(
            "生成した Wasm のモジュールでは、入力「__proto__」が JSON のキー `__proto__` になり、JavaScript のオブジェクトリテラルでは渡せません",
            "In the Wasm module, the input __proto__ is the JSON key `__proto__`, which a JavaScript object literal cannot carry"
        );
        let why = tr!(
            "Wasm: モジュールは入力を、名前をキーにした一つの JSON のオブジェクトで受け取ります。JavaScript のホストがそのオブジェクトをリテラルで書くと、`{{ __proto__: … }}` の `__proto__` はプロパティではなくオブジェクトのプロトタイプを決める書き方なので（文字列や数はプロトタイプにならず、捨てられます）、`JSON.stringify` の JSON にキー `__proto__` が無く、モジュールは入力が無いと答えます（`refused` が `input`）。",
            "Wasm: the module takes the inputs as one JSON object, by their names. A JavaScript host that writes the object as a literal writes in `{{ __proto__: … }}` the object's prototype, not a property (a string or a number is no prototype, and is dropped), so the JSON `JSON.stringify` writes has no key `__proto__`, and the module answers that the input is missing (`refused` is `input`)."
        );
        let rename = tr!(
            "入力に `__proto__` でない名前を付けてください（キーになるのは別名ではなく名前です）",
            "give the input a name other than `__proto__` (the key is its name, not its alias)"
        );
        out.push(found(vec![Target::Wasm], &m.file, n.span, msg, vec![why], rename));
    }
    out
}

/// A name as PostgreSQL keeps it: its first 63 bytes. An alias is ASCII, so a byte is a character.
fn pg(s: &str) -> &str {
    if s.len() > PG_NAME_MAX { &s[..PG_NAME_MAX] } else { s }
}

/// PostgreSQL: two functions whose names share their first 63 bytes and that take the same types,
/// so that PostgreSQL holds one function for both (DESIGN 15).
fn pg_functions(m: &Model, fs: &[Function]) -> Vec<Found> {
    let types = |f: &Function| -> Vec<Ty> { m.dates[f.date].params.iter().map(|k| m.inputs[*k].ty).collect() };
    let names = |f: &Function| -> Vec<String> { m.dates[f.date].params.iter().map(|k| pg(&m.inputs[*k].alias).to_string()).collect() };
    let mut out = Vec::new();
    for (i, f) in fs.iter().enumerate() {
        let Some(first) = fs[..i].iter().find(|g| pg(&g.ident) == pg(&f.ident) && types(g) == types(f)) else { continue };
        let cut = pg(&f.ident);
        let (x, y) = (first.what(), f.what());
        let how = if first.at != f.at {
            tr!(
                "後の関数を作るところで PostgreSQL が止まり（`cannot change return type of existing function`）、SQL のファイルを読み込めません。",
                "PostgreSQL stops at the second (`cannot change return type of existing function`), and the SQL file does not load."
            )
        } else if names(first) != names(f) {
            tr!(
                "後の関数を作るところで PostgreSQL が止まり（`cannot change name of input parameter`）、SQL のファイルを読み込めません。",
                "PostgreSQL stops at the second (`cannot change name of input parameter`), and the SQL file does not load."
            )
        } else {
            tr!(
                "`CREATE OR REPLACE FUNCTION` で後の関数が前の関数を置き換え、残った関数が両方に答えます。",
                "`CREATE OR REPLACE FUNCTION` replaces the first with the second, and the one left answers for both."
            )
        };
        // One date's own two functions are one when its alias is 63 bytes or longer: the fix is a
        // shorter alias, not another one for either function.
        let one_date = first.date == f.date;
        let l = span(first.name).line;
        let heading = if one_date {
            tr!(
                "`{}` と `{}` は、生成した PostgreSQL のコードで同じ識別子になります",
                "`{}` and `{}` are one identifier in the code generated for PostgreSQL",
                first.ident,
                f.ident
            )
        } else {
            tr!(
                "`{}`（{l} 行目）と `{}` は、生成した PostgreSQL のコードで同じ識別子になります",
                "`{}` (line {l}) and `{}` are one identifier in the code generated for PostgreSQL",
                first.ident,
                f.ident
            )
        };
        let t = &f.name.text;
        let rename = if one_date {
            tr!("62 バイトまでの別名を付けてください（`{t}(<別名>)`）", "give the date an alias of 62 bytes or fewer (`{t}(<alias>)`)")
        } else {
            rename_one_of_two(f.name)
        };
        let why = tr!(
            "PostgreSQL: 名前は先頭の 63 バイトしか残らないので、{}{}は、どちらも `{cut}` です。引数の型も同じなので、{}",
            "PostgreSQL: a name keeps its first 63 bytes, so {} and {} are both `{cut}`, and they take the same types: {}",
            then(&x.ja, "と"),
            then(&y.ja, ""),
            how.ja;
            x.en,
            y.en,
            how.en
        );
        out.push(found(vec![Target::Sql], &m.file, span(f.name), heading, vec![why], rename));
    }
    out
}

/// PostgreSQL: two inputs whose names share their first 63 bytes, and a function that takes both.
fn pg_parameters(m: &Model) -> Vec<Found> {
    let mut out = Vec::new();
    for (j, b) in m.inputs.iter().enumerate() {
        for (i, a) in m.inputs[..j].iter().enumerate() {
            if pg(&a.alias) != pg(&b.alias) {
                continue;
            }
            let Some(k) = m.dates.iter().position(|d| d.params.contains(&i) && d.params.contains(&j)) else { continue };
            let (an, bn) = (&m.file.inputs[i].name, &m.file.inputs[j].name);
            let cut = pg(&b.alias);
            let taker = function_of(&m.dates[k].name, &m.dates[k].alias, false);
            let l = span(an).line;
            let msg = tr!(
                "`{}`（{l} 行目）と `{}` は、生成した PostgreSQL のコードで同じ識別子になります",
                "`{}` (line {l}) and `{}` are one identifier in the code generated for PostgreSQL",
                a.alias,
                b.alias
            );
            let why = tr!(
                "PostgreSQL: 名前は先頭の 63 バイトしか残らないので、入力「{}」と入力「{}」の引数は、どちらも `{cut}` です。{}両方を取るので、その関数を作るところで PostgreSQL が止まり（`parameter name used more than once`）、SQL のファイルを読み込めません。",
                "PostgreSQL: a name keeps its first 63 bytes, so the parameters of the inputs {} and {} are both `{cut}`, and {} takes both: PostgreSQL stops at it (`parameter name used more than once`), and the SQL file does not load.",
                an.text,
                bn.text,
                then(&taker.ja, "が");
                an.text,
                bn.text,
                taker.en
            );
            out.push(found(vec![Target::Sql], &m.file, span(bn), msg, vec![why], rename_one_of_two(bn)));
        }
    }
    out
}

/// What a file's alias makes of it, when the file has an alias: a file that has none is said
/// (E010) before this is looked at.
fn module_alias(f: &File) -> Option<&str> {
    f.name.ascii().filter(|a| crate::ast::is_alias(a))
}

/// The forms of one file with the files given before it in one run (`koyomi check` or `gen` of
/// several, and the files of `ritsu check`), told of the later file, which names the earlier one:
/// two aliases that are one Go package (`pay_days`, `paydays`), one package of WIT at one version
/// (`pay_days`, `pay__days`), or one PostgreSQL schema (two long aliases that share their first 63
/// bytes). Two files of one alias are not among them: every target has the same name for both, a
/// directory can hold the English and the Japanese version of one file, and `gen` stops when it is
/// to write both ("both write"). Each form is told once, of the later file, with the first file that
/// makes it. A file that fails its check is not compared (`check::compare_across`).
pub fn across(earlier: &[File], f: &File) -> Vec<Found> {
    let Some(alias) = module_alias(f) else { return Vec::new() };
    let others: Vec<(&File, &str)> = earlier.iter().filter_map(|e| module_alias(e).filter(|o| *o != alias).map(|o| (e, o))).collect();
    let mut out = Vec::new();
    let pkg = go_package(alias);
    if let Some((e, other)) = others.iter().find(|(_, o)| go_package(o) == pkg) {
        let path = &e.path;
        let msg = tr!(
            "生成した Go のコードでは、ファイルの別名 `{alias}` と、`{path}` の別名 `{other}` が、同じパッケージ `{pkg}` になります",
            "In the code generated for Go, the file's alias `{alias}` and the alias `{other}` of `{path}` are one package, `{pkg}`"
        );
        let why = tr!(
            "Go: パッケージの名前は、ファイルの別名から `_` を除いたものなので、どちらも `{pkg}` です。二つのファイルが同じ `go/{pkg}/{pkg}.go` を書くことになります（`ritsu gen` のパッケージでは `dates/{pkg}/{pkg}.go`）。",
            "Go: a package is named by the file's alias without its `_`, so both are `{pkg}`, and both files write `go/{pkg}/{pkg}.go` (`dates/{pkg}/{pkg}.go` in a package of `ritsu gen`)."
        );
        out.push(found(vec![Target::Go], f, span(&f.name), msg, vec![why], rename_file(f)));
    }
    let package = crate::codegen::wasm::package(alias, &f.version);
    if let Some((e, other)) = others.iter().find(|(e, o)| crate::codegen::wasm::package(o, &e.version) == package) {
        let path = &e.path;
        let msg = tr!(
            "生成した Wasm の `.wit` では、ファイルの別名 `{alias}` と、`{path}` の別名 `{other}` が、同じパッケージ `{package}` になります",
            "In the `.wit` generated for Wasm, the file's alias `{alias}` and the alias `{other}` of `{path}` are one package, `{package}`"
        );
        let why = tr!(
            "Wasm: WIT の名前は、ファイルの別名の `_` を `-` にしたものです（続く `_` と端の `_` は一つの区切りか、無いものになります）。版も同じなので、二つの `.wit` を一緒に読むと、WIT のツールが止まります（`is defined in two different locations`）。",
            "Wasm: a WIT name is the file's alias with `-` for its `_` (a run of `_` is one, and one at either end none). The versions are the same too, so the WIT tools stop when the two `.wit` are read together (`is defined in two different locations`)."
        );
        out.push(found(vec![Target::Wasm], f, span(&f.name), msg, vec![why], rename_file(f)));
    }
    let cut = pg(alias);
    if let Some((e, other)) = others.iter().find(|(_, o)| pg(o) == cut) {
        let path = &e.path;
        let msg = tr!(
            "生成した PostgreSQL のコードでは、ファイルの別名 `{alias}` と、`{path}` の別名 `{other}` が、同じスキーマ `{cut}` になります",
            "In the code generated for PostgreSQL, the file's alias `{alias}` and the alias `{other}` of `{path}` are one schema, `{cut}`"
        );
        let why = tr!(
            "PostgreSQL: 名前は先頭の 63 バイトしか残らないので、どちらのスキーマも `{cut}` です。一つのデータベースに読み込むと、後のファイルの関数が、前のファイルの同じ名前で同じ引数の型の関数を置き換えるので（返す型が違えば、読み込みが止まります）、前のファイルの関数の呼び出しに、後のファイルの答えが返ります。",
            "PostgreSQL: a name keeps its first 63 bytes, so both schemas are `{cut}`. Loaded into one database, the later file's functions replace the earlier file's of the same name and parameter types (where the type returned differs, the load stops), so a call of the earlier file's function gets the later file's answer."
        );
        out.push(found(vec![Target::Sql], f, span(&f.name), msg, vec![why], rename_file(f)));
    }
    out
}

/// A Japanese phrase and the particle after it: with a space between them when the phrase ends
/// with a name in backquotes, as the suite's Japanese writes a name (`payment` と).
fn then(phrase: &str, particle: &str) -> String {
    if phrase.ends_with('`') { format!("{phrase} {particle}") } else { format!("{phrase}{particle}") }
}

/// Where a name's alias is written, or the name when it is its own alias.
fn span(n: &Name) -> Span {
    n.alias.as_ref().map(|(_, s)| *s).unwrap_or(n.span)
}

fn warning(f: &File, s: Span, msg: Text) -> Diag {
    Diag::warning("W001", &f.path, s.line, s.col, msg).source(&f.src)
}

/// What to change when two names are one: one of them.
fn rename_one_of_two(n: &Name) -> Text {
    let t = &n.text;
    tr!("どちらかに別の別名を付けてください（`{t}(<別名>)`）", "give one of the two another alias (`{t}(<alias>)`)")
}

/// What to change when a target cannot take a name: the name's alias.
fn rename_it(n: &Name) -> Text {
    let t = &n.text;
    tr!("別の別名を付けてください（`{t}(<別名>)`）", "give it another alias (`{t}(<alias>)`)")
}
