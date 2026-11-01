//! The Rust kaname generates (DESIGN 10, 36), held to the reference interpreter. Every module of the
//! examples and of `tests/modules/` goes into one package, a crate, beside the Rust rulec, koyomi and
//! chobo write for the rules, the dates and the books they read (each language's own generator,
//! called as `ritsu gen` calls it), with the messages in English and in Japanese, built with
//! `tools/rust`'s Cargo.toml and Cargo.lock (the crates the books' clients name):
//!
//! - the crate builds with no warning on the 2021 and the 2024 editions, clippy's lints among them
//!   (`cargo clippy --all-targets -- -D warnings`), with the harness;
//! - the harness, which answers each call through the package's door (DESIGN 36.9), runs every vector
//!   of every function, the inputs outside what the parameters declare, and the values of types that
//!   hold themselves nested as deep as a host may give one and one deeper (DESIGN 28.1), and every
//!   answer is the reference interpreter's: the value, or the kind, the module, the line and the
//!   message of the failure, and a vector's whole line the line `kaname vectors` writes;
//! - the transfers the modules build, made through the functions of `glue/books/` and chobo's clients
//!   on PostgreSQL and on TigerBeetle, are answered and leave the balances and the holds as chobo's
//!   reference interpreter says (`the_generated_rust_makes_the_transfers_in_the_books`); with chobo's
//!   clients of the MySQL family the crate is built, and not run;
//! - the crates `tools/rust` pins are those chobo's clients name, at their versions
//!   (`the_crates_are_those_the_clients_name`);
//! - a `kaname::Date` of no calendar a host makes (2026-02-30) is outside the range of every
//!   parameter, the ends written or not (`a_day_of_no_calendar_is_outside_every_range`).
//!
//! Without cargo's clippy, the crate is held to rustc's lints alone and a SKIP line says so; without
//! PostgreSQL or TigerBeetle, the transfers are not made there, and a SKIP line says so.

mod common;

use chobo::target::Backend;
use common::targets::*;
use kaname::check::Checked;
use kaname::r#gen::Target;
use ritsu_base::json::Json;
use ritsu_base::text::Lang;
use ritsu_testkit::{Need, TempDir};
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

const LIMIT: Duration = Duration::from_secs(1800);

#[test]
fn the_generated_rust_answers_as_the_reference_does() {
    if !ritsu_testkit::need(Need::Rustc) {
        return;
    }
    let clippy = clippy();
    if !clippy {
        ritsu_testkit::skip("cargo clippy is not here; the Rust kaname generates is held to rustc's lints alone");
    }
    let t = TempDir::new("kaname-rust");
    let root = keep_in_for("answers").unwrap_or_else(|| t.path().to_path_buf());
    let report = deep(move || {
        let modules = modules();
        let checked: Vec<_> = modules.iter().map(|(c, _)| c.clone()).collect();
        let mut out = Vec::new();
        for lang in [Lang::En, Lang::Ja] {
            let base = root.join(format!("rust-{}", if lang == Lang::Ja { "ja" } else { "en" }));
            write_package(&base, Target::Rust, lang, &modules);
            let dir = package_dir(&base, Target::Rust);
            std::fs::create_dir_all(dir.join("harness")).unwrap();
            std::fs::write(dir.join("harness/main.rs"), kaname::r#gen::harness::program(&checked, Target::Rust, PY_NAME, GO_PACKAGE)).unwrap();
            let (cases, counts) = cases(&modules, lang, Target::Rust);
            std::fs::write(dir.join("calls.jsonl"), kaname::r#gen::harness::call_lines(&checked, &cases)).unwrap();
            // the 2021 edition first, then the 2024 edition, which the harness is built on
            for edition in ["2021", "2024"] {
                crate_files(&dir, edition, "harness");
                let mut c = cargo_in(&dir);
                if clippy {
                    c.args(["clippy", "--locked", "--quiet", "--all-targets", "--", "-D", "warnings"]);
                } else {
                    c.args(["rustc", "--locked", "--quiet", "--lib", "--", "-D", "warnings"]);
                }
                let r = ritsu_testkit::run(&mut c, LIMIT);
                assert!(r.ok, "the Rust kaname generates does not build with no warning on the {edition} edition ({lang:?}):\n{}", r.both());
            }
            let r = ritsu_testkit::run(cargo_in(&dir).args(["build", "--locked", "--quiet", "--bin", "harness"]), LIMIT);
            assert!(r.ok, "cargo build of the harness failed ({lang:?}):\n{}", r.both());
            let bin = rust_target().join("debug").join("harness");
            let r = ritsu_testkit::run(Command::new(&bin).arg(dir.join("calls.jsonl")).current_dir(&dir), LIMIT);
            assert!(r.ok, "the harness failed ({lang:?}):\n{}", r.both());
            let (agree, wrong) = kaname::r#gen::harness::compare_lines(&cases, &r.stdout);
            assert!(wrong.is_empty(), "the Rust ({lang:?}) answers otherwise than the reference:\n{}", wrong.join("\n"));
            out.push(counts.line(lang, agree, modules.len()));
            if lang == Lang::En {
                let v = ritsu_testkit::run(cargo_in(&dir).args(["--version"]), LIMIT);
                let rustc = ritsu_testkit::run(Command::new("rustc").arg("--version"), LIMIT);
                out.insert(0, format!("{} ({}, {}) passes the crate on the 2021 and the 2024 editions in English and in Japanese", if clippy { "cargo clippy --all-targets -- -D warnings" } else { "rustc -D warnings" }, v.stdout.trim(), rustc.stdout.trim()));
            }
        }
        out
    });
    for line in report {
        println!("{line}");
    }
}

#[test]
fn the_generated_rust_makes_the_transfers_in_the_books() {
    if !ritsu_testkit::need(Need::Rustc) {
        return;
    }
    let clippy = clippy();
    let t = TempDir::new("kaname-rust-books");
    let root = keep_in_for("books").unwrap_or_else(|| t.path().to_path_buf());
    let report = deep(move || {
        let modules = with_books(&modules());
        let checked: Vec<Rc<Checked>> = modules.iter().map(|(c, _)| c.clone()).collect();
        let books = built(&checked);
        // the vectors whose answers hold transfers, each with what the reference makes of them
        let (cases, _) = kaname::r#gen::harness::cases(&checked, &common::joined(), Lang::En);
        let mut runs = Vec::new();
        for c in kaname::r#gen::harness::of_target(&cases, Target::Rust) {
            let Some(want) = c.expected.as_ref() else { continue };
            let mut transfers = Vec::new();
            transfers_in(want, "", &mut transfers);
            if !transfers.is_empty() {
                let want = reference(&books, &transfers);
                runs.push((c, transfers, want));
            }
        }
        assert!(!runs.is_empty(), "no vector of the modules answers a transfer");
        // a case is two lines: what to read (its tenant, where its transfers are in the answer, the
        // accounts and the holds the reference names), then the call
        let mut lines = String::new();
        for (i, (c, transfers, want)) in runs.iter().enumerate() {
            let named = |key: &str, n: usize| Json::arr(want.get(key).and_then(Json::as_arr).unwrap_or_default().iter().map(|x| Json::arr(x.as_arr().unwrap_or_default().iter().take(n).cloned())));
            let meta = Json::obj([
                ("tenant", Json::str(format!("t{i}"))),
                ("paths", Json::arr(transfers.iter().map(|(p, _)| Json::str(p.clone())))),
                ("accounts", named("balances", 3)),
                ("holds", named("holds", 3)),
            ]);
            lines.push_str(&meta.compact());
            lines.push('\n');
            lines.push_str(&kaname::r#gen::harness::call_lines(&checked, std::slice::from_ref(c)));
        }
        let made: usize = runs.iter().map(|(_, ts, _)| ts.len()).sum();
        let mut outcomes: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for (_, _, want) in &runs {
            for o in want.get("outcomes").and_then(Json::as_arr).unwrap_or_default() {
                let o = o.as_str().unwrap_or("");
                *outcomes.entry(o.split(':').next().unwrap_or(o).to_string()).or_default() += 1;
            }
        }
        let shown: Vec<String> = outcomes.iter().map(|(k, n)| format!("{n} {k}")).collect();
        let mut out = vec![format!("{} vectors of {} modules answer {made} transfers of {} books: the reference makes {}", runs.len(), checked.len(), books.len(), shown.join(", "))];
        for (backend, name) in [(Backend::Postgres, "pg"), (Backend::TigerBeetle, "tb"), (Backend::MySql, "my")] {
            let base = root.join(format!("books-{name}"));
            write_package_for(&base, Target::Rust, Lang::En, &modules, backend);
            let dir = package_dir(&base, Target::Rust);
            std::fs::create_dir_all(dir.join("harness")).unwrap();
            std::fs::write(dir.join("harness/main.rs"), apply_program(&books, backend)).unwrap();
            std::fs::write(dir.join("cases.txt"), &lines).unwrap();
            let bin = format!("books-{name}");
            crate_files(&dir, "2024", &bin);
            if clippy {
                let r = ritsu_testkit::run(cargo_in(&dir).args(["clippy", "--locked", "--quiet", "--lib", "--", "-D", "warnings"]), LIMIT);
                assert!(r.ok, "the crate with chobo's clients of {backend:?} does not pass clippy:\n{}", r.both());
            }
            let r = ritsu_testkit::run(cargo_in(&dir).args(["build", "--locked", "--quiet", "--bin", &bin]), LIMIT);
            assert!(r.ok, "cargo build of the harness of the books on {backend:?} failed:\n{}", r.both());
            let exe = rust_target().join("debug").join(&bin);
            let (said, on) = match backend {
                Backend::MySql => {
                    out.push(format!("the crate with chobo's clients of the MySQL family builds{}, and is not run", if clippy { " and passes clippy" } else { "" }));
                    continue;
                }
                Backend::Postgres => {
                    let pg = match ritsu_testkit::pg::Postgres::start() {
                        Ok(pg) => pg,
                        Err(why) => {
                            ritsu_testkit::skip(&format!("{why}; the transfers are not made on PostgreSQL"));
                            continue;
                        }
                    };
                    // the SQL of each book, which chobo's client calls
                    for b in &books {
                        for (rel, text) in chobo::target::build(&b.model, &b.stem, chobo::target::Target::Postgres, &b.origin).unwrap_or_else(|_| panic!("{}", b.file.display())) {
                            let f = dir.join("sql").join(rel);
                            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
                            std::fs::write(&f, text).unwrap();
                            let r = ritsu_testkit::run(pg.psql("postgres").arg("-f").arg(&f), LIMIT);
                            assert!(r.ok, "{}: {}", f.display(), r.both());
                        }
                    }
                    let r = ritsu_testkit::run(Command::new(&exe).arg(dir.join("cases.txt")).arg(&pg.socket).arg(pg.port.to_string()).arg(&pg.user), LIMIT);
                    let v = ritsu_testkit::tools::version(pg.bin.join("postgres"), "--version");
                    (r, v.trim().to_string())
                }
                Backend::TigerBeetle => {
                    let tb = match ritsu_testkit::tigerbeetle::TigerBeetle::start() {
                        Ok(tb) => tb,
                        Err(why) => {
                            ritsu_testkit::skip(&format!("{why}; the transfers are not made on TigerBeetle"));
                            continue;
                        }
                    };
                    let r = ritsu_testkit::run(Command::new(&exe).arg(dir.join("cases.txt")).arg(&tb.address), LIMIT);
                    let v = ritsu_testkit::tigerbeetle::bin().map(|b| ritsu_testkit::tools::version(b, "version")).unwrap_or_default();
                    (r, format!("TigerBeetle {}", v.trim()))
                }
            };
            assert!(said.ok, "the harness of the books failed on {on}:\n{}", said.both());
            let got: Vec<&str> = said.stdout.lines().collect();
            assert_eq!(got.len(), runs.len(), "the harness answered {} of {} cases on {on}:\n{}", got.len(), runs.len(), said.both());
            let mut wrong = Vec::new();
            for ((c, _, want), line) in runs.iter().zip(&got) {
                let g = ritsu_base::json::parse(line).ok();
                let alike = g.as_ref().is_some_and(|g| ["outcomes", "balances", "holds"].iter().all(|k| g.get(k) == want.get(k)));
                if !alike && wrong.len() < 20 {
                    wrong.push(format!("{}.{} {}:\n  reference: {}\n  {on}: {line}", c.module, c.function, c.input.compact(), want.compact()));
                }
            }
            assert!(wrong.is_empty(), "the transfers on {on} come to otherwise than the reference says:\n{}", wrong.join("\n"));
            out.push(format!("on {on}: the {made} transfers come to what the reference says, and so do the balances and the holds"));
        }
        out
    });
    for line in report {
        println!("{line}");
    }
}

#[test]
fn the_crates_are_those_the_clients_name() {
    let toml = std::fs::read_to_string(tools().join("rust/Cargo.toml")).unwrap();
    for backend in [Backend::Postgres, Backend::TigerBeetle, Backend::MySql] {
        for c in chobo::client::rust::crates(backend) {
            let line = toml.lines().find(|l| l.starts_with(&format!("{} = ", c.name))).unwrap_or_else(|| panic!("tools/rust/Cargo.toml has no {}", c.name));
            let version = c.version.trim_start_matches('=');
            assert!(line.contains(&format!("\"={version}\"")), "tools/rust/Cargo.toml pins {} otherwise than chobo's clients name it ({version}): {line}", c.name);
            if !c.default_features {
                assert!(line.contains("default-features = false"), "{}: {line}", c.name);
            }
            for f in c.features {
                assert!(line.contains(&format!("\"{f}\"")), "tools/rust/Cargo.toml does not turn on the feature {f} of {}: {line}", c.name);
            }
        }
    }
}

/// The modules that build a transfer, and the modules each reads, which go into the package with it.
fn with_books(modules: &[(Rc<Checked>, String)]) -> Vec<(Rc<Checked>, String)> {
    let mut out: Vec<(Rc<Checked>, String)> = Vec::new();
    for (c, path) in modules {
        if kaname::r#gen::books::built(std::slice::from_ref(c)).is_empty() {
            continue;
        }
        for canon in c.world.modules.keys().chain(std::iter::once(&c.canon)) {
            if !out.iter().any(|(x, _)| x.canon == *canon)
                && let Some(m) = modules.iter().find(|(x, _)| x.canon == *canon)
            {
                out.push(m.clone());
            }
        }
        if !out.iter().any(|(x, _)| x.canon == c.canon) {
            out.push((c.clone(), path.clone()));
        }
    }
    out
}

/// A book the modules make transfers of: its file and its stem, what chobo knows of it, chobo's model
/// of it (whose names its Rust client has) and where its SQL comes from, and the operations the
/// modules build.
struct Built {
    file: PathBuf,
    stem: String,
    facts: ritsu_ports::BookFacts,
    model: chobo::model::Book,
    origin: ritsu_emit::header::Origin,
    ops: Vec<kaname::ty::TransferId>,
}

fn built(checked: &[Rc<Checked>]) -> Vec<Built> {
    kaname::r#gen::books::built(checked)
        .into_iter()
        .map(|(canon, ops)| {
            let (facts, file) = checked
                .iter()
                .find_map(|c| {
                    let p = kaname::value::Program::of(c);
                    p.books.get(&canon).map(|f| ((**f).clone(), p.file(&canon)))
                })
                .expect("a module reads the book");
            let (src, c) = chobo::check::check_file(&file).unwrap();
            let stem = file.file_name().unwrap().to_string_lossy().trim_end_matches(".book").to_string();
            let origin = ritsu_emit::header::Origin::at(&file.to_string_lossy(), src.as_bytes());
            Built { file, stem, facts, model: c.book.clone().expect("the book passes chobo's check"), origin, ops: ops.into_iter().map(|(id, _)| id).collect() }
        })
        .collect()
}

/// The transfers in an answer of the reference interpreter, in the order the answer writes them:
/// each object of `book`, `transfer`, `op` and `args`, with where it is in the answer (a JSON
/// pointer, `/out/capture`).
fn transfers_in(j: &Json, at: &str, out: &mut Vec<(String, Json)>) {
    match j {
        Json::Obj(fs) if ["book", "transfer", "op", "args"].iter().all(|k| fs.iter().any(|(x, _)| x == k)) => out.push((at.to_string(), j.clone())),
        Json::Obj(fs) => fs.iter().for_each(|(k, v)| transfers_in(v, &format!("{at}/{}", k.replace('~', "~0").replace('/', "~1")), out)),
        Json::Arr(xs) => xs.iter().enumerate().for_each(|(i, x)| transfers_in(x, &format!("{at}/{i}"), out)),
        _ => {}
    }
}

/// The operation of chobo's reference interpreter a transfer of an answer is: the arguments the
/// operation takes, an amount as its number and a string as itself, and a `post`'s amounts where it
/// gives them.
fn book_call(facts: &ritsu_ports::BookFacts, t: &Json) -> ritsu_ports::BookCall {
    let s = |k: &str| t.get(k).and_then(Json::as_str).unwrap_or("").to_string();
    let (transfer, op) = (s("transfer"), s("op"));
    let tr = facts.transfers.iter().find(|x| x.name == transfer).expect("a transfer of the book");
    let mut call = ritsu_ports::BookCall { transfer: transfer.clone(), op: op.clone(), args: Vec::new(), amounts: None };
    let mut amounts = Vec::new();
    for (i, optional) in kaname::r#gen::books::op_params(facts, &transfer, &op) {
        let p = &tr.params[i];
        let Some(v) = t.get("args").and_then(|a| a.get(&p.name)) else { continue };
        if optional {
            amounts.push((p.name.clone(), v.as_int().expect("an amount")));
        } else {
            call.args.push((p.name.clone(), if p.unit.is_some() { Ok(v.as_int().expect("an amount")) } else { Err(v.as_str().expect("a string").to_string()) }));
        }
    }
    if !amounts.is_empty() {
        call.amounts = Some(amounts);
    }
    call
}

/// What chobo's reference interpreter makes of a case's transfers, each book opened with nothing in
/// it: the outcome of each (`done`, `done_before`, or `refused:` and the reason), then the balance of
/// every account they name and the state of every hold, by book in the order the case first names
/// each.
fn reference(books: &[Built], transfers: &[(String, Json)]) -> Json {
    use ritsu_ports::{BookOutcome, Books};
    let mut open: Vec<(usize, Box<dyn ritsu_ports::Ledger>)> = Vec::new();
    let mut outcomes = Vec::new();
    for (_, t) in transfers {
        let name = t.get("book").and_then(Json::as_str).unwrap_or("");
        let b = books.iter().position(|b| b.facts.name == name).unwrap_or_else(|| panic!("no book {name}"));
        if !open.iter().any(|(x, _)| *x == b) {
            let Ok(ledger) = chobo::ports::Engine.open(&books[b].file) else { panic!("{}: chobo does not open it", books[b].file.display()) };
            open.push((b, ledger));
        }
        let ledger = &mut open.iter_mut().find(|(x, _)| *x == b).expect("opened").1;
        outcomes.push(Json::str(match ledger.apply(&book_call(&books[b].facts, t)) {
            Ok(BookOutcome::Done) => "done".to_string(),
            Ok(BookOutcome::DoneBefore) => "done_before".to_string(),
            Ok(BookOutcome::Refused(r)) => format!("refused:{r}"),
            Err(e) => format!("error: {}", e.en),
        }));
    }
    let strs = |xs: &[String]| Json::arr(xs.iter().map(|x| Json::str(x.clone())));
    let (mut balances, mut holds) = (Vec::new(), Vec::new());
    for (b, ledger) in &open {
        let name = &books[*b].facts.name;
        for (kind, args, x) in ledger.accounts() {
            balances.push(Json::arr([Json::str(name.clone()), Json::str(kind), strs(&args), Json::int(x.posted), Json::int(x.held_in), Json::int(x.held_out)]));
        }
        for (transfer, key, state) in ledger.holds() {
            holds.push(Json::arr([Json::str(name.clone()), Json::str(transfer), strs(&key), Json::str(state)]));
        }
    }
    Json::obj([("outcomes", Json::arr(outcomes)), ("balances", Json::arr(balances)), ("holds", Json::arr(holds))])
}

/// The harness of the books on `backend`: for each case, two lines (what to read, then the call), it
/// answers the call through the package's door, makes each transfer of the answer the case points to
/// through the function of `glue/books/` and chobo's client of the backend, on the case's tenant, and
/// reads back the balances and the holds the case names, a line of JSON each. The transfers of the
/// answer are read into the types of `glue/books/` by the book's names of them, as a host reads them
/// off the wire.
fn apply_program(books: &[Built], backend: Backend) -> String {
    let lit = ritsu_emit::lit::rust;
    let id = ritsu_emit::ident::rust_name;
    let (mut variants, mut decode, mut apply, mut balance, mut status) = (String::new(), String::new(), String::new(), String::new(), String::new());
    for (bi, b) in books.iter().enumerate() {
        let module = &b.facts.rust.module;
        let client = format!("pkg::books::{}", id(module));
        let glue = format!("pkg::glue::books::{}", id(module));
        let glue_apply = format!("pkg::glue::books::{}", id(&format!("{module}_apply")));
        let open = match backend {
            Backend::Postgres => format!("{client}::postgres(db, tenant)"),
            Backend::TigerBeetle => format!("{client}::tigerbeetle(db, tenant)"),
            Backend::MySql => format!("{client}::mysql(db, tenant, false)"),
        };
        let book = lit(&b.facts.name);
        variants.push_str(&format!("    B{bi}({glue}::Call),\n"));
        apply.push_str(&format!(
            "        Call::B{bi}(c) => match {glue_apply}::apply(&{open}, c).await {{\n            Ok(o) => shown(o.result(), o.reason().map(|r| r.as_str())),\n            Err(e) => format!(\"error: {{e}}\"),\n        }},\n"
        ));
        for op in &b.ops {
            let tr = b.facts.transfers.iter().find(|t| t.name == op.transfer).expect("a transfer of the book");
            let names = kaname::r#gen::books::param_names(Target::Rust, &b.facts, &op.transfer);
            let fields: Vec<String> = kaname::r#gen::books::op_params(&b.facts, &op.transfer, &op.op)
                .into_iter()
                .map(|(i, optional)| {
                    let p = &tr.params[i];
                    let key = lit(&p.name);
                    let value = match &p.unit {
                        None => format!("text(args, {key})?"),
                        Some(u) => match kaname::names::book_unit(&b.facts, u).and_then(|u| u.type_name()) {
                            Some(t) => format!("pkg::units::{t}(amount(args, {key})?)"),
                            None => format!("amount(args, {key})?"),
                        },
                    };
                    if optional { format!("{}: if args.get({key}).is_some() {{ Some({value}) }} else {{ None }}", names[i]) } else { format!("{}: {value}", names[i]) }
                })
                .collect();
            let ty = kaname::r#gen::books::op_type(Target::Rust, &op.transfer, &op.op);
            decode.push_str(&format!("        ({book}, {}, {}) => Call::B{bi}({glue}::{ty} {{ {} }}.into()),\n", lit(&op.transfer), lit(&op.op), fields.join(", ")));
        }
        let methods = chobo::client::rust::balance_methods(&b.model);
        let types = chobo::client::rust::account_types(&b.model);
        for (k, a) in b.model.accounts.iter().enumerate() {
            let fs = chobo::client::rust::account_fields(a);
            let arg = if fs.is_empty() { String::new() } else { format!("{client}::{}Account {{ {} }}", types[k], fs.iter().enumerate().map(|(j, f)| format!("{f}: &args[{j}]")).collect::<Vec<_>>().join(", ")) };
            balance.push_str(&format!("        ({book}, {}) => {open}.balance().{}({arg}).await.map(|b| [b.posted, b.held_in, b.held_out]).map_err(|e| e.to_string()),\n", lit(&a.name), methods[k]));
        }
        for (k, t) in b.facts.transfers.iter().enumerate() {
            if t.pending.is_none() {
                continue;
            }
            let c = &b.facts.rust.transfers[k];
            let key: Vec<String> = t.key.iter().enumerate().map(|(j, n)| format!("{}: &key[{j}]", c.params[t.params.iter().position(|p| &p.name == n).expect("a parameter of the key")])).collect();
            status.push_str(&format!(
                "        ({book}, {}) => {open}.{}().status({client}::{}Key {{ {} }}).await.map(|s| s.map_or(\"none\", |s| s.as_str()).to_string()).map_err(|e| e.to_string()),\n",
                lit(&t.name),
                c.member,
                c.type_name,
                key.join(", ")
            ));
        }
    }
    let (on, db, connect) = match backend {
        Backend::Postgres => (
            "PostgreSQL",
            "sqlx::PgPool",
            "let options = sqlx::postgres::PgConnectOptions::new().socket(&args[0]).port(args[1].parse().expect(\"a port\")).username(&args[2]).database(\"postgres\");\n    sqlx::postgres::PgPoolOptions::new().max_connections(4).connect_with(options).await.expect(\"a connection\")",
        ),
        Backend::TigerBeetle => ("TigerBeetle", "tigerbeetle::Client", "tigerbeetle::Client::new(0, &args[0]).expect(\"a client\")"),
        Backend::MySql => (
            "the MySQL family",
            "sqlx::MySqlPool",
            "let options = sqlx::mysql::MySqlConnectOptions::new().host(&args[0]).port(args[1].parse().expect(\"a port\")).username(&args[2]).database(&args[3]).pipes_as_concat(false);\n    sqlx::mysql::MySqlPoolOptions::new().max_connections(4).connect_with(options).await.expect(\"a connection\")",
        ),
    };
    format!(
        r#"//! The harness of kaname's tests of the books (tests/rust.rs), on {on}: for each case, a line of
//! what to read and a line of the call, it answers the call through the package's door, makes each
//! transfer of the answer the case points to through `glue/books/` on the case's tenant, and reads
//! back the balances and the holds the case names: a line of JSON each.

use serde_json::{{json, Value}};

type Db = {db};

/// A transfer of a book the modules build.
enum Call {{
{variants}}}

fn text(args: &Value, key: &str) -> Result<String, String> {{
    args.get(key).and_then(Value::as_str).map(str::to_string).ok_or_else(|| format!("the argument {{key}} is no string"))
}}

fn amount(args: &Value, key: &str) -> Result<i64, String> {{
    args.get(key).and_then(Value::as_i64).ok_or_else(|| format!("the argument {{key}} is no amount"))
}}

/// The transfer `t` of an answer (`{{"book","transfer","op","args"}}`), as the type of `glue/books/`.
fn decode(t: &Value) -> Result<Call, String> {{
    let args = t.get("args").ok_or("a transfer with no arguments")?;
    let s = |k: &str| t.get(k).and_then(Value::as_str).unwrap_or("");
    Ok(match (s("book"), s("transfer"), s("op")) {{
{decode}        _ => return Err(format!("no such transfer: {{t}}")),
    }})
}}

fn shown(result: &str, reason: Option<&str>) -> String {{
    match reason {{
        Some(r) => format!("{{result}}:{{r}}"),
        None => result.to_string(),
    }}
}}

/// Makes `call` through the function of `glue/books/`: its outcome.
async fn apply(db: &Db, tenant: &str, call: &Call) -> String {{
    match call {{
{apply}    }}
}}

/// The balance of an account: what is posted, what holds put in, and what holds take out.
async fn balance(db: &Db, tenant: &str, book: &str, kind: &str, args: &[String]) -> Result<[i64; 3], String> {{
    match (book, kind) {{
{balance}        _ => Err(format!("no such account: {{book}}.{{kind}}")),
    }}
}}

/// The state of a hold, `none` where there is none.
async fn status(db: &Db, tenant: &str, book: &str, transfer: &str, key: &[String]) -> Result<String, String> {{
    match (book, transfer) {{
{status}        _ => Err(format!("no such hold: {{book}}.{{transfer}}")),
    }}
}}

fn strings(v: &Value) -> Vec<String> {{
    v.as_array().map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()
}}

async fn case(db: &Db, meta: &Value, call: &str) -> Result<Value, String> {{
    let tenant = meta["tenant"].as_str().unwrap_or("");
    let said = pkg::glue::door::answer(call);
    let answer: Value = serde_json::from_str(&said).map_err(|e| format!("{{e}}: {{said}}"))?;
    let mut outcomes = Vec::new();
    for p in meta["paths"].as_array().into_iter().flatten() {{
        let p = p.as_str().unwrap_or("");
        let t = answer.pointer(p).ok_or_else(|| format!("the answer has no transfer at {{p}}: {{said}}"))?;
        outcomes.push(apply(db, tenant, &decode(t)?).await);
    }}
    let mut balances = Vec::new();
    for a in meta["accounts"].as_array().into_iter().flatten() {{
        let (book, kind, args) = (a[0].as_str().unwrap_or(""), a[1].as_str().unwrap_or(""), strings(&a[2]));
        let [posted, held_in, held_out] = balance(db, tenant, book, kind, &args).await?;
        balances.push(json!([book, kind, args, posted, held_in, held_out]));
    }}
    let mut holds = Vec::new();
    for h in meta["holds"].as_array().into_iter().flatten() {{
        let (book, transfer, key) = (h[0].as_str().unwrap_or(""), h[1].as_str().unwrap_or(""), strings(&h[2]));
        let state = status(db, tenant, book, transfer, &key).await?;
        holds.push(json!([book, transfer, key, state]));
    }}
    Ok(json!({{"outcomes": outcomes, "balances": balances, "holds": holds}}))
}}

async fn connect(args: &[String]) -> Db {{
    {connect}
}}

fn main() {{
    let args: Vec<String> = std::env::args().skip(1).collect();
    let text = std::fs::read_to_string(&args[0]).expect("the file of the cases");
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("a runtime");
    rt.block_on(async {{
        let db = connect(&args[1..]).await;
        let lines: Vec<&str> = text.lines().collect();
        for pair in lines.chunks(2) {{
            let meta: Value = serde_json::from_str(pair[0]).expect("a case");
            match case(&db, &meta, pair.get(1).copied().unwrap_or("")).await {{
                Ok(v) => println!("{{v}}"),
                Err(e) => println!("{{}}", json!({{"error": e}})),
            }}
        }}
    }});
}}
"#
    )
}

/// The program of [`a_day_of_no_calendar_is_outside_every_range`]: the checks of the entry called as a
/// host calls them, with dates a host can make, each answer a line.
const CALENDAR: &str = r#"//! Dates of no calendar given to the checks of the entry.

use pkg::glue::kaname::Date;

fn main() {
    let d = |year, month, day| Date { year, month, day };
    let said = |r: Result<(), pkg::glue::kaname::KanameError>| r.map_or_else(|e| e.to_string(), |()| "ok".into());
    for a in [d(2026, 2, 30), d(2026, 13, 1), d(2026, 3, 2)] {
        println!("calc {a}: {}", said(pkg::glue::calc::check_days(a, d(2026, 3, 1))));
    }
    for a in [d(2026, 2, 30), d(10000, 1, 1), d(2026, 3, 2)] {
        println!("open {a}: {}", said(pkg::glue::open::check_days(a, d(2026, 3, 1))));
    }
}
"#;

/// A host of Rust can make a `kaname::Date` of no calendar: the entry finds it outside the range of
/// the parameter, where the range writes both ends (calc's `days`: the day number of 2026-02-30 is
/// that of 2026-03-02, between the ends) and where it leaves them to the type (open's `days`). The
/// door, which reads JSON, refuses such a day before the entry (DESIGN 36.11).
#[test]
fn a_day_of_no_calendar_is_outside_every_range() {
    if !ritsu_testkit::need(Need::Rustc) {
        return;
    }
    let t = TempDir::new("kaname-rust-calendar");
    let root = t.path().to_path_buf();
    let said = deep(move || {
        let modules: Vec<_> = modules().into_iter().filter(|(_, s)| s == "tests/modules/calc.kn" || s == "tests/modules/open.kn").collect();
        assert_eq!(modules.len(), 2);
        write_package(&root, Target::Rust, Lang::En, &modules);
        let dir = package_dir(&root, Target::Rust);
        std::fs::create_dir_all(dir.join("harness")).unwrap();
        std::fs::write(dir.join("harness/main.rs"), CALENDAR).unwrap();
        crate_files(&dir, "2024", "calendar");
        let r = ritsu_testkit::run(cargo_in(&dir).args(["run", "--locked", "--quiet", "--bin", "calendar"]), LIMIT);
        assert!(r.ok, "{}", r.both());
        r.stdout
    });
    assert_eq!(
        said,
        "calc 2026-02-30: The argument `a` of `days` is 2026-02-30, outside its range >=2026-01-01 <=2026-12-31\n\
         calc 2026-13-01: The argument `a` of `days` is 2026-13-01, outside its range >=2026-01-01 <=2026-12-31\n\
         calc 2026-03-02: ok\n\
         open 2026-02-30: The argument `first` of `days` is 2026-02-30, outside its range >=0001-01-01 <=9999-12-31\n\
         open 10000-01-01: The argument `first` of `days` is 10000-01-01, outside its range >=0001-01-01 <=9999-12-31\n\
         open 2026-03-02: ok\n"
    );
    print!("{said}");
}
