//! The names a client spells as one identifier, or cannot take as they are (W108 when the book is
//! checked, E064 when a target is built; DESIGN 16).
//!
//! Every book of tests/spellings gives W108 once, naming the targets [`FORMS`] gives it, and what
//! `check` prints of it, in English and in Japanese, is the golden file beside it
//! (`CHOBO_BLESS=1 cargo test` writes them again; read the diff). The build of each of those
//! targets stops with E064, and every other target builds. The same book with the name its note
//! points to changed (tests/spellings/renamed) gives nothing and builds every target; that each of
//! those answers every scenario as the reference interpreter does, on every target, is
//! tests/backends.rs's, which runs them beside the test books.
//!
//! With each language's own tools, the code a target would write of each book
//! (`target::unchecked`, which leaves out E064) breaks for the targets W108 names and for no
//! other, and the code of every renamed book passes them all: tsc and node calling the TypeScript
//! client with arguments written as object literals, the Python client's `Book` as a caller
//! reaches it and as `import <name>` does from beside the client, go vet, cargo check, and
//! PostgreSQL loading the SQL and calling it as a driver that sends no types does.

mod common;
use chobo::check;
use chobo::model::Book;
use chobo::spellings::{Name, Subject};
use chobo::target::{self, Target};
use common::runners::*;
use common::servers::{Pg, Postgres};
use common::*;
use ritsu_base::text::Lang;
use std::collections::BTreeSet;
use std::process::Command;

const GO: &[&str] = &["postgres-go", "tigerbeetle-go", "mysql-go"];
const GO_AND_RUST: &[&str] = &["postgres-go", "postgres-rust", "tigerbeetle-go", "tigerbeetle-rust", "mysql-go", "mysql-rust"];
const RUST: &[&str] = &["postgres-rust", "tigerbeetle-rust", "mysql-rust"];
const PYTHON: &[&str] = &["postgres-python", "tigerbeetle-python", "mysql-python"];
const TYPESCRIPT: &[&str] = &["postgres-typescript", "tigerbeetle-typescript", "mysql-typescript"];
const PYTHON_AND_RUST: &[&str] = &["postgres-python", "postgres-rust", "tigerbeetle-python", "tigerbeetle-rust", "mysql-python", "mysql-rust"];
/// every client: the `Book` of each has `balance`
const CLIENTS: &[&str] = &[
    "postgres-typescript",
    "postgres-python",
    "postgres-go",
    "postgres-rust",
    "tigerbeetle-typescript",
    "tigerbeetle-python",
    "tigerbeetle-go",
    "tigerbeetle-rust",
    "mysql-typescript",
    "mysql-python",
    "mysql-go",
    "mysql-rust",
];
/// the clients of PostgreSQL and of the MySQL family, whose `Book` has `expire`
const EXPIRE: &[&str] = &["postgres-typescript", "postgres-python", "postgres-go", "postgres-rust", "mysql-typescript", "mysql-python", "mysql-go", "mysql-rust"];
/// PostgreSQL's SQL, and every client that calls it
const POSTGRES: &[&str] = &["postgres", "postgres-typescript", "postgres-python", "postgres-go", "postgres-rust"];

/// Each book of tests/spellings by its file's name, the English one first and the Japanese one
/// after it, with the targets its W108 names, in the order of `Target::ALL`.
const FORMS: &[(&str, &[&str])] = &[
    // accounts that Go writes as one method of `Balances`
    ("go_accounts", GO),
    ("Goの勘定", GO),
    // transfers that Go writes as one field of `Book`, and Rust as one type
    ("go_and_rust_transfers", GO_AND_RUST),
    ("GoとRustの振替", GO_AND_RUST),
    // transfers that Rust writes as one type
    ("rust_types", RUST),
    ("Rustの型", RUST),
    // parameters that Rust writes as one field
    ("rust_fields", RUST),
    ("Rustのフィールド", RUST),
    // reasons that Rust writes as one variant of `Reason`
    ("rust_reasons", RUST),
    ("Rustの理由", RUST),
    // a reason that Rust writes as the variant of a reason chobo gives itself
    ("rust_own_reason", RUST),
    ("Rustの自分の理由", RUST),
    // parameters that Python writes as one
    ("python_parameters", PYTHON),
    ("Pythonの引数", PYTHON),
    // an account that Python writes as the attribute of `_Balances` its methods call through
    ("python_balances", PYTHON),
    ("Pythonの残高", PYTHON),
    // a transfer whose class Python changes inside `Book`
    ("python_mangled", PYTHON),
    ("Pythonの名前修飾", PYTHON),
    // a book named with a word Python keeps, which the Python client is the module of
    ("python_keyword_module", PYTHON),
    ("Pythonのキーワードのモジュール", PYTHON),
    // a book named like a module of Python's standard library that Python imports as it starts
    ("python_library_module", PYTHON),
    ("Pythonの標準ライブラリのモジュール", PYTHON),
    // a parameter named `__proto__`, which a TypeScript object literal takes for the prototype
    ("typescript_proto", TYPESCRIPT),
    ("TypeScriptのプロトタイプ", TYPESCRIPT),
    // a transfer named like what reads the balances
    ("own_balance", CLIENTS),
    ("自分の残高", CLIENTS),
    // a transfer named like the method that gives back the holds past their expiry
    ("own_expire", EXPIRE),
    ("自分のexpire", EXPIRE),
    // an account and a transfer whose functions in PostgreSQL's SQL are one, of the same types
    ("postgresql_functions", POSTGRES),
    ("PostgreSQLの関数", POSTGRES),
    // the same, of other types: a call that sends no types reaches the account's
    ("postgresql_calls", POSTGRES),
    ("PostgreSQLの呼び出し", POSTGRES),
    // the schema of a book named with `pg_` in front, which PostgreSQL does not create
    ("postgresql_schema", POSTGRES),
    ("PostgreSQLのスキーマ", POSTGRES),
    // two parameters that Python, which compares identifiers in NFKC, takes for one
    ("python_nfkc_parameters", PYTHON),
    ("PythonのNFKCの引数", PYTHON),
    // two parameters that Rust, which compares them in NFC, takes for one, and so Python
    ("rust_nfc_parameters", PYTHON_AND_RUST),
    ("RustのNFCの引数", PYTHON_AND_RUST),
    // a transfer that NFKC makes the attribute that reads the balances
    ("python_nfkc_own", PYTHON),
    ("PythonのNFKCの自分の名前", PYTHON),
    // a parameter that NFKC makes `self`
    ("python_nfkc_self", PYTHON),
    ("PythonのNFKCのself", PYTHON),
    // a book named like the library the TigerBeetle client imports
    ("python_tigerbeetle", &["tigerbeetle-python"]),
    ("Pythonのtigerbeetle", &["tigerbeetle-python"]),
    // a book whose name Python reads as another
    ("python_nfkc_module", PYTHON),
    ("PythonのNFKCのモジュール", PYTHON),
    // a book whose name is no identifier of Python's
    ("python_identifier_module", PYTHON),
    ("Pythonの識別子にならないモジュール", PYTHON),
];

/// A book of tests/spellings, or of its renamed copies, as the tests take it.
struct Form {
    stem: &'static str,
    src: String,
    book: Book,
    origin: target::Origin,
    /// the targets W108 names: none, for a renamed book
    named: Vec<Target>,
    found: Vec<chobo::spellings::Found>,
}

impl Form {
    /// Whether W108 names `t` for what `lang` does with the names: the language of its client, or
    /// PostgreSQL's SQL, which every client of PostgreSQL calls.
    fn breaks(&self, t: Target, lang: target::Lang) -> bool {
        self.found.iter().flat_map(|f| &f.said).any(|s| s.lang == lang && s.targets().contains(&t))
    }

    /// What `t` would write of the book, E064 or not.
    fn files(&self, t: Target) -> Vec<(String, String)> {
        target::unchecked(&self.book, self.stem, t, &self.origin, None).unwrap_or_else(|d| panic!("{}: {}: {}", self.stem, t.name(), d[0].message.en))
    }
}

fn load(stem: &'static str, dir: &str, named: &[&str]) -> Form {
    let file = format!("{dir}/{stem}.book");
    let src = std::fs::read_to_string(root().join(&file)).unwrap_or_else(|e| panic!("{file}: {e}"));
    let (book, d) = chobo::model::load(&src);
    assert!(d.is_empty(), "{file} does not load");
    let origin = target::Origin::named(&file, src.as_bytes());
    let named = named.iter().map(|n| Target::parse(n).unwrap()).collect();
    let book = book.unwrap();
    let found = chobo::spellings::find(&book);
    Form { stem, src, book, origin, named, found }
}

fn forms() -> Vec<Form> {
    FORMS.iter().map(|(stem, named)| load(stem, "tests/spellings", named)).collect()
}

fn renamed() -> Vec<Form> {
    FORMS.iter().map(|(stem, _)| load(stem, "tests/spellings/renamed", &[])).collect()
}

/// The books of tests/spellings, then their renamed copies.
fn every_form() -> Vec<Form> {
    forms().into_iter().chain(renamed()).collect()
}

/// Where a test writes what target `t` would write of the `i`-th of [`every_form`]: a directory
/// whose name is a Go package's and a Rust module's.
fn key(i: usize, t: Target) -> String {
    let b = match t.backend() {
        target::Backend::Postgres => "pg",
        target::Backend::TigerBeetle => "tb",
        target::Backend::MySql => "my",
    };
    format!("f{i}_{b}")
}

/// What one test found against what W108 says: every target it compared, and where they differ.
#[derive(Default)]
struct Tally {
    compared: usize,
    broke: usize,
    wrong: Vec<String>,
}

impl Tally {
    /// What the tools of `lang` found of target `t` of `f`: why it breaks, or None.
    fn add(&mut self, f: &Form, t: Target, lang: target::Lang, why: Option<String>) {
        self.compared += 1;
        match (f.breaks(t, lang), why) {
            (true, Some(why)) => {
                self.broke += 1;
                eprintln!("broken as W108 says: {} {}: {}", f.stem, t.name(), why.lines().next().unwrap_or(""));
            }
            (false, None) => {}
            (true, None) => self.wrong.push(format!("{} {}: W108 names it, and it does not break", f.stem, t.name())),
            (false, Some(why)) => self.wrong.push(format!("{} {}: it breaks, and W108 does not name it:\n{why}", f.stem, t.name())),
        }
    }

    fn end(self, what: &str) {
        assert!(self.wrong.is_empty(), "{}", self.wrong.join("\n"));
        eprintln!("compared: {what}: {} targets of the books of tests/spellings and their renamed copies, {} broken as W108 says", self.compared, self.broke);
    }
}

#[test]
fn every_book_gives_w108_once_for_the_targets_it_names_and_says_what_its_golden_files_say() {
    let mut failures = Vec::new();
    for (stem, named) in FORMS {
        let path = root().join(format!("tests/spellings/{stem}.book"));
        let src = std::fs::read_to_string(&path).unwrap();
        let c = check::check_source(&src);
        let codes: Vec<&str> = c.diags.iter().map(|d| d.code).collect();
        if codes != ["W108"] {
            failures.push(format!("{stem} gives {codes:?}, not W108 once"));
            continue;
        }
        let found = chobo::spellings::find(c.book.as_ref().unwrap());
        let targets: Vec<&str> = found.iter().flat_map(|f| f.targets()).map(|t| t.name()).collect();
        if targets != *named {
            failures.push(format!("{stem}: W108 names {targets:?}"));
        }
        let file = format!("{stem}.book");
        for (lang, ext) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
            golden(&path.with_file_name(format!("{stem}.{ext}.txt")), &check::render(&file, &src, &c, lang));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The build of each target W108 names stops with E064, which is at the same name and says what
/// W108 says of that target's language; every other target builds.
#[test]
fn the_build_of_each_target_it_names_stops_with_e064_and_every_other_builds() {
    let mut failures = Vec::new();
    for f in forms() {
        let c = check::check_source(&f.src);
        let w = &c.diags[0];
        for t in Target::ALL {
            let d = target::check(&f.book, t);
            let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
            let built = target::build(&f.book, f.stem, t, &f.origin).is_ok();
            if f.named.contains(&t) {
                if codes != ["E064"] || built {
                    failures.push(format!("{} {}: the build gives {codes:?}", f.stem, t.name()));
                    continue;
                }
                let e = &d[0];
                if (e.line, e.col) != (w.line, w.col) || e.notes.is_empty() || !e.notes.iter().all(|n| w.notes.iter().any(|m| m.en == n.en && m.ja == n.ja)) {
                    failures.push(format!("{} {}: E064 says otherwise than W108:\n{}\n{}", f.stem, t.name(), e.render(Lang::En), w.render(Lang::En)));
                }
            } else if !codes.is_empty() || !built {
                failures.push(format!("{} {}: the build gives {codes:?}", f.stem, t.name()));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_renamed_books_give_nothing_and_build_every_target() {
    for f in renamed() {
        let c = check::check_source(&f.src);
        let codes: Vec<&str> = c.diags.iter().map(|d| d.code).collect();
        assert!(codes.is_empty(), "renamed/{} gives {codes:?}", f.stem);
        for t in Target::ALL {
            if let Err(d) = target::build(&f.book, f.stem, t, &f.origin) {
                panic!("renamed/{} {}: {}", f.stem, t.name(), d[0].message.en);
            }
        }
    }
}

/// Every file under tests/spellings is a book of FORMS, its renamed copy or one of its golden
/// files, and every Japanese book follows the English one it goes with.
#[test]
fn every_book_is_listed_in_english_and_in_japanese() {
    let listed: BTreeSet<&str> = FORMS.iter().map(|(s, _)| *s).collect();
    for (dir, exts) in [("tests/spellings", &[".book", ".en.txt", ".ja.txt"][..]), ("tests/spellings/renamed", &[".book"][..])] {
        for e in std::fs::read_dir(root().join(dir)).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                continue;
            }
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            let stem = exts.iter().find_map(|x| name.strip_suffix(x));
            assert!(stem.is_some_and(|s| listed.contains(s)), "{dir}/{name} is not a file of a book of FORMS");
        }
    }
    for (i, (stem, _)) in FORMS.iter().enumerate() {
        assert_eq!(!stem.is_ascii(), i % 2 == 1, "{stem}: each English book, then its Japanese one");
        for f in [format!("tests/spellings/renamed/{stem}.book"), format!("tests/spellings/{stem}.en.txt"), format!("tests/spellings/{stem}.ja.txt")] {
            assert!(root().join(&f).is_file(), "{f} is not there");
        }
    }
}

/// The JSON of W108: a warning, with one note for each language, and the book ok.
#[test]
fn the_json_of_w108() {
    let out = chobo().args(["check", "--format", "json", "tests/spellings/go_and_rust_transfers.book"]).current_dir(root()).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let f = &v["files"][0];
    assert_eq!(f["ok"], true);
    let d = &f["diagnostics"][0];
    assert_eq!(d["code"], "W108");
    assert_eq!(d["severity"], "warning");
    assert_eq!((d["line"].as_u64(), d["col"].as_u64()), (Some(19), Some(10)), "at the later of the two names");
    let notes: Vec<&str> = d["notes"].as_array().unwrap().iter().map(|n| n.as_str().unwrap()).collect();
    assert!(notes.len() == 2 && notes[0].starts_with("Go (") && notes[1].starts_with("Rust ("), "{notes:?}");
}

/// The names no book of tests/spellings takes, each in a book of its own: what W108 names of them.
#[test]
fn the_other_names() {
    let base = |more: &str| {
        format!(
            "book other v1\nunit pcs\naccount stock(sku: string) : pcs\n  at least 0 refused as out_of_stock\naccount suppliers : pcs outside\naccount customers : pcs outside\n\
             transfer receive(delivery: string, sku: string, qty: pcs)\n  key delivery, sku\n  move qty from suppliers to stock(sku)\n\
             transfer ship(order: string, sku: string, qty: pcs)\n  key order, sku\n  move qty from stock(sku) to customers\n{more}"
        )
    };
    let named = |src: &str| -> Vec<String> {
        let c = check::check_source(src);
        assert!(!chobo::diag::has_errors(&c.diags), "{src}");
        assert!(c.diags.iter().all(|d| d.code == "W108"), "{src}: {:?}", c.diags.iter().map(|d| d.code).collect::<Vec<_>>());
        chobo::spellings::find(c.book.as_ref().unwrap()).iter().map(|f| f.targets().iter().map(|t| t.name()).collect::<Vec<_>>().join(" ")).collect()
    };
    let python = PYTHON.join(" ");
    let ts = TYPESCRIPT.join(" ");
    let both: Vec<&str> = Target::ALL.iter().map(|t| t.name()).filter(|n| PYTHON.contains(n) || RUST.contains(n)).collect();
    let cases: &[(&str, String)] = &[
        // two parameters that are one in NFC: the syllable of Hangul and its letters
        (
            &format!(
                "transfer adjust(note: string, {0}: string, {1}: string, sku: string, qty: pcs)\n  key note, {0}, {1}\n  move qty from suppliers to stock(sku)\n",
                ['\u{D55C}'].iter().collect::<String>(),
                ['\u{1112}', '\u{1161}', '\u{11AB}'].iter().collect::<String>()
            ),
            both.join(" "),
        ),
        // what a method uses beside its parameters, written in a way NFKC makes it
        ("transfer adjust(note: string, _ｓｔｒ: string, sku: string, qty: pcs)\n  key note, _ｓｔｒ\n  move qty from suppliers to stock(sku)\n", python.clone()),
        // `__init__` makes `_Balances`, and an account of that name takes its place
        ("account __init__ : pcs outside\ntransfer count(note: string, qty: pcs)\n  key note\n  move qty from __init__ to customers\n", python.clone()),
        // Python changes a name that begins with `__` inside a class: a parameter's, an account's
        // and the one of an account's parameter
        ("transfer adjust(__note: string, sku: string, qty: pcs)\n  key __note, sku\n  move qty from suppliers to stock(sku)\n", python.clone()),
        (
            "account __bin(sku: string) : pcs\n  at least 0 refused as empty\ntransfer bin(note: string, sku: string, qty: pcs)\n  key note, sku\n  move qty from stock(sku) to __bin(sku)\ntransfer unbin(note: string, sku: string, qty: pcs)\n  key note, sku\n  move qty from __bin(sku) to customers\n",
            python.clone(),
        ),
        (
            "account bin(__sku: string) : pcs\n  at least 0 refused as empty\ntransfer bin_it(note: string, __sku: string, qty: pcs)\n  key note, __sku\n  move qty from customers to bin(__sku)\ntransfer unbin(note: string, __sku: string, qty: pcs)\n  key note, __sku\n  move qty from bin(__sku) to customers\n",
            // the parameter of the transfers too: both are found, at one name each
            format!("{python}\n{python}\n{python}"),
        ),
        // `self` is `self_` in Python and in Rust
        ("transfer adjust(note: string, self: string, self_: string, sku: string, qty: pcs)\n  key note, self, self_\n  move qty from suppliers to stock(sku)\n", both.join(" ")),
        // a name a Python method uses itself
        ("transfer adjust(note: string, _str: string, _str_: string, sku: string, qty: pcs)\n  key note, _str, _str_\n  move qty from suppliers to stock(sku)\n", python.clone()),
        // an object literal takes `__proto__` for the prototype: an account's parameter, and a
        // transfer's, each found at its own name
        (
            "account bin(__proto__: string) : pcs\n  at least 0 refused as empty\ntransfer bin_it(note: string, __proto__: string, qty: pcs)\n  key note, __proto__\n  move qty from customers to bin(__proto__)\ntransfer unbin(note: string, bin: string, qty: pcs)\n  key note, bin\n  move qty from bin(bin) to customers\n",
            format!("{ts}\n{ts}"),
        ),
    ];
    for (more, want) in cases {
        let src = base(more);
        let got = named(&src).join("\n");
        assert_eq!(&got, want, "{more}");
    }
    // The book's name is the Python client's module: a word Python keeps, or a module of the standard
    // library, any of them (`typing` and `random` are what the client imports itself), but not a
    // word Python keeps only where it reads one (`match`), nor a name that only starts like one.
    let py = python.as_str();
    for (name, want) in [("pass", py), ("typing", py), ("random", py), ("collections", py), ("match", ""), ("typing_", ""), ("abcd", "")] {
        let src = base("").replacen("book other v1", &format!("book {name} v1"), 1);
        assert_eq!(named(&src).join("\n"), want, "book {name}");
    }
    // A book named `main` is no Go package: the Go client takes the file's name instead, as for a
    // keyword, and nothing is warned of.
    let c = check::check_source(&base("").replacen("book other v1", "book main v1", 1));
    assert!(c.diags.is_empty(), "{:?}", c.diags.iter().map(|d| d.code).collect::<Vec<_>>());
    let files = target::build(c.book.as_ref().unwrap(), "shop", Target::PostgresGo, &target::Origin::named("main.book", b"")).unwrap();
    assert!(files.iter().all(|(_, text)| text.contains("\npackage shop\n")), "{:?}", files.iter().map(|(p, _)| p).collect::<Vec<_>>());
}

/// A package of ritsu's holds the Python client as `<package>.books.<name>` (ritsu's DESIGN 9.3),
/// so the book's name does not stop it there (ritsu gen refuses a word Python keeps itself, as a
/// name its index cannot import); whatever else breaks a client's own code stops it in a package
/// too.
#[test]
fn a_package_stops_at_what_breaks_the_clients_own_code() {
    for f in forms() {
        let module = matches!(
            f.stem,
            "python_keyword_module"
                | "Pythonのキーワードのモジュール"
                | "python_library_module"
                | "Pythonの標準ライブラリのモジュール"
                | "python_tigerbeetle"
                | "Pythonのtigerbeetle"
                | "python_nfkc_module"
                | "PythonのNFKCのモジュール"
                | "python_identifier_module"
                | "Pythonの識別子にならないモジュール"
        );
        for t in Target::ALL {
            let alone: Vec<&str> = target::check(&f.book, t).iter().map(|d| d.code).collect();
            let in_package: Vec<&str> = target::check_in(&f.book, t, true).iter().map(|d| d.code).collect();
            if module {
                assert!(in_package.is_empty(), "{} {}: a package gives {in_package:?}", f.stem, t.name());
            } else {
                assert_eq!(alone, in_package, "{} {}", f.stem, t.name());
            }
        }
    }
}

/// E063, by the language of each client (ritsu-emit's tables): `²` is no identifier's in any of
/// them, the vowel sign of `कीमत` and the number `Ⅻ` only Go's are without, `ͺ` begins no identifier
/// of Rust's or Python's, and the SQL quotes every name. A reason is an identifier only in Rust.
#[test]
fn a_name_a_language_takes_as_no_identifier_stops_its_clients() {
    use target::Lang as L;
    let book = |param: &str, reason: &str| {
        let src = format!(
            "book rooms v1\nunit sqm\naccount floor(site: string) : sqm\n  at least 0 refused as {reason}\naccount builders : sqm outside\naccount tenants : sqm outside\n\
             transfer build(permit: string, site: string, {param}: sqm)\n  key permit, site\n  move {param} from builders to floor(site)\n\
             transfer lease(contract: string, site: string, {param}: sqm)\n  key contract, site\n  move {param} from floor(site) to tenants\n"
        );
        let c = check::check_source(&src);
        // `check` leaves these to the build, as the other limits of a target (E060 to E063)
        assert!(c.diags.is_empty(), "{src}: {:?}", c.diags.iter().map(|d| d.code).collect::<Vec<_>>());
        c.book.unwrap()
    };
    let clients = [L::TypeScript, L::Python, L::Go, L::Rust];
    for (param, reason, stopped, said) in [
        ("area_m²", "no_room", &clients[..], "has ² (U+00B2)"),
        ("कीमत", "no_room", &[L::Go][..], "has ी (U+0940)"),
        ("Ⅻ", "no_room", &[L::Go][..], "begins with Ⅻ (U+216B)"),
        ("ͺarea", "no_room", &[L::Python, L::Rust][..], "begins with ͺ (U+037A)"),
        ("area", "no_room²", &[L::Rust][..], "has ² (U+00B2)"),
        ("area", "no_room", &[][..], ""),
    ] {
        let b = book(param, reason);
        for t in Target::ALL {
            let d = target::check(&b, t);
            let got: Vec<&str> = d.iter().map(|x| x.code).collect();
            if stopped.contains(&t.lang()) {
                assert!(got.contains(&"E063"), "{param} {reason} {}: {got:?}", t.name());
                let e = d.iter().find(|x| x.code == "E063").unwrap();
                assert!(e.message.en.contains(said), "{param} {reason} {}: {}", t.name(), e.message.en);
                let lang = match t.lang() {
                    L::TypeScript => "TypeScript",
                    L::Python => "Python",
                    L::Go => "Go",
                    _ => "Rust",
                };
                assert!(e.message.en.contains(&format!("no {lang} identifier")), "{}: {}", t.name(), e.message.en);
            } else {
                assert!(!got.contains(&"E063"), "{param} {reason} {}: {got:?}", t.name());
            }
        }
    }
}

/// Two books that are one package of the Go client, given together: the later book's W108 names
/// the earlier one, and what it says is the golden file beside it. `gen` writes each into the same
/// package of one directory, so the later client takes the place of the earlier's; each with the
/// book or the file renamed as the note says gives nothing, and has a package of its own.
#[test]
fn two_books_given_together_that_are_one_go_package() {
    let dir = root().join("tests/spellings/across");
    let read = |rel: String| std::fs::read_to_string(dir.join(rel)).unwrap();
    let gen_go = |rel: String, out: &Path| {
        let r = chobo().arg("gen").arg(dir.join(rel)).args(["--target", "go", "--books", "postgres", "--out"]).arg(out).output().unwrap();
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    };
    for (first, later, renamed, package, apart) in [("shop", "shop_upper", "renamed/shop_upper", "shop", "shop2"), ("本店", "支店", "renamed/shiten", "book", "shiten")] {
        let (a, b) = (read(format!("{first}.book")), read(format!("{later}.book")));
        let mut earlier = Vec::new();
        let mut c = check::check_source(&a);
        check::compare_across(&format!("{first}.book"), &mut c, &mut earlier);
        assert!(c.diags.is_empty(), "{first}: {:?}", c.diags.iter().map(|d| d.code).collect::<Vec<_>>());
        let mut c = check::check_source(&b);
        check::compare_across(&format!("{later}.book"), &mut c, &mut earlier);
        assert_eq!(c.diags.iter().map(|d| d.code).collect::<Vec<_>>(), ["W108"], "{later}");
        let file = format!("{later}.book");
        for (lang, ext) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
            golden(&dir.join(format!("{later}.{ext}.txt")), &check::render(&file, &b, &c, lang));
        }
        // the first of the two, as `chobo check` given them the other way round says it
        let mut earlier = Vec::new();
        let mut c = check::check_source(&b);
        check::compare_across(&format!("{later}.book"), &mut c, &mut earlier);
        assert!(c.diags.is_empty());
        let mut c = check::check_source(&a);
        check::compare_across(&format!("{first}.book"), &mut c, &mut earlier);
        assert_eq!(c.diags.iter().map(|d| d.code).collect::<Vec<_>>(), ["W108"], "{first}");
        // the note's way out
        let fixed = read(format!("{renamed}.book"));
        let mut earlier = Vec::new();
        let mut c = check::check_source(&a);
        check::compare_across(&format!("{first}.book"), &mut c, &mut earlier);
        let mut c = check::check_source(&fixed);
        let name = Path::new(renamed).file_name().unwrap().to_string_lossy().to_string();
        check::compare_across(&format!("{name}.book"), &mut c, &mut earlier);
        assert!(c.diags.is_empty(), "{renamed}: {:?}", c.diags.iter().map(|d| d.code).collect::<Vec<_>>());
        // what the two write: one directory, and the later client alone in it
        let work = TempDir::new("spellings-across");
        let (both, alone, two) = (work.path().join("both"), work.path().join("alone"), work.path().join("two"));
        gen_go(format!("{first}.book"), &both);
        gen_go(format!("{later}.book"), &both);
        gen_go(format!("{later}.book"), &alone);
        let client = |d: &Path, p: &str| std::fs::read_to_string(d.join("go").join(p).join("book.go")).unwrap();
        assert_eq!(client(&both, package), client(&alone, package), "{first} and {later}");
        gen_go(format!("{first}.book"), &two);
        gen_go(format!("{renamed}.book"), &two);
        for p in [package, apart] {
            assert!(two.join("go").join(p).join("book.go").is_file(), "{renamed}: {p}");
        }
    }
    // the command, given the two
    let r = chobo().arg("check").arg(dir.join("shop.book")).arg(dir.join("shop_upper.book")).output().unwrap();
    assert!(r.status.success());
    let said = String::from_utf8_lossy(&r.stdout).to_string() + &String::from_utf8_lossy(&r.stderr);
    assert_eq!(said.matches("W108").count(), 1, "{said}");
    assert!(said.contains("are one package, `shop`, in the Go client"), "{said}");
    // one book's versions are not compared
    let r = chobo().arg("check").arg(dir.join("shop.book")).arg(dir.join("renamed/shiten.book")).arg(dir.join("shop.book")).output().unwrap();
    assert!(!String::from_utf8_lossy(&r.stdout).contains("W108") && !String::from_utf8_lossy(&r.stderr).contains("W108"));
}

/// `information_schema` is a schema every database has: a superuser makes the book's functions in
/// it, and any other role stops. The renamed book loads as either.
#[test]
fn the_schema_postgresql_has_itself_stops_a_role_that_is_no_superuser() {
    if !need(Need::Postgres) {
        return;
    }
    let pg = match Postgres::start() {
        Ok(pg) => pg,
        Err(why) => {
            skip(&format!("{why}; the book named information_schema is not loaded"));
            return;
        }
    };
    let src = std::fs::read_to_string(root().join("tests/spellings/postgresql_schema.book")).unwrap().replace("book pg_stock v1", "book information_schema v1");
    let c = check::check_source(&src);
    assert_eq!(c.diags.iter().map(|d| d.code).collect::<Vec<_>>(), ["W108"]);
    let b = c.book.unwrap();
    let origin = target::Origin::named("information_schema.book", src.as_bytes());
    assert_eq!(target::check(&b, Target::Postgres).iter().map(|d| d.code).collect::<Vec<_>>(), ["E064"]);
    let work = TempDir::new("spellings-information-schema");
    let (_, sql) = target::unchecked(&b, "information_schema", Target::Postgres, &origin, None).unwrap().remove(0);
    let file = work.path().join("information_schema.sql");
    std::fs::write(&file, sql).unwrap();
    // a role that may create in the database, so that only the schema is what stops it
    let _ = pg.db().args(["-c", "create role chobo_application nosuperuser"]).output();
    let grant = pg.db().args(["-c", "do $$ begin execute format('grant all on database %I to chobo_application', current_database()); end $$"]).output().unwrap();
    assert!(grant.status.success(), "{}", String::from_utf8_lossy(&grant.stderr));
    let out = pg.db().args(["-c", "set role chobo_application"]).arg("-f").arg(&file).output().unwrap();
    let said = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(said.contains("permission denied for schema information_schema"), "{said}");
    eprintln!("compared: the schema information_schema stops PostgreSQL, as a role that is no superuser");
}

// ── the tools of each language ────────────────────────────────────────────

/// What the TypeScript probe does with each client: the book made on a driver that stops every
/// call with `SENT`, and every operation of every transfer, and every account's balance, called
/// with its arguments written as an object literal, as a caller writes them. One line of JSON a
/// file, with what is wrong ("broken"), or null when every call reaches the driver.
const PROBE_TS: &str = r#"import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const SENT = new Error("sent");
// every property is the driver itself, and calling it stops with SENT
const driver = new Proxy(function () {}, { get: (_, p) => (p === "then" ? undefined : driver), apply: () => { throw SENT; } });
// the arguments as a caller writes them: an object literal, `{ "sku": "v", "qty": 1n }`
const literal = (params) => (0, eval)("({" + params.map(([n, amount]) => JSON.stringify(n) + ": " + (amount ? "1n" : JSON.stringify("v"))).join(", ") + "})");

async function reaches(call) {
  try {
    await call();
  } catch (e) {
    if (e !== SENT) return String(e);
  }
  return null;
}

async function broken(spec) {
  let book;
  try {
    book = (await import(pathToFileURL(spec.file).href))[spec.make](driver);
  } catch (e) {
    return `Book cannot be made: ${e}`;
  }
  for (const t of spec.transfers) {
    const ops = book[t.name];
    for (const op of Object.keys(ops)) {
      const why = await reaches(() => ops[op](literal(t.params)));
      if (why) return `book.${t.name}.${op}: ${why}`;
    }
  }
  for (const a of spec.accounts) {
    const why = await reaches(() => (a.params.length ? book.balance[a.name](literal(a.params.map((p) => [p, false]))) : book.balance[a.name]()));
    if (why) return `book.balance.${a.name}: ${why}`;
  }
  return null;
}

for (const spec of JSON.parse(readFileSync(process.argv[2], "utf8"))) {
  console.log(JSON.stringify({ file: spec.file, broken: await broken(spec) }));
}
"#;

#[test]
fn typescript_breaks_where_w108_says() {
    if !need(Need::Node) {
        return;
    }
    if let Err(why) = node() {
        skip(&format!("{why}; the TypeScript clients of tests/spellings are not type-checked"));
        return;
    }
    let all = every_form();
    let work = TempDir::new("spellings-ts");
    let dir = ts_dir(work.path());
    let ts = [Target::PostgresTypeScript, Target::TigerBeetleTypeScript, Target::MySqlTypeScript];
    for (i, f) in all.iter().enumerate() {
        for t in ts {
            for (rel, text) in f.files(t) {
                let p = dir.join(key(i, t)).join(rel);
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(p, text).unwrap();
            }
        }
    }
    std::fs::write(
        dir.join("tsconfig.json"),
        r#"{"compilerOptions": {"strict": true, "noEmit": true, "target": "esnext", "module": "nodenext", "moduleResolution": "nodenext", "allowImportingTsExtensions": true, "verbatimModuleSyntax": true, "erasableSyntaxOnly": true, "noUnusedLocals": true, "types": ["node"]}, "include": ["*/*.ts"]}"#,
    )
    .unwrap();
    // tsc type-checks every file and names the file of each error: `f3_pg/x.ts(12,5): error TS2300: …`
    let out = Command::new(runner_dir().join("node_modules/.bin/tsc")).args(["-p", ".", "--pretty", "false"]).current_dir(&dir).output().unwrap();
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    // node calls each client as a caller does (PROBE_TS)
    let mut specs = Vec::new();
    for (i, f) in all.iter().enumerate() {
        for (t, make) in ts.iter().zip(["postgres", "tigerbeetle", "mysql"]) {
            for (rel, _) in f.files(*t) {
                let transfers: Vec<serde_json::Value> = f.book.transfers.iter().map(|x| serde_json::json!({"name": x.name, "params": x.params.iter().map(|p| (p.name.clone(), matches!(p.ty, chobo::model::Ty::Amount(_)))).collect::<Vec<_>>()})).collect();
                let accounts: Vec<serde_json::Value> = f.book.accounts.iter().map(|a| serde_json::json!({"name": a.name, "params": a.params})).collect();
                specs.push(serde_json::json!({"key": key(i, *t), "file": dir.join(key(i, *t)).join(rel).display().to_string(), "make": make, "transfers": transfers, "accounts": accounts}));
            }
        }
    }
    std::fs::write(dir.join("probe.mjs"), PROBE_TS).unwrap();
    std::fs::write(dir.join("probe.json"), serde_json::to_string(&specs).unwrap()).unwrap();
    let probe = Command::new("node").args(["--disable-warning=ExperimentalWarning", "probe.mjs", "probe.json"]).current_dir(&dir).output().unwrap();
    assert!(probe.status.success(), "the probe:\n{}", String::from_utf8_lossy(&probe.stderr));
    let ran: Vec<serde_json::Value> = String::from_utf8_lossy(&probe.stdout).lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(ran.len(), specs.len());
    let mut tally = Tally::default();
    for (i, f) in all.iter().enumerate() {
        for t in ts {
            let k = format!("{}/", key(i, t));
            let mut errors: Vec<String> = said.lines().filter(|l| l.starts_with(&k)).map(String::from).collect();
            let at = specs.iter().position(|s| s["key"] == key(i, t)).unwrap();
            errors.extend(ran[at]["broken"].as_str().map(|b| format!("node: {b}")));
            tally.add(f, t, target::Lang::TypeScript, (!errors.is_empty()).then(|| errors.join("\n")));
        }
    }
    tally.end("tsc and node");
}

/// What the Python probe is given of each client: the file, and where each name of the book is to
/// be found on `Book` as a caller reaches it.
const PROBE: &str = r#"# The Book of each client, as a caller reaches it: one line of JSON a file, with what is wrong
# ("broken"), or null when every transfer, every account's balance and expire() are where their
# names say.
import importlib.util, inspect, json, sys, unicodedata

class Nothing:
    """What a client is given in place of a connection: making a Book calls nothing on it."""
    def __getattr__(self, name):
        return lambda *a, **k: None

def broken(spec):
    s = importlib.util.spec_from_file_location(spec["module"], spec["file"])
    m = importlib.util.module_from_spec(s)
    sys.modules[spec["module"]] = m
    try:
        s.loader.exec_module(m)
    except BaseException as e:
        return f"the module does not load: {type(e).__name__}: {e}"
    try:
        book = getattr(m, spec["make"])(Nothing())
    except BaseException as e:
        return f"Book cannot be made: {type(e).__name__}: {e}"
    # what a caller writes as book.<name>, which Python reads in NFKC
    read = lambda name: unicodedata.normalize("NFKC", name)
    for attr, cls in spec["transfers"]:
        got = getattr(book, read(attr), None)
        if type(got).__name__ != cls:
            return f"book.{attr} is a {type(got).__name__}, not {cls}"
    balances = getattr(book, "balance", None)
    if type(balances).__name__ != "_Balances":
        return f"book.balance is a {type(balances).__name__}, not _Balances"
    for n in map(read, spec["accounts"]):
        got = getattr(balances, n, None)
        if not inspect.ismethod(got) or got.__func__ is not vars(type(balances)).get(n):
            return f"book.balance.{n} is a {type(got).__name__}, not the account's method"
    own = vars(type(book)).get("expire")
    if own is not None and getattr(getattr(book, "expire", None), "__func__", None) is not own:
        return f"book.expire is a {type(getattr(book, 'expire', None)).__name__}, not the method"
    return None

for spec in json.load(open(sys.argv[1], encoding="utf-8")):
    print(json.dumps({"file": spec["file"], "broken": broken(spec)}))
"#;

/// What the second Python probe does with each client, written beside the caller under its own
/// name (`<book>.py`), as `chobo gen` writes it: the caller's `import <book>`, and the `Book` it
/// makes. One line of JSON a client, with what is wrong ("broken"), or null.
const BY_NAME: &str = r#"import json, sys

class Nothing:
    def __getattr__(self, name):
        return lambda *a, **k: None

def broken(spec):
    name, beside = spec["name"], spec["dir"]
    try:
        code = compile(f"import {name} as m", "<caller>", "exec")
    except SyntaxError as e:
        return f"import {name}: SyntaxError: {e.msg}"
    sys.path.insert(0, beside)
    # a client that imported the library of its name before this one is another program's
    sys.modules.pop(name, None)
    ns = {}
    try:
        exec(code, ns)
        getattr(ns["m"], spec["make"])(Nothing())
    except BaseException as e:
        return f"import {name}: {type(e).__name__}: {e}"
    finally:
        sys.path.remove(beside)
        m = sys.modules.get(name)
        if m is not None and (getattr(m, "__file__", None) or "").startswith(beside):
            del sys.modules[name]
    return None

for spec in json.load(open(sys.argv[1], encoding="utf-8")):
    print(json.dumps({"dir": spec["dir"], "broken": broken(spec)}))
"#;

#[test]
fn python_breaks_where_w108_says() {
    use chobo::client::python::name;
    if !need(Need::Python) {
        return;
    }
    let py = match python() {
        Ok(p) => p,
        Err(why) => {
            skip(&format!("{why}; the Python clients of tests/spellings are not made"));
            return;
        }
    };
    let all = every_form();
    let work = TempDir::new("spellings-py");
    let mut specs = Vec::new();
    let mut at = Vec::new();
    for (i, f) in all.iter().enumerate() {
        for (t, make) in [(Target::PostgresPython, "postgres"), (Target::TigerBeetlePython, "tigerbeetle"), (Target::MySqlPython, "mysql")] {
            for (rel, text) in f.files(t) {
                let p = work.path().join(key(i, t)).join(rel);
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(&p, text).unwrap();
                let transfers: Vec<(String, String)> = f.book.transfers.iter().map(|x| (name(&x.name), format!("_{}Transfer", x.name))).collect();
                let accounts: Vec<String> = f.book.accounts.iter().map(|a| name(&a.name)).collect();
                specs.push(serde_json::json!({"module": key(i, t), "file": p.display().to_string(), "make": make, "transfers": transfers, "accounts": accounts}));
                at.push((i, t, p.display().to_string()));
            }
        }
    }
    std::fs::write(work.path().join("probe.py"), PROBE).unwrap();
    std::fs::write(work.path().join("probe.json"), serde_json::to_string(&specs).unwrap()).unwrap();
    let out = Command::new(&py).arg("-I").arg(work.path().join("probe.py")).arg(work.path().join("probe.json")).output().unwrap();
    assert!(out.status.success(), "the probe:\n{}", String::from_utf8_lossy(&out.stderr));
    let lines: Vec<serde_json::Value> = String::from_utf8_lossy(&out.stdout).lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines.len(), at.len());
    // the same clients under their own names, each in a directory of its own, imported by name
    let mut named = Vec::new();
    for (i, t, file) in &at {
        let beside = work.path().join("by_name").join(key(*i, *t));
        std::fs::create_dir_all(&beside).unwrap();
        let rel = format!("{}.py", all[*i].book.name);
        std::fs::copy(file, beside.join(&rel)).unwrap();
        let make = match t.backend() {
            target::Backend::Postgres => "postgres",
            target::Backend::TigerBeetle => "tigerbeetle",
            target::Backend::MySql => "mysql",
        };
        named.push(serde_json::json!({"dir": beside.display().to_string(), "name": all[*i].book.name, "make": make}));
    }
    std::fs::write(work.path().join("by_name.py"), BY_NAME).unwrap();
    std::fs::write(work.path().join("by_name.json"), serde_json::to_string(&named).unwrap()).unwrap();
    let out = Command::new(&py).arg("-I").arg(work.path().join("by_name.py")).arg(work.path().join("by_name.json")).output().unwrap();
    assert!(out.status.success(), "the probe of the names:\n{}", String::from_utf8_lossy(&out.stderr));
    let by_name: Vec<serde_json::Value> = String::from_utf8_lossy(&out.stdout).lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(by_name.len(), at.len());
    let mut tally = Tally::default();
    for (((i, t, file), got), imported) in at.iter().zip(&lines).zip(&by_name) {
        assert_eq!(got["file"].as_str(), Some(file.as_str()));
        let why: Vec<&str> = [got["broken"].as_str(), imported["broken"].as_str()].into_iter().flatten().collect();
        tally.add(&all[*i], *t, target::Lang::Python, (!why.is_empty()).then(|| why.join("\n")));
    }
    tally.end("Python");
}

#[test]
fn go_breaks_where_w108_says() {
    if !need(Need::Go) {
        return;
    }
    if let Err(why) = go() {
        skip(&format!("{why}; the Go clients of tests/spellings are not vetted"));
        return;
    }
    let all = every_form();
    let work = TempDir::new("spellings-go");
    let module = work.path();
    for f in ["go.mod", "go.sum"] {
        std::fs::copy(runner_dir().join("go").join(f), module.join(f)).unwrap();
    }
    let go = [Target::PostgresGo, Target::TigerBeetleGo, Target::MySqlGo];
    for (i, f) in all.iter().enumerate() {
        for t in go {
            for (rel, text) in f.files(t) {
                let p = module.join("books").join(key(i, t)).join(Path::new(&rel).file_name().unwrap());
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(p, text).unwrap();
            }
        }
    }
    // go vet type-checks each package, and names the file of each error it stops at
    let out = go_in(module).args(["vet", "./..."]).output().unwrap();
    let said = String::from_utf8_lossy(&out.stderr).to_string();
    let mut tally = Tally::default();
    for (i, f) in all.iter().enumerate() {
        for t in go {
            let k = format!("books/{}/", key(i, t));
            let errors: Vec<&str> = said.lines().filter(|l| l.contains(&k)).collect();
            tally.add(f, t, target::Lang::Go, (!errors.is_empty()).then(|| errors.join("\n")));
        }
    }
    assert!(out.status.success() || tally.broke > 0, "go vet:\n{said}");
    tally.end("go vet");
}

/// A crate of the runner's Cargo.toml and Cargo.lock in `dir`, with `modules` under src/books and
/// a main.rs that holds them to no warning; what `cargo check` said when it did not pass.
fn cargo_check(dir: &Path, name: &str, modules: &[(String, String)]) -> Result<(), String> {
    let toml = std::fs::read_to_string(runner_dir().join("rust/Cargo.toml")).unwrap();
    let toml = toml.replace("name = \"chobo-runner\"\npath", &format!("name = \"{name}\"\npath"));
    std::fs::create_dir_all(dir.join("src/books")).unwrap();
    std::fs::write(dir.join("Cargo.toml"), toml).unwrap();
    std::fs::copy(runner_dir().join("rust/Cargo.lock"), dir.join("Cargo.lock")).unwrap();
    let mut main = String::from("//! The Rust clients of the books of tests/spellings, each a module.\n#![deny(warnings)]\n// A name NFKC changes is uncommon_codepoints, which only a crate's root turns off (DESIGN 14.6).\n#![allow(uncommon_codepoints)]\n\n");
    for (key, text) in modules {
        std::fs::write(dir.join("src/books").join(format!("{key}.rs")), text).unwrap();
        main.push_str(&format!("#[path = \"books/{key}.rs\"]\npub mod {key};\n"));
    }
    main.push_str("\nfn main() {}\n");
    std::fs::write(dir.join("src/main.rs"), main).unwrap();
    let out = cargo_in(dir).args(["check", "--locked", "--quiet", "--color", "never"]).output().unwrap();
    if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).to_string()) }
}

/// The modules W108 names, a crate for each book, do not build, each with an error of its own
/// file; every other module, in one crate, builds with no warning.
#[test]
fn rust_breaks_where_w108_says() {
    if !need(Need::Rustc) {
        return;
    }
    if let Err(why) = rust() {
        skip(&format!("{why}; the Rust clients of tests/spellings are not built"));
        return;
    }
    let all = every_form();
    let work = TempDir::new("spellings-rust");
    let rust = [Target::PostgresRust, Target::TigerBeetleRust, Target::MySqlRust];
    let mut others = Vec::new();
    let mut tally = Tally::default();
    for (i, f) in all.iter().enumerate() {
        let mut named = Vec::new();
        for t in rust {
            let files = f.files(t);
            assert_eq!(files.len(), 1, "{}: {}", f.stem, t.name());
            let m = (key(i, t), files[0].1.clone());
            if f.breaks(t, target::Lang::Rust) { named.push((t, m)) } else { others.push((i, t, m)) }
        }
        if named.is_empty() {
            continue;
        }
        let modules: Vec<(String, String)> = named.iter().map(|(_, m)| m.clone()).collect();
        let said = cargo_check(&work.path().join(format!("f{i}")), &format!("spellings-{i}"), &modules).err();
        for (t, (k, _)) in &named {
            let file = format!("src/books/{k}.rs:");
            let why = said.as_ref().and_then(|s| s.lines().any(|l| l.contains(&file)).then(|| s.lines().filter(|l| l.starts_with("error")).take(3).collect::<Vec<_>>().join("\n")));
            tally.add(f, *t, target::Lang::Rust, why);
        }
    }
    let modules: Vec<(String, String)> = others.iter().map(|(_, _, m)| m.clone()).collect();
    let said = cargo_check(&work.path().join("others"), "spellings-others", &modules).err();
    let mut own = 0;
    for (i, t, (k, _)) in &others {
        let file = format!("src/books/{k}.rs:");
        let why = said.as_ref().and_then(|s| s.lines().any(|l| l.contains(&file)).then(|| s.lines().filter(|l| l.starts_with("error")).take(3).collect::<Vec<_>>().join("\n")));
        own += why.is_some() as usize;
        tally.add(&all[*i], *t, target::Lang::Rust, why);
    }
    if let Some(s) = &said {
        assert!(own > 0, "the crate of the modules W108 does not name does not build, and no module's own error says why:\n{s}");
    }
    tally.end("cargo check");
}

/// Each book's SQL loads in PostgreSQL but where W108 names `postgres`: there, either it does not
/// load (two functions of one name and of the same types: `create or replace` would change the
/// one before), or a call that sends its values with no type, as pg, the TypeScript client's
/// driver, does, and as the literals here do, reaches the account's balance instead of the
/// transfer's function of the same name.
#[test]
fn the_sql_of_postgresql_breaks_where_w108_says() {
    if !need(Need::Postgres) {
        return;
    }
    let pg = match Postgres::start() {
        Ok(pg) => pg,
        Err(why) => {
            skip(&format!("{why}; the SQL of tests/spellings is not loaded"));
            return;
        }
    };
    let all = every_form();
    let work = TempDir::new("spellings-sql");
    let mut tally = Tally::default();
    for (i, f) in all.iter().enumerate() {
        let (rel, sql) = f.files(Target::Postgres).remove(0);
        let file = work.path().join(format!("{i}-{rel}"));
        std::fs::write(&file, sql).unwrap();
        let out = pg.db().arg("-f").arg(&file).output().unwrap();
        // psql names the file and the line before what PostgreSQL said: only the latter is kept
        let said = String::from_utf8_lossy(&out.stderr).to_string();
        let mut why = (!out.status.success()).then(|| said.lines().filter_map(|l| l.split_once("ERROR:").map(|(_, e)| format!("ERROR:{e}"))).collect::<Vec<_>>().join("\n")).map(|e| if e.is_empty() { said.clone() } else { e });
        if why.is_none() {
            // the functions W108 says are one: an account's balance, called as an untyped driver calls
            for found in chobo::spellings::find(&f.book).into_iter().filter(|x| x.said.iter().any(|s| s.lang == target::Lang::Sql)) {
                let account = match found.subject {
                    Subject::Two(Name::Account(k), _) | Subject::Two(_, Name::Account(k)) => &f.book.accounts[k],
                    _ => continue,
                };
                let args: Vec<&str> = std::iter::repeat_n("'1'", 1 + account.params.len()).collect();
                let call = format!("select posted from {}.{}({})", chobo::postgres::ident(&f.book.name), chobo::postgres::ident(&format!("balance_{}", account.name)), args.join(", "));
                if pg.sql(&call).is_ok() {
                    why = Some(format!("{call}: the balance of `{}` answers it", account.name));
                }
            }
        }
        tally.add(f, Target::Postgres, target::Lang::Sql, why);
    }
    tally.end("PostgreSQL");
}
