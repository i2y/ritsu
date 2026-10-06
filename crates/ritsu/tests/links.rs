//! The links of a release (DESIGN 2.3, 8.2): one binary, `ritsu`, and a link to it named for each of
//! the eight languages. `tests/entry.rs` calls two of the links; this file calls all eight, holds
//! each against the same words after `ritsu`, reads what the four languages that read others get
//! from theirs, takes the name from the last part of `argv[0]`, closes a pipe on a command, and
//! has `rulec mcp` call the binary the way a link does. The links are what a release makes:
//! symbolic links in a temporary directory.

use ritsu_testkit::TempDir;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::symlink;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const LANGUAGES: [&str; 8] = ["rulec", "dandori", "koyomi", "chobo", "geas", "yuen", "sakai", "sekisho"];

fn ritsu() -> &'static str {
    env!("CARGO_BIN_EXE_ritsu")
}

fn crates() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// A directory holding a link to `ritsu` for each of the eight names, and one for a name that is no
/// language.
fn links_in(t: &TempDir) -> PathBuf {
    for name in LANGUAGES.iter().copied().chain(["not-a-language"]) {
        symlink(ritsu(), t.path().join(name)).unwrap();
    }
    t.path().to_path_buf()
}

/// What a program printed and exited with.
type Ran = (i32, String, String);

/// Run `c` with no language asked of the environment.
fn run(mut c: Command, args: &[&str]) -> Ran {
    c.args(args);
    for var in ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG", "CHOBO_LANG", "GEAS_LANG", "YUEN_LANG", "SAKAI_LANG", "SEKISHO_LANG"] {
        c.env_remove(var);
    }
    let o = c.output().expect("could not run it");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// `<links>/<name> <args>`, run in the directory of that language's crate.
fn linked(links: &Path, name: &str, args: &[&str]) -> Ran {
    let mut c = Command::new(links.join(name));
    c.current_dir(crates().join(name));
    run(c, args)
}

/// `ritsu <name> <args>`, run in the same directory.
fn after_ritsu(name: &str, args: &[&str]) -> Ran {
    let mut c = Command::new(ritsu());
    c.current_dir(crates().join(name)).arg(name);
    run(c, args)
}

/// The first map under sakai's examples: its name follows the language the example is written in.
fn sakai_map() -> String {
    let examples = crates().join("sakai/examples");
    let mut maps: Vec<String> = std::fs::read_dir(&examples)
        .unwrap()
        .flatten()
        .filter(|d| d.path().is_dir())
        .flat_map(|d| std::fs::read_dir(d.path()).unwrap().flatten().map(|f| f.path()).collect::<Vec<_>>())
        .filter(|p| p.extension().is_some_and(|e| e == "ctx"))
        .map(|p| p.strip_prefix(crates().join("sakai")).unwrap().to_string_lossy().into_owned())
        .collect();
    maps.sort();
    maps.remove(0)
}

/// An example of each language, and the words that run its command on it. The last four read
/// others, so the example reads another language: a flow that uses rules, a requirement that names
/// a rule's source, a map that crosses into rules, calendars and flows, a gate that takes a rule's
/// answer, dates and a calendar as conditions and names a workflow's flow.
fn examples() -> Vec<(&'static str, Vec<String>)> {
    let s = |v: &[&str]| v.iter().map(|w| w.to_string()).collect::<Vec<_>>();
    vec![
        ("rulec", s(&["check", "tests/corpus/ec261.rule"])),
        ("koyomi", s(&["check", "examples/net30.cal"])),
        ("chobo", s(&["check", "examples/inventory/inventory.book"])),
        ("geas", s(&["explain", "E001"])),
        ("dandori", s(&["check", "examples/hotel/temporal/hotel.flow"])),
        ("yuen", s(&["check", "--root", "tests/fixtures/rulec", "tests/fixtures/rulec"])),
        ("sakai", vec!["check".to_string(), sakai_map()]),
        ("sekisho", s(&["check", "examples/refunds/refunds.gate", "--root", "examples/refunds"])),
    ]
}

/// Called by the name of a language, `ritsu` says what that language's command says: `<name>
/// --version` is one line, the name and a version, and is what `ritsu <name> --version` says.
#[test]
fn every_link_says_its_own_name_and_a_version() {
    let t = TempDir::new("links-version");
    let links = links_in(&t);
    for name in LANGUAGES {
        let (code, out, err) = linked(&links, name, &["--version"]);
        assert_eq!(code, 0, "{name}: {err}");
        let mut words = out.trim_end().split(' ');
        assert_eq!(words.next(), Some(name), "{out}");
        let version = words.next().unwrap_or("");
        assert!(words.next().is_none() && out.lines().count() == 1, "{name} --version is one line, `{name} <version>`: {out}");
        assert!(version.split('.').count() == 3 && version.split('.').all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())), "{name}: `{version}` is not a version");
        assert_eq!(after_ritsu(name, &["--version"]), (0, out, String::new()), "ritsu {name} --version");
    }
}

/// A link and the same words after `ritsu` are one command: the same output, the same exit code,
/// on an example of each language, in English and in Japanese.
#[test]
fn a_link_and_the_same_words_after_ritsu_are_one_command() {
    let t = TempDir::new("links-same");
    let links = links_in(&t);
    for (name, args) in examples() {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let by_link = linked(&links, name, &args);
        assert_eq!(by_link.0, 0, "{name} {args:?}: {}{}", by_link.1, by_link.2);
        assert_eq!(by_link, after_ritsu(name, &args), "{name} {args:?}");
        let mut ja = args.clone();
        ja.extend(["--lang", "ja"]);
        assert_eq!(linked(&links, name, &ja), after_ritsu(name, &ja), "{name} {ja:?}");
    }
}

/// What the four that read others get from their links: the ports are joined. A flow that uses
/// rules passes check (the binary of dandori's crate stops at it), a requirement that names a
/// rule's source is read (yuen's crate says to run it through ritsu), a map crosses into the
/// rules, calendars and flows it names (sakai's crate stops at each), and a gate reads its rules,
/// dates, calendar and flow (sekisho's crate stops at the first `use rule` with E209).
#[test]
fn the_links_of_the_languages_that_read_others_join_the_ports() {
    let t = TempDir::new("links-ports");
    let links = links_in(&t);
    let flow = "examples/hotel/temporal/hotel.flow";
    assert_eq!(linked(&links, "dandori", &["check", flow]), (0, String::new(), format!("{flow}: ok\n")));
    assert_eq!(linked(&links, "dandori", &["check", flow, "--lang", "ja"]), (0, String::new(), format!("{flow}: 検査を通りました\n")));
    let (code, out, err) = linked(&links, "yuen", &["check", "--root", "tests/fixtures/rulec", "tests/fixtures/rulec"]);
    assert!(code == 0 && out.starts_with("tests/fixtures/rulec: ok — "), "{out}{err}");
    let (code, out, err) = linked(&links, "sakai", &["check", &sakai_map()]);
    assert!(code == 0 && out.contains(" crossings checked (") && out.contains("rulec ") && out.contains("koyomi ") && out.contains("dandori "), "{out}{err}");
    let gate = "examples/refunds/refunds.gate";
    let want = format!("{gate}: ok — 3 actions, 10 policies (7 permits, 3 forbids), 3 expectations, 1 separation\n");
    assert_eq!(linked(&links, "sekisho", &["check", gate, "--root", "examples/refunds"]), (0, want, String::new()));
}

/// A name that is no language is `ritsu` itself: a file called `ritsu-0.23.0`, or a link a user
/// named as they liked, is not an error.
#[test]
fn a_name_that_is_no_language_is_ritsu() {
    let t = TempDir::new("links-other");
    let links = links_in(&t);
    let other = |args: &[&str]| run(Command::new(links.join("not-a-language")), args);
    let ritsu_itself = |args: &[&str]| run(Command::new(ritsu()), args);
    for args in [&["--version"][..], &["--help"], &["nope"], &[]] {
        assert_eq!(other(args), ritsu_itself(args), "{args:?}");
    }
    assert_eq!(other(&["--version"]), (0, format!("ritsu {}\n", env!("CARGO_PKG_VERSION")), String::new()));
}

/// The name a program was called by is the last part of `argv[0]`, whatever comes before it: a
/// bare name from the PATH, a relative path, an absolute one. `rulec mcp` calls this binary by that
/// name for each tool call (a child cannot be told the link it came through, on Linux).
#[test]
fn the_name_is_the_last_part_of_the_path_it_was_called_by() {
    for argv0 in ["rulec", "./rulec", "/usr/local/bin/rulec", "some/dir/rulec"] {
        let mut c = Command::new(ritsu());
        c.arg0(argv0);
        let (code, out, _) = run(c, &["--version"]);
        assert!(code == 0 && out.starts_with("rulec "), "{argv0}: {out}");
    }
}

/// A reader that goes away ends the command quietly, as `cat` does, called by the name of a language
/// as well as through `ritsu`: the process is stopped by SIGPIPE, with nothing on its standard
/// error. (`rulec explain --all` is more than a pipe holds.)
#[test]
fn a_reader_that_goes_away_ends_the_command_quietly() {
    let t = TempDir::new("links-pipe");
    let links = links_in(&t);
    let by_link = Command::new(links.join("rulec"));
    let mut after = Command::new(ritsu());
    after.arg("rulec");
    for mut c in [by_link, after] {
        let mut child = c.args(["explain", "--all", "--format", "markdown"]).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("could not start it");
        {
            // one line, then the reading end of the pipe is closed
            let mut reader = BufReader::new(child.stdout.take().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert!(line.starts_with("<!-- Output of `rulec explain --all"), "{line}");
        }
        let status = child.wait().unwrap();
        let mut err = String::new();
        child.stderr.take().unwrap().read_to_string(&mut err).unwrap();
        assert_eq!(status.signal(), Some(13), "stopped by SIGPIPE, not by an error of its own: {status:?} {err}");
        assert_eq!(err, "");
    }
}

/// `rulec mcp` runs the binary itself for each tool call. Called through a link, or as `rulec` by
/// its real path (what `current_exe` gives on Linux), the binary has to call itself as `rulec` too.
/// Called as `ritsu` it would be asked for `check`, which `ritsu` has as a command of its own that
/// prints one more line, and for `fmt`, which it has not.
#[test]
fn rulec_mcp_runs_its_calls_as_rulec() {
    let t = TempDir::new("links-mcp");
    let links = links_in(&t);
    let by_link = Command::new(links.join("rulec"));
    let mut by_path = Command::new(ritsu());
    by_path.arg0("rulec");
    for mut c in [by_link, by_path] {
        let mut child = c
            .current_dir(crates().join("rulec"))
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("could not start rulec mcp");
        {
            let mut stdin = child.stdin.take().unwrap();
            writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-06-18","capabilities":{{}},"clientInfo":{{"name":"t","version":"0"}}}}}}"#).unwrap();
            writeln!(stdin, r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"rulec_check","arguments":{{"files":["tests/corpus/ec261.rule"]}}}}}}"#).unwrap();
            writeln!(stdin, r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"rulec_fmt","arguments":{{"files":["tests/corpus/ec261.rule"],"check":true}}}}}}"#).unwrap();
        }
        let answers: Vec<serde_json::Value> = BufReader::new(child.stdout.take().unwrap()).lines().map(|l| serde_json::from_str(&l.unwrap()).unwrap()).collect();
        assert!(child.wait().unwrap().success());
        assert_eq!(answers.len(), 3, "{answers:?}");
        assert_eq!(answers[0]["result"]["serverInfo"]["name"], "rulec", "{answers:?}");
        // `rulec check`, and nothing more than it prints
        let call = &answers[1]["result"];
        assert_eq!(call["isError"], false, "the call ran as rulec: {call}");
        assert_eq!(call["content"][0]["text"], "ok tests/corpus/ec261.rule\n", "{call}");
        assert_eq!(call["content"][1]["text"], "exit code 0", "{call}");
        // `rulec fmt --check`, which `ritsu` does not have as a command
        let call = &answers[2]["result"];
        assert_eq!(call["isError"], false, "the call ran as rulec: {call}");
        assert_eq!(call["content"][0]["text"], "", "{call}");
        assert_eq!(call["content"][1]["text"], "exit code 0", "{call}");
    }
}
