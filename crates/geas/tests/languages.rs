//! The four runtimes end to end, on the five examples: `geas map` before and after
//! an agent-style change (`tests/changes/<example>/`), and `geas affected` on the
//! change as a git diff and as a plain one. Each example needs its toolchain and
//! prints SKIP without it. A Go build keeps its cache in the scratch directory, so
//! nothing of it is left behind; a Rust build is one `rustc` into the example.
//!
//! A change is the files of `after/`. Its `change.diff` is `git diff` between the
//! example and the example with `after/` copied in, each written into a scratch
//! index as a tree (`git add -A`, `git write-tree`, no commit), with the paths a
//! root holding `examples/<example>` gives them; `change.plain.diff` is
//! `diff -u <file>.orig <file>` from the same root.

mod common;
use common::*;
use std::path::Path;
use std::process::{Command, Stdio};

struct Example {
    name: &'static str,
    stem: &'static str,
    /// The files the change touches, relative to the example.
    changed: &'static [&'static str],
}

/// The environment a Go build and geas share in a test: a build cache in the
/// scratch directory, and nothing from the developer's own Go settings.
fn go_env(s: &Scratch) -> Vec<(String, String)> {
    vec![
        ("GOCACHE".into(), s.path().join("gocache").to_string_lossy().into_owned()),
        ("GOTOOLCHAIN".into(), "local".into()),
        ("GOFLAGS".into(), String::new()),
    ]
}

fn build(e: &Example, s: &Scratch) {
    let dir = s.path().join("examples").join(e.name);
    let mut cmd = match e.name {
        "tally-go" => {
            let mut c = Command::new("go");
            c.args(["build", "-cover", "-coverpkg=./...", "-trimpath", "-o", "tally", "."]);
            for (k, v) in go_env(s) {
                c.env(k, v);
            }
            c
        }
        "tally-rust" => {
            let mut c = Command::new("rustc");
            c.args(["--edition", "2024", "-C", "instrument-coverage", "-o", "tally", "src/main.rs"]);
            c
        }
        _ => return,
    };
    let out = cmd.current_dir(&dir).stdin(Stdio::null()).output().expect("start the build");
    assert!(out.status.success(), "the build of {} failed:\n{}", e.name, String::from_utf8_lossy(&out.stderr));
}

/// Whether the example's toolchain is here; prints the SKIP line when it is not.
fn at_hand(e: &Example) -> bool {
    let (ok, what) = match e.name {
        "calc" | "greeter" => (have("python3", &["--version"]), "python3 is not on PATH"),
        "tally-node" => (have("node", &["--version"]), "node is not on PATH"),
        "tally-go" => (have("go", &["version"]), "go is not on PATH"),
        _ => (
            have("rustc", &["--version"]) && llvm_bin().is_some(),
            "rustc with the llvm-tools component is not at hand",
        ),
    };
    if !ok {
        skip(&format!("{what}; examples/{} is not run", e.name));
    }
    ok
}

/// The `index` line of a file in a git diff: its two blobs, abbreviated.
fn index_of(diff: &str, path: &str) -> (String, String) {
    let header = format!("diff --git a/{path} b/{path}");
    let at = diff.find(&header).unwrap_or_else(|| panic!("no {path} in the diff"));
    let line = diff[at..].lines().find(|l| l.starts_with("index ")).expect("an index line");
    let (a, b) = line["index ".len()..].split_whitespace().next().expect("the blobs").split_once("..").expect("..");
    (a.to_string(), b.to_string())
}

/// A record's blob of a file.
fn blob_in(record: &str, path: &str) -> String {
    record
        .lines()
        .map(json)
        .find(|v| matches!(v, Json::Obj(p) if p.iter().any(|(k, x)| k == "file" && x == &Json::Str(path.to_string())) && p.iter().any(|(k, _)| k == "blob")))
        .unwrap_or_else(|| panic!("no {path} in the record"))
        .get("blob")
        .str()
        .to_string()
}

fn end_to_end(e: &Example) {
    if !at_hand(e) {
        return;
    }
    let s = Scratch::new(&format!("languages-{}", e.name));
    copy_example(e.name, &s);
    let spec = format!("examples/{}/{}.geas", e.name, e.stem);
    let record = format!("examples/{}/.geas/{}.map.jsonl", e.name, e.stem);
    let log = pid_log(&s);
    let mut env: Vec<(String, String)> = vec![("GEAS_PID_LOG".into(), log.to_string_lossy().into_owned())];
    if e.name == "tally-go" {
        env.extend(go_env(&s));
    }
    let env_ref: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let golden_dir = format!("en/languages/{}", e.name);

    // the record of the code before the change
    build(e, &s);
    let (out, err, code) = run(s.path(), &["map", &spec, "--root", "."], &env_ref);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    golden(&format!("{golden_dir}/map.txt"), &out);
    let before = s.read(&record);
    golden(&format!("{golden_dir}/before.map.jsonl"), &before);
    s.write("before.map.jsonl", &before);
    same_with_four_jobs(e, &s, &spec, &env_ref, &out, &before);

    // the change, and the record of the code after it
    let change = root().join("tests/changes").join(e.name);
    for f in e.changed {
        let text = std::fs::read_to_string(change.join("after").join(f)).expect("a changed file");
        s.write(&format!("examples/{}/{f}", e.name), &text);
    }
    build(e, &s);
    let (out, err, code) = run(s.path(), &["map", &spec, "--root", "."], &env_ref);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    golden(&format!("{golden_dir}/map-after.txt"), &out);
    let after = s.read(&record);
    golden(&format!("{golden_dir}/after.map.jsonl"), &after);

    // the diffs name the blobs geas computed for the two sides
    let git_diff = std::fs::read_to_string(change.join("change.diff")).expect("change.diff");
    for f in e.changed {
        let path = format!("examples/{}/{f}", e.name);
        let (a, b) = index_of(&git_diff, &path);
        assert!(blob_in(&before, &path).starts_with(&a), "{path}: the index line's {a} is not the record's");
        assert!(blob_in(&after, &path).starts_with(&b), "{path}: the index line's {b} is not the record's");
    }
    s.write("change.diff", &git_diff);
    s.write("change.plain.diff", &std::fs::read_to_string(change.join("change.plain.diff")).expect("change.plain.diff"));

    // what the change touches, from both diffs: the same but for the diff's name
    let (git, err, code) = run(s.path(), &["affected", &spec, "change.diff", "--root", "."], &[]);
    assert_eq!((err.as_str(), code), ("", 1), "{git}");
    golden(&format!("{golden_dir}/affected.txt"), &git);
    let (plain, err, code) = run(s.path(), &["affected", &spec, "change.plain.diff", "--root", "."], &[]);
    assert_eq!((err.as_str(), code), ("", 1), "{plain}");
    golden(&format!("{golden_dir}/affected-plain.txt"), &plain);
    assert_eq!(git.lines().skip(1).collect::<Vec<_>>(), plain.lines().skip(1).collect::<Vec<_>>());

    // with the record of each side, removed lines are exact
    let (both, err, code) = run(s.path(), &["affected", &spec, "change.diff", "--root", ".", "--map", "before.map.jsonl", "--map", &record], &[]);
    assert_eq!((err.as_str(), code), ("", 1), "{both}");
    golden(&format!("{golden_dir}/affected-two-records.txt"), &both);

    assert!(no_process_left(&log) > 0, "the log of started processes is empty");
    let leftovers = ["hook", "cover"].map(|d| Path::new(&format!("examples/{}/.geas/{d}", e.name)).to_path_buf());
    for d in leftovers {
        assert!(!s.path().join(&d).exists(), "{} was left behind", d.display());
    }
}

/// Four claims at once print and write, byte for byte, what one at a time does
/// (DESIGN §10): `map` its report, record and journal; `check` its report and
/// journal; `snap` its report, journal and baseline.
fn same_with_four_jobs(e: &Example, s: &Scratch, spec: &str, env: &[(&str, &str)], map_out: &str, record: &str) {
    let journal = format!("examples/{}/.geas/{}.journal.jsonl", e.name, e.stem);
    let baseline = format!("examples/{}/.geas/{}.baseline.jsonl", e.name, e.stem);
    let map_journal = s.read(&journal);
    let (out, err, code) = run(s.path(), &["map", spec, "--root", ".", "-j4", "--out", "four.map.jsonl"], env);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    // the last line names the record, which `--out` put elsewhere
    let but_last = |t: &str| t.lines().rev().skip(1).map(str::to_string).collect::<Vec<_>>();
    assert_eq!(but_last(&out), but_last(map_out), "{}: map -j4 printed another report", e.name);
    assert_eq!(s.read("four.map.jsonl"), record, "{}: map -j4 wrote another record", e.name);
    assert_eq!(s.read(&journal), map_journal, "{}: map -j4 wrote another journal", e.name);
    for cmd in ["check", "snap"] {
        let one = run(s.path(), &[cmd, spec], env);
        let (one_journal, one_baseline) = (s.read(&journal), s.exists(&baseline).then(|| s.read(&baseline)));
        let four = run(s.path(), &[cmd, spec, "-j4"], env);
        assert_eq!(four, one, "{}: {cmd} -j4 printed something else", e.name);
        assert_eq!(s.read(&journal), one_journal, "{}: {cmd} -j4 wrote another journal", e.name);
        assert_eq!(s.exists(&baseline).then(|| s.read(&baseline)), one_baseline, "{}: {cmd} -j4 wrote another baseline", e.name);
    }
}

#[test]
fn python_command() {
    end_to_end(&Example { name: "calc", stem: "calc", changed: &["calc.py"] });
}

#[test]
fn python_service() {
    end_to_end(&Example { name: "greeter", stem: "greeter", changed: &["server.py"] });
}

#[test]
fn typescript_service_on_node() {
    end_to_end(&Example { name: "tally-node", stem: "tally", changed: &["server.ts"] });
}

#[test]
fn go_service() {
    end_to_end(&Example { name: "tally-go", stem: "tally", changed: &["main.go"] });
}

#[test]
fn rust_command() {
    end_to_end(&Example { name: "tally-rust", stem: "tally", changed: &["src/main.rs"] });
}

/// E066 from Go: a service that leaves SIGTERM to Go's default dies at once,
/// without writing its counters, and leaves only its coverage metadata.
#[test]
fn a_go_service_killed_by_sigterm_writes_no_counters() {
    if !have("go", &["version"]) {
        skip("go is not on PATH; a Go service killed by SIGTERM is not run");
        return;
    }
    let _port = port_lock();
    let s = Scratch::new("languages-go-sigterm");
    s.write("bare/go.mod", "module bare\n\ngo 1.22\n");
    s.write(
        "bare/main.go",
        "// A service that leaves SIGTERM to Go's default, which ends the program at once.\npackage main\n\nimport (\n\t\"net/http\"\n\t\"os\"\n)\n\nfunc main() {\n\thttp.HandleFunc(\"/\", func(w http.ResponseWriter, r *http.Request) { w.Write([]byte(\"ok\")) })\n\thttp.ListenAndServe(\"127.0.0.1:\"+os.Args[1], nil)\n}\n",
    );
    s.write("bare/bare.geas", "target api {\n  serve \"./bare 8125\"\n  port 8125\n}\n\nclaim \"answers\" {\n  when api.get(\"/\")\n  then body is \"ok\"\n}\n");
    let env = go_env(&s);
    let mut cmd = Command::new("go");
    cmd.args(["build", "-cover", "-coverpkg=./...", "-trimpath", "-o", "bare", "."]).current_dir(s.path().join("bare"));
    for (k, v) in &env {
        cmd.env(k, v);
    }
    let out = cmd.stdin(Stdio::null()).output().expect("start go build");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let env_ref: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    for (lang, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"])] {
        let mut args = vec!["map", "bare/bare.geas", "--root", "bare"];
        args.extend(extra);
        let (out, err, code) = run(s.path(), &args, &env_ref);
        assert_eq!(code, 2, "{out}{err}");
        assert!(err.contains("[E066]") && (err.contains("no counters") || err.contains("カウンター")), "{err}");
        golden(&format!("{lang}/languages/go-no-counters.txt"), &format!("{out}--- stderr\n{err}"));
    }
    assert!(!s.exists("bare/.geas/bare.map.jsonl"), "a record was written");
}
