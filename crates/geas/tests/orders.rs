//! Calls in any order (DESIGN §23). The steps of an `in any order { … }` block run in each of
//! their orders, one run of the claim to an order, each on services started afresh; the claim holds
//! when it holds in every order. The specs are under `tests/orders/`, and their service,
//! `tests/impl/events.py`, keeps the state of a subscription from the events posted to it: with
//! `newest` it drops an event older than the last it applied, with `as-it-comes` it writes each as
//! it comes, with `hang-up` it closes the connection on an older one. The orders come in a fixed
//! order, the written one first and then in the dictionary order of where the steps are written, so
//! the report, the journal and the baseline are the same bytes in every run and with any `--jobs`.

mod common;
use common::*;

/// A scratch directory holding a spec from `tests/orders/` and the service, under `o/`.
fn with_events(name: &str, spec: &str) -> Scratch {
    let s = Scratch::new(name);
    s.write("o/events.py", &repo_file("tests/impl/events.py"));
    s.write(&format!("o/{spec}"), &repo_file(&format!("tests/orders/{spec}")));
    s
}

/// Whether a JSON object has the member.
fn has(v: &Json, key: &str) -> bool {
    matches!(v, Json::Obj(pairs) if pairs.iter().any(|(k, _)| k == key))
}

/// The lines of the `when`s of a claim in its journal, order by order; a `when` that got no
/// answer is written as an error on its line.
fn orders_in(journal: &str, claim: &str) -> Vec<Vec<usize>> {
    let mut orders: Vec<Vec<usize>> = Vec::new();
    for e in journal.lines().map(json).filter(|e| e.get("claim").str() == claim && ["when", "error"].contains(&e.get("event").str())) {
        let n = e.get("order").num() as usize;
        if orders.len() < n {
            assert_eq!(orders.len() + 1, n, "the orders come one after another: {journal}");
            orders.push(Vec::new());
        }
        orders[n - 1].push(e.get("line").num() as usize);
    }
    orders
}

/// The numbers of the failed orders of a claim in `--json`, with the lines of their steps.
fn failed_orders(claim: &Json) -> Vec<(usize, Vec<usize>)> {
    claim
        .get("failed_orders")
        .arr()
        .iter()
        .map(|f| (f.get("order").num() as usize, f.get("lines").arr().iter().map(|l| l.num() as usize).collect()))
        .collect()
}

/// Runs `args` with a pid log, and holds geas to stopping everything it started; how many it did.
fn run_counted(s: &Scratch, args: &[&str]) -> (String, String, i32, usize) {
    let log = pid_log(s);
    let _ = std::fs::remove_file(&log);
    let (out, err, code) = run(s.path(), args, &[("GEAS_PID_LOG", log.to_str().expect("a UTF-8 path"))]);
    let n = no_process_left(&log);
    (out, err, code, n)
}

#[test]
fn every_order_holds_with_the_handler_that_drops_older_events() {
    if !python3("the events service") {
        return;
    }
    let s = with_events("orders-holds", "holds.geas");
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["check", "o/holds.geas"];
        args.extend(extra);
        let (out, err, code, started) = run_counted(&s, &args);
        assert_eq!((err.as_str(), code), ("", 0), "{out}");
        golden(&format!("{lang}/orders/holds.txt"), &out);
        // a new server for each order: 6, 12 and 2 of them
        assert_eq!(started, 6 + 12 + 2, "{lang}");
    }
    assert_eq!(s.read("o/starts.log").lines().count(), 2 * 20);
    let journal = s.read("o/.geas/holds.journal.jsonl");

    // the written order first, then the dictionary order of where the steps are written; the
    // `when` after the block in every order, and each order on a server that has had 3 events
    let latest = orders_in(&journal, "the latest event wins in any order");
    assert_eq!(
        latest,
        [[11, 13, 15, 18], [11, 15, 13, 18], [13, 11, 15, 18], [13, 15, 11, 18], [15, 11, 13, 18], [15, 13, 11, 18]],
        "{journal}"
    );
    // two steps written the same are one step in an order: 4!/2! = 12 orders, none twice
    let twice = orders_in(&journal, "an event delivered twice changes nothing");
    assert_eq!(twice.len(), 12);
    assert_eq!(twice[0], [25, 26, 27, 28, 30]);
    let bodies: Vec<Vec<usize>> = twice.iter().map(|o| o.iter().map(|l| if *l == 27 { 26 } else { *l }).collect()).collect();
    let mut distinct = bodies.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), 12, "an order counted twice: {bodies:?}");
    // a name kept before the block is used in it, and one kept in it after it
    assert_eq!(orders_in(&journal, "names kept before the block, in it and after it"), [[36, 39, 40, 43], [36, 40, 39, 43]]);

    // the same bytes in a second run and with four jobs
    let (out, err, code, _) = run_counted(&s, &["check", "o/holds.geas", "--jobs", "4"]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    golden("en/orders/holds.txt", &out);
    assert_eq!(s.read("o/.geas/holds.journal.jsonl"), journal, "-j4 writes what -j1 does");
    let (_, _, code, _) = run_counted(&s, &["check", "o/holds.geas"]);
    assert_eq!(code, 0);
    assert_eq!(s.read("o/.geas/holds.journal.jsonl"), journal, "a second run writes the same bytes");

    // `--json` adds the number of orders to a claim that holds
    let (out, _, code, _) = run_counted(&s, &["check", "o/holds.geas", "--json"]);
    assert_eq!(code, 0);
    let v = json(&out);
    let counts: Vec<usize> = v.get("claims").arr().iter().map(|c| c.get("orders").num() as usize).collect();
    assert_eq!(counts, [6, 12, 2]);
    assert!(v.get("claims").arr().iter().all(|c| !has(c, "failed_orders")), "{out}");

    // snap keeps every order, and drift on the same service compares them order by order
    let (out, err, code, _) = run_counted(&s, &["snap", "o/holds.geas", "--jobs", "4"]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    let baseline = s.read("o/.geas/holds.baseline.jsonl");
    let recs: Vec<Json> = baseline.lines().skip(1).map(json).collect();
    assert_eq!(recs.len(), 6 * 4 + 12 * 5 + 2 * 4, "{baseline}");
    assert!(recs.iter().all(|r| has(r, "order")), "every record of a claim with a block has its order");
    let (out, err, code, _) = run_counted(&s, &["snap", "o/holds.geas"]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    assert_eq!(s.read("o/.geas/holds.baseline.jsonl"), baseline, "-j1 snaps what -j4 does");
    let (out, err, code, _) = run_counted(&s, &["drift", "o/holds.geas"]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    assert!(out.contains("92 interactions compared · 0 drifted"), "{out}");
}

#[test]
fn the_handler_that_writes_events_as_they_come_fails_in_the_orders_counted() {
    if !python3("the events service") {
        return;
    }
    let s = with_events("orders-as-it-comes", "as-it-comes.geas");
    let mut shown = Vec::new();
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["check", "o/as-it-comes.geas"];
        args.extend(extra.clone());
        let (out, err, code, started) = run_counted(&s, &args);
        assert_eq!((err.as_str(), code), ("", 1), "{out}");
        golden(&format!("{lang}/orders/as-it-comes.txt"), &out);
        // every order ran, the failing ones too
        assert_eq!(started, 6 + 12 + 2, "{lang}");
        args.push("--json");
        let (out, err, code, _) = run_counted(&s, &args);
        assert_eq!((err.as_str(), code), ("", 1));
        shown.push(out);
    }
    // nothing of a failed order is in words, so `--json` is the same in both languages
    golden("en/orders/as-it-comes.json", &shown[0]);
    assert_eq!(shown[1], shown[0]);
    let journal = s.read("o/.geas/as-it-comes.journal.jsonl");
    assert_eq!(orders_in(&journal, "the latest event wins in any order").len(), 6);
    assert_eq!(orders_in(&journal, "an event delivered twice changes nothing").len(), 12);

    let v = json(&shown[0]);
    let claims = v.get("claims").arr();
    // the orders that end on an older event fail; the first of them in the fixed order is shown
    assert_eq!(claims[0].get("orders").num(), 6.0);
    assert_eq!(failed_orders(&claims[0]), [(2, vec![11, 15, 13]), (4, vec![13, 15, 11]), (5, vec![15, 11, 13]), (6, vec![15, 13, 11])]);
    let run: Vec<usize> = claims[0].get("run").arr().iter().map(|r| r.get("line").num() as usize).collect();
    assert_eq!(run, [11, 15, 13, 18], "the run of the first failed order");
    let failed: Vec<&Json> = claims[0].get("checks").arr().iter().filter(|c| c.get("ok") == &Json::Bool(false)).collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].get("actual").str(), "\"active\"");
    // the 3 orders of 12 that end with the latest event hold
    assert_eq!(claims[1].get("orders").num(), 12.0);
    let numbers: Vec<usize> = failed_orders(&claims[1]).into_iter().map(|(n, _)| n).collect();
    assert_eq!(numbers, [2, 3, 5, 7, 8, 9, 10, 11, 12]);
    assert_eq!(failed_orders(&claims[2]), [(2, vec![36, 35])]);

    // four jobs print and write what one does
    let (out, err, code, _) = run_counted(&s, &["check", "o/as-it-comes.geas", "--jobs", "4"]);
    assert_eq!((err.as_str(), code), ("", 1));
    golden("en/orders/as-it-comes.txt", &out);
    assert_eq!(s.read("o/.geas/as-it-comes.journal.jsonl"), journal);
}

#[test]
fn an_order_that_ends_in_an_error_ends_the_claim() {
    if !python3("the events service") {
        return;
    }
    let s = with_events("orders-hang-up", "hang-up.geas");
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["check", "o/hang-up.geas"];
        args.extend(extra);
        let (out, err, code, started) = run_counted(&s, &args);
        assert_eq!((err.as_str(), code), ("", 1), "{out}");
        golden(&format!("{lang}/orders/hang-up.txt"), &out);
        // the first two orders, then the claim after it
        assert_eq!(started, 2 + 1, "{lang}");
    }
    let journal = s.read("o/.geas/hang-up.journal.jsonl");
    assert_eq!(orders_in(&journal, "the latest event wins in any order"), [vec![11, 13, 15, 18], vec![11, 15, 13]]);
    let (out, _, code, _) = run_counted(&s, &["check", "o/hang-up.geas", "--json"]);
    assert_eq!(code, 1);
    let v = json(&out);
    let claims = v.get("claims").arr();
    assert_eq!(claims[0].get("status").str(), "error");
    assert_eq!(claims[0].get("error").get("code").str(), "E033");
    assert_eq!(claims[0].get("orders").num(), 6.0);
    assert_eq!(claims[1].get("status").str(), "ok");
}

/// Each case of E018 is refused before anything runs; the diagnostics are held by tests/spec.rs.
#[test]
fn a_block_geas_cannot_run_starts_nothing() {
    let names = [
        "E018-two-blocks.geas",
        "E018-a-block-in-a-block.geas",
        "E018-no-when.geas",
        "E018-one-when.geas",
        "E018-more-than-720-orders.geas",
        "E018-keep-from-the-same-block.geas",
        "E018-check-after-the-block.geas",
        "E018-keep-after-the-block.geas",
    ];
    let s = Scratch::new("orders-e018");
    s.write("events.py", &repo_file("tests/impl/events.py"));
    for name in names {
        s.write(name, &repo_file(&format!("tests/specs/{name}")));
        let (out, err, code, started) = run_counted(&s, &["check", name]);
        assert_eq!((out.as_str(), code), ("", 2), "{name}: {err}");
        assert!(err.contains("[E018]"), "{name}: {err}");
        assert_eq!(started, 0, "{name}");
    }
    assert!(!s.exists(".geas"));
}

/// The lines `map` records for a claim are those of all its orders: the drop of an older event runs
/// only in the orders where one comes after a newer one, never in the order written.
#[test]
fn map_records_the_lines_of_every_order() {
    if !python3("the events service") {
        return;
    }
    let s = with_events("orders-map", "map.geas");
    let (out, err, code, started) = run_counted(&s, &["map", "o/map.geas", "--root", "."]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    assert_eq!(started, 6 + 1);
    let source = s.read("o/events.py");
    let drop = 1 + source.lines().position(|l| l.contains("# an older event, or the same one again")).expect("the line that drops an event");
    let record = s.read("o/.geas/map.map.jsonl");
    let ran = |claim: &str| -> Vec<usize> {
        let r = record.lines().map(json).find(|r| has(r, "claim") && r.get("claim").str() == claim).expect("the claim's record");
        r.get("ran")
            .str()
            .split(',')
            .flat_map(|part| match part.split_once('-') {
                Some((a, b)) => (a.parse::<usize>().expect("a line")..=b.parse().expect("a line")).collect::<Vec<_>>(),
                None => vec![part.parse().expect("a line")],
            })
            .collect()
    };
    assert!(ran("in any order").contains(&drop), "{record}");
    assert!(!ran("in the order they happened").contains(&drop), "{record}");
}

/// A GUI target in a block: each order starts the driver afresh, and runs as one run of the claim.
#[test]
fn a_gui_target_runs_in_each_order() {
    if !python3("the fake driver") {
        return;
    }
    let s = Scratch::new("orders-driver");
    s.write("g/fake_driver.py", &repo_file("tests/drivers/fake_driver.py"));
    s.write("g/greeter.json", &repo_file("tests/drivers/greeter.json"));
    s.write("g/echo.py", &repo_file("tests/impl/echo.py"));
    s.write("g/driver.geas", &repo_file("tests/orders/driver.geas"));
    let (out, err, code, started) = run_counted(&s, &["check", "g/driver.geas"]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    assert!(out.contains("ok 1 - a name typed in and a command, in any order (2 orders)"), "{out}");
    // a driver and a command in each order
    assert_eq!(started, 2 * 2);
    let journal = s.read("g/.geas/driver.journal.jsonl");
    assert_eq!(orders_in(&journal, "a name typed in and a command, in any order"), [[13, 15, 18], [15, 13, 18]]);
}

/// A page in Chrome in a block: each order is a run of its own, in a browser context of its own, so
/// the counter the page keeps in its storage starts again, and the list the service keeps does too.
#[test]
fn a_page_in_chrome_starts_afresh_in_each_order() {
    if !python3("the test page") || chrome("the test page").is_none() {
        return;
    }
    let s = Scratch::new("orders-page");
    for f in ["index.html", "server.py"] {
        s.write(&format!("w/{f}"), &repo_file(&format!("tests/pages/app/{f}")));
    }
    s.write("w/page.geas", &repo_file("tests/orders/page.geas"));
    let log = pid_log(&s);
    let log_s = log.to_string_lossy().into_owned();
    let chrome = chrome_path().map(|c| c.to_string_lossy().into_owned()).expect("a Chrome");
    let tmp = ChromeTmp::new();
    let mut env: Vec<(&str, &str)> = vec![("GEAS_PID_LOG", log_s.as_str()), ("GEAS_CHROME", chrome.as_str())];
    env.extend(tmp.vars());
    let (out, err, code) = run(s.path(), &["check", "w/page.geas"], &env);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    assert!(out.contains("ok 1 - an item and a visit, in any order (2 orders)"), "{out}");
    no_process_left(&log);
    no_chrome_left(&s);
}

/// pixie's greeter in a block: each order replays a script of its own, that order's actions, on an
/// app started afresh. Run when `GEAS_PIXIE_GREETER` names a built greeter, as tests/pixie.rs is.
#[test]
fn a_pixie_app_replays_each_order() {
    if !ritsu_testkit::need(ritsu_testkit::Need::Pixie) {
        return;
    }
    let Some(app) = std::env::var_os("GEAS_PIXIE_GREETER").map(std::path::PathBuf::from).filter(|p| p.is_file()) else {
        skip("GEAS_PIXIE_GREETER does not name a built pixie greeter; pixie in a block is not run");
        return;
    };
    let s = Scratch::new("orders-pixie");
    s.write("p/pixie.geas", &repo_file("tests/orders/pixie.geas"));
    std::os::unix::fs::symlink(&app, s.path().join("p/greeter")).expect("link the greeter");
    let (out, err, code, started) = run_counted(&s, &["check", "p/pixie.geas"]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    assert!(out.contains("ok 1 - a name and a note, typed in any order (2 orders)"), "{out}");
    // one replay of the app in each order
    assert!(started >= 2, "{started}");
    let journal = s.read("p/.geas/pixie.journal.jsonl");
    assert_eq!(orders_in(&journal, "a name and a note, typed in any order"), [[9, 11, 14], [11, 9, 14]]);
}
