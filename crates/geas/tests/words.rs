//! Command strings, split into words by geas itself (DESIGN §11): a file name with
//! a space, written three ways; every kind of word; a shell, named when one is
//! wanted; and `{port}` in a service's command. The E010 specs are with the other
//! static codes, under `tests/specs/`.

mod common;
use common::*;

const SPEC: &str = r#"# geas splits each command into words itself, never through a shell.

target single {
  run "python3 'my calc.py'"
}

target double {
  run "python3 \"my calc.py\""
}

target escaped {
  run "python3 my\\ calc.py"
}

target words {
  run "python3 args.py '' 'a b' \"c 'd'\" e\\'f $HOME ~ *.py"
}

target shell {
  run "sh -c 'echo $0 && echo $1' first second"
}

claim "a file name with a space, in single quotes" {
  when single.run("2", "3")
  then stdout is "5"
}

claim "a file name with a space, in double quotes" {
  when double.run("2", "3")
  then stdout is "5"
}

claim "a file name with a space, after a backslash" {
  when escaped.run("2", "3")
  then stdout is "5"
}

claim "every word as it was written, and nothing expanded" {
  when words.run("g h")
  then stdout is "[\"\", \"a b\", \"c 'd'\", \"e'f\", \"$HOME\", \"~\", \"*.py\", \"g h\"]"
}

claim "a shell when one is named" {
  when shell.run()
  then stdout is "first\nsecond"
}
"#;

#[test]
fn a_file_name_with_a_space_and_every_kind_of_word() {
    if !python3("commands written in Python") {
        return;
    }
    let s = Scratch::new("words");
    s.write("calc/calc.geas", SPEC);
    s.write("calc/my calc.py", "import sys\nprint(int(sys.argv[1]) + int(sys.argv[2]))\n");
    s.write("calc/args.py", "import json, sys\nprint(json.dumps(sys.argv[1:]))\n");
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let (out, err, code) = run(s.path(), &["check", "calc/calc.geas"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    golden("en/words/calc.txt", &out);
    golden("en/words/calc.journal.jsonl", &s.read("calc/.geas/calc.journal.jsonl"));
    assert_eq!(no_process_left(&log), 5);
}

/// `{port}` in a service's command becomes its port, inside a word as well.
#[test]
fn the_port_in_a_services_command() {
    if !python3("a service written in Python") {
        return;
    }
    let _port = port_lock();
    let s = Scratch::new("words-port");
    s.write(
        "api/api.geas",
        "target api {\n  serve \"python3 'my server.py' --port={port}\"\n  port 8123\n}\n\nclaim \"says its port\" {\n  when api.get(\"/\")\n  then body is \"8123\"\n}\n",
    );
    s.write(
        "api/my server.py",
        "import sys\nfrom http.server import BaseHTTPRequestHandler, HTTPServer\n\nport = int(sys.argv[1].removeprefix(\"--port=\"))\n\n\nclass Handler(BaseHTTPRequestHandler):\n    def do_GET(self):\n        body = str(port).encode()\n        self.send_response(200)\n        self.send_header(\"Content-Length\", str(len(body)))\n        self.end_headers()\n        self.wfile.write(body)\n\n    def log_message(self, *args):\n        pass\n\n\nHTTPServer((\"127.0.0.1\", port), Handler).serve_forever()\n",
    );
    let log = pid_log(&s);
    let log_s = log.to_str().expect("a UTF-8 path");
    let (out, err, code) = run(s.path(), &["check", "api/api.geas"], &[("GEAS_PID_LOG", log_s)]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    assert_eq!(out, "ok 1 - says its port\n1 claim · 1 ok · 0 failed · journal: api/.geas/api.journal.jsonl\n");
    assert_eq!(no_process_left(&log), 1);
    assert!(port_closed(8123), "the service is still answering on 8123");
}
