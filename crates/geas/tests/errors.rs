//! The codes a run, a baseline or the command line gives (E030-E033, E050, E051,
//! E080, E081), each in English and in Japanese. Every test that starts a process
//! holds geas to stopping it (`GEAS_PID_LOG`).

mod common;
use common::*;

/// Runs `args` in the scratch in both languages, holds the stream that has the
/// output to `<lang>/errors/<name>.txt`, the other stream empty, and the exit status
/// to `exit`.
fn both(s: &Scratch, name: &str, args: &[&str], exit: i32, on_stdout: bool) {
    let log = pid_log(s);
    let log_s = log.to_str().expect("a UTF-8 path");
    for lang in ["en", "ja"] {
        let mut a: Vec<&str> = args.to_vec();
        if lang == "ja" {
            a.extend(["--lang", "ja"]);
        }
        let (out, err, code) = run(s.path(), &a, &[("GEAS_PID_LOG", log_s)]);
        assert_eq!(code, exit, "{name} ({lang}): stdout {out:?}, stderr {err:?}");
        let (shown, other) = if on_stdout { (out, err) } else { (err, out) };
        assert_eq!(other, "", "{name} ({lang})");
        golden(&format!("{lang}/errors/{name}.txt"), &shown);
    }
    no_process_left(&log);
}

const ANSWERS: &str = "claim \"answers\" {\n  when api.get(\"/\")\n  then status is 200\n}\n";

#[test]
fn e030_a_program_that_does_not_exist() {
    let s = Scratch::new("e030");
    s.write(
        "e030.geas",
        "target calc {\n  run \"geas-test-no-such-program --flag\"\n}\n\nclaim \"adds\" {\n  when calc.run(\"2\", \"+\", \"3\")\n  then stdout is \"5\"\n}\n",
    );
    both(&s, "E030-no-such-program", &["check", "e030.geas"], 1, true);
}

/// Runs `args` in English and in Japanese at the same time, for the codes that cost
/// 5 s of waiting each, and holds each to its golden.
fn both_at_once(s: &Scratch, name: &str, args: &[&str]) -> usize {
    let log = pid_log(s);
    let log_s = log.to_str().expect("a UTF-8 path").to_string();
    std::thread::scope(|scope| {
        let runs: Vec<_> = ["en", "ja"]
            .into_iter()
            .map(|lang| {
                let log_s = log_s.clone();
                scope.spawn(move || {
                    let mut a: Vec<&str> = args.to_vec();
                    a.extend(["--lang", lang]);
                    (lang, run(s.path(), &a, &[("GEAS_PID_LOG", &log_s)]))
                })
            })
            .collect();
        for r in runs {
            let (lang, (out, err, code)) = r.join().expect("a run");
            assert_eq!((err.as_str(), code), ("", 1), "{name} ({lang})");
            golden(&format!("{lang}/errors/{name}.txt"), &out);
        }
    });
    no_process_left(&log)
}

/// Costs 5 s: geas waits that long before it stops the command.
#[test]
fn e031_a_run_that_does_not_finish() {
    let s = Scratch::new("e031");
    s.write(
        "e031.geas",
        "target slow {\n  run \"sleep 10\"\n}\n\nclaim \"finishes\" {\n  when slow.run()\n  then exit is 0\n}\n",
    );
    assert_eq!(both_at_once(&s, "E031-sleep", &["check", "e031.geas"]), 2, "one `sleep` per language");
}

#[test]
fn e032_a_service_that_exits_at_once() {
    if !python3("a service written in Python") {
        return;
    }
    let _port = port_lock();
    let s = Scratch::new("e032-exits");
    s.write(
        "e032.geas",
        &format!("target api {{\n  serve \"python3 exits.py 8123\"\n  port 8123\n}}\n\n{ANSWERS}"),
    );
    s.write(
        "exits.py",
        "import sys\nprint(\"starting on\", sys.argv[1], file=sys.stderr)\nprint(\"OSError: [Errno 48] Address already in use\", file=sys.stderr)\nsys.exit(1)\n",
    );
    both(&s, "E032-exits", &["check", "e032.geas"], 1, true);
}

/// Costs 5 s: geas waits that long for the port. Neither run's service opens it, so
/// the two can wait side by side.
#[test]
fn e032_a_service_that_never_opens_its_port() {
    let _port = port_lock();
    let s = Scratch::new("e032-never");
    s.write("e032.geas", &format!("target api {{\n  serve \"sleep 30\"\n  port 8123\n}}\n\n{ANSWERS}"));
    assert_eq!(both_at_once(&s, "E032-never-opens", &["check", "e032.geas"]), 2, "one service per language, stopped");
}

#[test]
fn e033_a_service_that_hangs_up() {
    if !python3("a service written in Python") {
        return;
    }
    let _port = port_lock();
    let s = Scratch::new("e033");
    s.write(
        "e033.geas",
        &format!("target api {{\n  serve \"python3 hangup.py 8123\"\n  port 8123\n}}\n\n{ANSWERS}"),
    );
    s.write(
        "hangup.py",
        "import socket, sys\nserver = socket.create_server((\"127.0.0.1\", int(sys.argv[1])))\nwhile True:\n    connection, _ = server.accept()\n    print(\"closing a connection\", file=sys.stderr, flush=True)\n    connection.close()\n",
    );
    both(&s, "E033-hangs-up", &["check", "e033.geas"], 1, true);
}

#[test]
fn e050_no_baseline() {
    let s = Scratch::new("e050");
    s.write("dir/e050.geas", "target calc {\n  run \"python3 calc.py\"\n}\n\nclaim \"adds\" {\n  when calc.run(\"2\")\n  then exit is 0\n}\n");
    both(&s, "E050-no-baseline", &["drift", "dir/e050.geas"], 2, false);
    // a baseline under the name the spike gave it is pointed at
    s.write("dir/.geas/baseline.jsonl", "");
    both(&s, "E050-old-name", &["drift", "dir/e050.geas"], 2, false);
}

#[test]
fn e051_a_baseline_that_cannot_be_read() {
    let s = Scratch::new("e051");
    s.write("e051.geas", "target calc {\n  run \"python3 calc.py\"\n}\n\nclaim \"adds\" {\n  when calc.run(\"2\")\n  then exit is 0\n}\n");
    s.write(
        ".geas/e051.baseline.jsonl",
        "{\"claim\":\"adds\",\"idx\":0,\"target\":\"calc\",\"call\":\"run(\\\"2\\\")\",\"obs\":{\"kind\":\"proc\",\"stdout\":\"\",\"stderr\":\"\",\"exit\":0}}\n{\"claim\":\"adds\",\"idx\":1,\"target\":\"calc\"}\n",
    );
    both(&s, "E051-no-obs", &["drift", "e051.geas"], 2, false);
    s.write(".geas/e051.baseline.jsonl", "not json\n");
    both(&s, "E051-not-json", &["drift", "e051.geas"], 2, false);
}

#[test]
fn e080_arguments_the_command_does_not_take() {
    let s = Scratch::new("e080");
    for (name, args) in [
        ("no-command", vec![]),
        ("unknown-command", vec!["chekc", "x.geas"]),
        ("unknown-option", vec!["check", "--nope", "x.geas"]),
        ("option-of-another-command", vec!["check", "--all", "x.geas"]),
        ("no-spec", vec!["snap"]),
        ("explain-without-a-code", vec!["explain"]),
        ("explain-both", vec!["explain", "E001", "--all"]),
        ("explain-unknown-code", vec!["explain", "E999"]),
        ("jobs-zero", vec!["check", "x.geas", "--jobs", "0"]),
    ] {
        both(&s, &format!("E080-{name}"), &args, 2, false);
    }
    // a bad --lang is said in English, since the language is what is wrong
    for (name, args) in [("lang-value", vec!["check", "x.geas", "--lang", "fr"]), ("lang-without-value", vec!["check", "x.geas", "--lang"])] {
        let (out, err, code) = run(s.path(), &args, &[]);
        assert_eq!((out.as_str(), code), ("", 2));
        golden(&format!("en/errors/E080-{name}.txt"), &err);
    }
    // GEAS_LANG gives the language when --lang does not
    let (_, err, _) = run(s.path(), &["chekc"], &[("GEAS_LANG", "ja")]);
    assert!(err.starts_with("エラー[E080]"), "{err}");
}

#[test]
fn e081_files_that_cannot_be_read_or_written() {
    let s = Scratch::new("e081");
    both(&s, "E081-missing", &["check", "missing.geas"], 2, false);
    let (out, _, code) = run(s.path(), &["check", "missing.geas", "--json"], &[]);
    assert_eq!(code, 2);
    golden("en/errors/E081-missing.json", &out);
    // `.geas` is a file, so the journal has nowhere to go: the report is printed, then E081
    s.write("calc/calc.geas", "target calc {\n  run \"echo 5\"\n}\n\nclaim \"echoes\" {\n  when calc.run()\n  then stdout is \"5\"\n}\n");
    s.write("calc/.geas", "");
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    for lang in ["en", "ja"] {
        let (out, err, code) = run(s.path(), &["check", "calc/calc.geas", "--lang", lang], &[("GEAS_PID_LOG", log_s)]);
        assert_eq!(code, 2);
        golden(&format!("{lang}/errors/E081-no-journal.txt"), &format!("{out}--- stderr\n{err}"));
    }
    no_process_left(&log);
}
