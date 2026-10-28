//! Waiting for an answer that settles later (DESIGN §18). A `when` with `within <n> seconds` is
//! made again until every check under it holds on one answer, or its time runs out. The specs are
//! under `tests/wait/`; their service, `tests/impl/later.py`, answers differently after a number of
//! calls or a while, and `tests/impl/tick.py` prints differently from its third run. A wait that
//! holds writes what any `when` writes, the same bytes however many calls it took; one whose time
//! runs out fails its claim on the last answer, says how many calls it made and how long they took,
//! and the claim goes on. The calls and the time differ from run to run, so the goldens hold them
//! through `steady`.

mod common;
use common::*;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A scratch directory holding a spec from `tests/wait/` and the programs it runs.
fn with_later(name: &str, spec: &str) -> Scratch {
    let s = Scratch::new(name);
    for f in ["later.py", "tick.py"] {
        s.write(&format!("w/{f}"), &repo_file(&format!("tests/impl/{f}")));
    }
    s.write(&format!("w/{spec}"), &repo_file(&format!("tests/wait/{spec}")));
    s
}

/// The `when` events of a journal.
fn whens(journal: &str) -> Vec<Json> {
    journal.lines().map(json).filter(|e| e.get("event").str() == "when").collect()
}

/// Whether a JSON object has the member.
fn has(v: &Json, key: &str) -> bool {
    matches!(v, Json::Obj(pairs) if pairs.iter().any(|(k, _)| k == key))
}

#[test]
fn a_wait_holds_on_the_first_answer_every_check_holds_on() {
    if !python3("the waiting service") {
        return;
    }
    let s = with_later("wait-holds", "holds.geas");
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let _ = std::fs::remove_file(s.path().join("w/tick.count"));
        let mut args = vec!["check", "w/holds.geas"];
        args.extend(extra);
        let (out, err, code) = run(s.path(), &args, &[("GEAS_PID_LOG", log_s)]);
        assert_eq!((err.as_str(), code), ("", 0), "{out}");
        golden(&format!("{lang}/wait/holds.txt"), &out);
    }
    // the command ran three times, the third printing "done"
    assert_eq!(s.read("w/tick.count"), "3");
    let journal = s.read("w/.geas/holds.journal.jsonl");
    let w = whens(&journal);
    let body = |i: usize| json(w[i].get("obs").get("body").str());
    // the third answer, the first on which the check holds; then the fourth, on which both do
    assert_eq!(body(0).get("calls").num(), 3.0);
    assert_eq!(body(1).get("calls").num(), 4.0);
    assert_eq!(body(3).get("state").str(), "done");
    assert_eq!(w[5].get("obs").get("stdout").str(), "done\n");
    // a wait that held writes its `within` and nothing of the calls or the time
    for i in [0, 1, 3, 5] {
        assert_eq!(w[i].get("within"), &Json::Num(5.0), "{journal}");
    }
    assert!(!journal.contains("\"tries\"") && !journal.contains("\"ms\""), "{journal}");
    assert!(!has(&w[2], "within") && !has(&w[4], "within"), "a `when` without `within` writes none: {journal}");
    golden("en/wait/holds.journal.jsonl", &journal);

    // snap keeps the answers the waits held on, and drift on the same service is quiet
    let (out, err, code) = run(s.path(), &["snap", "w/holds.geas"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    golden("en/wait/holds.baseline.jsonl", &s.read("w/.geas/holds.baseline.jsonl"));
    let (out, err, code) = run(s.path(), &["drift", "w/holds.geas"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    assert!(out.contains(" 0 drifted "), "{out}");
    no_process_left(&log);
}

#[test]
fn a_wait_that_runs_out_fails_on_the_last_answer_and_the_claim_goes_on() {
    if !python3("the waiting service") {
        return;
    }
    let s = with_later("wait-runs-out", "runs-out.geas");
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["check", "w/runs-out.geas"];
        args.extend(extra);
        let begun = Instant::now();
        let (out, err, code) = run(s.path(), &args, &[("GEAS_PID_LOG", log_s)]);
        assert_eq!((err.as_str(), code), ("", 1), "{out}");
        assert!(begun.elapsed() >= Duration::from_secs(1), "the wait did not take its time");
        golden(&format!("{lang}/wait/runs-out.txt"), &steady(&out));
    }
    let (out, _, code) = run(s.path(), &["check", "w/runs-out.geas", "--json"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!(code, 1);
    let v = json(&out);
    let claim = &v.get("claims").arr()[0];
    let checks: Vec<bool> = claim.get("checks").arr().iter().map(|c| c.get("ok") == &Json::Bool(true)).collect();
    // the state is not done on the last answer; the status is 200 on it; the next step ran
    assert_eq!(checks, [false, true, true]);
    let step = &claim.get("run").arr()[0];
    assert_eq!(step.get("within").num(), 1.0);
    let (tries, ms) = (step.get("tries").num(), step.get("ms").num());
    assert!(tries >= 2.0 && (1000.0..6000.0).contains(&ms), "{out}");
    golden("en/wait/runs-out.json", &steady(&out));
    let journal = s.read("w/.geas/runs-out.journal.jsonl");
    let w = whens(&journal);
    assert_eq!(w.len(), 2, "one event for the wait, one for the step after it: {journal}");
    assert!(w[0].get("tries").num() >= 2.0 && w[0].get("ms").num() >= 1000.0, "{journal}");
    golden("en/wait/runs-out.journal.jsonl", &steady(&journal));
    no_process_left(&log);
}

#[test]
fn an_error_while_waiting_ends_the_claim_at_once() {
    if !python3("the waiting service") {
        return;
    }
    let s = with_later("wait-gone", "gone.geas");
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    for (lang, extra, said) in [("en", vec![], "= call 3 of the wait, "), ("ja", vec!["--lang", "ja"], "= 待ちの 3 回目の呼び出しで、")] {
        let mut args = vec!["check", "w/gone.geas"];
        args.extend(extra);
        let begun = Instant::now();
        let (out, err, code) = run(s.path(), &args, &[("GEAS_PID_LOG", log_s)]);
        assert_eq!((err.as_str(), code), ("", 1), "{out}");
        assert!(begun.elapsed() < Duration::from_secs(5), "geas waited out the time of a claim that had ended");
        assert!(out.contains("[E033]") && out.contains(said), "{out}");
        golden(&format!("{lang}/wait/gone.txt"), &steady(&out));
    }
    no_process_left(&log);
}

#[test]
fn drift_compares_the_answer_the_wait_kept() {
    if !python3("the waiting service") {
        return;
    }
    let s = with_later("wait-drift", "settles.geas");
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let (out, err, code) = run(s.path(), &["snap", "w/settles.geas"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    assert!(s.read("w/.geas/settles.baseline.jsonl").contains(r#"\"calls\": 3"#), "the baseline keeps the third answer");
    let (out, err, code) = run(s.path(), &["drift", "w/settles.geas"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    // the service stays "running": the time runs out, and drift says what it compared
    s.write("w/never", "");
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["drift", "w/settles.geas"];
        args.extend(extra);
        let (out, err, code) = run(s.path(), &args, &[("GEAS_PID_LOG", log_s)]);
        assert_eq!((err.as_str(), code), ("", 1), "{out}");
        golden(&format!("{lang}/wait/drift.txt"), &steady(&out));
    }
    no_process_left(&log);
}

/// Claims that wait, run side by side, write what they write one at a time: a wait that held
/// writes nothing of how many calls it took.
#[test]
fn waits_side_by_side_write_what_they_write_one_at_a_time() {
    if !python3("the waiting service") {
        return;
    }
    let s = with_later("wait-parallel", "holds.geas");
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let mut runs = Vec::new();
    for jobs in ["1", "4"] {
        let _ = std::fs::remove_file(s.path().join("w/tick.count"));
        let (out, err, code) = run(s.path(), &["snap", "w/holds.geas", "-j", jobs], &[("GEAS_PID_LOG", log_s)]);
        assert_eq!((err.as_str(), code), ("", 0), "{out}");
        runs.push((out, s.read("w/.geas/holds.journal.jsonl"), s.read("w/.geas/holds.baseline.jsonl")));
    }
    assert!(runs[0] == runs[1], "-j1 and -j4 differ:\n{:?}\n{:?}", runs[0], runs[1]);
    no_process_left(&log);
}

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

/// A signal ends a wait at once: geas does not go on calling for the rest of its minute.
#[test]
fn a_signal_ends_a_wait() {
    if !python3("the waiting service") {
        return;
    }
    let s = with_later("wait-signal", "long.geas");
    let log = pid_log(&s);
    let mut child = Command::new(geas())
        .args(["check", "w/long.geas"])
        .current_dir(s.path())
        .env("GEAS_PID_LOG", &log)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start geas");
    // wait until the service is up, and the wait is calling it
    let deadline = Instant::now() + Duration::from_secs(20);
    while !std::fs::read_to_string(&log).unwrap_or_default().contains(" api\n") {
        assert!(Instant::now() < deadline, "geas never started `api`");
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(500));
    let sent = Instant::now();
    // SAFETY: SIGINT to the geas this test started.
    unsafe {
        kill(child.id() as i32, 2);
    }
    let status = child.wait().expect("wait for geas");
    assert_eq!(status.code(), Some(130), "{status:?}");
    assert!(sent.elapsed() < Duration::from_secs(3), "geas took {:?} to end", sent.elapsed());
    let started: Vec<i32> = std::fs::read_to_string(&log)
        .expect("the pid log")
        .lines()
        .filter(|l| l.starts_with("start "))
        .map(|l| l.split(' ').nth(1).expect("a pid").parse().expect("a pid"))
        .collect();
    assert_eq!(started.len(), 1, "the service");
    let deadline = Instant::now() + Duration::from_secs(5);
    // SAFETY: kill(pid, 0) only asks whether the process exists.
    let alive = |p: i32| unsafe { kill(p, 0) } == 0;
    while started.iter().any(|p| alive(*p)) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!started.iter().any(|p| alive(*p)), "the service is still alive after geas was interrupted");
}
