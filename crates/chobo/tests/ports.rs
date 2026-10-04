//! ritsu's ports, as chobo answers them (ritsu's DESIGN 3.2, PLAN D.2): the facts of a book are
//! what `chobo api` says of it; a ledger opened through the port comes out of every generated
//! scenario as the reference interpreter does; and the definition of a unit, an account or a
//! transfer is the one yuen's DESIGN 3.2 settled, with the hashes its prototype computed.

use chobo::ports::Engine;
use ritsu_ports::{BookCall, BookOutcome, Books, Items, MoveAmount};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn books() -> Vec<String> {
    let mut out = Vec::new();
    for dir in ["examples", "tests/books"] {
        let mut all = Vec::new();
        ritsu_base::paths::walk(&root(), dir, &[], &mut all);
        out.extend(all.into_iter().filter(|f| f.ends_with(".book")));
    }
    out.sort();
    out
}

#[test]
fn the_facts_are_what_chobo_api_says() {
    for path in books() {
        let src = std::fs::read_to_string(root().join(&path)).unwrap();
        let c = chobo::check::check_source(&src);
        let book = c.book.clone().unwrap();
        let stem = Path::new(&path).file_stem().unwrap().to_string_lossy().to_string();
        let api = chobo::api::api(&book, &src, c.report.as_ref().unwrap(), &stem);
        let f = Engine.facts(&root().join(&path)).unwrap_or_else(|e| panic!("{path}: {e:?}"));
        assert_eq!((f.name.as_str(), f.version as u64, f.sha256.as_str()), (api["book"].as_str().unwrap(), api["version"].as_u64().unwrap(), api["source_sha256"].as_str().unwrap()), "{path}");
        let units: Vec<(String, u32)> = api["units"].as_array().unwrap().iter().map(|u| (u["name"].as_str().unwrap().to_string(), u["scale"].as_u64().unwrap() as u32)).collect();
        assert_eq!(f.units.iter().map(|u| (u.name.clone(), u.scale)).collect::<Vec<_>>(), units, "{path}");
        for (a, j) in f.accounts.iter().zip(api["accounts"].as_array().unwrap()) {
            assert_eq!(a.name, j["name"].as_str().unwrap(), "{path}");
            assert_eq!(a.params, j["params"].as_array().unwrap().iter().map(|p| p.as_str().unwrap().to_string()).collect::<Vec<_>>(), "{path}");
            assert_eq!((a.unit.as_str(), a.outside), (j["unit"].as_str().unwrap(), j["outside"].as_bool().unwrap()), "{path}");
            for (b, k) in [(&a.lower, "lower"), (&a.upper, "upper")] {
                let want = j[k].as_object().map(|o| (o["value"].as_i64().unwrap() as i128, o["refused_as"].as_str().unwrap().to_string()));
                assert_eq!(b.as_ref().map(|b| (b.value, b.refusal.clone())), want, "{path} {}", a.name);
            }
        }
        let machines = api["machines"].as_array().unwrap();
        for (t, j) in f.transfers.iter().zip(api["transfers"].as_array().unwrap()) {
            assert_eq!(t.name, j["name"].as_str().unwrap(), "{path}");
            let params: Vec<(String, Option<String>)> = j["params"].as_array().unwrap().iter().map(|p| (p["name"].as_str().unwrap().to_string(), p["unit"].as_str().map(String::from))).collect();
            assert_eq!(t.params.iter().map(|p| (p.name.clone(), p.unit.clone())).collect::<Vec<_>>(), params, "{path}");
            assert_eq!(t.key, j["key"].as_array().unwrap().iter().map(|k| k.as_str().unwrap().to_string()).collect::<Vec<_>>(), "{path}");
            let pending = match (&j["pending"]["expires_after_seconds"], &j["pending"]["never_expires"]) {
                (serde_json::Value::Number(n), _) => Some(ritsu_ports::Expiry::After(n.as_u64().unwrap())),
                (_, serde_json::Value::Bool(true)) => Some(ritsu_ports::Expiry::Never),
                _ => None,
            };
            assert_eq!(t.pending, pending, "{path}");
            for (m, jm) in t.moves.iter().zip(j["moves"].as_array().unwrap()) {
                match &m.amount {
                    MoveAmount::Param(p) => assert_eq!(p, jm["amount"]["param"].as_str().unwrap()),
                    MoveAmount::Literal(v) => assert_eq!(*v as i64, jm["amount"]["literal"].as_i64().unwrap()),
                }
                assert_eq!(m.from.account, jm["from"]["account"].as_str().unwrap(), "{path}");
                assert_eq!(m.to.account, jm["to"]["account"].as_str().unwrap(), "{path}");
                assert_eq!(m.from.args.len(), jm["from"]["args"].as_array().unwrap().len(), "{path}");
            }
            let ops = j["operations"].as_object().unwrap();
            let want: Vec<(String, Vec<String>)> = ops.iter().map(|(op, o)| (op.clone(), o["refusals"].as_array().unwrap().iter().map(|r| r["name"].as_str().unwrap().to_string()).collect())).collect();
            assert_eq!(t.refusals, want, "{path} {}", t.name);
            // a hold's life, as `machines` writes it
            match (&t.machine, machines.iter().find(|m| m["name"] == j["name"])) {
                (None, None) => {}
                (Some(m), Some(jm)) => {
                    let cert = &jm["certificate"];
                    let table = &cert["tables"][0];
                    assert_eq!(m.states, jm["states"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect::<Vec<_>>());
                    assert_eq!(m.axes.iter().map(|a| a.coords.clone()).collect::<Vec<_>>(), table["axes"].as_array().unwrap().iter().map(|a| a["coords"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect::<Vec<_>>()).collect::<Vec<_>>());
                    for (r, jr) in m.rows.iter().zip(table["rows"].as_array().unwrap()) {
                        assert_eq!(r.row as u64, jr["row"].as_u64().unwrap());
                        let accepts: Vec<Vec<usize>> = jr["accepts"].as_array().unwrap().iter().map(|a| a.as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize).collect()).collect();
                        assert_eq!(r.accepts, accepts, "{path} {}", t.name);
                        let produces: Vec<Option<String>> = jr["produces"].as_array().unwrap().iter().map(|p| p.as_str().map(String::from)).collect();
                        assert_eq!(r.produces, produces, "{path} {}", t.name);
                        let mv = cert["machine"]["rows"].as_array().unwrap().iter().find(|x| x["row"] == jr["row"]).unwrap();
                        assert_eq!(r.to, mv["to"].as_u64().map(|x| x as usize), "{path} {}", t.name);
                    }
                }
                (a, b) => panic!("{path}: {a:?} {b:?}"),
            }
        }
    }
}

/// A ledger opened through the port, run through every scenario chobo writes for a book, comes
/// out of each call as the reference interpreter does (scenarios with callers at the same time
/// are the interpreter's own business, and are left to it).
#[test]
fn a_ledger_runs_every_scenario_as_the_interpreter_does() {
    use chobo::scenario::{Step, StepOut};
    let mut calls = 0;
    for path in books() {
        let src = std::fs::read_to_string(root().join(&path)).unwrap();
        let book = chobo::check::check_source(&src).book.unwrap();
        for s in chobo::scenarios::generate(&book) {
            if s.steps.iter().any(|st| matches!(st, Step::Together(_))) {
                continue;
            }
            let runs = chobo::scenario::run(&book, &s).unwrap();
            let mut ledger = Engine.open(&root().join(&path)).unwrap();
            for (st, out) in s.steps.iter().zip(&runs[0].steps) {
                match (st, out) {
                    (Step::Call(c), StepOut::Call(o)) => {
                        let t = &book.transfers[c.kind];
                        let call = BookCall {
                            transfer: t.name.clone(),
                            op: c.op.name().to_string(),
                            args: t
                                .params
                                .iter()
                                .zip(&c.args)
                                .filter_map(|(p, a)| {
                                    a.as_ref().map(|a| {
                                        (p.name.clone(), match a {
                                            chobo::interp::Val::Amt(n) => Ok(*n),
                                            chobo::interp::Val::Str(s) => Err(s.clone()),
                                        })
                                    })
                                })
                                .collect(),
                            amounts: c.amounts.as_ref().map(|a| a.iter().map(|(i, v)| (t.params[*i].name.clone(), *v)).collect()),
                        };
                        let got = ledger.apply(&call).unwrap_or_else(|e| panic!("{path} {}: {}", s.name, e.en));
                        let want = match o {
                            chobo::interp::Outcome::Done => BookOutcome::Done,
                            chobo::interp::Outcome::DoneBefore => BookOutcome::DoneBefore,
                            chobo::interp::Outcome::Refused(r) => BookOutcome::Refused(r.reason.clone()),
                        };
                        assert_eq!(got, want, "{path} {}", s.name);
                        calls += 1;
                    }
                    (Step::Pass(secs, _), StepOut::Pass(expired)) => {
                        let got = ledger.pass(*secs);
                        let want: Vec<(String, Vec<String>)> = expired.iter().map(|(k, key)| (book.transfers[*k].name.clone(), key.clone())).collect();
                        assert_eq!(got, want, "{path} {}", s.name);
                    }
                    _ => unreachable!(),
                }
            }
            // every account the run touched has the balance the interpreter left it with
            for (id, bal) in &runs[0].state.accounts {
                let got = ledger.balance(&book.accounts[id.kind].name, &id.args).unwrap();
                assert_eq!((got.posted, got.held_in, got.held_out), (bal.posted, bal.held_in, bal.held_out), "{path} {}", s.name);
            }
        }
    }
    assert!(calls > 100, "{calls} calls");
    // a call that does not fit the book is said so, not refused
    let mut l = Engine.open(&root().join("tests/books/在庫.book")).unwrap();
    assert!(l.apply(&BookCall { transfer: "無い".into(), op: "do".into(), args: vec![], amounts: None }).is_err());
    let f = Engine.facts(&root().join("tests/books/在庫.book")).unwrap();
    let t = &f.transfers[0];
    // the refusals with every amount held to a range (ritsu's DESIGN 7.6): the same search as
    // the check's, which finds the same reasons here, where the check's own amounts are in range
    match Engine.refusals(&root().join("tests/books/在庫.book"), &t.name, (1, 10)).unwrap() {
        ritsu_ports::Found::Value(ops) => assert_eq!(ops, t.refusals, "{}", t.name),
        other => panic!("{other:?}"),
    }
    // a range with no amount chobo takes, and a transfer the book does not have
    assert!(matches!(Engine.refusals(&root().join("tests/books/在庫.book"), &t.name, (-10, 0)).unwrap(), ritsu_ports::Found::Undecided(_)));
    assert!(Engine.refusals(&root().join("tests/books/在庫.book"), "無い", (1, 10)).is_err());
}

fn short(text: &str) -> String {
    ritsu_base::sha256::short(text.as_bytes())
}

/// The definitions yuen's DESIGN 3.2 settled for a book's items, and the hashes its prototype
/// computed for them (yuen's DESIGN 19): 返金 is 1,206 bytes; 売上 and 返金済み, both outside
/// accounts in yen, are the same definition.
#[test]
fn a_books_items_are_defined_as_yuen_hashes_them() {
    let items = Engine.items(&root(), "examples/refunds/refunds.ja.book").unwrap();
    let refund = items.iter().find(|i| i.kind() == "transfer" && i.name() == "返金").unwrap();
    assert_eq!(refund.text.len(), 1206, "{}", refund.text);
    assert_eq!(short(&refund.text), "84e9ce254075c697");
    for a in ["売上", "返金済み"] {
        let it = items.iter().find(|i| i.kind() == "account" && i.name() == a).unwrap();
        assert_eq!(short(&it.text), "35a4ec5a2ee5eb06", "{a}: {}", it.text);
    }
    assert!(items.iter().any(|i| i.kind() == "unit"));
    for path in books() {
        let n = std::fs::read_to_string(root().join(&path)).unwrap().lines().count();
        for it in Engine.items(&root(), &path).unwrap() {
            assert!(it.lines.0 >= 1 && it.lines.0 <= it.lines.1 && it.lines.1 <= n, "{path}: {it:?}");
            assert_eq!(ritsu_base::naming::parse_one(&it.naming.text()).map(|x| x.text()).ok(), Some(it.naming.text()), "{path}");
            assert!(it.text.ends_with("}\n"), "{path}: {}", it.text);
        }
    }
}

/// What `a_ledger_runs_every_scenario_as_the_interpreter_does` ends with, on the English book (the
/// loop above it runs the English books of `tests/books` too): a call that does not fit the book
/// is said so, not refused.
#[test]
fn a_call_that_does_not_fit_the_english_book_is_said_so() {
    let mut l = Engine.open(&root().join("tests/books/stock_reservation.book")).unwrap();
    assert!(l.apply(&BookCall { transfer: "nothing".into(), op: "do".into(), args: vec![], amounts: None }).is_err());
    let f = Engine.facts(&root().join("tests/books/stock_reservation.book")).unwrap();
    let t = &f.transfers[0];
    // the refusals with every amount held to a range (ritsu's DESIGN 7.6), as for the Japanese book
    match Engine.refusals(&root().join("tests/books/stock_reservation.book"), &t.name, (1, 10)).unwrap() {
        ritsu_ports::Found::Value(ops) => assert_eq!(ops, t.refusals, "{}", t.name),
        other => panic!("{other:?}"),
    }
    assert!(matches!(Engine.refusals(&root().join("tests/books/stock_reservation.book"), &t.name, (-10, 0)).unwrap(), ritsu_ports::Found::Undecided(_)));
    assert!(Engine.refusals(&root().join("tests/books/stock_reservation.book"), "nothing", (1, 10)).is_err());
}

/// The refusals of a transfer that holds, with every amount held to a range that ends below what a
/// hold needs to be partly posted (2): the witness gives up on that hold rather than look for an
/// amount the range never reaches, and the answer comes at once (a hold of exactly 1 used to keep
/// `ritsu check` from ending).
#[test]
fn refusals_with_amounts_held_below_a_partial_post_come_at_once() {
    let book = root().join("tests/books/stock_reservation.book");
    let f = Engine.facts(&book).unwrap();
    let t = f.transfers.iter().find(|t| t.name == "reserve").expect("stock_reservation.book holds with reserve").clone();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(Engine.refusals(&book, &t.name, (1, 1)).map(|f| matches!(f, ritsu_ports::Found::Value(_))));
    });
    match rx.recv_timeout(std::time::Duration::from_secs(60)) {
        Ok(answer) => assert_eq!(answer.ok(), Some(true), "the refusals of reserve with every amount 1"),
        Err(_) => panic!("the refusals of reserve with every amount 1 did not come within 60 seconds"),
    }
}
