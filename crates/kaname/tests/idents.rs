//! The names of a function's code that kaname writes as they are (DESIGN 36.3): a `let`, a
//! parameter of a function given to a builtin and the value `some` names keep the name of the
//! source where the target takes it, by the tables ritsu-emit makes from the Unicode Character
//! Database, and are `v` where not. Each test puts every character a name of kaname's may hold, as
//! a name (`ä`) and after a first letter (`aä`), through a tool of the target, and holds what the
//! tool says to what the target's table says:
//!
//! - rustc (`cargo check`) takes every name the table lets in, and names each one it keeps out;
//! - Python's `str.isidentifier`, which its parser reads a name by;
//! - Node (`new Function`) and tsc (`--noEmit`);
//! - Go's `go/token.IsIdentifier`, the specification's rule over Go's `unicode`.
//!
//! A tool whose Unicode is older than the table's does not take a letter Unicode added after it:
//! the table lets such a name in, and the tool's Unicode has no such character. The test counts
//! those apart, and fails on any other name the two part on. Without the tool, the test says SKIP
//! and passes.

use ritsu_testkit::{Need, TempDir};
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const LIMIT: Duration = Duration::from_secs(600);

/// Every character a name of kaname's may begin with (`kaname::lex::starts_name`), as a name, and
/// every one it may hold after its first (`is_name_char`), after `a`; `_`, which every target
/// takes, aside. Each with the character it is put through for.
fn names() -> Vec<(String, char)> {
    let mut out = Vec::new();
    for c in (0..=0x10FFFFu32).filter_map(char::from_u32).filter(|&c| c != '_') {
        if kaname::lex::starts_name(c) {
            out.push((c.to_string(), c));
        }
        if kaname::lex::is_name_char(c) {
            out.push((format!("a{c}"), c));
        }
    }
    out
}

/// What a tool said of a name: whether it takes it, and whether its Unicode has the character the
/// name is put through for.
#[derive(Clone, Copy)]
struct Said {
    takes: bool,
    knows: bool,
}

/// What a tool printed of each name, a line of two digits a name (`10`: it takes the name, and its
/// Unicode has no such character).
fn said(lines: &str) -> Vec<Said> {
    lines.lines().map(|l| Said { takes: l.as_bytes()[0] == b'1', knows: l.as_bytes()[1] == b'1' }).collect()
}

/// Holds what a tool said of each name to what `table` says of it: the two agree, but where the
/// table lets in a name whose character the tool's Unicode does not have. The counts of the names
/// both let in, of those both keep out, and of those the table alone lets in, for the tool's
/// older Unicode.
fn hold(tool: &str, names: &[(String, char)], table: impl Fn(&str) -> bool, said: &[Said]) -> (usize, usize, usize) {
    assert_eq!(said.len(), names.len(), "{tool} said what it takes of {} names, not of {}", said.len(), names.len());
    let (mut both_in, mut both_out, mut newer) = (0, 0, 0);
    let mut parted = Vec::new();
    for ((n, c), s) in names.iter().zip(said) {
        match (table(n), s.takes) {
            (true, true) => both_in += 1,
            (false, false) => both_out += 1,
            (true, false) if !s.knows => newer += 1,
            (t, _) => parted.push(format!("{n:?} (U+{:04X}): the table {}", *c as u32, if t { "lets it in and the tool keeps it out" } else { "keeps it out and the tool takes it" })),
        }
    }
    assert!(parted.is_empty(), "{tool} and the table part on {} names:\n{}", parted.len(), parted.iter().take(30).cloned().collect::<Vec<_>>().join("\n"));
    (both_in, both_out, newer)
}

/// The line a test prints of what it compared, `unicode` the Unicode the tool reads names by.
fn report(tool: &str, unicode: &str, n: usize, (both_in, both_out, newer): (usize, usize, usize)) {
    let newer = if newer == 0 { String::new() } else { format!("; {newer} names the table lets in hold a character its Unicode does not have yet") };
    eprintln!("compared: {n} names with {tool} ({unicode}): it takes the {both_in} the table lets in and keeps out the {both_out} the table keeps out{newer}");
}

#[test]
fn rustc_takes_the_names_of_rust_s_table() {
    if !ritsu_testkit::need(Need::Rustc) {
        return;
    }
    if !ritsu_testkit::tools::runs("rustc", &["--version"]) {
        ritsu_testkit::skip("rustc is not on the PATH; the names kaname writes in Rust are not held to it");
        return;
    }
    let names = names();
    // the names in the body of a macro that takes any tokens, which rustc reads as a name or stops
    // at: those the table lets in, 64 characters to a name after the first, all in one file that
    // builds, and those it keeps out, a line each, in one that does not
    let head = "#![allow(uncommon_codepoints, mixed_script_confusables, confusable_idents)]\nmacro_rules! m {\n    ($($t:tt)*) => {};\n}\nm! {\n";
    let (mut good, mut bad) = (String::from(head), String::from(head));
    let (mut let_in, mut kept_out) = (0usize, Vec::new());
    let mut run = String::new();
    for (n, c) in &names {
        if ritsu_emit::ident::rust_takes(n) {
            let_in += 1;
            if n.chars().count() == 1 {
                writeln!(good, "{n}").unwrap();
            } else {
                run.push(*c);
                if run.chars().count() == 64 {
                    writeln!(good, "a{run}").unwrap();
                    run.clear();
                }
            }
        } else {
            kept_out.push(*c);
            writeln!(bad, "{n}").unwrap();
        }
    }
    writeln!(good, "a{run}\n}}").unwrap();
    bad.push_str("}\n");
    let work = TempDir::new("kaname-idents-rustc");
    let check = |name: &str, text: &str| {
        let dir = work.path().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Cargo.toml"), format!("[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[lib]\npath = \"lib.rs\"\n\n[workspace]\n")).unwrap();
        std::fs::write(dir.join("lib.rs"), text).unwrap();
        // rustc's errors are read by their words, so they come with no colour, whatever
        // CARGO_TERM_COLOR the run has
        let mut c = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
        c.current_dir(&dir).env("CARGO_TARGET_DIR", Path::new(env!("CARGO_TARGET_TMPDIR")).join("kaname-idents")).env_remove("RUSTFLAGS").env_remove("CARGO_ENCODED_RUSTFLAGS");
        c.args(["check", "--offline", "--quiet", "--lib", "--color", "never"]);
        ritsu_testkit::run(&mut c, LIMIT)
    };
    let r = check("idents-let-in", &good);
    assert!(r.ok, "rustc keeps out a name the table lets in:\n{}", r.stderr.lines().take(40).collect::<Vec<_>>().join("\n"));
    let r = check("idents-kept-out", &bad);
    assert!(!r.ok, "rustc takes every name the table keeps out");
    let unnamed: Vec<String> = kept_out
        .iter()
        .filter(|c| !r.stderr.contains(&format!("unknown start of token: {}", c.escape_unicode())) && !r.stderr.lines().any(|l| l.starts_with("error: identifiers cannot contain emoji: ") && l.contains(**c)))
        .map(|c| format!("U+{:04X}", *c as u32))
        .collect();
    assert!(unnamed.is_empty(), "rustc takes {} of the names the table keeps out: {}", unnamed.len(), unnamed.join(" "));
    let rustc = ritsu_testkit::tools::version("rustc", "--version");
    let unicode = format!("{}.{}.{}", char::UNICODE_VERSION.0, char::UNICODE_VERSION.1, char::UNICODE_VERSION.2);
    report(&rustc, &format!("Unicode {unicode}"), names.len(), (let_in, kept_out.len(), 0));
}

#[test]
fn python_takes_the_names_of_python_s_table() {
    if !ritsu_testkit::need(Need::Python) {
        return;
    }
    if !ritsu_testkit::tools::runs("python3", &["--version"]) {
        ritsu_testkit::skip("python3 is not on the PATH; the names kaname writes in Python are not held to it");
        return;
    }
    let names = names();
    let input: String = names.iter().map(|(n, _)| format!("{n}\n")).collect();
    // the input read as UTF-8 whatever the locale (-I leaves out PYTHONIOENCODING), and the answer
    // in ASCII
    let script = "import sys, unicodedata\nprint(unicodedata.unidata_version)\nfor n in sys.stdin.buffer.read().decode('utf-8').split('\\n')[:-1]:\n    print(int(n.isidentifier()), int(unicodedata.category(n[-1]) != 'Cn'), sep='')\n";
    let r = ritsu_testkit::run::run_with_input(Command::new("python3").args(["-I", "-c", script]), Some(input.as_bytes()), LIMIT);
    assert!(r.ok, "python3 does not read the names:\n{}", r.both());
    let (unicode, lines) = r.stdout.split_once('\n').unwrap();
    let counts = hold("Python", &names, ritsu_emit::ident::python_takes, &said(lines));
    report(&ritsu_testkit::tools::version("python3", "--version"), &format!("Unicode {unicode}"), names.len(), counts);
}

/// The script Node runs: whether a name is one `let` declares, and whether Node's Unicode has the
/// character (no `\p{Cn}`, of the code points no character is yet).
const NODE: &str = r#"const names = require("fs").readFileSync(0, "utf8").split("\n");
names.pop();
const out = [process.versions.unicode];
for (const n of names) {
  let takes = 1;
  try {
    new Function("let " + n + ";");
  } catch {
    takes = 0;
  }
  out.push(`${takes}${/\p{Cn}/u.test([...n].pop()) ? 0 : 1}`);
}
process.stdout.write(out.join("\n") + "\n");
"#;

#[test]
fn node_and_tsc_take_the_names_of_typescript_s_table() {
    if !ritsu_testkit::need(Need::Node) {
        return;
    }
    if !ritsu_testkit::tools::runs("node", &["--version"]) {
        ritsu_testkit::skip("node is not on the PATH; the names kaname writes in TypeScript are not held to it");
        return;
    }
    let names = names();
    let input: String = names.iter().map(|(n, _)| format!("{n}\n")).collect();
    let r = ritsu_testkit::run::run_with_input(Command::new("node").args(["-e", NODE]), Some(input.as_bytes()), LIMIT);
    assert!(r.ok, "node does not read the names:\n{}", r.both());
    let (unicode, lines) = r.stdout.split_once('\n').unwrap();
    let node = said(lines);
    let counts = hold("Node", &names, ritsu_emit::ident::typescript_takes, &node);
    report(&format!("Node {}", ritsu_testkit::tools::version("node", "--version")), &format!("Unicode {unicode}"), names.len(), counts);
    // tsc: a module that declares every name, a line each, and the lines it finds fault with
    let Some(tsc) = ritsu_testkit::tools::find("TSC", Some(Path::new("tools/node_modules/.bin/tsc")), "tsc", &["--version"]) else {
        ritsu_testkit::skip("tsc is not installed (npm ci --prefix tools); the names kaname writes in TypeScript are not held to it");
        return;
    };
    let work = TempDir::new("kaname-idents-tsc");
    let mut text = String::from("export {};\n");
    for (n, _) in &names {
        writeln!(text, "const {n} = 0;").unwrap();
    }
    let file = work.path().join("names.ts");
    std::fs::write(&file, text).unwrap();
    let r = ritsu_testkit::run(Command::new(&tsc).args(["--noEmit", "--pretty", "false", "--target", "es2022", "--module", "esnext"]).arg(&file).current_dir(work.path()), LIMIT);
    let mut faulted = std::collections::BTreeSet::new();
    for l in r.stdout.lines().chain(r.stderr.lines()) {
        // names.ts(12,7): error TS1127: Invalid character.
        if let Some((_, at)) = l.split_once("names.ts(")
            && let Some((line, _)) = at.split_once(',')
            && let Ok(line) = line.parse::<usize>()
        {
            faulted.insert(line);
        }
    }
    assert!(r.ok == faulted.is_empty(), "tsc fails with no line it finds fault with:\n{}", r.both().lines().take(20).collect::<Vec<_>>().join("\n"));
    // a name is on line i + 2 (the first holds `export {};`). tsc reads names by tables of its
    // own, of a Unicode it does not say: a character its Unicode has not is one it keeps out
    // wherever it stands, first and after the first
    let takes: Vec<bool> = (0..names.len()).map(|i| !faulted.contains(&(i + 2))).collect();
    let mut known = std::collections::BTreeSet::new();
    for ((_, c), t) in names.iter().zip(&takes) {
        if *t {
            known.insert(*c);
        }
    }
    let tsc_said: Vec<Said> = names.iter().zip(&takes).map(|((_, c), t)| Said { takes: *t, knows: known.contains(c) }).collect();
    let counts = hold("tsc", &names, ritsu_emit::ident::typescript_takes, &tsc_said);
    report(&format!("tsc {}", ritsu_testkit::tools::version(&tsc, "--version")), "its own tables", names.len(), counts);
}

/// The program Go runs: whether a name is one of Go's, by the rule of the specification
/// (`go/token.IsIdentifier`, over the `unicode` the compiler reads names by), and whether Go's
/// Unicode has the character.
const GO: &str = r#"package main

import (
	"bufio"
	"fmt"
	"go/token"
	"io"
	"os"
	"strings"
	"unicode"
	"unicode/utf8"
)

func main() {
	in, err := io.ReadAll(os.Stdin)
	if err != nil {
		panic(err)
	}
	w := bufio.NewWriter(os.Stdout)
	defer w.Flush()
	fmt.Fprintln(w, unicode.Version)
	for _, name := range strings.Split(strings.TrimSuffix(string(in), "\n"), "\n") {
		r, _ := utf8.DecodeLastRuneInString(name)
		takes, knows := 0, 0
		if token.IsIdentifier(name) {
			takes = 1
		}
		if unicode.In(r, unicode.L, unicode.M, unicode.N, unicode.P, unicode.S, unicode.Z, unicode.Cc, unicode.Cf, unicode.Co, unicode.Cs) {
			knows = 1
		}
		fmt.Fprintf(w, "%d%d\n", takes, knows)
	}
}
"#;

#[test]
fn go_takes_the_names_of_go_s_table() {
    if !ritsu_testkit::need(Need::Go) {
        return;
    }
    if !ritsu_testkit::tools::runs("go", &["version"]) {
        ritsu_testkit::skip("go is not on the PATH; the names kaname writes in Go are not held to it");
        return;
    }
    let names = names();
    let input: String = names.iter().map(|(n, _)| format!("{n}\n")).collect();
    let work = TempDir::new("kaname-idents-go");
    std::fs::write(work.path().join("main.go"), GO).unwrap();
    let mut c = Command::new("go");
    c.args(["run", "main.go"]).current_dir(work.path()).env("GOWORK", "off").env("GOFLAGS", "-trimpath");
    let r = ritsu_testkit::run::run_with_input(&mut c, Some(input.as_bytes()), LIMIT);
    assert!(r.ok, "go does not read the names:\n{}", r.both());
    let (unicode, lines) = r.stdout.split_once('\n').unwrap();
    let counts = hold("Go", &names, ritsu_emit::ident::go_takes, &said(lines));
    report(&ritsu_testkit::tools::version("go", "version"), &format!("Unicode {unicode}"), names.len(), counts);
}
