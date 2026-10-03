//! Chrome, the second driver (DESIGN §8.4), on the test page (`tests/pages/app/`):
//! the first screen; typing and submitting; a status 300 ms after a click and a
//! reminder a minute after one, with no waiting in real time; a pinned clock, time zone
//! and locale; one seed giving one number in two claims; storage that each claim
//! starts without; `get` and `post` in the same claim as the page; drift; E034,
//! E035 and E036; `-j2` printing what `-j1` prints. After each test no process
//! names the scratch directory and no profile is left. Run with Chrome and python3,
//! SKIP otherwise.

mod common;
use common::*;
use std::time::Instant;

/// A scratch directory with the page, its server and a spec from `tests/browser/`.
fn with_page(name: &str, spec: &str) -> Scratch {
    let s = Scratch::new(name);
    for f in ["index.html", "server.py"] {
        s.write(&format!("w/{f}"), &repo_file(&format!("tests/pages/app/{f}")));
    }
    s.write(&format!("w/{spec}"), &repo_file(&format!("tests/browser/{spec}")));
    s
}

/// Whether the browser tests can run.
fn ready(what: &str) -> bool {
    python3(what) && chrome(what).is_some()
}

/// The variables geas runs with: the pid log, and `GEAS_CHROME` when the tests
/// were given one.
fn run_logged(s: &Scratch, args: &[&str], extra: &[(&str, &str)]) -> (String, String, i32) {
    let log = pid_log(s);
    let log_s = log.to_string_lossy().into_owned();
    let chrome = std::env::var("GEAS_CHROME").ok();
    let mut env: Vec<(&str, &str)> = vec![("GEAS_PID_LOG", log_s.as_str())];
    if let Some(c) = &chrome {
        env.push(("GEAS_CHROME", c.as_str()));
    }
    env.extend(extra);
    let r = run(s.path(), args, &env);
    no_process_left(&log);
    no_chrome_left(s);
    r
}

#[test]
fn the_page_in_chrome() {
    if !ready("the browser tests") {
        return;
    }
    let s = with_page("browser", "page.geas");
    let started = Instant::now();
    let (out, err, code) = run_logged(&s, &["check", "w/page.geas"], &[]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    // the reminder came a minute of the page's time after the click, and no real minute passed
    assert!(started.elapsed().as_secs() < 30, "the claims took {:?}", started.elapsed());
    golden("en/browser/page.txt", &out);
    // the first screen, as the journal keeps it
    let journal = s.read("w/.geas/page.journal.jsonl");
    golden("en/browser/page.journal.jsonl", &journal);
    let (out, err, code) = run_logged(&s, &["check", "w/page.geas", "--lang", "ja"], &[]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    golden("ja/browser/page.txt", &out);
    // two workers, two Chromes, the same bytes
    let (two, err, code) = run_logged(&s, &["check", "w/page.geas", "-j2"], &[]);
    assert_eq!((err.as_str(), code), ("", 0), "{two}");
    let (one, _, _) = run_logged(&s, &["check", "w/page.geas"], &[]);
    assert_eq!(two, one);
    assert_eq!(s.read("w/.geas/page.journal.jsonl"), journal);
}

/// A snap, then drift: quiet on the unchanged page; when the page gains a line,
/// that line is an unclaimed change.
#[test]
fn drift_on_a_page() {
    if !ready("the browser tests") {
        return;
    }
    let s = with_page("browser-drift", "page.geas");
    let (out, err, code) = run_logged(&s, &["snap", "w/page.geas"], &[]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    let (out, err, code) = run_logged(&s, &["drift", "w/page.geas"], &[]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    let page = s.read("w/index.html");
    s.write("w/index.html", &page.replace("<h1>Shopping list</h1>", "<h1>Shopping list</h1>\n<p>Delivery on Fridays</p>"));
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["drift", "w/page.geas"];
        args.extend(extra);
        let (out, err, code) = run_logged(&s, &args, &[]);
        assert_eq!((err.as_str(), code), ("", 1), "{out}");
        assert!(out.contains("text \"Delivery on Fridays\"   [unclaimed]") || lang == "ja", "{out}");
        golden(&format!("{lang}/browser/drift.txt"), &out);
    }
}

/// E034, E035 and E036, in both languages.
#[test]
fn chrome_missing_an_action_refused_and_a_page_that_never_loads() {
    if !ready("the browser tests") {
        return;
    }
    let s = with_page("browser-errors", "refused.geas");
    s.write("w/slow.geas", &repo_file("tests/browser/slow.geas"));
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["check", "w/refused.geas"];
        args.extend(extra.iter().copied());
        let (out, err, code) = run_logged(&s, &args, &[]);
        assert_eq!((err.as_str(), code), ("", 1), "{out}");
        assert_eq!(out.matches("[E035]").count(), 2, "{out}");
        assert_eq!(out.matches("[E036]").count(), 1, "{out}");
        golden(&format!("{lang}/browser/refused.txt"), &out);
        // with GEAS_CHROME naming a file that is not there, each claim says so
        let (out, err, code) = run_logged(&s, &args, &[("GEAS_CHROME", "no-such-chrome")]);
        assert_eq!((err.as_str(), code), ("", 1), "{out}");
        assert_eq!(out.matches("[E034]").count(), 3, "{out}");
        golden(&format!("{lang}/browser/no-chrome.txt"), &out);
    }
    let started = Instant::now();
    let (out, err, code) = run_logged(&s, &["check", "w/slow.geas"], &[]);
    assert_eq!((err.as_str(), code), ("", 1), "{out}");
    assert!(started.elapsed().as_secs() >= 10);
    golden("en/browser/slow.txt", &out);
}

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

/// geas interrupted while a page loads: Chrome, its helpers and the service are
/// killed, the profile is removed, and geas exits with 128 and the signal.
#[test]
fn interrupted_while_a_page_loads() {
    use std::process::{Command, Stdio};
    use std::time::Duration;
    if !ready("the browser tests") {
        return;
    }
    let s = with_page("browser-interrupt", "slow.geas");
    let log = pid_log(&s);
    let mut cmd = Command::new(geas());
    cmd.args(["check", "w/slow.geas"])
        .current_dir(s.path())
        .env("GEAS_PID_LOG", &log)
        .env_remove("GEAS_LANG")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Ok(c) = std::env::var("GEAS_CHROME") {
        cmd.env("GEAS_CHROME", c);
    }
    let mut child = cmd.spawn().expect("start geas");
    // wait until Chrome runs and the page has begun to load
    let deadline = Instant::now() + Duration::from_secs(20);
    while !std::fs::read_to_string(&log).unwrap_or_default().contains(" chrome\n") {
        assert!(Instant::now() < deadline, "geas never started Chrome");
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(500));
    // SAFETY: SIGINT to the geas this test started.
    unsafe {
        kill(child.id() as i32, 2);
    }
    let status = child.wait().expect("wait for geas");
    assert_eq!(status.code(), Some(130), "{status:?}");
    let started: Vec<i32> = std::fs::read_to_string(&log)
        .expect("the pid log")
        .lines()
        .filter(|l| l.starts_with("start "))
        .map(|l| l.split(' ').nth(1).expect("a pid").parse().expect("a pid"))
        .collect();
    assert_eq!(started.len(), 2, "the service and Chrome");
    let deadline = Instant::now() + Duration::from_secs(5);
    // SAFETY: signal 0 delivers nothing; it asks whether the process exists.
    let alive = |pid: i32| unsafe { kill(pid, 0) } == 0;
    while started.iter().any(|p| alive(*p)) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    for pid in &started {
        assert!(!alive(*pid), "{pid} is still alive after geas was interrupted");
    }
    no_chrome_left(&s);
}

/// `map` runs claims on a page as `check` does, and records the service behind it.
#[test]
fn map_records_the_service_behind_a_page() {
    if !ready("the browser tests") {
        return;
    }
    let s = with_page("browser-map", "page.geas");
    let (out, err, code) = run_logged(&s, &["map", "w/page.geas", "--root", "w"], &[]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    let record = s.read("w/.geas/page.map.jsonl");
    assert!(record.contains("\"target\":\"web\",\"file\":\"server.py\""), "{record}");
}
