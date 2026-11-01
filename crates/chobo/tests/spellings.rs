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
//! other, and the code of every renamed book passes them all: tsc, the Python client's `Book` as a
//! caller reaches it, go vet, cargo check, and PostgreSQL loading the SQL and calling it as a
//! driver that sends no types does.

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
    let both: Vec<&str> = Target::ALL.iter().map(|t| t.name()).filter(|n| PYTHON.contains(n) || RUST.contains(n)).collect();
    let cases: &[(&str, String)] = &[
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
    ];
    for (more, want) in cases {
        let src = base(more);
        let got = named(&src).join("\n");
        assert_eq!(&got, want, "{more}");
    }
    // A book named `main` is no Go package: the Go client takes the file's name instead, as for a
    // keyword, and nothing is warned of.
    let c = check::check_source(&base("").replacen("book other v1", "book main v1", 1));
    assert!(c.diags.is_empty(), "{:?}", c.diags.iter().map(|d| d.code).collect::<Vec<_>>());
    let files = target::build(c.book.as_ref().unwrap(), "shop", Target::PostgresGo, &target::Origin::named("main.book", b"")).unwrap();
    assert!(files.iter().all(|(_, text)| text.contains("\npackage shop\n")), "{:?}", files.iter().map(|(p, _)| p).collect::<Vec<_>>());
}

// ── the tools of each language ────────────────────────────────────────────

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
    let mut tally = Tally::default();
    for (i, f) in all.iter().enumerate() {
        for t in ts {
            let k = format!("{}/", key(i, t));
            let errors: Vec<&str> = said.lines().filter(|l| l.starts_with(&k)).collect();
            tally.add(f, t, target::Lang::TypeScript, (!errors.is_empty()).then(|| errors.join("\n")));
        }
    }
    tally.end("tsc");
}

/// What the Python probe is given of each client: the file, and where each name of the book is to
/// be found on `Book` as a caller reaches it.
const PROBE: &str = r#"# The Book of each client, as a caller reaches it: one line of JSON a file, with what is wrong
# ("broken"), or null when every transfer, every account's balance and expire() are where their
# names say.
import importlib.util, inspect, json, sys

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
    for attr, cls in spec["transfers"]:
        got = getattr(book, attr, None)
        if type(got).__name__ != cls:
            return f"book.{attr} is a {type(got).__name__}, not {cls}"
    balances = getattr(book, "balance", None)
    if type(balances).__name__ != "_Balances":
        return f"book.balance is a {type(balances).__name__}, not _Balances"
    for n in spec["accounts"]:
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
    let mut tally = Tally::default();
    for ((i, t, file), got) in at.iter().zip(&lines) {
        assert_eq!(got["file"].as_str(), Some(file.as_str()));
        tally.add(&all[*i], *t, target::Lang::Python, got["broken"].as_str().map(String::from));
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
    let mut main = String::from("//! The Rust clients of the books of tests/spellings, each a module.\n#![deny(warnings)]\n\n");
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
