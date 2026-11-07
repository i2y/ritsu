//! The five calls, the headers a request sends, and what a claim keeps from one step for the next
//! (DESIGN §22). The specs are under `tests/keep/`. Their services are `tests/impl/mirror.py`, which
//! answers with what it was sent, and `tests/impl/things.py`, which numbers what it makes with a new
//! uuid on every run. A report shows the values it was given, so what differs from run to run goes
//! through `uuid_steady` before a golden; the journal and the baseline write the values as `{name}`,
//! and are the same bytes as they are.

mod common;
use common::*;
use std::time::{Duration, Instant};

/// A scratch directory with the services and every file under `tests/keep/`.
fn scratch(name: &str) -> Scratch {
    let s = Scratch::new(name);
    for f in ["mirror.py", "things.py", "echo.py"] {
        s.write(&format!("k/{f}"), &repo_file(&format!("tests/impl/{f}")));
    }
    for e in std::fs::read_dir(root().join("tests/keep")).expect("tests/keep") {
        let f = e.expect("an entry").file_name().into_string().expect("a UTF-8 name");
        s.write(&format!("k/{f}"), &repo_file(&format!("tests/keep/{f}")));
    }
    s
}

/// The value `keep env` reads in these tests, which nothing may show.
const KEY: &str = "s3cretValue42";
const WITH_KEY: (&str, &str) = ("GEAS_TEST_KEY", KEY);

/// `check` in both languages, holding what it prints to `<lang>/keep/<stem>.txt` as `uuid_steady`
/// writes it, and the exit status to `exit`.
fn check_both(s: &Scratch, stem: &str, exit: i32, env: &[(&str, &str)]) {
    let log = pid_log(s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let spec = format!("k/{stem}.geas");
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["check", spec.as_str()];
        args.extend(extra);
        let mut vars = vec![("GEAS_PID_LOG", log_s)];
        vars.extend_from_slice(env);
        let (out, err, code) = run(s.path(), &args, &vars);
        assert_eq!((err.as_str(), code), ("", exit), "{stem} ({lang}):\n{out}");
        golden(&format!("{lang}/keep/{stem}.txt"), &steady(&out));
    }
    no_process_left(&log);
}

/// The journal of a spec.
fn journal(s: &Scratch, stem: &str) -> String {
    s.read(&format!("k/.geas/{stem}.journal.jsonl"))
}

/// Events of a journal of one kind.
fn events(journal: &str, event: &str) -> Vec<Json> {
    journal.lines().map(json).filter(|e| e.get("event").str() == event).collect()
}

#[test]
fn the_five_calls_reach_the_service_with_their_headers() {
    if !python3("the service") {
        return;
    }
    let s = scratch("keep-calls");
    check_both(&s, "calls", 0, &[]);
    let j = journal(&s, "calls");
    golden("en/keep/calls.journal.jsonl", &j);
    // the call as the journal writes it, the headers after the rest, in the order they are written
    let whens = events(&j, "when");
    let call = |i: usize| whens[i].get("call").str().to_string();
    assert_eq!(call(0), r#"get("/a?b=c", header "X-One": "1", header "x-two": "two words", header "X-One": "3")"#);
    assert_eq!(call(1), r#"post("/a", body: "made")"#);
    assert_eq!(call(2), r#"put("/a", body: "whole", header "content-type": "text/plain")"#);
    assert_eq!(call(3), r#"patch("/a", body: "part", header "x-part": "yes")"#);
    assert_eq!(call(4), r#"delete("/a")"#);
    assert_eq!(call(5), r#"delete("/a", body: "why")"#);
}

#[test]
fn a_number_the_service_chooses_is_used_and_written_the_same_in_every_run() {
    if !python3("the service") {
        return;
    }
    let s = scratch("keep-numbers");
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    check_both(&s, "numbers", 0, &[]);
    let first = journal(&s, "numbers");
    golden("en/keep/numbers.journal.jsonl", &first);
    // nothing of what the service numbered is left
    assert_eq!(uuid_steady(&first), first, "{first}");
    assert!(first.contains(r#"\"id\":\"{id}\""#) && first.contains(r#""location":"/things/{id}""#), "{first}");
    assert!(first.contains(r#"put(\"/things/{id}\", body: \"a table\", header \"x-about\": \"{id}\")"#), "the call as it is written: {first}");
    // what was kept is a check of its `when`
    let keep = events(&first, "check").into_iter().find(|c| c.get("check").str().starts_with("keep")).expect("the keep");
    assert_eq!(keep.get("check").str(), r#"keep body json ".id""#);
    assert_eq!(keep.get("expected").str(), "as id");
    assert_eq!(keep.get("actual").str(), "{id}");
    assert_eq!(keep.get("ok"), &Json::Bool(true));
    // one at a time or side by side: the same bytes, new numbers each time
    let mut runs = Vec::new();
    for jobs in ["1", "4", "1", "4"] {
        let (out, err, code) = run(s.path(), &["snap", "k/numbers.geas", "-j", jobs], &[("GEAS_PID_LOG", log_s)]);
        assert_eq!((err.as_str(), code), ("", 0), "{out}");
        runs.push((out, journal(&s, "numbers"), s.read("k/.geas/numbers.baseline.jsonl")));
    }
    assert!(runs.windows(2).all(|w| w[0] == w[1]), "the runs differ: {runs:?}");
    assert_eq!(runs[0].1, first);
    golden("en/keep/numbers.baseline.jsonl", &runs[0].2);
    // drift on new numbers says nothing has changed
    let (out, err, code) = run(s.path(), &["drift", "k/numbers.geas"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    assert!(out.contains(" 0 drifted "), "{out}");
    // a change the claims do not read is drift all the same
    let things = s.read("k/things.py");
    s.write("k/things.py", &things.replace(r#""qty": 1}"#, r#""qty": 2}"#));
    let (out, err, code) = run(s.path(), &["drift", "k/numbers.geas"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!((err.as_str(), code), ("", 1), "{out}");
    assert!(out.contains(r#"body json ".qty": 1 → 2"#), "{out}");
    assert_eq!(uuid_steady(&out), out, "drift writes the numbers as {{id}}:\n{out}");
    no_process_left(&log);
}

#[test]
fn a_small_number_is_compared_as_text_and_written_where_it_stands_alone() {
    if !python3("the service") {
        return;
    }
    let s = scratch("keep-small");
    check_both(&s, "small", 0, &[]);
    let j = journal(&s, "small");
    golden("en/keep/small.journal.jsonl", &j);
    let whens = events(&j, "when");
    let body = whens[0].get("obs").get("body").str();
    // as a number and as a string it stays JSON; `v1` keeps its 1; `qty` happens to be the same value
    assert_eq!(body, r#"{"id":"{id}","qty":"{id}","note":"thing {id} of v1"}"#);
    assert_eq!(whens[0].get("obs").get("headers").get("location").str(), "/small/{id}");
}

#[test]
fn what_a_keep_cannot_read_fails_its_claim_and_the_rest_does_not_run() {
    if !python3("the service") {
        return;
    }
    let s = scratch("keep-unread");
    check_both(&s, "unread", 1, &[]);
    let j = journal(&s, "unread");
    golden("en/keep/unread.journal.jsonl", &uuid_steady(&j));
    // the first claim made its one call, and read nothing after the keep that failed
    let of_claim: Vec<Json> = j.lines().map(json).filter(|e| e.get("claim").str() == "a path that is not there").collect();
    assert_eq!(of_claim.len(), 3, "{j}");
    assert_eq!(of_claim[2].get("check").str(), r#"keep body json ".number""#);
    assert_eq!(of_claim[2].get("expected").str(), "as id");
    assert_eq!(of_claim[2].get("ok"), &Json::Bool(false));
    let (out, _, code) = run(s.path(), &["check", "k/unread.geas", "--json"], &[]);
    assert_eq!(code, 1);
    golden("en/keep/unread.json", &uuid_steady(&out));
    let v = json(&out);
    let claims = v.get("claims").arr();
    assert!(claims.iter().all(|c| c.get("status").str() == "fail"), "a failure, not an error: {out}");
    let first = &claims[0];
    assert_eq!(first.get("checks").arr().len(), 2, "{out}");
    assert_eq!(first.get("run").arr().len(), 1, "{out}");
    // what the last claim kept is in its run, in order, and what it could not is not
    let last = claims.last().expect("a claim");
    let run: Vec<&str> = last.get("run").arr().iter().map(|r| r.get("call").str()).collect();
    assert_eq!(run, ["api.get(\"/gaps\")", "keep body as whole"], "{out}");
}

#[test]
fn a_failed_check_tells_the_values_it_goes_by() {
    if !python3("the service") {
        return;
    }
    let s = scratch("keep-told");
    check_both(&s, "told", 1, &[]);
    let (out, _, code) = run(s.path(), &["check", "k/told.geas", "--json"], &[]);
    assert_eq!(code, 1);
    golden("en/keep/told.json", &out);
    let v = json(&out);
    let claim = &v.get("claims").arr()[0];
    let failed: Vec<&Json> = claim.get("checks").arr().iter().filter(|c| c.get("ok") == &Json::Bool(false)).collect();
    assert_eq!(failed.len(), 4, "{out}");
    assert_eq!(failed[0].get("values").get("qty").str(), "1");
    assert_eq!(failed[0].get("values").get("id").str(), "1");
    let passed = claim.get("checks").arr().iter().find(|c| c.get("ok") == &Json::Bool(true)).expect("a check that holds");
    assert!(!matches!(passed, Json::Obj(p) if p.iter().any(|(k, _)| k == "values")), "{out}");
    // the journal writes the names
    let j = journal(&s, "told");
    assert!(j.contains(r#""expected":"\"{qty}{id}\"","actual":"\"{id}\"","ok":false"#), "{j}");
}

#[test]
fn a_keep_under_a_wait_waits_for_its_value() {
    if !python3("the service") {
        return;
    }
    let s = scratch("keep-wait");
    check_both(&s, "wait", 0, &[]);
    let j = journal(&s, "wait");
    golden("en/keep/wait.journal.jsonl", &j);
    let whens = events(&j, "when");
    let body = json(whens[0].get("obs").get("body").str());
    assert_eq!(body.get("calls").num(), 3.0, "it stopped on the answer that has the value: {j}");
    assert_eq!(body.get("result").str(), "{result}", "{j}");
    assert_eq!(whens[0].get("within"), &Json::Num(5.0));
    // the check under it reads the value kept from the same answer
    let body = json(whens[2].get("obs").get("body").str());
    assert_eq!(body.get("calls").num(), 2.0, "{j}");
    // the same bytes again, the value being a new one
    check_both(&s, "wait", 0, &[]);
    assert_eq!(journal(&s, "wait"), j);
}

#[test]
fn a_keep_under_a_wait_that_runs_out_fails_and_stops() {
    if !python3("the service") {
        return;
    }
    let s = scratch("keep-wait-out");
    check_both(&s, "wait-runs-out", 1, &[]);
    let j = journal(&s, "wait-runs-out");
    assert_eq!(events(&j, "when").len(), 1, "{j}");
}

#[test]
fn a_value_from_the_environment_is_told_to_nobody() {
    if !python3("the service") {
        return;
    }
    let s = scratch("keep-env");
    check_both(&s, "env", 1, &[WITH_KEY]);
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let env = [WITH_KEY, ("GEAS_PID_LOG", log_s)];
    // it went out: the service saw it in the header, in the body and in the path
    let saved = s.read("k/saved.jsonl");
    assert!(saved.contains(&format!(r#"["authorization", "Bearer {KEY}"]"#)), "{saved}");
    assert!(saved.contains(&format!(r#""body": "{KEY}""#)), "{saved}");
    assert!(saved.contains(&format!(r#""path": "/saved/{KEY}/b""#)), "{saved}");
    // and geas shows it nowhere, though the service sent it back
    let journal_now = journal(&s, "env");
    assert!(!journal_now.contains(KEY), "{journal_now}");
    golden("en/keep/env.journal.jsonl", &journal_now);
    let (out, err, code) = run(s.path(), &["check", "k/env.geas"], &env);
    assert_eq!(code, 1);
    assert!(!out.contains(KEY) && !err.contains(KEY), "{out}{err}");
    let (out, err, code) = run(s.path(), &["check", "k/env.geas", "--json"], &env);
    assert_eq!(code, 1);
    assert!(!out.contains(KEY) && !err.contains(KEY), "{out}{err}");
    golden("en/keep/env.json", &out);
    // one claim of it fails on purpose, and snap says so in its status
    let (out, err, code) = run(s.path(), &["snap", "k/env.geas"], &env);
    assert_eq!(code, 1, "{out}{err}");
    assert!(!out.contains(KEY) && !err.contains(KEY), "{out}{err}");
    let baseline = s.read("k/.geas/env.baseline.jsonl");
    assert!(!baseline.contains(KEY), "{baseline}");
    let (out, err, code) = run(s.path(), &["drift", "k/env.geas"], &env);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!out.contains(KEY) && !err.contains(KEY), "{out}{err}");
    // another value of the variable is the same record
    let (out, err, code) = run(s.path(), &["drift", "k/env.geas"], &[("GEAS_TEST_KEY", "anotherKey77"), ("GEAS_PID_LOG", log_s)]);
    assert_eq!(code, 0, "{out}{err}");
    no_process_left(&log);
}

/// What a program does with the value, the words of its command and what it prints when it fails.
#[test]
fn a_value_from_the_environment_stays_out_of_what_a_failure_says() {
    if !python3("the service") {
        return;
    }
    let s = scratch("keep-env-failing");
    s.write(
        "k/gone.geas",
        "target missing {\n  run \"geas-test-no-such-program\"\n}\n\ntarget slow {\n  serve \"python3 gone.py {port}\"\n  port auto\n}\n\nclaim \"a program that is not there\" {\n  keep env \"GEAS_TEST_KEY\" as key\n  when missing.run(\"--key\", \"{key}\")\n  then exit is 0\n}\n\nclaim \"a service that dies\" {\n  keep env \"GEAS_TEST_KEY\" as key\n  when slow.get(\"/{key}\", header \"x-key\": \"{key}\")\n  then status is 200\n}\n",
    );
    // it writes what it is sent on its stderr, and ends without an answer
    s.write(
        "k/gone.py",
        "import os, socket, sys\nserver = socket.socket()\nserver.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)\nserver.bind((\"127.0.0.1\", int(sys.argv[1])))\nserver.listen()\nwhile True:\n    conn, _ = server.accept()\n    print(conn.recv(4096).decode(), file=sys.stderr, flush=True)\n    conn.close()\n",
    );
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    for args in [&["check", "k/gone.geas"][..], &["check", "k/gone.geas", "--lang", "ja"][..], &["check", "k/gone.geas", "--json"][..]] {
        let (out, err, code) = run(s.path(), args, &[WITH_KEY, ("GEAS_PID_LOG", log_s)]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("E030") && out.contains("E033"), "{out}{err}");
        assert!(!out.contains(KEY) && !err.contains(KEY), "{args:?}:\n{out}{err}");
        // the service's stderr is there, without the value
        assert!(out.contains("x-key: {key}"), "{out}");
    }
    let j = journal(&s, "gone");
    assert!(!j.contains(KEY), "{j}");
    no_process_left(&log);
}

#[test]
fn a_variable_that_is_not_set_is_an_error_and_the_other_claims_go_on() {
    if !python3("the service") {
        return;
    }
    let s = scratch("keep-no-env");
    let begun = Instant::now();
    check_both(&s, "no-env", 1, &[]);
    assert!(begun.elapsed() < Duration::from_secs(25), "geas waited for what no answer can give: {:?}", begun.elapsed());
    let j = journal(&s, "no-env");
    golden("en/keep/no-env.journal.jsonl", &j);
    assert_eq!(events(&j, "error").len(), 3, "{j}");
    assert!(events(&j, "error").iter().all(|e| e.get("code").str() == "E038"), "{j}");
    let (out, _, code) = run(s.path(), &["check", "k/no-env.geas", "--json"], &[]);
    assert_eq!(code, 1);
    let v = json(&out);
    let statuses: Vec<&str> = v.get("claims").arr().iter().map(|c| c.get("status").str()).collect();
    assert_eq!(statuses, ["error", "error", "error", "ok"], "{out}");
    assert_eq!(v.get("claims").arr()[0].get("error").get("code").str(), "E038");
    // an empty variable is one that gives nothing to send
    let (out, _, code) = run(s.path(), &["check", "k/no-env.geas"], &[("GEAS_TEST_NOT_SET", "")]);
    assert_eq!(code, 1);
    assert!(out.contains("it is empty") && !out.contains("it is not set"), "{out}");
}

#[test]
fn a_kept_value_goes_out_in_the_form_each_place_takes() {
    if !python3("the service") {
        return;
    }
    let s = scratch("keep-sent");
    check_both(&s, "sent", 0, &[]);
    let j = journal(&s, "sent");
    golden("en/keep/sent.journal.jsonl", &j);
    assert_eq!(uuid_steady(&j), j, "{j}");
    // a value with `{port}` in it went out with the port of the service it went to
    let saved = s.read("k/saved.jsonl");
    assert!(saved.contains("http://127.0.0.1:") && !saved.contains("{port}"), "{saved}");
}

#[test]
fn what_a_kept_value_cannot_do_is_an_error_when_it_runs() {
    if !python3("the service") {
        return;
    }
    let s = scratch("keep-wrong");
    check_both(&s, "wrong", 1, &[]);
    let (out, _, code) = run(s.path(), &["check", "k/wrong.geas", "--json"], &[]);
    assert_eq!(code, 1);
    let v = json(&out);
    let codes: Vec<&str> = v.get("claims").arr().iter().filter(|c| c.get("status").str() == "error").map(|c| c.get("error").get("code").str()).collect();
    assert_eq!(codes, ["E038", "E038", "E038", "E038"], "{out}");
    assert_eq!(v.get("claims").arr()[4].get("status").str(), "ok", "{out}");
}

#[test]
fn every_answer_of_the_five_calls_is_held_to_the_contract() {
    if !python3("the service") {
        return;
    }
    let s = scratch("keep-held");
    check_both(&s, "held", 0, &[]);
    let j = journal(&s, "held");
    golden("en/keep/held.journal.jsonl", &j);
    let held: Vec<Json> = events(&j, "check").into_iter().filter(|c| c.get("check").str() == "contract").collect();
    let expected: Vec<&str> = held.iter().map(|c| c.get("expected").str()).collect();
    assert_eq!(expected, ["POST /things", "GET /things/{id}", "PUT /things/{id}", "PATCH /things/{id}", "DELETE /things/{id}", "GET /things/{id}", "POST /absolute", "GET /things/{id}"], "{j}");
    assert!(held.iter().all(|c| c.get("ok") == &Json::Bool(true)), "{j}");
}

#[test]
fn a_call_the_contract_does_not_have_is_said_before_anything_runs() {
    let s = scratch("keep-outside");
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["check", "k/outside.geas"];
        args.extend(extra);
        let (out, err, code) = run(s.path(), &args, &[]);
        assert_eq!((out.as_str(), code), ("", 2), "{err}");
        golden(&format!("{lang}/keep/outside.txt"), &err);
        assert_eq!(err.matches("[E016]").count(), 4, "the paths that start with a value are not looked at: {err}");
    }
    assert!(!s.exists("k/.geas"), "nothing ran");
}

#[test]
fn a_path_only_the_run_knows_is_held_where_it_comes_to() {
    if !python3("the service") {
        return;
    }
    let s = scratch("keep-at-run");
    check_both(&s, "at-run", 1, &[]);
    let j = journal(&s, "at-run");
    golden("en/keep/at-run.journal.jsonl", &j);
    assert_eq!(uuid_steady(&j), j, "{j}");
    let failed: Vec<Json> = events(&j, "check").into_iter().filter(|c| c.get("check").str() == "contract" && c.get("ok") == &Json::Bool(false)).collect();
    assert_eq!(failed.len(), 2, "{j}");
    assert_eq!(failed[0].get("expected").str(), "GET {at}/parts");
    assert_eq!(failed[0].get("actual").str(), "the contract things.yaml has no GET {at}/parts");
    assert_eq!(failed[1].get("actual").str(), "the contract things.yaml has no GET /things/{at}");
}

#[test]
fn a_name_that_looks_kept_and_is_not_is_said_by_check_alone() {
    if !python3("the service") {
        return;
    }
    let s = scratch("keep-typo");
    check_both(&s, "typo", 0, &[]);
    let (out, _, code) = run(s.path(), &["check", "k/typo.geas", "--json"], &[]);
    assert_eq!(code, 0);
    let v = json(out.trim_end());
    let d = v.get("diagnostics").arr();
    assert_eq!(d.len(), 4, "{out}");
    assert!(d.iter().all(|x| x.get("code").str() == "W016"), "{out}");
    assert_eq!(d[0].get("line").num(), 14.0);
    assert_eq!(d[0].get("col").num(), 25.0, "at the name inside the string: {out}");
    // a run of another command says nothing of it, and the string stays as it is written
    let (out, err, code) = run(s.path(), &["snap", "k/typo.geas"], &[]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!out.contains("W016") && !err.contains("W016"), "{out}{err}");
    let saved = s.read("k/saved.jsonl");
    assert!(saved.contains(r#""path": "/saved/{idd}""#), "{saved}");
}

/// A claim that keeps nothing means what it always meant: its braces are its own.
#[test]
fn a_spec_without_keep_is_what_it_was() {
    if !python3("the service") {
        return;
    }
    let s = scratch("keep-none");
    s.write("k/none.geas", "target api {\n  serve \"python3 mirror.py {port}\"\n  port auto\n}\n\nclaim \"braces\" {\n  when api.post(\"/a b/{id}\", body: \"{id}\")\n  then body json \".body\" is \"{id}\"\n  and  body json \".path\" is \"/a%20b/{id}\"\n}\n");
    let (out, err, code) = run(s.path(), &["check", "k/none.geas"], &[]);
    // the path went as it is written: a blank in a request line is what the service then reads
    assert!(!out.contains("W016") && !err.contains("W016"), "{out}{err}");
    let _ = code;
    let j = journal(&s, "none");
    assert!(j.contains(r#"post(\"/a b/{id}\", body: \"{id}\")"#), "{j}");
}

/// A baseline written before there were headers: a call is written as it always was, so drift reads
/// it and has nothing to say.
#[test]
fn a_baseline_written_before_is_read_as_it_was() {
    if !python3("the waiting service") {
        return;
    }
    let s = Scratch::new("keep-old-baseline");
    for f in ["later.py", "tick.py"] {
        s.write(&format!("w/{f}"), &repo_file(&format!("tests/impl/{f}")));
    }
    s.write("w/holds.geas", &repo_file("tests/wait/holds.geas"));
    let old = repo_file("tests/golden/en/wait/holds.baseline.jsonl");
    assert!(old.contains(r#""call":"get(\"/after/3\")""#) && old.contains(r#""call":"post(\"/jobs\")""#), "{old}");
    s.write("w/.geas/holds.baseline.jsonl", &old);
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let (out, err, code) = run(s.path(), &["drift", "w/holds.geas"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    assert!(out.contains(" 0 drifted "), "{out}");
    no_process_left(&log);
}

/// The output of a command, kept for the next; how a `run` target goes.
#[test]
fn a_command_is_given_what_the_last_one_printed() {
    if !python3("the service") {
        return;
    }
    let s = scratch("keep-run");
    check_both(&s, "run", 0, &[]);
    let j = journal(&s, "run");
    golden("en/keep/run.journal.jsonl", &j);
    let whens = events(&j, "when");
    // one line break is dropped from the value, as a comparison drops it: the program printed
    // two, and the value holds the first
    assert_eq!(whens[0].get("obs").get("stdout").str(), "{out}\n", "{j}");
    assert_eq!(whens[0].get("obs").get("stderr").str(), "{err}\n", "{j}");
    assert!(events(&j, "check").iter().all(|c| c.get("ok") == &Json::Bool(true)), "{j}");
}

/// W901: a key written in a request's header is said as any key in the file is, without the key.
#[test]
fn a_key_written_in_a_header_is_said() {
    if !python3("the service") {
        return;
    }
    // the materials' one fake key, put together here so that this file does not hold it whole
    let key = ["AIzaSyD-ritsu-fake-", "key-for-tests-000000"].concat();
    let s = scratch("keep-key");
    s.write("k/key.geas", format!("target api {{\n  serve \"python3 mirror.py {{port}}\"\n  port auto\n}}\n\nclaim \"sends the key\" {{\n  when api.get(\"/\", header \"authorization\": \"Bearer {key}\")\n  then body json \".headers[0][1]\" is \"not it\"\n}}\n"));
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["check", "k/key.geas"];
        args.extend(extra);
        let (out, err, code) = run(s.path(), &args, &[]);
        assert_eq!(code, 1, "{out}{err}");
        assert_eq!(out.matches("[W901]").count(), 1, "{out}");
        assert!(!out.contains(&key) && !err.contains(&key), "{out}{err}");
        golden(&format!("{lang}/keep/key.txt"), &out);
    }
    // and what to do about it, which `geas explain` says
    let (out, _, code) = run(s.path(), &["explain", "W901"], &[]);
    assert_eq!(code, 0);
    assert!(out.contains("keep env \"NAME\" as key"), "{out}");
}
