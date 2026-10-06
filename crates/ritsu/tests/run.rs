//! `ritsu run` (DESIGN 7.9, 8.1; PLAN E.6): the flows of the test project `tests/projects/invoice`
//! (English, and its Japanese version beside it), run with their rules computed by rulec, their
//! dates by koyomi and their books by chobo, every other task answered by the scenario. Each run is
//! held to its golden text (in the language of its version) and JSON; to `dandori run` on the
//! scenario filled in with what the languages answered (the same trace); to each language's own
//! command on what it was asked (`rulec replay`, `koyomi eval`, `chobo run`); and the two versions
//! to each other. Then what stops a run.

use ritsu_testkit::TempDir;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn project() -> PathBuf {
    here().join("tests/projects/invoice")
}

/// `ritsu`, run in `dir`, with no language asked of the environment.
fn ritsu_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(dir).args(args);
    for v in ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG", "CHOBO_LANG", "GEAS_LANG", "YUEN_LANG", "SAKAI_LANG", "SEKISHO_LANG"] {
        c.env_remove(v);
    }
    let o = c.output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// One version of the project: its flow, the files its languages read, how its scenarios are
/// named, and the language its text is read in.
struct Version {
    flow: &'static str,
    rule: &'static str,
    dates: &'static str,
    date: &'static str,
    book: &'static str,
    suffix: &'static str,
    lang: &'static str,
}

const VERSIONS: [Version; 2] = [
    Version { flow: "invoice.flow", rule: "rules/payment_method.rule", dates: "dates/payment_terms.cal", date: "payment", book: "books/stock.book", suffix: "", lang: "en" },
    Version { flow: "invoice.ja.flow", rule: "rules/支払方法.rule", dates: "dates/支払条件.cal", date: "支払日", book: "books/在庫.book", suffix: ".ja", lang: "ja" },
];

/// Every scenario, each in both versions: paid by invoice; by invoice past the hold's 30 days;
/// unpaid after a retry; paid first; more than the shelf holds.
const SCENARIOS: [&str; 5] = ["invoice_paid", "invoice_expired", "invoice_unpaid", "prepay", "out_of_stock"];

fn scenario(v: &Version, s: &str) -> String {
    format!("scenarios/{s}{}.json", v.suffix)
}

/// `ritsu run` of a scenario as one JSON object, its calls as `target` shows them.
fn run_json(v: &Version, s: &str, target: &str) -> Value {
    let sc = scenario(v, s);
    let (code, out, err) = ritsu_in(&project(), &["run", v.flow, "--scenario", &sc, "--target", target, "--format", "json", "--lang", v.lang]);
    assert_eq!(code, 0, "{} {sc}: {err}", v.flow);
    serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}"))
}

fn golden(path: &str, got: &str, failures: &mut Vec<String>) {
    if let Err(e) = ritsu_testkit::golden::check(&here().join(path), got) {
        failures.push(e);
    }
}

/// Each scenario of each version: the text in the version's language, and the JSON, as their golden
/// files hold them; nothing on standard error (the flows pass their check without a warning).
#[test]
fn every_scenario_runs_as_its_golden_says() {
    let mut failures = Vec::new();
    for v in &VERSIONS {
        for s in SCENARIOS {
            let sc = scenario(v, s);
            let (code, out, err) = ritsu_in(&project(), &["run", v.flow, "--scenario", &sc, "--lang", v.lang]);
            assert!(code == 0 && err.is_empty(), "{} {sc}: {err}", v.flow);
            golden(&format!("tests/golden/run/{s}.{}.txt", v.lang), &out, &mut failures);
            let (code, out, err) = ritsu_in(&project(), &["run", v.flow, "--scenario", &sc, "--format", "json", "--lang", v.lang]);
            assert!(code == 0 && err.is_empty(), "{} {sc}: {err}", v.flow);
            golden(&format!("tests/golden/run/{s}{}.json", v.suffix), &out, &mut failures);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The run is the one `dandori run` makes of the scenario with every answer in it (`replay`):
/// rulec, koyomi and chobo only decide the answers, and dandori's meaning of the flow is dandori's.
/// Seen as the reference view and as Temporal's.
#[test]
fn the_replay_is_the_run_dandori_makes() {
    let t = TempDir::new("replay");
    for v in &VERSIONS {
        for s in SCENARIOS {
            for target in ["reference", "temporal"] {
                let ran = run_json(v, s, target);
                let replay = t.path().join(format!("{s}{}.{target}.json", v.suffix));
                std::fs::write(&replay, ran["replay"].to_string()).unwrap();
                let (code, out, err) = ritsu_in(&project(), &["dandori", "run", v.flow, "--scenario", &replay.to_string_lossy(), "--target", target]);
                assert_eq!(code, 0, "{} {s} {target}: {err}", v.flow);
                let trace: Value = serde_json::from_str(&out).unwrap();
                assert_eq!(trace, ran["trace"], "{} {s} {target}: `dandori run` on the replay makes another trace", v.flow);
                // every answer is in the replay, the computed ones and the scenario's, in order
                let answers = ran["events"].as_array().unwrap().iter().filter(|e| e.get("answer").is_some()).count();
                assert_eq!(ran["replay"]["answers"].as_array().map(Vec::len), Some(answers), "{} {s}", v.flow);
            }
        }
    }
}

/// What each language answered is what its own command answers for the same question: rulec's
/// outputs are what `rulec replay` finds the rule gives for the same inputs, koyomi's day and time
/// what `koyomi eval` computes for the day, and chobo's operations, from the operations before the
/// run to the end, with the run's time going by between them, what `chobo run` answers, ending in
/// the same accounts and holds.
#[test]
fn each_language_answers_as_its_own_command_does() {
    let t = TempDir::new("languages");
    let (mut rules, mut dates, mut books) = (0, 0, 0);
    for v in &VERSIONS {
        for s in SCENARIOS {
            let ran = run_json(v, s, "temporal");
            let events = ran["events"].as_array().unwrap();
            let by = |who: &str| events.iter().filter(|e| e["by"] == who).collect::<Vec<_>>();
            // rulec: each call a record of what came in and what came out
            let records: Vec<String> = by("rulec").iter().map(|e| json!({ "in": e["args"], "observed": e["answer"]["ok"] }).to_string()).collect();
            if !records.is_empty() {
                let fixtures = t.path().join(format!("{s}{}.jsonl", v.suffix));
                std::fs::write(&fixtures, records.join("\n") + "\n").unwrap();
                let (code, out, err) = ritsu_in(&project(), &["rulec", "replay", v.rule, "--fixtures", &fixtures.to_string_lossy(), "--format", "json"]);
                assert_eq!(code, 0, "{} {s}: {out}{err}", v.flow);
                let r: Value = serde_json::from_str(&out).unwrap();
                assert!(r["compared"] == json!(records.len()) && r["matched"] == json!(records.len()), "{} {s}: {r}", v.flow);
                rules += records.len();
            }
            // koyomi: the day handed in (a time, read in the calendar's offset, +00:00) and what the date comes to
            for e in by("koyomi") {
                let (name, given) = e["args"].as_object().unwrap().iter().next().map(|(k, x)| (k.clone(), x.as_str().unwrap().to_string())).unwrap();
                let (code, out, err) = ritsu_in(&project(), &["koyomi", "eval", v.dates, &format!("{name}={}", &given[..10]), "--format", "json"]);
                assert_eq!(code, 0, "{} {s}: {err}", v.flow);
                let k: Value = serde_json::from_str(&out).unwrap();
                assert_eq!(e["answer"]["ok"]["day"], k["dates"][v.date], "{} {s}", v.flow);
                assert_eq!(e["answer"]["ok"]["at"], k["times"][v.date]["utc"], "{} {s}", v.flow);
                dates += 1;
            }
            // chobo: the operations before the run, then each of the run's in order, the time passing between them
            let book = &ran["books"][0];
            let mut steps: Vec<Value> = book["before"].as_array().unwrap().iter().map(|op| json!({ "op": op["op"], "kind": op["kind"], "args": op["args"] })).collect();
            let skip = steps.len();
            let mut calls = ran["trace"]["steps"].as_array().unwrap().iter().filter_map(|st| st.get("call").filter(|c| c.get("book").is_some()));
            let mut said = Vec::new();
            for e in events {
                if e["by"] == "chobo" {
                    let c = calls.next().expect("a call of the book in the trace for each of chobo's answers");
                    let mut op = json!({ "op": c["op"], "kind": c["transfer"], "args": c["args"] });
                    if let Some(a) = c.get("amounts") {
                        op["amounts"] = a.clone();
                    }
                    steps.push(op);
                    said.push(e["answer"].clone());
                } else if let Some(secs) = e.get("pass").and_then(|p| p.as_u64()).filter(|p| *p > 0) {
                    steps.push(json!({ "op": "pass", "duration": format!("{secs} seconds") }));
                }
            }
            let file = t.path().join(format!("{s}{}.chobo.json", v.suffix));
            std::fs::write(&file, json!({ "name": s, "steps": steps }).to_string()).unwrap();
            let (code, out, err) = ritsu_in(&project(), &["chobo", "run", v.book, "--scenario", &file.to_string_lossy(), "--format", "json"]);
            assert_eq!(code, 0, "{} {s}: {err}", v.flow);
            let c: Value = serde_json::from_str(&out).unwrap();
            let outcomes: Vec<&Value> = c["steps"].as_array().unwrap().iter().skip(skip).filter(|st| st["op"] != "pass").collect();
            assert_eq!(outcomes.len(), said.len(), "{} {s}", v.flow);
            for (o, a) in outcomes.iter().zip(&said) {
                let answered = match a.get("ok") {
                    Some(ok) => ok["result"].clone(),
                    None => json!("refused"),
                };
                assert_eq!(o["result"], answered, "{} {s}: {o} and {a}", v.flow);
                if o["result"] == "refused" {
                    assert_eq!(o["reason"], a["cause"], "{} {s}", v.flow);
                }
                books += 1;
            }
            assert_eq!(c["accounts"], book["accounts"], "{} {s}: the accounts at the end", v.flow);
            assert_eq!(c["holds"], book["holds"], "{} {s}: the holds at the end", v.flow);
        }
    }
    // every language was asked something in some run
    assert!(rules >= 8 && dates >= 6 && books >= 18, "rulec {rules}, koyomi {dates}, chobo {books}");
}

/// The Japanese version is the English one with Japanese names: every run takes the same time, the
/// same languages answer in the same order with the same days and the same results, and the books
/// end with the same balances and holds in the same states.
#[test]
fn the_english_and_the_japanese_versions_run_alike() {
    // what a run comes to, without its names
    let shape = |r: &Value| -> Value {
        let events: Vec<Value> = r["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                if let Some(a) = e.get("answer") {
                    // a refusal's reason is the book's own name for it (`out_of_stock`, `在庫切れ`)
                    let what = match (e["by"].as_str(), a.get("ok")) {
                        (Some("koyomi"), Some(ok)) => ok.clone(),
                        (Some("chobo"), Some(ok)) => ok["result"].clone(),
                        (Some("chobo"), None) => json!("refused"),
                        (_, Some(_)) => json!("ok"),
                        (_, None) => a["error"].clone(),
                    };
                    json!([e["by"], what])
                } else if let Some(p) = e.get("pass") {
                    json!(["pass", p, e["why"], e["after"]])
                } else {
                    json!(["expired"])
                }
            })
            .collect();
        let book = &r["books"][0];
        let balances: Vec<Value> = book["accounts"].as_array().unwrap().iter().map(|a| json!([a["posted"], a["held_in"], a["held_out"]])).collect();
        let holds: Vec<Value> = book["holds"].as_array().unwrap().iter().map(|h| h["state"].clone()).collect();
        let end: Vec<&String> = r["trace"]["end"].as_object().unwrap().keys().collect();
        json!({ "start": r["start"], "end": r["end"], "elapsed": r["elapsed"], "events": events, "balances": balances, "holds": holds, "ends": end })
    };
    for s in SCENARIOS {
        let (en, ja) = (run_json(&VERSIONS[0], s, "reference"), run_json(&VERSIONS[1], s, "reference"));
        assert_eq!(shape(&en), shape(&ja), "{s}");
    }
}

/// A rule called at its Connect service (`connect` under `use rule`) is computed by rulec all the
/// same, and answers as the service writes it (an enum's value by the `.proto`'s name, a number as
/// its decimal string): the flow reads it back to the same outputs, and the run ends as the one
/// that calls the rule's code does. `dandori run` on its replay makes the same trace.
#[test]
fn a_rule_called_at_its_service_answers_as_the_service_writes_it() {
    let copy = TempDir::new("connect");
    ritsu_testkit::tmp::copy_dir(&project(), copy.path());
    let flow = copy.path().join("invoice.flow");
    let src = std::fs::read_to_string(&flow).unwrap();
    std::fs::write(&flow, src.replacen("use rule method from \"rules/payment_method.rule\"\n", "use rule method from \"rules/payment_method.rule\"\n  connect \"https://rules.example.com\"\n", 1)).unwrap();
    let (code, out, err) = ritsu_in(copy.path(), &["run", "invoice.flow", "--scenario", "scenarios/invoice_paid.json", "--format", "json"]);
    assert_eq!(code, 0, "{err}");
    let ran: Value = serde_json::from_str(&out).unwrap();
    let rule = ran["events"].as_array().unwrap().iter().find(|e| e["by"] == "rulec").expect("rulec answers the rule");
    assert_eq!(rule["answer"], json!({ "ok": { "billing": "BILLING_INVOICE" } }));
    let call = ran["trace"]["steps"].as_array().unwrap().iter().find_map(|st| st["call"]["url"].as_str().filter(|u| u.starts_with("https://rules.example.com/"))).expect("the rule is called at its service");
    assert_eq!(call, "https://rules.example.com/rulec.payment_method.v1.PaymentMethodService/Decide");
    let plain = run_json(&VERSIONS[0], "invoice_paid", "reference");
    assert_eq!(ran["trace"]["end"], plain["trace"]["end"]);
    assert_eq!(ran["books"], plain["books"]);
    let replay = copy.path().join("replay.json");
    std::fs::write(&replay, ran["replay"].to_string()).unwrap();
    let (code, out, err) = ritsu_in(copy.path(), &["dandori", "run", "invoice.flow", "--scenario", &replay.to_string_lossy()]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(serde_json::from_str::<Value>(&out).unwrap(), ran["trace"]);
}

/// What stops `ritsu run`, with the exit code and what it says: the arguments (2), a scenario that
/// cannot be read or run (2), a flow with errors (1, in the text and in the JSON), and a run that
/// cannot go on (1).
#[test]
fn what_stops_ritsu_run() {
    let p = project();
    let (code, out, _) = ritsu_in(&p, &["run", "--help"]);
    assert!(code == 0 && out.starts_with("ritsu run — run a workflow in dandori's reference interpreter"), "{out}");
    let (code, _, err) = ritsu_in(&p, &["run"]);
    assert!(code == 2 && err.starts_with("ritsu run <file.flow> --scenario <file.json> "), "{err}");
    assert_eq!(ritsu_in(&p, &["run", "invoice.flow"]), (2, String::new(), "error: `--scenario <file.json>` is required\n".into()));
    assert_eq!(ritsu_in(&p, &["run", "invoice.flow", "--lang", "ja"]), (2, String::new(), "エラー: `--scenario <file.json>` が要ります\n".into()));
    let (code, _, err) = ritsu_in(&p, &["run", "invoice.flow", "--scenario", "scenarios/none.json"]);
    assert!(code == 2 && err.starts_with("error: cannot read the scenario `scenarios/none.json`: "), "{err}");

    let t = TempDir::new("stops");
    let write = |name: &str, v: Value| {
        let f = t.path().join(name);
        std::fs::write(&f, v.to_string()).unwrap();
        f.to_string_lossy().into_owned()
    };
    let paid: Value = serde_json::from_str(&std::fs::read_to_string(p.join("scenarios/invoice_paid.json")).unwrap()).unwrap();
    // a book the flow does not use
    let mut s = paid.clone();
    s["books"] = json!({ "warehouse": [] });
    let f = write("no_such_book.json", s);
    let (code, out, err) = ritsu_in(&p, &["run", "invoice.flow", "--scenario", &f]);
    assert!(code == 2 && out.is_empty() && err == format!("error: the scenario `{f}` cannot be run: the flow uses no book `warehouse`, which the scenario's `books` names\n"), "{err}");
    // an operation the book refuses before the run: the same delivery twice, of another quantity
    let mut s = paid.clone();
    s["books"]["stock"].as_array_mut().unwrap().push(json!({ "op": "do", "kind": "receive", "args": { "delivery": "D-1", "sku": "pen", "qty": 4 } }));
    let f = write("refused_before.json", s);
    let (code, _, err) = ritsu_in(&p, &["run", "invoice.flow", "--scenario", &f, "--lang", "ja"]);
    assert!(code == 2 && err == format!("エラー: シナリオ `{f}` を流せません: 帳簿 `stock` は、走らせる前の 2 番目の操作を `key_conflict` で拒否します\n"), "{err}");
    // an operation that does not fit the book
    let mut s = paid.clone();
    s["books"]["stock"] = json!([{ "op": "do", "kind": "receive", "args": { "delivery": "D-1", "sku": "pen", "qty": "ten" } }]);
    let f = write("not_an_amount.json", s);
    let (code, _, err) = ritsu_in(&p, &["run", "invoice.flow", "--scenario", &f]);
    assert!(code == 2 && err.ends_with("cannot be run: operation 1 of the book `stock`: `qty` is an amount, a whole number\n"), "{err}");
    // a `now` that is not a time
    let mut s = paid.clone();
    s["now"] = json!("tomorrow");
    let f = write("not_a_time.json", s);
    let (code, _, err) = ritsu_in(&p, &["run", "invoice.flow", "--scenario", &f]);
    assert!(code == 2 && err.ends_with("cannot be run: the scenario's `now`, `tomorrow`, is not a time (`2026-03-31T15:30:00Z`)\n"), "{err}");
    // the scenario runs out of answers: the run cannot go on
    let mut s = paid.clone();
    s["answers"] = json!([]);
    let f = write("no_answers.json", s);
    let (code, out, err) = ritsu_in(&p, &["run", "invoice.flow", "--scenario", &f]);
    assert!(code == 1 && out.is_empty() && err == "error: the run could not go on: the scenario has no answer for call 1 (check_payment)\n", "{err}");

    // a flow with an error: what dandori's check says, and no run
    let copy = TempDir::new("broken");
    ritsu_testkit::tmp::copy_dir(&p, copy.path());
    let flow = copy.path().join("invoice.flow");
    let src = std::fs::read_to_string(&flow).unwrap();
    std::fs::write(&flow, src.replace("  let how = method(member: order.member, amount: order.amount)", "  let how = method(member: order.member, amount: order.id)")).unwrap();
    let (code, out, err) = ritsu_in(copy.path(), &["run", "invoice.flow", "--scenario", "scenarios/invoice_paid.json"]);
    assert!(code == 1 && out.is_empty() && err.starts_with("error[E003]: invoice.flow:"), "{err}");
    let (code, out, err) = ritsu_in(copy.path(), &["run", "invoice.flow", "--scenario", "scenarios/invoice_paid.json", "--format", "json"]);
    assert!(code == 1 && err.is_empty(), "{err}");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert!(v["diagnostics"][0]["code"] == "E003" && v.get("trace").is_none(), "{v}");
}
