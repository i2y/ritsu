//! Running claims side by side (DESIGN §10): the greeter prints and writes the same
//! bytes with one job and with four; a slow service shows which claims overlap
//! (with `port auto` some do, with `serial` or a fixed port none do); a service's
//! own address is written `{port}`, so drift is quiet across runs on different
//! ports; `GEAS_JOBS`; and process groups: a server behind a shell is stopped with
//! it, recorded by `map`, and killed when geas itself is interrupted.

mod common;
use common::*;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const GREETER: &str = "examples/greeter/greeter.geas";

/// One run's report, journal and baseline, for `-j1` against `-j4`.
fn snap(s: &Scratch, jobs: &str) -> (String, String, String) {
    let (out, err, code) = run(s.path(), &["snap", GREETER, "--jobs", jobs], &[]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    (
        out,
        s.read("examples/greeter/.geas/greeter.journal.jsonl"),
        s.read("examples/greeter/.geas/greeter.baseline.jsonl"),
    )
}

#[test]
fn the_greeter_writes_the_same_bytes_with_one_job_and_with_four() {
    if !python3("examples/greeter") {
        return;
    }
    let s = Scratch::new("parallel-greeter");
    copy_example("greeter", &s);
    let one = snap(&s, "1");
    let four = snap(&s, "4");
    assert_eq!(four, one);
    let check = |extra: &[&str]| {
        let mut args = vec!["check", GREETER];
        args.extend(extra);
        let r = run(s.path(), &args, &[]);
        (r, s.read("examples/greeter/.geas/greeter.journal.jsonl"))
    };
    assert_eq!(check(&["-j4"]), check(&[]));
    assert_eq!(check(&["-j4", "--json"]), check(&["--json"]));
    // and drift with four jobs is as quiet as with one
    let (out, err, code) = run(s.path(), &["drift", GREETER, "-j", "4"], &[]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
}

/// The intervals the slow service spent answering, one per request, by pid.
fn intervals(log: &str) -> Vec<(String, f64, f64)> {
    let mut open: Vec<(String, f64)> = Vec::new();
    let mut out = Vec::new();
    for line in log.lines() {
        let parts: Vec<&str> = line.split(' ').collect();
        let (what, pid, at) = (parts[0], parts[1].to_string(), parts[2].parse::<f64>().expect("a time"));
        match what {
            "start" => open.push((pid, at)),
            _ => {
                let i = open.iter().position(|(p, _)| *p == pid).expect("a start before the end");
                let (pid, from) = open.remove(i);
                out.push((pid, from, at));
            }
        }
    }
    out
}

/// Whether two of the intervals overlap.
fn some_overlap(log: &str) -> bool {
    let iv = intervals(log);
    iv.iter().enumerate().any(|(i, a)| iv.iter().skip(i + 1).any(|b| a.1 < b.2 && b.1 < a.2))
}

/// A spec of six claims on the slow service, its target written `target`.
fn slow_spec(target: &str) -> String {
    let claims: String = (1..=6)
        .map(|i| format!("claim \"answers, {i}\" {{\n  when slow.get(\"/\")\n  then body is \"done\"\n}}\n\n"))
        .collect();
    format!("{target}\n{claims}")
}

/// Runs the slow spec in a fresh directory with these arguments and environment;
/// whether two claims overlapped.
fn slow_run(name: &str, target: &str, args: &[&str], env: &[(&str, &str)]) -> bool {
    let s = Scratch::new(name);
    s.write("slow/slow.py", &repo_file("tests/impl/slow.py"));
    s.write("slow/slow.geas", &slow_spec(target));
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let mut all_env = env.to_vec();
    all_env.push(("GEAS_PID_LOG", log_s));
    let mut all_args = vec!["check", "slow/slow.geas"];
    all_args.extend(args);
    let (out, err, code) = run(s.path(), &all_args, &all_env);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    assert_eq!(no_process_left(&log), 6);
    let overlap = some_overlap(&s.read("slow/slow.log"));
    assert_eq!(intervals(&s.read("slow/slow.log")).len(), 6, "one request a claim");
    overlap
}

const AUTO: &str = "target slow {\n  serve \"python3 slow.py {port}\"\n  port auto\n}\n";

#[test]
fn claims_on_port_auto_run_side_by_side() {
    if !python3("the slow service") {
        return;
    }
    assert!(slow_run("parallel-auto", AUTO, &["-j3"], &[]), "with -j3, no two claims ran at the same time");
    assert!(!slow_run("parallel-one", AUTO, &[], &[]), "with one job, two claims ran at the same time");
}

#[test]
fn a_serial_target_runs_one_claim_at_a_time() {
    if !python3("the slow service") {
        return;
    }
    let serial = "target slow {\n  serve \"python3 slow.py {port}\"\n  port auto\n  serial\n}\n";
    assert!(!slow_run("parallel-serial", serial, &["-j3"], &[]), "two claims on a serial target ran at the same time");
}

#[test]
fn a_fixed_port_runs_one_claim_at_a_time() {
    if !python3("the slow service") {
        return;
    }
    let _port = port_lock();
    let fixed = "target slow {\n  serve \"python3 slow.py {port}\"\n  port 8123\n}\n";
    assert!(!slow_run("parallel-fixed", fixed, &["-j3"], &[]), "two claims on one fixed port ran at the same time");
}

#[test]
fn geas_jobs_sets_the_jobs_when_the_flag_does_not() {
    if !python3("the slow service") {
        return;
    }
    assert!(slow_run("parallel-env", AUTO, &[], &[("GEAS_JOBS", "3")]), "GEAS_JOBS=3 ran one claim at a time");
    // a value that is not a whole number from 1 counts as unset, as GEAS_LANG's does
    assert!(!slow_run("parallel-env-bad", AUTO, &[], &[("GEAS_JOBS", "three")]));
    // the flag wins
    assert!(!slow_run("parallel-env-flag", AUTO, &["--jobs", "1"], &[("GEAS_JOBS", "3")]));
}

const REDIRECT: &str = "# A service that sends the client to its own address.\n\ntarget web {\n  serve \"python3 redirect.py {port}\"\n  port auto\n}\n\nclaim \"an old path sends the client to the new one\" {\n  when web.get(\"/old\")\n  then status is 302\n  and  header \"location\" is \"http://127.0.0.1:{port}/new\"\n  and  body is \"moved to localhost:{port}/new\"\n  when web.get(\"/new\")\n  then body is \"here\"\n}\n";

/// The port a service answers with is written `{port}` before checks, the journal
/// and drift see it: the claim holds, and drift is quiet across two runs on two
/// ports.
#[test]
fn a_services_own_address_is_written_with_port() {
    if !python3("the redirecting service") {
        return;
    }
    let s = Scratch::new("parallel-redirect");
    s.write("web/redirect.py", &repo_file("tests/impl/redirect.py"));
    s.write("web/web.geas", REDIRECT);
    let (out, err, code) = run(s.path(), &["snap", "web/web.geas"], &[]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    golden("en/parallel/redirect.journal.jsonl", &s.read("web/.geas/web.journal.jsonl"));
    let (out, err, code) = run(s.path(), &["drift", "web/web.geas"], &[]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    assert_eq!(out, "drift: 2 interactions compared · 0 drifted · 0 unclaimed change(s) · 0 claimed\n");
    let ports: Vec<String> = s.read("web/ports.log").lines().map(str::to_string).collect();
    assert_eq!(ports.len(), 2);
    assert_ne!(ports[0], ports[1], "the two runs were given one port");
}

const WRAPPED: &str = "# The server runs behind a shell, which waits for it.\n\ntarget slow {\n  serve \"sh -c 'python3 slow.py {port}; true'\"\n  port auto\n}\n\nclaim \"answers through a shell\" {\n  when slow.get(\"/\")\n  then body is \"done\"\n}\n";

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

/// Whether a process is alive (a zombie counts).
fn alive(pid: i32) -> bool {
    // SAFETY: signal 0 delivers nothing; it asks whether the process exists.
    unsafe { kill(pid, 0) == 0 }
}

/// The pids the slow service wrote in its log.
fn slow_pids(log: &str) -> Vec<i32> {
    let mut pids: Vec<i32> = log.lines().map(|l| l.split(' ').nth(1).expect("a pid").parse().expect("a pid")).collect();
    pids.dedup();
    pids
}

/// A server a shell starts is in the shell's process group, so it is stopped with
/// it: by `check` at once, and by `map` with SIGTERM, which reaches the server and
/// lets it write what it ran.
#[test]
fn a_server_behind_a_shell_is_stopped_with_it() {
    if !python3("the slow service") {
        return;
    }
    let s = Scratch::new("parallel-wrapped");
    s.write("slow/slow.py", &repo_file("tests/impl/slow.py"));
    s.write("slow/slow.geas", WRAPPED);
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let (out, err, code) = run(s.path(), &["check", "slow/slow.geas"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    no_process_left(&log);
    let server = slow_pids(&s.read("slow/slow.log"));
    assert_eq!(server.len(), 1);
    assert!(!alive(server[0]), "the server behind the shell ({}) is still alive", server[0]);
    let (out, err, code) = run(s.path(), &["map", "slow/slow.geas", "--root", "slow"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    let record = s.read("slow/.geas/slow.map.jsonl");
    assert!(record.contains("\"target\":\"slow\",\"file\":\"slow.py\""), "{record}");
    no_process_left(&log);
    for pid in slow_pids(&s.read("slow/slow.log")) {
        assert!(!alive(pid), "the server behind the shell ({pid}) is still alive");
    }
}

/// Interrupted while a claim runs, geas kills the groups of the programs it
/// started, a server behind a shell included, and exits as SIGINT asks (130).
#[test]
fn interrupted_geas_leaves_no_process_behind() {
    if !python3("the slow service") {
        return;
    }
    let s = Scratch::new("parallel-interrupt");
    s.write("slow/slow.py", &repo_file("tests/impl/slow.py"));
    s.write(
        "slow/slow.geas",
        "target slow {\n  serve \"sh -c 'python3 slow.py {port}; true'\"\n  port auto\n}\n\ntarget nap {\n  run \"sleep 30\"\n}\n\nclaim \"waits\" {\n  when slow.get(\"/\")\n  when nap.run()\n  then exit is 0\n}\n",
    );
    let log = pid_log(&s);
    let mut child = Command::new(geas())
        .args(["check", "slow/slow.geas"])
        .current_dir(s.path())
        .env("GEAS_PID_LOG", &log)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start geas");
    // wait until the claim sleeps
    let deadline = Instant::now() + Duration::from_secs(20);
    while !std::fs::read_to_string(&log).unwrap_or_default().contains(" nap\n") {
        assert!(Instant::now() < deadline, "geas never started `nap`");
        std::thread::sleep(Duration::from_millis(20));
    }
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
    assert_eq!(started.len(), 2, "the shell and `sleep`");
    let server = slow_pids(&s.read("slow/slow.log"));
    // the processes are killed at once; give the system a moment to reap them
    let deadline = Instant::now() + Duration::from_secs(5);
    while started.iter().chain(&server).any(|p| alive(*p)) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    for pid in started.iter().chain(&server) {
        assert!(!alive(*pid), "{pid} is still alive after geas was interrupted");
    }
}
