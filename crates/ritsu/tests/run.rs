//! `ritsu run` (DESIGN 7.9, 8.1; PLAN E.6): the flows of the test project `tests/projects/invoice`
//! (English, and its Japanese version beside it), run with their rules computed by rulec, their
//! dates by koyomi and their books by chobo, every other task answered by the scenario. Each run is
//! held to its golden text (in the language of its version) and JSON; to `dandori run` on the
//! scenario filled in with what the languages answered (the same trace); to each language's own
//! command on what it was asked (`rulec replay`, `koyomi eval`, `chobo run`); and the two versions
//! to each other. Then what stops a run. Then the flow of `tests/projects/ticket_desk`, which
//! answers the events it is told with a rule of its own: every scenario `ritsu dandori scenarios`
//! writes for it runs under `ritsu run` to its end. Then a flow whose rule has preconditions ritsu
//! cannot decide at the call: the run checks them where their values are made, as the code dandori
//! writes does, and every scenario written for it runs as `ritsu dandori run` runs it. Then what a
//! scenario covers, which is what its run goes through; the arms of a `match` on a rule's answer,
//! which the scenarios reach by what the rule is given, or say no run takes where rulec says none
//! can; and the page of a flow, which shows the runs its scenarios make. Then the
//! clock: `now` read when the statement that reads it runs, after the waits before it; a run that
//! leaves its period; time that went by before the run. Then every scenario `ritsu dandori
//! scenarios` writes for flows that call dates and do operations of books runs under `ritsu run` to
//! the end `ritsu dandori run` comes to. Last, the fulfillment flow of the shop example: the way it
//! succeeds and each way it fails, in English and in Japanese, with what it gives back, whether it
//! cancels the order, and what it tells the customer.

mod common;

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
    for v in common::LANG_VARS {
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
/// Seen as the reference view and as Temporal's. `dandori run` takes every answer of the replay
/// (one left over would end it with 1).
#[test]
fn the_replay_is_the_run_dandori_makes() {
    let t = TempDir::new("replay");
    for v in &VERSIONS {
        for s in SCENARIOS {
            for target in ["reference", "temporal-typescript"] {
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
            let ran = run_json(v, s, "temporal-typescript");
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
/// cannot be read or run (2), a run that cannot go on (1), and a flow with errors (1, in the text and
/// in the JSON): dandori's, or one the check across the languages finds at a call of the flow.
#[test]
fn what_stops_ritsu_run() {
    let p = project();
    let (code, out, _) = ritsu_in(&p, &["run", "--help"]);
    assert!(code == 0 && out.starts_with("ritsu run — run a workflow in dandori's reference interpreter"), "{out}");
    // what stops it before its work, in the words of every command (DESIGN 6.2, item 10)
    let usage = "ritsu run <file.flow> --scenario <file.json> [--target reference|asl|temporal-typescript|temporal-python|temporal-go|durable|argo|pydantic-graph] [--format json]";
    assert_eq!(ritsu_in(&p, &["run"]), (2, String::new(), format!("error: `ritsu run` takes <file.flow> --scenario <file.json>\nusage: {usage}\n")));
    assert_eq!(ritsu_in(&p, &["run", "invoice.flow"]), (2, String::new(), format!("error: `ritsu run` needs `--scenario <file.json>`\nusage: {usage}\n")));
    assert_eq!(ritsu_in(&p, &["run", "invoice.flow", "--lang", "ja"]), (2, String::new(), format!("エラー: `ritsu run` には `--scenario <file.json>` が要ります\n使い方: {usage}\n")));
    assert_eq!(ritsu_in(&p, &["run", "invoice.flow", "--scenario", "scenarios/none.json"]), (2, String::new(), "error: `scenarios/none.json` is not there\n".into()));

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
    let (code, out, err) = ritsu_in(&p, &["run", "invoice.flow", "--scenario", &f, "--lang", "ja"]);
    assert!(code == 1 && out.is_empty() && err == "エラー: 流れが途中で止まりました: シナリオに、1 番目の呼び出し（check_payment）の結果がありません\n", "{err}");
    // answers the run does not take (dandori's DESIGN 36): the run is shown to its end, then the
    // answers left over and why, and the run ends with 1, as one that runs out of answers does
    let mut s = paid.clone();
    let n = s["answers"].as_array().unwrap().len();
    s["answers"].as_array_mut().unwrap().push(json!({ "ok": "left over" }));
    let f = write("left_over.json", s);
    for lang in ["en", "ja"] {
        let (_, whole, _) = ritsu_in(&p, &["run", "invoice.flow", "--scenario", "scenarios/invoice_paid.json", "--lang", lang]);
        let (code, out, err) = ritsu_in(&p, &["run", "invoice.flow", "--scenario", &f, "--lang", lang]);
        assert_eq!(code, 1, "{err}");
        assert_eq!(out, whole.replace("scenarios/invoice_paid.json", &f), "the run is shown to its end");
        let said = if lang == "en" {
            format!("error: the run did not take answer {} of the scenario: {{\"ok\":\"left over\"}}\n  The run came to its end before a call took it: the scenario may count the calls wrong, or be written for another way through the flow, or for the flow before it changed.\n  A call of a rule, a date or a book that the run computes takes no answer from the scenario.\n", n + 1)
        } else {
            format!("エラー: シナリオの {} 番目の結果は使われませんでした: {{\"ok\":\"left over\"}}\n  実行は、その結果を受け取る呼び出しの前に終わりました。シナリオが呼び出しの数を数え違えているか、フローの別の分岐や、変わる前のフローに合わせて書かれているのかもしれません。\n  実行が計算する規則・日付・帳簿の呼び出しは、シナリオの結果を受け取りません。\n", n + 1)
        };
        assert_eq!(err, said);
    }
    // the same in the JSON, beside the whole run; the replay holds the answers the run took, and
    // `ritsu dandori run` on it takes every one
    let (_, whole, _) = ritsu_in(&p, &["run", "invoice.flow", "--scenario", "scenarios/invoice_paid.json", "--format", "json"]);
    let (code, out, _) = ritsu_in(&p, &["run", "invoice.flow", "--scenario", &f, "--format", "json"]);
    assert_eq!(code, 1);
    let (mut ran, mut want): (Value, Value) = (serde_json::from_str(&out).unwrap(), serde_json::from_str(&whole).unwrap());
    assert_eq!(ran["unused"], json!({ "from": n + 1, "count": 1, "answers": [{ "ok": "left over" }] }));
    ran.as_object_mut().unwrap().remove("unused");
    want["scenario"] = json!(f);
    assert_eq!(ran, want, "the run is the one without the answer left over");
    let replay = write("left_over_replay.json", ran["replay"].clone());
    let (code, _, err) = ritsu_in(&p, &["dandori", "run", "invoice.flow", "--scenario", &replay]);
    assert!(code == 0 && err.is_empty(), "{err}");
    // `ritsu dandori run` on the replay with two answers added: it computes nothing, so the line on
    // computed calls is not said
    let mut more = ran["replay"].clone();
    let taken = more["answers"].as_array().unwrap().len();
    more["answers"].as_array_mut().unwrap().extend([json!({ "ok": 1 }), json!({ "ok": 2 })]);
    let g = write("replay_left_over.json", more);
    let (code, out, err) = ritsu_in(&p, &["dandori", "run", "invoice.flow", "--scenario", &g]);
    assert_eq!(serde_json::from_str::<Value>(&out).unwrap(), want["trace"], "the run is printed to its end");
    assert_eq!((code, err), (1, format!("error: the run did not take answers {} to {} of the scenario (2 of them): {{\"ok\":1}}, {{\"ok\":2}}\n  The run came to its end before a call took it: the scenario may count the calls wrong, or be written for another way through the flow, or for the flow before it changed.\n", taken + 1, taken + 2)));

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

    // a flow that dandori's check passes, with an error the check across the languages finds at a
    // call: it can give the dates file a day before the days it takes (ritsu's E205). It is said as
    // `ritsu check` says it, and the flow is not run, though the scenario's day is one it takes.
    let crossed = TempDir::new("crossed");
    let c = crossed.path();
    std::fs::create_dir_all(c.join("calendars")).unwrap();
    std::fs::create_dir_all(c.join("dates")).unwrap();
    std::fs::write(c.join("calendars/office.cal"), "calendar office v1\ndescription \"The business days of an office (fictional): Saturdays and Sundays are closed\"\noffset +09:00\n\nclosed weekly sat, sun\n").unwrap();
    std::fs::write(c.join("dates/terms.cal"), "dates terms v1\ndescription \"A due date three business days after a request arrives\"\nuse calendar \"../calendars/office.cal\"\n\ninputs\n  received : date  range >=2026-01-01 <=2027-12-31\n\ndate due = received\n  + 3 business days\n  at end of day\n\nexamples\n| received   | -> due      |\n| 2026-11-13 | 2026-11-18  |\n").unwrap();
    std::fs::write(c.join("late.flow"), "workflow late v1\ndescription \"Give a koyomi date a day from before the range it takes\"\n\nuse dates terms from \"dates/terms.cal\"\n\ninputs\n  received : date  range >=2025-01-01 <=2027-12-31\n\noutputs\n  due : date\n\nflow\n  let d = terms.due(received: received)\n  succeed due = d.day\n").unwrap();
    std::fs::write(c.join("received.json"), json!({ "input": { "received": "2026-03-02" } }).to_string()).unwrap();
    let (code, checked, _) = ritsu_in(c, &["check", "late.flow"]);
    assert_eq!(code, 1, "{checked}");
    let at = checked.find("error[ritsu E205]: late.flow:13:1: ").unwrap_or_else(|| panic!("{checked}"));
    let said = &checked[at..checked.rfind("ritsu check: ").unwrap()];
    let (code, out, err) = ritsu_in(c, &["run", "late.flow", "--scenario", "received.json"]);
    assert_eq!((code, out.as_str(), err.as_str()), (1, "", said));
    let (code, out, err) = ritsu_in(c, &["run", "late.flow", "--scenario", "received.json", "--format", "json"]);
    assert!(code == 1 && err.is_empty(), "{err}");
    let v: Value = serde_json::from_str(&out).unwrap();
    let errors: Vec<(&Value, &Value)> = v["diagnostics"].as_array().unwrap().iter().filter(|d| d["severity"] == "error").map(|d| (&d["tool"], &d["code"])).collect();
    assert!(errors == [(&json!("ritsu"), &json!("E205"))] && v.get("trace").is_none(), "{v}");
}

/// A flow that answers the events it is told with a rule of its own (`tests/projects/ticket_desk`,
/// in English and its Japanese version). Every scenario `ritsu dandori scenarios` writes for it says
/// the calls of the rule are rulec's (`"rules": "rulec"`) and gives no answer for them, and runs
/// under `ritsu run` to its end: every answer it gives is used, every call of the rule is answered
/// by rulec, and the trace is the one `ritsu dandori run` makes of the same scenario. The generator
/// chose those runs with the rule computed the same way, so they reach the ends it chose, the
/// resolved ticket among them. A scenario of the form `dandori run` reads, which answers the call of
/// the rule itself, stops `ritsu run` at that call and says why; `ritsu dandori run` runs it.
#[test]
fn every_scenario_of_a_flow_that_calls_a_rule_runs_under_ritsu_run() {
    let dir = here().join("tests/projects/ticket_desk");
    for (flow, rule, lang, why) in [
        ("ticket_desk.flow", "step", "en", "cannot be run: it answers the call of the rule step itself"),
        ("ticket_desk.ja.flow", "一歩", "ja", "を流せません: 規則 一歩 の呼び出しの結果を、シナリオが自分で持っています"),
    ] {
        let (code, out, err) = ritsu_in(&dir, &["dandori", "scenarios", flow]);
        assert_eq!(code, 0, "{flow}: {err}");
        let scenarios: Vec<Value> = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}"));
        assert!(scenarios.len() >= 5, "{flow}: {} scenarios", scenarios.len());
        let t = TempDir::new("ticket-desk");
        let mut ends = std::collections::BTreeSet::new();
        let mut old = None;
        for (i, sc) in scenarios.iter().enumerate() {
            assert_eq!(sc["rules"], "rulec", "{flow} {i}: {sc}");
            let f = t.path().join(format!("{i:03}.json"));
            std::fs::write(&f, sc.to_string()).unwrap();
            let f = f.to_string_lossy().into_owned();
            let (code, out, err) = ritsu_in(&dir, &["run", flow, "--scenario", &f, "--format", "json", "--lang", lang]);
            assert_eq!(code, 0, "{flow} {i}: {err}\n{sc}");
            let ran: Value = serde_json::from_str(&out).unwrap();
            let events = ran["events"].as_array().unwrap();
            let by = |who: &str| events.iter().filter(|e| e["by"] == who).count();
            assert_eq!(by("scenario"), sc["answers"].as_array().unwrap().len(), "{flow} {i}: every answer of the scenario is used, and no more:\n{out}");
            assert!(events.iter().filter(|e| e["call"] == rule).all(|e| e["by"] == "rulec"), "{flow} {i}: {out}");
            let (code, trace, err) = ritsu_in(&dir, &["dandori", "run", flow, "--scenario", &f]);
            assert_eq!(code, 0, "{flow} {i}: {err}");
            assert_eq!(serde_json::from_str::<Value>(&trace).unwrap(), ran["trace"], "{flow} {i}");
            let end = &ran["trace"]["end"];
            ends.insert(if end.get("succeed").is_some() { "succeed".to_string() } else { format!("fail {}", end["fail"]["error"]) });
            // the first run that calls the rule, written as `dandori run` reads it: the rule's
            // answer in its place among the answers
            if old.is_none() && by("rulec") > 0 {
                let answers: Vec<Value> = events.iter().filter(|e| e.get("answer").is_some()).map(|e| e["answer"].clone()).collect();
                let mut o = sc.clone();
                o.as_object_mut().unwrap().remove("rules");
                o["answers"] = Value::Array(answers);
                old = Some((o, ran["trace"].clone()));
            }
        }
        assert!(ends.contains("succeed") && ends.iter().any(|e| e.starts_with("fail")), "{flow}: {ends:?}");
        let (o, trace) = old.unwrap_or_else(|| panic!("{flow}: no scenario calls the rule"));
        let f = t.path().join("old.json");
        std::fs::write(&f, o.to_string()).unwrap();
        let f = f.to_string_lossy().into_owned();
        let (code, out, err) = ritsu_in(&dir, &["run", flow, "--scenario", &f, "--lang", lang]);
        assert!(code == 2 && out.is_empty() && err.contains(why), "{flow}: {code} {err}");
        let (code, out, err) = ritsu_in(&dir, &["dandori", "run", flow, "--scenario", &f]);
        assert_eq!(code, 0, "{flow}: {err}");
        assert_eq!(serde_json::from_str::<Value>(&out).unwrap(), trace, "{flow}");
    }
}

/// The preconditions of a rule that ritsu cannot decide where a flow calls it are checked where the
/// values they read are made, under `ritsu run` as in the code dandori writes and under `ritsu
/// dandori run` (dandori's DESIGN 1.17): the flow of `crates/dandori/tests/flows/never_lines.flow`,
/// whose rule has three `never` lines over truth values and a count with no range, in English and
/// its Japanese version. A run whose values break a line fails there with
/// `Dandori.BrokenPrecondition`, and rulec is not asked. Every scenario `ritsu dandori scenarios`
/// writes for it runs under `ritsu run` to the end `ritsu dandori run` comes to, with every answer it
/// gives used: the scenarios keep each line and break it, and reach the rule past them.
#[test]
fn a_precondition_ritsu_cannot_decide_is_checked_where_its_values_are_made() {
    let dir = here().join("../dandori/tests/flows");
    for (flow, lang) in [("never_lines.flow", "en"), ("never_lines.ja.flow", "ja")] {
        let (code, out, err) = ritsu_in(&dir, &["dandori", "scenarios", flow]);
        assert_eq!(code, 0, "{flow}: {err}");
        let scenarios: Vec<Value> = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}"));
        let t = TempDir::new("never-lines");
        let mut covers = std::collections::BTreeSet::new();
        let mut broken = std::collections::BTreeSet::new();
        for (i, sc) in scenarios.iter().enumerate() {
            covers.extend(sc["covers"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()));
            let f = t.path().join(format!("{i:03}.json"));
            std::fs::write(&f, sc.to_string()).unwrap();
            let f = f.to_string_lossy().into_owned();
            let (code, out, err) = ritsu_in(&dir, &["run", flow, "--scenario", &f, "--format", "json", "--lang", lang]);
            assert_eq!(code, 0, "{flow} {i}: {err}\n{sc}");
            let ran: Value = serde_json::from_str(&out).unwrap();
            let events = ran["events"].as_array().unwrap();
            assert_eq!(events.iter().filter(|e| e["by"] == "scenario").count(), sc["answers"].as_array().unwrap().len(), "{flow} {i}: every answer of the scenario is used, and no more:\n{out}");
            let (code, trace, err) = ritsu_in(&dir, &["dandori", "run", flow, "--scenario", &f]);
            assert_eq!(code, 0, "{flow} {i}: {err}");
            assert_eq!(serde_json::from_str::<Value>(&trace).unwrap(), ran["trace"], "{flow} {i}");
            let end = &ran["trace"]["end"]["fail"];
            if end["error"] == "Dandori.BrokenPrecondition" {
                assert!(events.iter().all(|e| e["by"] != "rulec"), "{flow} {i}: the rule is asked:\n{out}");
                broken.insert(end["cause"].as_str().unwrap().to_string());
            }
        }
        for site in [8, 9, 10] {
            for way in ["holds", "breaks"] {
                assert!(covers.contains(&format!("{site}:{way}")), "{flow}: no scenario covers {site}:{way}: {covers:?}");
            }
        }
        assert!(covers.contains("4:rulec"), "{flow}: no scenario reaches the rule: {covers:?}");
        assert_eq!(broken.len(), 3, "{flow}: each line is broken by a run of its own: {broken:?}");
    }
}

/// What a scenario of `ritsu dandori scenarios` says it `covers` is what its run goes through
/// (dandori's DESIGN 36): a run that the answer's check stops (`Dandori.BadResponse`,
/// `Dandori.UnexpectedState`) covers the way of the call that fails the check, and a run that ends
/// otherwise covers none; a run that covers a `succeed` succeeds, and one that covers a `fail`
/// fails. Two walks took a way their runs did not: the warehouse of the fulfillment example
/// answering a reservation with its fields at their zero values left out, where the zero value of
/// `stock` says it is not set and the check refuses it; and the order of `tests/projects/look_again`
/// looked at again after its cancellation is refused, where it can only be cancelled already. Each
/// flow in English and its Japanese version.
#[test]
fn what_a_scenario_covers_is_what_its_run_goes_through() {
    let failing = [":malformed", ":out of range", ":unexpected state", ":rule enum left out"];
    for (dir, flow, lang, way) in [
        ("../dandori/examples/fulfillment/temporal", "fulfillment.flow", "en", "3:zero values"),
        ("../dandori/examples/fulfillment/temporal", "fulfillment.ja.flow", "ja", "3:zero values"),
        ("tests/projects/look_again", "look_again.flow", "en", "4:sees:cancelled"),
        ("tests/projects/look_again", "look_again.ja.flow", "ja", "4:sees:取消"),
    ] {
        let dir = here().join(dir);
        let (code, out, err) = ritsu_in(&dir, &["dandori", "scenarios", flow]);
        assert_eq!(code, 0, "{flow}: {err}");
        let scenarios: Vec<Value> = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}"));
        let t = TempDir::new("covers");
        let mut all = std::collections::BTreeSet::new();
        for (i, sc) in scenarios.iter().enumerate() {
            let covers: Vec<&str> = sc["covers"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
            all.extend(covers.iter().map(|c| c.to_string()));
            let f = t.path().join(format!("{i:03}.json"));
            std::fs::write(&f, sc.to_string()).unwrap();
            let f = f.to_string_lossy().into_owned();
            let (code, out, err) = ritsu_in(&dir, &["run", flow, "--scenario", &f, "--format", "json", "--lang", lang]);
            assert_eq!(code, 0, "{flow} {i}: {err}\n{sc}");
            let ran: Value = serde_json::from_str(&out).unwrap();
            let end = &ran["trace"]["end"];
            let refused = ["Dandori.BadResponse", "Dandori.UnexpectedState"].contains(&end["fail"]["error"].as_str().unwrap_or(""));
            let fails_a_check = covers.iter().any(|c| failing.iter().any(|w| c.ends_with(w)));
            assert_eq!(refused, fails_a_check, "{flow} {i}: the run ends with {end}, and the scenario covers {covers:?}");
            if covers.iter().any(|c| c.ends_with(":succeed")) {
                assert!(end.get("succeed").is_some(), "{flow} {i}: the run ends with {end}, and the scenario covers {covers:?}");
            }
            if covers.iter().any(|c| c.ends_with(":fail")) {
                assert!(end.get("fail").is_some() && !refused, "{flow} {i}: the run ends with {end}, and the scenario covers {covers:?}");
            }
        }
        // the way the walk took wrong still has a run of its own, one that goes on past it
        assert!(all.contains(way), "{flow}: no scenario covers {way}: {all:?}");
    }
}

/// The walk that writes the scenarios reaches the arms of a `match` on a rule's answer by the values
/// the rule is given, as rulec answers them (dandori's DESIGN 36): `tests/projects/rule_arms`, whose
/// rule has a basic order's tools looked at when they count three or more, in English and its
/// Japanese version. A run is given a count of three, and its order is looked at; every scenario
/// runs under `ritsu run`, with each answer it gives taken. The rule sends a gold order's tools out
/// at once whatever the count, so the arm of the second `match` that would have them looked at is
/// one no run takes: `ritsu dandori scenarios` says so after the scenarios, with rulec's word for
/// it, and the page of the flow says it under its scenarios, which are the ones `scenarios` writes.
#[test]
fn every_arm_of_a_match_on_a_rule_s_answer_is_taken_or_said() {
    let dir = here().join("tests/projects/rule_arms");
    for (flow, lang, note, but) in [
        (
            "tool_orders.flow",
            "en",
            "note: no scenario takes the arm `review` of the `match` on line 26: rulec says the rule tool_check never comes to `route` = review with what the workflow can give it on line 25\n",
            "7 scenarios that together take every arm and every handler of an error but those below,",
        ),
        (
            "tool_orders.ja.flow",
            "ja",
            "備考: 26 行目の `match` の分岐 `確認` を通るシナリオはありません。rulec によると、25 行目でワークフローが渡しうる値では、規則 経路の判断 の `行き先` は 確認 になりません\n",
            "下に挙げるものを除く、すべての分岐と、エラーを処理するすべての箇所を通る 7 本のシナリオを、",
        ),
    ] {
        let (code, out, err) = ritsu_in(&dir, &["dandori", "scenarios", flow, "--lang", lang]);
        assert_eq!((code, err.as_str()), (0, note), "{flow}");
        let scenarios: Vec<Value> = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}"));
        let t = TempDir::new("rule-arms");
        let mut covers = std::collections::BTreeSet::new();
        for (i, sc) in scenarios.iter().enumerate() {
            covers.extend(sc["covers"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()));
            let f = t.path().join(format!("{i:03}.json"));
            std::fs::write(&f, sc.to_string()).unwrap();
            let f = f.to_string_lossy().into_owned();
            let (code, out, err) = ritsu_in(&dir, &["run", flow, "--scenario", &f, "--format", "json", "--lang", lang]);
            assert_eq!(code, 0, "{flow} {i}: {err}\n{sc}");
            let ran: Value = serde_json::from_str(&out).unwrap();
            let events = ran["events"].as_array().unwrap();
            assert_eq!(events.iter().filter(|e| e["by"] == "scenario").count(), sc["answers"].as_array().unwrap().len(), "{flow} {i}: {out}");
            // the run looked at reads a count of three, the least the rule looks at
            if sc["covers"].as_array().unwrap().iter().any(|c| c == "3:arm1") {
                assert_eq!(sc["answers"][0], json!({ "ok": 3 }), "{flow} {i}");
            }
        }
        assert!(covers.contains("3:arm0") && covers.contains("3:arm1"), "{flow}: the first `match` has an arm no scenario takes: {covers:?}");
        assert!(covers.contains("7:arm0") && !covers.contains("7:arm1"), "{flow}: {covers:?}");
        let (code, page, err) = ritsu_in(&dir, &["dandori", "doc", flow, "--format", "html", "--lang", lang]);
        assert_eq!(code, 0, "{flow}: {err}");
        assert_eq!(page.matches("<template id=\"t-run-").count(), scenarios.len(), "{flow}: the page shows other runs than the scenarios");
        assert!(page.contains(but), "{flow}: the page says its scenarios take every arm");
        // the item under them, as `scenarios` says it, its code between backquotes as code
        let said = note.split_once(": ").unwrap().1.trim_end();
        let item: String = said.split('`').enumerate().map(|(k, part)| if k % 2 == 1 { format!("<code>{part}</code>") } else { part.to_string() }).collect();
        assert!(page.contains(&format!("<ul class=\"untaken\"><li>{item}</li></ul>")), "{flow}: the page does not say {said}");
    }
}

/// The page of a flow shows the runs `ritsu dandori scenarios` writes, each as `ritsu run` runs it
/// (dandori's DESIGN 36): on the page of each flow here, as many runs as scenarios, and run k ends
/// where `ritsu run` of scenario k ends. The flows call rules (`tests/projects/rule_arms`, the
/// review example), a dates file and a book (the invoice example), and run on a clock that moves
/// out of their period (dandori's tests/flows/date_ranges.flow and its Japanese version), where a
/// page that drew the runs the platforms are tried with showed runs `ritsu run` does not make. The
/// page names an error the run makes of a failed or malformed answer in the words of one platform,
/// and `ritsu run` in those of the flow's, so for those only that the run fails is held.
#[test]
fn the_page_of_a_flow_shows_the_runs_its_scenarios_make() {
    let dandori = here().join("../dandori");
    let flows = [
        (here().join("tests/projects/rule_arms"), "tool_orders.flow"),
        (dandori.join("examples/review/temporal"), "review.flow"),
        (dandori.join("examples/invoice"), "invoice.flow"),
        (dandori.join("tests/flows"), "date_ranges.flow"),
        (dandori.join("tests/flows"), "date_ranges.ja.flow"),
    ];
    let made = ["Dandori.Test.Failure", "Dandori.Test.Timeout", "States.QueryEvaluationError", "States.TaskFailed", "States.Timeout"];
    let mut runs = 0;
    for (dir, flow) in flows {
        let (code, out, err) = ritsu_in(&dir, &["dandori", "scenarios", flow]);
        assert_eq!(code, 0, "{flow}: {err}");
        let scenarios: Vec<Value> = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}"));
        let (code, page, err) = ritsu_in(&dir, &["dandori", "doc", flow, "--format", "html", "--lang", "en"]);
        assert_eq!(code, 0, "{flow}: {err}");
        // each run's end, as the heading of its steps says it
        let ends: Vec<String> = page
            .split("<template id=\"t-run-")
            .skip(1)
            .map(|t| {
                let h = t.split_once("<h2 class=\"dh\">").map_or("", |x| x.1);
                let chip = h.split_once("<span class=\"chip ").and_then(|x| x.1.split_once("\">")).map_or("", |x| x.1);
                chip.split_once("</span>").map_or("", |x| x.0).replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
            })
            .collect();
        assert_eq!(ends.len(), scenarios.len(), "{flow}: {} runs on the page, {} scenarios", ends.len(), scenarios.len());
        let t = TempDir::new("page-runs");
        for (i, sc) in scenarios.iter().enumerate() {
            let f = t.path().join(format!("{i:03}.json"));
            std::fs::write(&f, sc.to_string()).unwrap();
            let f = f.to_string_lossy().into_owned();
            let (code, out, err) = ritsu_in(&dir, &["run", flow, "--scenario", &f, "--format", "json", "--lang", "en"]);
            assert_eq!(code, 0, "{flow} {i}: {err}\n{sc}");
            let ran: Value = serde_json::from_str(&out).unwrap();
            let end = &ran["trace"]["end"];
            let shown = &ends[i];
            let same = if end.get("succeed").is_some() {
                shown == "succeeds"
            } else if end.get("cancel").is_some() {
                shown == "ends cancelled"
            } else {
                let error = end["fail"]["error"].as_str().unwrap_or_default();
                if made.contains(&error) {
                    shown.starts_with("fail")
                } else {
                    shown.starts_with("fail") && shown.contains(error)
                }
            };
            assert!(same, "{flow} {i}: the page says the run {shown}, and `ritsu run` ends it with {end}");
            runs += 1;
        }
    }
    assert!(runs >= 40, "only {runs} runs");
}

/// `ritsu run --with` (rulec's §15.231): the contributions it is given are merged into the rules the
/// flow calls. With the product line, an item of the kind it adds is priced with its markup; without
/// it, the kind is no value of the rule and the input is turned away; a kind the rule has is priced
/// alike either way. A path that is not there stops the run.
#[test]
fn ritsu_run_with_merges_the_contributions_it_is_given() {
    let dir = here().join("tests/projects/product_line");
    let total = |args: &[&str]| -> Value {
        let mut a = vec!["run", "pricing.flow", "--format", "json", "--lang", "en"];
        a.extend_from_slice(args);
        let (code, out, err) = ritsu_in(&dir, &a);
        assert_eq!(code, 0, "{args:?}: {err}");
        let ran: Value = serde_json::from_str(&out).unwrap();
        ran["trace"]["end"].clone()
    };
    assert_eq!(total(&["--scenario", "scenarios/garden.json", "--with", "rules/contrib"]), json!({"succeed": {"total": 23}}));
    assert_eq!(total(&["--scenario", "scenarios/garden.json", "--with", "rules/contrib/garden_kind.rule", "--with", "rules/contrib/garden_price.rule"]), json!({"succeed": {"total": 23}}));
    assert_eq!(total(&["--scenario", "scenarios/garden.json"])["fail"]["error"], "Dandori.BadInput");
    for with in [&["--with", "rules/contrib"][..], &[][..]] {
        let mut args = vec!["--scenario", "scenarios/toys.json"];
        args.extend_from_slice(with);
        assert_eq!(total(&args), json!({"succeed": {"total": 24}}));
    }
    let (code, out, err) = ritsu_in(&dir, &["run", "pricing.flow", "--scenario", "scenarios/garden.json", "--with", "rules/nowhere", "--lang", "en"]);
    assert!(code == 2 && out.is_empty() && err.contains("`rules/nowhere` is not there"), "{code} {out} {err}");
}

/// A call whose answer is waited for until a time (`timeout until`, dandori's DESIGN 16), here the
/// time of a koyomi date given `now`: when the scenario times it out, the clock goes on to that time,
/// rounded up to the second, and the hold the run made expires on the way, as the book says when the
/// void comes. The text says until when the call waited, in both languages.
#[test]
fn a_timeout_until_a_time_moves_the_clock_to_it() {
    let t = TempDir::new("until");
    let write = |name: &str, body: &str| std::fs::write(t.path().join(name), body).unwrap();
    write("weekdays.cal", "calendar weekdays v1\ndescription \"Open Monday to Friday, in UTC\"\noffset +00:00\n\nclosed weekly sat, sun\n");
    write("approval_terms.cal", "dates approval_terms v1\ndescription \"An approval is due at 17:00 on the third business day after the order\"\nuse calendar \"weekdays.cal\"\n\ninputs\n  received : date  range >=2026-01-01 <=2027-12-31\n\ndate due = received\n  + 3 business days\n  at 17:00\n");
    write("stock.book", "book stock v1\ndescription \"Stock, held for 2 days at most\"\n\nunit pcs\n\naccount shelf(sku: string) : pcs\n  at least 0 refused as out_of_stock\naccount suppliers : pcs outside\naccount customers : pcs outside\n\ntransfer receive(delivery: string, sku: string, qty: pcs)\n  key delivery, sku\n  move qty from suppliers to shelf(sku)\n\ntransfer reserve(order: string, sku: string, qty: pcs)\n  key order, sku\n  pending expires after 2 days\n  move qty from shelf(sku) to customers\n");
    write("approval.flow", "workflow approval v1\ndescription \"Holds an order's goods while someone approves it, until 17:00 on the third business day, and puts them back when no one has\"\n\nuse dates terms from \"approval_terms.cal\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:approval-terms\"\nuse book stock from \"stock.book\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:stock\"\n\nrecord Approval\n  approver : string\n\ninputs\n  order : string\n  sku   : string\n  qty   : int  range >=1 <=100\n\noutputs\n  outcome : string\n\ntask reserve(order: string, sku: string, qty: int) -> stock.reserve\n  book stock.reserve.hold\n  starts stock.reserve\n  errors out_of_stock\n\ntask ship(order: string, sku: string) -> stock.reserve\n  book stock.reserve.post\n  sends post\n  errors expired\n\ntask put_back(order: string, sku: string) -> stock.reserve\n  book stock.reserve.void\n  sends void\n  errors expired, already_posted\n\ntask ask(order: string, by: timestamp) -> Approval\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:ask\"\n  callback\n  timeout until by\n\ncase goods : stock.reserve follows stock.reserve\n\nflow\n  goods <- reserve(order: order, sku: sku, qty: qty)\n    on out_of_stock => fail OutOfStock \"nothing left on the shelf\"\n  let due = terms.due(received: now)\n  let a = ask(order: order, by: due.at)\n    on timeout =>\n      goods <- put_back(order: order, sku: sku)\n        on expired => succeed outcome = \"expired\"\n      succeed outcome = \"put back\"\n  goods <- ship(order: order, sku: sku)\n    on expired => succeed outcome = \"expired\"\n  succeed outcome = \"approved by {a.approver}\"\n\non failure\n  match goods.state\n    none => pass\n    held =>\n      goods <- put_back(order: order, sku: sku)\n        on expired => pass\n        on already_posted => pass\n        on failure => fail NotPutBack \"the goods held for the order could not be put back\" leaving goods\n");
    write("late.json", &json!({ "execution": "late", "now": "2026-04-03T10:00:00Z", "input": { "order": "A-1", "sku": "pen", "qty": 3 }, "books": { "stock": [{ "op": "do", "kind": "receive", "args": { "delivery": "D-1", "sku": "pen", "qty": 10 } }] }, "answers": [{ "error": "timeout" }] }).to_string());
    let mut failures = Vec::new();
    for lang in ["en", "ja"] {
        let (code, out, err) = ritsu_in(t.path(), &["run", "approval.flow", "--scenario", "late.json", "--lang", lang]);
        assert!(code == 0 && err.is_empty(), "{lang}: {code} {err}");
        golden(&format!("tests/golden/run/until_timeout.{lang}.txt"), &out, &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// A workflow that runs in a period (dandori's DESIGN 20), under `ritsu run`: `now` reads the
/// scenario's moments, as `dandori run` reads them, the start's, and the next for the first statement
/// that reads `now` after the start. A run that starts in the period and reads `now` past its end,
/// after a wait of three days that moves the clock to that moment, fails where it reads it, and its
/// `on failure` puts the goods back; a run that starts past the end calls nothing. The text says so
/// in both languages.
#[test]
fn a_run_past_the_end_of_its_period_fails_where_it_reads_now() {
    let t = TempDir::new("period");
    let write = |name: &str, body: &str| std::fs::write(t.path().join(name), body).unwrap();
    write("weekdays.cal", WEEKDAYS);
    write("approval_terms.cal", APPROVAL_TERMS);
    write("stock.book", STOCK);
    write("approval.flow", APPROVAL);
    let received = json!([{ "op": "do", "kind": "receive", "args": { "delivery": "D-1", "sku": "pen", "qty": 10 } }]);
    let input = json!({ "order": "A-1", "sku": "pen", "qty": 3 });
    write("past.json", &json!({ "execution": "past", "now": ["2027-12-30T10:00:00Z", "2028-01-02T10:00:00Z"], "input": input, "books": { "stock": received }, "answers": [] }).to_string());
    write("after.json", &json!({ "execution": "after", "now": "2028-01-02T10:00:00Z", "input": input, "books": { "stock": received }, "answers": [] }).to_string());
    let mut failures = Vec::new();
    for (scenario, name) in [("past.json", "period_past"), ("after.json", "period_after")] {
        for lang in ["en", "ja"] {
            let (code, out, err) = ritsu_in(t.path(), &["run", "approval.flow", "--scenario", scenario, "--lang", lang]);
            assert!(code == 0 && err.is_empty(), "{scenario} {lang}: {code} {err}");
            assert!(out.contains("Dandori.OutOfPeriod"), "{scenario} {lang}: {out}");
            golden(&format!("tests/golden/run/{name}.{lang}.txt"), &out, &mut failures);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    // with the start's moment alone, the wait takes the clock past the end by itself: the run fails
    // where it reads `now`, as the one given the moment after the wait does
    write("one.json", &json!({ "execution": "past", "now": "2027-12-30T10:00:00Z", "input": input, "books": { "stock": received }, "answers": [] }).to_string());
    let trace = |scenario: &str| -> Value {
        let (code, out, err) = ritsu_in(t.path(), &["run", "approval.flow", "--scenario", scenario, "--format", "json"]);
        assert_eq!(code, 0, "{scenario}: {err}");
        serde_json::from_str::<Value>(&out).unwrap()["trace"].clone()
    };
    let one = trace("one.json");
    assert_eq!(one["end"]["fail"]["error"], "Dandori.OutOfPeriod", "{one}");
    assert_eq!(one, trace("past.json"));
}

/// The flow of the clock: it shows `now`, waits two days, shows it, waits a day, and shows it again.
const CLOCK: &str = "workflow clock v1\n  now range >=2026-01-01 <=2027-12-31 offset +09:00\ndescription \"Show the time before and after a wait of two days, and after one more day\"\n\ninputs\n  id : string\n\noutputs\n  id : string\n\ntask show(id: string, at: timestamp)\n\nflow\n  show(id: id, at: now)\n  wait 2 days\n  show(id: id, at: now)\n  wait 1 day\n  show(id: id, at: now)\n  succeed id = id\n";

/// `now` reads the clock when the statement that reads it runs (dandori's DESIGN 1.16), and the
/// clock goes on by each wait, under `ritsu run` as under `ritsu dandori run`. With the start's
/// moment alone, the run starts at it; given more moments, each statement that reads `now` after the
/// start reads the next of them, and the clock goes on to it; one that comes before the clock cannot
/// be read, and the scenario says so. The replay keeps the moments it was given, and `ritsu dandori
/// run` makes the same trace of it. A deadline counted from `now` after a wait counts from the moment
/// the wait took the run to: the approval asked for after six days is due on the third business day
/// after that day, and the timeout waits until then — not the second a timeout waits at the least
/// (DESIGN 16), as it would for a deadline counted from the start, which has gone by then.
#[test]
fn now_reads_the_clock_when_the_statement_runs() {
    let t = TempDir::new("clock");
    let write = |name: &str, body: &str| std::fs::write(t.path().join(name), body).unwrap();
    write("clock.flow", CLOCK);
    let shown = |now: Value| -> Vec<String> {
        write("clock.json", &json!({ "now": now, "input": { "id": "c-1" }, "answers": [{ "ok": null }, { "ok": null }, { "ok": null }] }).to_string());
        let (code, out, err) = ritsu_in(t.path(), &["run", "clock.flow", "--scenario", "clock.json", "--format", "json"]);
        assert_eq!(code, 0, "{now}: {err}");
        let ran: Value = serde_json::from_str(&out).unwrap();
        let (code, trace, err) = ritsu_in(t.path(), &["dandori", "run", "clock.flow", "--scenario", "clock.json"]);
        assert_eq!(code, 0, "{now}: {err}");
        assert_eq!(serde_json::from_str::<Value>(&trace).unwrap(), ran["trace"], "{now}");
        assert_eq!(ran["replay"]["now"], now);
        write("replay.json", &ran["replay"].to_string());
        let (code, trace, err) = ritsu_in(t.path(), &["dandori", "run", "clock.flow", "--scenario", "replay.json"]);
        assert_eq!(code, 0, "{now}: {err}");
        assert_eq!(serde_json::from_str::<Value>(&trace).unwrap(), ran["trace"], "{now}: the replay");
        ran["events"].as_array().unwrap().iter().filter(|e| e["call"] == "show").map(|e| e["args"]["at"].as_str().unwrap().to_string()).collect()
    };
    assert_eq!(shown(json!("2026-11-13T01:00:00Z")), ["2026-11-13T01:00:00Z", "2026-11-15T01:00:00Z", "2026-11-16T01:00:00Z"]);
    assert_eq!(shown(json!(["2026-11-13T01:00:00Z", "2026-11-15T01:00:00Z"])), ["2026-11-15T01:00:00Z", "2026-11-17T01:00:00Z", "2026-11-18T01:00:00Z"]);
    write("back.json", &json!({ "now": ["2026-11-13T01:00:00Z", "2026-11-12T01:00:00Z"], "input": { "id": "c-1" }, "answers": [{ "ok": null }, { "ok": null }, { "ok": null }] }).to_string());
    for (lang, said) in [
        ("en", "error: the scenario `back.json` cannot be run: moment 2 of the scenario's `now`, `2026-11-12T01:00:00Z`, comes before 2026-11-13T01:00:00Z, when the statement that reads it runs: time does not go back, so the run cannot read it\n"),
        ("ja", "エラー: シナリオ `back.json` を流せません: シナリオの `now` の 2 番目の時刻 `2026-11-12T01:00:00Z` は、それを読む文が走る時刻 2026-11-13T01:00:00Z より前です。時間は戻らないので、この実行はその時刻を読めません\n"),
    ] {
        assert_eq!(ritsu_in(t.path(), &["run", "clock.flow", "--scenario", "back.json", "--lang", lang]), (2, String::new(), said.to_string()));
    }

    write("weekdays.cal", WEEKDAYS);
    write("approval_terms.cal", APPROVAL_TERMS);
    write("stock.book", STOCK);
    write("approval.flow", &APPROVAL.replace("wait 3 days", "wait 6 days"));
    write("late.json", &json!({ "now": "2026-04-03T10:00:00Z", "input": { "order": "A-1", "sku": "pen", "qty": 3 }, "books": { "stock": [{ "op": "do", "kind": "receive", "args": { "delivery": "D-1", "sku": "pen", "qty": 10 } }] }, "answers": [{ "error": "timeout" }] }).to_string());
    let (code, out, err) = ritsu_in(t.path(), &["run", "approval.flow", "--scenario", "late.json", "--format", "json"]);
    assert_eq!(code, 0, "{err}");
    let ran: Value = serde_json::from_str(&out).unwrap();
    let events = ran["events"].as_array().unwrap();
    let due = events.iter().find(|e| e["call"] == "terms.due").unwrap();
    assert_eq!((&due["args"]["asked"], &due["answer"]["ok"]["at"]), (&json!("2026-04-09T10:00:00Z"), &json!("2026-04-14T17:00:00Z")), "{out}");
    let timeout = events.iter().find(|e| e["why"] == "timeout").unwrap();
    assert_eq!(timeout, &json!({ "pass": 457200, "why": "timeout", "until": "2026-04-14T17:00:00Z", "after": "2026-04-14T17:00:00Z" }));
    assert_eq!(ran["trace"]["end"], json!({ "succeed": { "outcome": "put back" } }));
}

/// What a scenario does to a book before the run may let time go by (`pass`, as `chobo run`'s
/// scenario writes it): a hold made before the run, thirty days before it starts, has expired by
/// then, and the shipment the run comes to is refused. The text says how long went by, in both
/// languages, and the JSON writes it with what it came to. A duration that is not one, and the
/// operations of `together`, which no run before this one does, stop the run.
#[test]
fn time_that_went_by_before_the_run_expires_a_hold() {
    let t = TempDir::new("aged");
    let write = |name: &str, body: &str| std::fs::write(t.path().join(name), body).unwrap();
    write("weekdays.cal", WEEKDAYS);
    write("approval_terms.cal", APPROVAL_TERMS);
    write("stock.book", STOCK);
    write("approval.flow", APPROVAL);
    let received = json!({ "op": "do", "kind": "receive", "args": { "delivery": "D-1", "sku": "pen", "qty": 10 } });
    let held = json!({ "op": "hold", "kind": "reserve", "args": { "order": "A-1", "sku": "pen", "qty": 3 } });
    let scenario = |then: Value| json!({ "now": "2026-04-03T10:00:00Z", "input": { "order": "A-1", "sku": "pen", "qty": 3 }, "books": { "stock": [received, held, then] }, "answers": [{ "ok": { "approver": "kim" } }] }).to_string();
    write("aged.json", &scenario(json!({ "op": "pass", "duration": "30 days" })));
    for (lang, line, refused) in [("en", "  pass 30 days                                      time goes by\n", "refused: expired"), ("ja", "  pass 30 days                                      時間が過ぎる\n", "expired で拒否される")] {
        let (code, out, err) = ritsu_in(t.path(), &["run", "approval.flow", "--scenario", "aged.json", "--lang", lang]);
        assert!(code == 0 && err.is_empty(), "{lang}: {err}");
        assert!(out.contains(line) && out.contains(refused) && out.contains("succeed {\"outcome\":\"expired\"}"), "{lang}: {out}");
    }
    let (code, out, err) = ritsu_in(t.path(), &["run", "approval.flow", "--scenario", "aged.json", "--format", "json"]);
    assert_eq!(code, 0, "{err}");
    let ran: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(ran["books"][0]["before"][2], json!({ "op": "pass", "duration": "30 days", "result": "passed" }));
    assert_eq!(ran["books"][0]["holds"], json!([{ "kind": "reserve", "key": ["A-1", "pen"], "state": "expired" }]));
    write("not_a_duration.json", &scenario(json!({ "op": "pass", "duration": "a month" })));
    let (code, _, err) = ritsu_in(t.path(), &["run", "approval.flow", "--scenario", "not_a_duration.json"]);
    assert!(code == 2 && err.ends_with("cannot be run: operation 3 of the book `stock`: a `pass` needs a duration like `\"duration\": \"7 days\"` (in seconds, minutes, hours or days)\n"), "{err}");
    write("together.json", &scenario(json!({ "op": "together", "callers": [[{ "op": "post", "kind": "reserve", "args": { "order": "A-1", "sku": "pen" } }]] })));
    let (code, _, err) = ritsu_in(t.path(), &["run", "approval.flow", "--scenario", "together.json", "--lang", "ja"]);
    assert!(code == 2 && err.ends_with("を流せません: 帳簿 `stock` の 3 番目の操作: 走らせる前の操作には `together` を書けません。操作を一つずつ順に書いてください\n"), "{err}");
}

/// Every scenario `ritsu dandori scenarios` writes for a flow that calls dates and does operations
/// of books (the approval flow above, the invoice of `tests/projects/invoice` in English and in
/// Japanese, and the order of `tests/projects/stockroom`) says the dates are koyomi's and what is done
/// to the books before the run, answers no call of a date and no operation of a book itself, and
/// runs under `ritsu run` to the end `ritsu dandori run` comes to, with every answer it gives used.
/// What was done before the run is how the runs reach a book's refusals and what the run does to a
/// hold made before it: of the approval flow, the hold done before the run, and since posted, voided
/// or expired; another order with the same key; a shelf with too little on it. A scenario written as
/// `dandori run` reads one, which answers the dates or the books itself, stops `ritsu run` at the
/// first such call and says why; `ritsu dandori run` runs it to the same trace.
#[test]
fn every_scenario_of_a_flow_with_dates_and_books_runs_under_ritsu_run() {
    let t = TempDir::new("dates-and-books");
    let write = |name: &str, body: &str| std::fs::write(t.path().join(name), body).unwrap();
    write("weekdays.cal", WEEKDAYS);
    write("approval_terms.cal", APPROVAL_TERMS);
    write("stock.book", STOCK);
    write("approval.flow", APPROVAL);
    let (invoice, stockroom) = (project(), here().join("tests/projects/stockroom"));
    let flows: [(&Path, &str, &str); 4] = [(t.path(), "approval.flow", "en"), (&invoice, "invoice.flow", "en"), (&invoice, "invoice.ja.flow", "ja"), (&stockroom, "orders/order.flow", "en")];
    let out_dir = TempDir::new("dates-and-books-scenarios");
    for (dir, flow, lang) in flows {
        let (code, out, err) = ritsu_in(dir, &["dandori", "scenarios", flow]);
        assert_eq!(code, 0, "{flow}: {err}");
        let scenarios: Vec<Value> = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}"));
        let mut covers = std::collections::BTreeSet::new();
        let mut old = None;
        for (i, sc) in scenarios.iter().enumerate() {
            assert!(sc["dates"] == "koyomi" && sc["books"].is_object(), "{flow} {i}: {sc}");
            covers.extend(sc["covers"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()));
            let f = out_dir.path().join(format!("{}-{i:03}.json", flow.replace('/', "-")));
            std::fs::write(&f, sc.to_string()).unwrap();
            let f = f.to_string_lossy().into_owned();
            let (code, out, err) = ritsu_in(dir, &["run", flow, "--scenario", &f, "--format", "json", "--lang", lang]);
            assert_eq!(code, 0, "{flow} {i}: {err}\n{sc}");
            let ran: Value = serde_json::from_str(&out).unwrap();
            let events = ran["events"].as_array().unwrap();
            let by = |who: &str| events.iter().filter(|e| e["by"] == who).count();
            assert_eq!(by("scenario"), sc["answers"].as_array().unwrap().len(), "{flow} {i}: every answer of the scenario is used, and no more:\n{out}");
            let (code, trace, err) = ritsu_in(dir, &["dandori", "run", flow, "--scenario", &f]);
            assert_eq!(code, 0, "{flow} {i}: {err}");
            assert_eq!(serde_json::from_str::<Value>(&trace).unwrap(), ran["trace"], "{flow} {i}");
            // the first run koyomi and chobo both answer in, written as `dandori run` read it: the
            // answers of the one or the other in their places among the scenario's
            if old.is_none() && by("koyomi") > 0 && by("chobo") > 0 {
                let answers = |also: &str| -> Value { events.iter().filter(|e| e["by"] == "scenario" || e["by"] == also).map(|e| e["answer"].clone()).collect() };
                let mut dates = sc.clone();
                dates.as_object_mut().unwrap().remove("dates");
                dates["answers"] = answers("koyomi");
                let mut books = sc.clone();
                books.as_object_mut().unwrap().remove("books");
                books["answers"] = answers("chobo");
                old = Some((dates, books, ran["trace"].clone()));
            }
        }
        if flow == "approval.flow" {
            for label in ["1:done before", "1:refused:key_conflict", "1:refused:out_of_stock", "9:done before", "9:refused:already_voided", "9:refused:expired", "6:refused:expired", "14:refused:already_posted"] {
                assert!(covers.contains(label), "{flow}: no scenario covers {label}: {covers:?}");
            }
        }
        let (dates, books, trace) = old.unwrap_or_else(|| panic!("{flow}: no run asks koyomi and chobo both"));
        for (name, sc, why) in [("dates", dates, if lang == "en" { "cannot be run: it answers the call of the date " } else { "を流せません: 日付 " }), ("books", books, if lang == "en" { "cannot be run: it answers the operation of the book " } else { "を流せません: 帳簿 " })] {
            let f = out_dir.path().join(format!("old-{name}.json"));
            std::fs::write(&f, sc.to_string()).unwrap();
            let f = f.to_string_lossy().into_owned();
            let (code, out, err) = ritsu_in(dir, &["run", flow, "--scenario", &f, "--lang", lang]);
            assert!(code == 2 && out.is_empty() && err.contains(why) && err.contains("`ritsu dandori scenarios`"), "{flow} {name}: {code} {err}");
            let (code, out, err) = ritsu_in(dir, &["dandori", "run", flow, "--scenario", &f]);
            assert_eq!(code, 0, "{flow} {name}: {err}");
            assert_eq!(serde_json::from_str::<Value>(&out).unwrap(), trace, "{flow} {name}");
        }
    }
}

const WEEKDAYS: &str = "calendar weekdays v1\ndescription \"Open Monday to Friday, in UTC\"\noffset +00:00\n\nclosed weekly sat, sun\n";

const APPROVAL_TERMS: &str = "dates approval_terms v1\ndescription \"An approval is due at 17:00 on the third business day after it is asked for\"\nuse calendar \"weekdays.cal\"\n\ninputs\n  asked : date  range >=2026-01-01 <=2027-12-31\n\ndate due = asked\n  + 3 business days\n  at 17:00\n";

const STOCK: &str = "book stock v1\ndescription \"Stock, held for 30 days at most\"\n\nunit pcs\n\naccount shelf(sku: string) : pcs\n  at least 0 refused as out_of_stock\naccount suppliers : pcs outside\naccount customers : pcs outside\n\ntransfer receive(delivery: string, sku: string, qty: pcs)\n  key delivery, sku\n  move qty from suppliers to shelf(sku)\n\ntransfer reserve(order: string, sku: string, qty: pcs)\n  key order, sku\n  pending expires after 30 days\n  move qty from shelf(sku) to customers\n";

const APPROVAL: &str = "workflow approval v1\n  now range >=2026-01-01 <=2027-12-31\ndescription \"Holds an order's goods, waits three days for the goods to be checked, and asks for an approval due on the third business day after; it runs in 2026 and 2027, the years the approval terms know\"\n\nuse dates terms from \"approval_terms.cal\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:approval-terms\"\nuse book stock from \"stock.book\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:stock\"\n\nrecord Approval\n  approver : string\n\ninputs\n  order : string\n  sku   : string\n  qty   : int  range >=1 <=100\n\noutputs\n  outcome : string\n\ntask reserve(order: string, sku: string, qty: int) -> stock.reserve\n  book stock.reserve.hold\n  starts stock.reserve\n  errors out_of_stock\n\ntask ship(order: string, sku: string) -> stock.reserve\n  book stock.reserve.post\n  sends post\n  errors expired\n\ntask put_back(order: string, sku: string) -> stock.reserve\n  book stock.reserve.void\n  sends void\n  errors expired, already_posted\n\ntask ask(order: string, by: timestamp) -> Approval\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:ask\"\n  callback\n  timeout until by\n\ncase goods : stock.reserve follows stock.reserve\n\nflow\n  goods <- reserve(order: order, sku: sku, qty: qty)\n    on out_of_stock => fail OutOfStock \"nothing left on the shelf\"\n  wait 3 days\n  let due = terms.due(asked: now)\n  let a = ask(order: order, by: due.at)\n    on timeout =>\n      goods <- put_back(order: order, sku: sku)\n        on expired => succeed outcome = \"expired\"\n      succeed outcome = \"put back\"\n  goods <- ship(order: order, sku: sku)\n    on expired => succeed outcome = \"expired\"\n  succeed outcome = \"approved by {a.approver}\"\n\non failure\n  match goods.state\n    none => pass\n    held =>\n      goods <- put_back(order: order, sku: sku)\n        on expired => pass\n        on already_posted => pass\n        on failure => fail NotPutBack \"the goods held for the order could not be put back\" leaving goods\n";

// ── The workflow of sakai's shop: how each way it ends leaves the holds and the order ─────────

/// One of the two versions of the example, in the words of its `.flow`.
struct Shop {
    dir: &'static str,
    flow: &'static str,
    order: fn() -> Value,
    delivery: fn() -> Value,
    packing: fn() -> Value,
    short: &'static str,
    /// The tasks: release, post, cancel, notify.
    calls: [&'static str; 4],
    /// The failures: short of stock, a conflict, the delivery, the packing late, the packing short.
    fails: [&'static str; 5],
}

const SHOPS: [Shop; 2] = [
    Shop {
        dir: "../sakai/examples/shop",
        flow: "ordering/fulfillment.flow",
        order: || {
            json!({"order": {"id": "o1", "member": false, "amount": 1000, "gift": null, "extra": {}, "lines": [
                {"id": "l1", "sku": "A", "quantity": 1},
                {"id": "l2", "sku": "B", "quantity": 1},
            ]}})
        },
        delivery: || json!({"tracking_number": "T1"}),
        packing: || json!({"boxes": 1}),
        short: "short",
        calls: ["release_stock", "post_stock", "cancel_order", "notify"],
        fails: ["OutOfStock", "StockConflict", "DeliveryFailed", "PackingLate", "PackingShort"],
    },
    Shop {
        dir: "../sakai/examples/shop.ja",
        flow: "ordering/受注.flow",
        order: || {
            json!({"注文": {"id": "o1", "会員": false, "金額": 1000, "贈り物": null, "付帯": {}, "明細": [
                {"id": "l1", "sku": "A", "数": 1},
                {"id": "l2", "sku": "B", "数": 1},
            ]}})
        },
        delivery: || json!({"追跡番号": "T1"}),
        packing: || json!({"箱の数": 1}),
        short: "欠品",
        calls: ["引当を戻す", "引当を確定する", "注文を取り消す", "知らせる"],
        fails: ["在庫不足", "明細の食い違い", "配送の手配の失敗", "梱包の遅れ", "梱包の欠品"],
    },
];

/// Plays a scenario of the shop: the calls after the holds, by name and first argument, and how it ended.
fn shop_ends(shop: &Shop, answers: Vec<Value>) -> (Vec<String>, Value) {
    let t = TempDir::new("shop-run");
    let scenario = t.path().join("s.json");
    std::fs::write(&scenario, json!({"name": "s", "input": (shop.order)(), "answers": answers, "rules": "rulec"}).to_string()).unwrap();
    let dir = here().join(shop.dir);
    let (code, out, err) = ritsu_in(&dir, &["run", shop.flow, "--scenario", scenario.to_str().unwrap(), "--format", "json"]);
    assert_eq!(code, 0, "{out}{err}");
    let ran: Value = serde_json::from_str(&out).unwrap();
    let calls: Vec<String> = ran["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| Some((e["call"].as_str()?, e["args"].as_object()?)))
        .filter(|(c, _)| shop.calls.contains(c))
        .map(|(c, a)| if c == shop.calls[3] { c.to_string() } else { format!("{c}({})", a["id"].as_str().unwrap()) })
        .collect();
    (calls, ran["trace"]["end"].clone())
}

/// In every way the workflow fails after the holds, it gives back what it held and cancels the order,
/// which is what `billing_need.rule` reads as not to be billed; and a line held before with another
/// sku or quantity is told from a shelf gone empty. The way it succeeds cancels nothing.
#[test]
fn the_shop_gives_back_and_cancels_in_every_way_it_fails() {
    let ok = |v: Value| json!({"ok": v});
    let done = || json!({"ok": null});
    let stock = |sku: &str, what: &str, id: Option<&str>| match id {
        Some(id) => json!({"ok": {"sku": sku, "stock": what, "id": id}}),
        None => json!({"ok": {"sku": sku, "stock": what}}),
    };
    let held = || vec![stock("A", "STOCK_SECURED", Some("l1")), stock("B", "STOCK_SECURED", Some("l2"))];
    let with = |mut a: Vec<Value>, more: Vec<Value>| {
        a.extend(more);
        a
    };
    for shop in &SHOPS {
        let [release, post, cancel, notify] = shop.calls;
        let (rel1, rel2, gone) = (format!("{release}(l1)"), format!("{release}(l2)"), format!("{cancel}(o1)"));

        let (calls, end) = shop_ends(shop, with(held(), vec![ok((shop.delivery)()), ok((shop.packing)()), done(), done(), done()]));
        assert_eq!(calls, [format!("{post}(l1)"), format!("{post}(l2)"), notify.to_string()], "{}", shop.dir);
        assert!(end.get("succeed").is_some(), "{end}");

        let failures = [
            (vec![stock("A", "STOCK_SECURED", Some("l1")), stock("B", "STOCK_SHORT", None), done(), done()], vec![rel1.clone(), gone.clone()], shop.fails[0]),
            (vec![stock("A", "STOCK_SECURED", Some("l1")), stock("B", "STOCK_CONFLICT", None), done(), done()], vec![rel1.clone(), gone.clone()], shop.fails[1]),
            // Both: the conflict is the one to look into, and nothing was held to give back.
            (vec![stock("A", "STOCK_CONFLICT", None), stock("B", "STOCK_SHORT", None), done()], vec![gone.clone()], shop.fails[1]),
            (with(held(), vec![json!({"error": "boom"}), done(), done(), done()]), vec![rel1.clone(), rel2.clone(), gone.clone()], shop.fails[2]),
            (with(held(), vec![ok((shop.delivery)()), json!({"error": "timeout"}), done(), done(), done()]), vec![rel1.clone(), rel2.clone(), gone.clone()], shop.fails[3]),
            (with(held(), vec![ok((shop.delivery)()), json!({"error": shop.short}), done(), done(), done()]), vec![rel1.clone(), rel2.clone(), gone.clone()], shop.fails[4]),
        ];
        for (answers, want, failure) in failures {
            let (calls, end) = shop_ends(shop, answers);
            assert_eq!(calls, want, "{} {failure}", shop.dir);
            assert_eq!(end["fail"]["error"], failure, "{} {end}", shop.dir);
        }
    }
}
