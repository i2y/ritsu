//! `geas map`: the record of the lines each claim runs, on the Python examples, and
//! the codes it gives (W060, W061, E065, E066). Every run passes `--root`, since the
//! scratch directories lie inside this repository and the search for `.git` would
//! find its own; the root's own search is tested apart, with a `.git` of its own.

mod common;
use common::*;

/// The report and the warnings, as one golden: stdout, then stderr when there is any.
fn shown(out: &str, err: &str) -> String {
    if err.is_empty() { out.to_string() } else { format!("{out}--- stderr\n{err}") }
}

/// Nothing of the run is left under `.geas/` but the journal and the record.
fn only_journal_and_record(s: &Scratch, geas_dir: &str) {
    let mut names: Vec<String> = std::fs::read_dir(s.path().join(geas_dir))
        .expect("a .geas directory")
        .map(|e| e.expect("an entry").file_name().into_string().expect("a UTF-8 name"))
        .collect();
    names.sort();
    assert!(
        names.iter().all(|n| n.ends_with(".journal.jsonl") || n.ends_with(".map.jsonl")),
        "left under {geas_dir}: {names:?}"
    );
}

#[test]
fn calc() {
    if !python3("examples/calc") {
        return;
    }
    let s = Scratch::new("map-calc");
    copy_example("calc", &s);
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["map", "examples/calc/calc.geas", "--root", "."];
        args.extend(extra);
        let (out, err, code) = run(s.path(), &args, &[("GEAS_PID_LOG", log_s)]);
        assert_eq!((err.as_str(), code), ("", 0));
        golden(&format!("{lang}/map/calc.txt"), &out);
    }
    golden("en/map/calc.map.jsonl", &s.read("examples/calc/.geas/calc.map.jsonl"));
    only_journal_and_record(&s, "examples/calc/.geas");
    // one run on one tree writes the same bytes
    let first = s.read("examples/calc/.geas/calc.map.jsonl");
    let (_, _, code) = run(s.path(), &["map", "examples/calc/calc.geas", "--root", ".", "--out", "again.jsonl"], &[]);
    assert_eq!(code, 0);
    assert_eq!(s.read("again.jsonl"), first);
    assert_eq!(no_process_left(&log), 8, "one process per `when`, twice");
    // as JSON, with the record's numbers
    let (out, _, code) = run(s.path(), &["map", "examples/calc/calc.geas", "--root", ".", "--json"], &[]);
    assert_eq!(code, 0);
    let v = json(&out);
    assert_eq!(v.get("map").get("record").str(), "examples/calc/.geas/calc.map.jsonl");
    assert_eq!(v.get("diagnostics").arr().len(), 0);
    golden("en/map/calc.json", &out);
}

#[test]
fn greeter_stops_with_sigterm_and_is_recorded() {
    if !python3("examples/greeter") {
        return;
    }
    let s = Scratch::new("map-greeter");
    copy_example("greeter", &s);
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let (out, err, code) = run(s.path(), &["map", "examples/greeter/greeter.geas", "--root", "."], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!((err.as_str(), code), ("", 0));
    golden("en/map/greeter.txt", &out);
    golden("en/map/greeter.map.jsonl", &s.read("examples/greeter/.geas/greeter.map.jsonl"));
    assert_eq!(no_process_left(&log), 4, "one service per claim");
    only_journal_and_record(&s, "examples/greeter/.geas");
}

/// `--out` puts the record elsewhere; a spec outside `--root` and `--out` with two
/// specs are refused.
#[test]
fn where_the_record_goes() {
    let s = Scratch::new("map-out");
    s.write("d/echo.geas", "target echo {\n  run \"echo hi\"\n}\n\nclaim \"says hi\" {\n  when echo.run()\n  then stdout is \"hi\"\n}\n");
    let (out, err, code) = run(s.path(), &["map", "d/echo.geas", "--root", "d", "--out", "kept.jsonl"], &[]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("→ kept.jsonl"), "{out}");
    assert!(s.read("kept.jsonl").starts_with("{\"geas_map\":1,\"spec\":\"echo.geas\",\"root\":\".\""));
    assert!(!s.exists("d/.geas/echo.map.jsonl"));
    s.write("e/other.geas", &s.read("d/echo.geas"));
    // several specs, one record each
    let (out, err, code) = run(s.path(), &["map", "d/echo.geas", "e/other.geas", "--root", "."], &[]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(s.exists("d/.geas/echo.map.jsonl") && s.exists("e/.geas/other.map.jsonl"));
    for (name, args) in [
        ("map-outside-the-root", vec!["map", "e/other.geas", "--root", "d"]),
        ("map-out-with-two-specs", vec!["map", "d/echo.geas", "e/other.geas", "--out", "x.jsonl"]),
        ("map-root-not-a-directory", vec!["map", "d/echo.geas", "--root", "kept.jsonl"]),
    ] {
        let (out, err, code) = run(s.path(), &args, &[]);
        assert_eq!((out.as_str(), code), ("", 2), "{name}");
        golden(&format!("en/errors/E080-{name}.txt"), &err);
    }
}

/// Without `--root`, the root is the nearest directory with `.git`, a directory or
/// the file a worktree has; the record says how far up it is.
#[test]
fn the_root_is_found_by_git() {
    let s = Scratch::new("map-root");
    let spec = "target echo {\n  run \"echo hi\"\n}\n\nclaim \"says hi\" {\n  when echo.run()\n  then stdout is \"hi\"\n}\n";
    s.write("repo/.git/HEAD", "ref: refs/heads/main\n");
    s.write("repo/tools/hello/echo.geas", spec);
    s.write("repo/tools/hello/hello.py", "print('hi')\n");
    s.write("worktree/.git", "gitdir: ../repo/.git/worktrees/w\n");
    s.write("worktree/spec/echo.geas", spec);
    for (dir, root, rec) in [
        ("repo/tools/hello/echo.geas", "../..", "repo/tools/hello/.geas/echo.map.jsonl"),
        ("worktree/spec/echo.geas", "..", "worktree/spec/.geas/echo.map.jsonl"),
    ] {
        let (out, err, code) = run(s.path(), &["map", dir], &[]);
        assert_eq!(code, 0, "{out}{err}");
        let head = s.read(rec);
        let first = json(head.lines().next().expect("a header"));
        assert_eq!(first.get("root").str(), root, "{dir}");
    }
    // the root's sources, and nothing above it
    let rec = s.read("repo/tools/hello/.geas/echo.map.jsonl");
    assert!(rec.contains("\"file\":\"tools/hello/hello.py\""), "{rec}");
}

/// W060: a target whose processes report nothing, here a program with no coverage.
#[test]
fn w060_a_target_that_gives_no_record() {
    let s = Scratch::new("map-w060");
    s.write("w060/w060.geas", "target five {\n  run \"echo 5\"\n}\n\nclaim \"prints five\" {\n  when five.run()\n  then stdout is \"5\"\n}\n");
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["map", "w060/w060.geas", "--root", "w060"];
        args.extend(extra);
        let (out, err, code) = run(s.path(), &args, &[]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(err.contains("[W060]"), "{err}");
        golden(&format!("{lang}/map/W060.txt"), &shown(&out, &err));
    }
    // the record is written, with no lines
    assert!(s.exists("w060/.geas/w060.map.jsonl"));
}

/// E065: a program wrote a Rust profile, and the LLVM tools are not where
/// GEAS_LLVM_BIN says. No record is written.
#[test]
fn e065_no_llvm_tools() {
    if !python3("a fixture written in Python") {
        return;
    }
    let s = Scratch::new("map-e065");
    s.write(
        "e065/e065.geas",
        "target fake {\n  run \"python3 fake.py\"\n}\n\nclaim \"runs\" {\n  when fake.run()\n  then exit is 0\n}\n",
    );
    s.write(
        "e065/fake.py",
        "# Writes a file where a Rust program built with coverage writes its profile.\nimport os\n\nname = os.environ[\"LLVM_PROFILE_FILE\"].replace(\"%p\", str(os.getpid())).replace(\"%m\", \"1_0\")\nwith open(name, \"w\") as f:\n    f.write(\"not a profile\")\n",
    );
    s.write("empty/.keep", "");
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["map", "e065/e065.geas", "--root", "e065"];
        args.extend(extra);
        let (out, err, code) = run(s.path(), &args, &[("GEAS_LLVM_BIN", "empty"), ("GEAS_PID_LOG", log_s)]);
        assert_eq!(code, 2, "{out}{err}");
        golden(&format!("{lang}/map/E065.txt"), &shown(&out, &err));
    }
    assert!(!s.exists("e065/.geas/e065.map.jsonl"), "a record was written");
    // as JSON: no record, and the error among the diagnostics
    let (out, _, code) = run(s.path(), &["map", "e065/e065.geas", "--root", "e065", "--json"], &[("GEAS_LLVM_BIN", "empty"), ("GEAS_PID_LOG", log_s)]);
    assert_eq!(code, 2);
    let v = json(&out);
    assert_eq!((v.get("ok"), v.get("map")), (&Json::Bool(false), &Json::Null));
    assert_eq!(v.get("diagnostics").arr()[0].get("code").str(), "E065");
    only_journal_and_record(&s, "e065/.geas");
    no_process_left(&log);
}

/// E066: a service that ignores SIGTERM is killed after 5 s, and no record is
/// written. Costs 5 s: the two languages run side by side, on two ports.
#[test]
fn e066_a_service_that_ignores_sigterm() {
    if !python3("a service written in Python") {
        return;
    }
    let _port = port_lock();
    let s = Scratch::new("map-e066");
    let stubborn = "# A service that ignores SIGTERM, so it never stops by itself.\nimport signal\nimport sys\nfrom http.server import BaseHTTPRequestHandler, HTTPServer\n\nsignal.signal(signal.SIGTERM, signal.SIG_IGN)\n\n\nclass Handler(BaseHTTPRequestHandler):\n    def do_GET(self):\n        self.send_response(200)\n        self.send_header(\"Content-Length\", \"0\")\n        self.end_headers()\n\n    def log_message(self, *args):\n        pass\n\n\nHTTPServer((\"127.0.0.1\", int(sys.argv[1])), Handler).serve_forever()\n";
    for (lang, port) in [("en", 8123), ("ja", 8126)] {
        s.write(
            &format!("{lang}/e066.geas"),
            &format!("target api {{\n  serve \"python3 stubborn.py {port}\"\n  port {port}\n}}\n\nclaim \"answers\" {{\n  when api.get(\"/\")\n  then status is 200\n}}\n"),
        );
        s.write(&format!("{lang}/stubborn.py"), stubborn);
    }
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path").to_string();
    std::thread::scope(|scope| {
        let runs: Vec<_> = ["en", "ja"]
            .into_iter()
            .map(|lang| {
                let log_s = log_s.clone();
                let s = &s;
                scope.spawn(move || {
                    let spec = format!("{lang}/e066.geas");
                    let args = ["map", spec.as_str(), "--root", lang, "--lang", lang];
                    (lang, run(s.path(), &args, &[("GEAS_PID_LOG", &log_s)]))
                })
            })
            .collect();
        for r in runs {
            let (lang, (out, err, code)) = r.join().expect("a run");
            assert_eq!(code, 2, "{out}{err}");
            golden(&format!("{lang}/map/E066.txt"), &shown(&out, &err));
        }
    });
    assert_eq!(no_process_left(&log), 2, "one service per language, killed");
    assert!(!s.exists("en/.geas/e066.map.jsonl") && !s.exists("ja/.geas/e066.map.jsonl"));
}

/// W061: a Rust program a script started wrote a profile; geas cannot read it
/// without the program, and says so. Needs rustc and the LLVM tools.
#[test]
fn w061_a_rust_program_geas_did_not_start() {
    if !have("rustc", &["--version"]) {
        skip("rustc is not on PATH; W061 is not run");
        return;
    }
    if llvm_bin().is_none() {
        skip("llvm-tools are not in the Rust toolchain; W061 is not run");
        return;
    }
    let s = Scratch::new("map-w061");
    s.write("w061/w061.geas", "target tally {\n  run \"sh tally.sh\"\n}\n\nclaim \"adds\" {\n  when tally.run(\"2\", \"3\")\n  then stdout is \"5\"\n}\n");
    s.write(
        "w061/tally.sh",
        "# Builds the program with coverage the first time, then runs it: the program is\n# started by sh, not by geas.\n[ -x tally ] || rustc --edition 2021 -C instrument-coverage -o tally tally.rs\n./tally \"$@\"\n",
    );
    s.write(
        "w061/tally.rs",
        "fn main() {\n    let sum: i64 = std::env::args().skip(1).map(|a| a.parse::<i64>().unwrap_or(0)).sum();\n    println!(\"{sum}\");\n}\n",
    );
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["map", "w061/w061.geas", "--root", "w061"];
        args.extend(extra);
        let (out, err, code) = run(s.path(), &args, &[]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(err.contains("[W061]") && !err.contains("[W060]"), "{err}");
        golden(&format!("{lang}/map/W061.txt"), &shown(&out, &err));
    }
}
