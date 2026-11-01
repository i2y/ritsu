//! The names a file writes that a target spells as one identifier, and the ones a target cannot
//! take (W001, DESIGN 15). Every alias is an identifier in each target as it is written, but for
//! three spellings: Go names a date's function in PascalCase (`payment_terms` is `PaymentTerms`)
//! and a package by the file's alias without its `_` (`paymentterms`), PostgreSQL keeps the first
//! 63 bytes of a name, and rustc's lint `non_snake_case` warns of a name with two `_` in a row,
//! which the Rust and Wasm code is built to pass with `-D warnings`. So two aliases that differ
//! only in their `_` are one function in Go, two long aliases that share their first 63 bytes are
//! one function or one parameter in PostgreSQL, an alias can be in Go a name the generated code
//! declares itself (`Date`, `TestMain`), a file's alias can make a package Go cannot take (`goto`,
//! `main`), and `a__b` is a function, a parameter or a module rustc warns of.
//!
//! Each of them breaks some of the targets, so it is a warning, as rulec's W121 is (rulec's DESIGN
//! 15.263): `check` passes, `gen` writes every target, and nothing is renamed. A name the
//! generated code uses in every target is E009 (`naming::GENERATED`), which stops the file before
//! this is looked at.

use crate::ast::{File, Kind, Name, Span, Ty};
use crate::diag::Diag;
use crate::naming::{self, Target, go_package, pascal};
use crate::resolve::Model;
use ritsu_base::text::Text;
use ritsu_emit::words::go::KEYWORDS as GO_KEYWORDS;

/// PostgreSQL keeps the first 63 bytes of a name (NAMEDATALEN − 1) and cuts a longer one short
/// with only a notice.
pub const PG_NAME_MAX: usize = 63;

/// One W001: the targets it breaks, and the diagnostic.
#[derive(Clone, Debug)]
pub struct Found {
    pub targets: Vec<Target>,
    pub diag: Diag,
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
    out.extend(rust_functions(m));
    out.extend(pg_functions(m, &fs));
    out.extend(pg_parameters(m));
    out.extend(rust_parameters(m));
    out
}

/// The diagnostics of [`file_alias`]: what a calendar file is told.
pub fn check_file_alias(f: &File) -> Vec<Diag> {
    file_alias(f).into_iter().map(|f| f.diag).collect()
}

/// What a file's alias names where a target cannot take it: the Go package ([`package`]) and the
/// Rust module ([`rust_module`]).
pub fn file_alias(f: &File) -> Vec<Found> {
    let mut v = package(f);
    v.extend(rust_module(f));
    v
}

/// The word a heading starts with: `dates` or `calendar`.
fn heading_word(f: &File) -> &'static str {
    match f.kind {
        Kind::Dates => "dates",
        Kind::Calendar => "calendar",
    }
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
    let what = heading_word(f);
    let (n, v) = (&f.name.text, &f.version);
    let d = warning(
        f,
        span(&f.name),
        if keyword {
            tr!(
                "生成した Go のコードでは、ファイルの別名 `{alias}` がパッケージの名前 `{pkg}` になり、Go はその名前を使えません",
                "In the code generated for Go, the file's alias `{alias}` is the package `{pkg}`, a name Go cannot take"
            )
        } else {
            tr!(
                "生成した Go のコードでは、ファイルの別名 `{alias}` がパッケージの名前 `{pkg}` になり、ほかのパッケージから読み込めません",
                "In the code generated for Go, the file's alias `{alias}` is the package `{pkg}`, which no other package can import"
            )
        },
    )
    .note(note)
    .note(tr!(
        "使わない出力先なら、このままで構いません。使うなら、ファイルに別の別名を付けてください（`{what} {n}(<別名>) v{v}`）。",
        "For a target you do not generate, leave it. For one you do, give the file another alias (`{what} {n}(<alias>) v{v}`)."
    ));
    vec![Found { targets: vec![Target::Go], diag: d }]
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
    let what = heading_word(f);
    let (n, v) = (&f.name.text, &f.version);
    let at = tr!(
        "モジュールを宣言する `mod {alias};`（`gen` がコードの横に書くランナーと、コードを使うクレートのもの）",
        "the `mod {alias};` that declares the module (in the runner `gen` writes beside the code, and in a crate that uses it)"
    );
    let d = warning(
        f,
        span(&f.name),
        tr!(
            "生成した Rust のコードでは、ファイルの別名 `{alias}` がモジュールの名前になり、rustc が警告します",
            "In the code generated for Rust, the file's alias `{alias}` is the module's name, which rustc warns of"
        ),
    )
    .note(rust_lint(&at))
    .note(tr!(
        "使わない出力先なら、このままで構いません。使うなら、ファイルに別の別名を付けてください（`{what} {n}(<別名>) v{v}`）。",
        "For a target you do not generate, leave it. For one you do, give the file another alias (`{what} {n}(<alias>) v{v}`)."
    ));
    vec![Found { targets: vec![Target::Rust], diag: d }]
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
        let fix = if is_at {
            tr!(
                "使わない出力先なら、このままで構いません。使うなら、`_` で終わらない別名を付けてください（`{t}(<別名>)`）。",
                "For a target you do not generate, leave it. For one you do, give the date an alias that does not end with `_` (`{t}(<alias>)`)."
            )
        } else {
            fix_it(name)
        };
        let d = warning(
            &m.file,
            span(name),
            tr!(
                "生成した Rust と Wasm のコードでは、`{id}` は rustc が警告する名前になります",
                "In the code generated for Rust and Wasm, `{id}` is a name rustc warns of"
            ),
        )
        .note(rust_lint(&w))
        .note(wasm_too())
        .note(fix);
        out.push(Found { targets: vec![Target::Rust, Target::Wasm], diag: d });
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
        let d = warning(
            &m.file,
            span(n),
            tr!(
                "生成した Rust と Wasm のコードでは、`{id}` は rustc が警告する名前になります",
                "In the code generated for Rust and Wasm, `{id}` is a name rustc warns of"
            ),
        )
        .note(rust_lint(&what))
        .note(wasm_too())
        .note(fix_it(n));
        out.push(Found { targets: vec![Target::Rust, Target::Wasm], diag: d });
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
            let d = warning(
                &m.file,
                span(f.name),
                tr!(
                    "`{}`（{l} 行目）と `{}` は、生成した Go のコードで同じ識別子になります",
                    "`{}` (line {l}) and `{}` are one identifier in the code generated for Go",
                    first.ident,
                    f.ident
                ),
            )
            .note(tr!(
                "Go: 関数の名前は PascalCase で書くので、{}{}は、どちらも `{id}` です。同じ名前を一つのパッケージに二度宣言することになり、コンパイルできません。",
                "Go: a function is named in PascalCase, so {} and {} are both `{id}`: one package declares the name twice, which does not compile.",
                then(&x.ja, "と"),
                then(&y.ja, "");
                x.en,
                y.en
            ))
            .note(fix_one_of_two(f.name));
            out.push(Found { targets: vec![Target::Go], diag: d });
            continue;
        }
        if let Some((_, own)) = go_own(m).into_iter().find(|(n, _)| *n == id) {
            let w = f.what();
            let d = warning(
                &m.file,
                span(f.name),
                tr!("`{}` は、生成した Go のコードが自分で使う名前と同じになります", "`{}` takes a name the code generated for Go uses itself", f.ident),
            )
            .note(tr!(
                "Go: {}{}も、PascalCase で書くと `{id}` です。同じ名前を一つのパッケージに二度宣言することになり、{}",
                "Go: {}, and in PascalCase {} is `{id}` as well: one package declares the name twice, {}",
                own.ja,
                then(&w.ja, ""),
                if id == "TestMain" { "`go test` と `go vet` が通りません。" } else { "コンパイルできません。" };
                own.en,
                w.en,
                if id == "TestMain" { "so `go test` and `go vet` do not compile it." } else { "which does not compile." }
            ))
            .note(fix_it(f.name));
            out.push(Found { targets: vec![Target::Go], diag: d });
        }
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
        let fix = if one_date {
            tr!(
                "使わない出力先なら、このままで構いません。使うなら、62 バイトまでの別名を付けてください（`{t}(<別名>)`）。",
                "For a target you do not generate, leave it. For one you do, give the date an alias of 62 bytes or fewer (`{t}(<alias>)`)."
            )
        } else {
            fix_one_of_two(f.name)
        };
        let d = warning(&m.file, span(f.name), heading)
            .note(tr!(
                "PostgreSQL: 名前は先頭の 63 バイトしか残らないので、{}{}は、どちらも `{cut}` です。引数の型も同じなので、{}",
                "PostgreSQL: a name keeps its first 63 bytes, so {} and {} are both `{cut}`, and they take the same types: {}",
                then(&x.ja, "と"),
                then(&y.ja, ""),
                how.ja;
                x.en,
                y.en,
                how.en
            ))
            .note(fix);
        out.push(Found { targets: vec![Target::Sql], diag: d });
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
            let d = warning(
                &m.file,
                span(bn),
                tr!(
                    "`{}`（{l} 行目）と `{}` は、生成した PostgreSQL のコードで同じ識別子になります",
                    "`{}` (line {l}) and `{}` are one identifier in the code generated for PostgreSQL",
                    a.alias,
                    b.alias
                ),
            )
            .note(tr!(
                "PostgreSQL: 名前は先頭の 63 バイトしか残らないので、入力「{}」と入力「{}」の引数は、どちらも `{cut}` です。{}両方を取るので、その関数を作るところで PostgreSQL が止まり（`parameter name used more than once`）、SQL のファイルを読み込めません。",
                "PostgreSQL: a name keeps its first 63 bytes, so the parameters of the inputs {} and {} are both `{cut}`, and {} takes both: PostgreSQL stops at it (`parameter name used more than once`), and the SQL file does not load.",
                an.text,
                bn.text,
                then(&taker.ja, "が");
                an.text,
                bn.text,
                taker.en
            ))
            .note(fix_one_of_two(bn));
            out.push(Found { targets: vec![Target::Sql], diag: d });
        }
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

/// The last note of two names that are one: leave it, or give one another alias.
fn fix_one_of_two(n: &Name) -> Text {
    let t = &n.text;
    tr!(
        "使わない出力先なら、このままで構いません。使うなら、どちらかに別の別名を付けてください（`{t}(<別名>)`）。",
        "For a target you do not generate, leave it. For one you do, give one of the two another alias (`{t}(<alias>)`)."
    )
}

/// The last note of a name a target cannot take: leave it, or give it another alias.
fn fix_it(n: &Name) -> Text {
    let t = &n.text;
    tr!(
        "使わない出力先なら、このままで構いません。使うなら、別の別名を付けてください（`{t}(<別名>)`）。",
        "For a target you do not generate, leave it. For one you do, give it another alias (`{t}(<alias>)`)."
    )
}
