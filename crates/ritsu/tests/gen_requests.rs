//! The code `ritsu gen` writes for a project's requests (ringi's DESIGN 12, its PLAN D.4, D.5):
//! each request's lowered rules and dates, the module of kaname that joins it to the machine of its
//! tickets, and the machine, the module `ticket`, in each language's package; and what `--target
//! requests` writes once for the kinds of request, apart from any language. The code answers the
//! vectors of the reference interpreter as it does, in TypeScript, Python, Go and Rust, and the
//! vectors `--target requests` writes are the reference's.
//!
//! The requests are ringi's examples ([`EXAMPLES`]) and its tests of the machine ([`MACHINE`]), with
//! the organization, the masters, the calendars, the books and the rules they read, laid out in a
//! temporary directory as they are in ringi's crate; the two run apart, at once. `ritsu gen` itself
//! holds the modules it lays to kaname's check, with no warning and every function total (ringi's
//! DESIGN 9.2): anything kaname says of them stops it. What is held to the reference is said in one
//! place, [`same`]: a refusal by its kind, the place it names and its words in both languages.
//!
//! The code of a request is checked by kaname through every function the module of its tickets
//! has, which takes about half a minute a request; at the fast level, where the code is not run,
//! a few requests of each stand for the rest ([`EXAMPLES_FAST`], [`MACHINE_FAST`]).

mod common;
mod type_checkers;

use ritsu_base::json::Json;
use ritsu_testkit::{Level, Need, TempDir, ready};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use type_checkers::*;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// How long a program the tests run may take ([`ritsu_testkit::run::finish`]): past it, the program
/// is stopped and the test fails, saying what it printed until then. The longest is the first build
/// of the package's crate of Rust with its dependencies.
const LIMIT: Duration = Duration::from_secs(1800);

/// How long `tsc` and a driver of the generated code may take: a few seconds each now, so that one
/// that hangs is stopped long before [`LIMIT`].
const RUN: Duration = Duration::from_secs(300);

/// The module the Go is built in, and the import path of the package's directory in it.
const MODULE: &str = "gocheck/generated";

/// The URL a server of the tests serves the screens' catalog at, which the screens name it by
/// (ringi's DESIGN 13.2).
const CATALOG: &str = "https://requests.example.com/a2ui/catalog.json";

/// ringi's examples, five kinds of request each in English and in Japanese, from the root of the
/// project laid out ([`lay`]).
const EXAMPLES: &[&str] = &[
    "examples/account.ringi",
    "examples/account.ja.ringi",
    "examples/contract.ringi",
    "examples/contract.ja.ringi",
    "examples/expense.ringi",
    "examples/expense.ja.ringi",
    "examples/leave.ringi",
    "examples/leave.ja.ringi",
    "examples/purchase.ringi",
    "examples/purchase.ja.ringi",
];

/// ringi's tests of the machine that have a scenario to make vectors from.
const MACHINE: &[&str] = &[
    "tests/machine/absent.ringi",
    "tests/machine/ahead.ringi",
    "tests/machine/ahead.ja.ringi",
    "tests/machine/apart.ringi",
    "tests/machine/behind.ringi",
    "tests/machine/behind.ja.ringi",
    "tests/machine/desk.ringi",
    "tests/machine/desk.ja.ringi",
    "tests/machine/flows.ringi",
    "tests/machine/held.ringi",
    "tests/machine/held.ja.ringi",
    "tests/machine/last.ringi",
    "tests/machine/last.ja.ringi",
    "tests/machine/late.ringi",
    "tests/machine/once.ringi",
    "tests/machine/ranges.ringi",
    "tests/machine/reasons.ringi",
    "tests/machine/reminds.ringi",
    "tests/machine/steps.ringi",
    "tests/machine/supplies.ringi",
    "tests/machine/supplies.ja.ringi",
    "tests/machine/top.ringi",
    "tests/machine/waits.ringi",
    "tests/machine/who.ringi",
];

/// The examples whose code is made at the fast level: a request with a calendar, a book and
/// `submitted by`, and one in Japanese with rows, masters and agents.
const EXAMPLES_FAST: &[&str] = &["examples/leave.ringi", "examples/expense.ja.ringi"];

/// The tests of the machine whose code is made at the fast level: conditions, computations and a
/// route that read values left empty, a stage held by a pause, numbers counted in steps of a part of
/// their unit (rates of several steps, cents and sen), fields made empty and dropped as the form no
/// longer shows them, one after another, and the president, whom some stages find nobody above.
const MACHINE_FAST: &[&str] = &["tests/machine/absent.ringi", "tests/machine/held.ringi", "tests/machine/steps.ringi", "tests/machine/supplies.ringi", "tests/machine/top.ringi"];

/// What the examples' code reads beside the files they are lowered to, given to `ritsu gen` with
/// them: the rule purchase's route reads (`use rule authority`), whose code the package holds.
const READ: &[&str] = &["examples/shared/authority.rule"];

/// `all`, or at the fast level `fast`.
fn picked<'a>(all: &'a [&'a str], fast: &'a [&'a str]) -> &'a [&'a str] {
    if ritsu_testkit::level::allows(Level::Tools) { all } else { fast }
}

/// `ritsu`, run in `dir`, with no language asked of the environment.
fn ritsu_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    ritsu_in_with(dir, args, &[])
}

/// `ritsu_in` with environment variables for the child (the build's temporary directory, say).
fn ritsu_in_with(dir: &Path, args: &[&str], envs: &[(&str, &Path)]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(dir).args(args);
    for v in common::LANG_VARS {
        c.env_remove(v);
    }
    for (k, v) in envs {
        c.env(k, v);
    }
    let o = c.output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The requests and what they read, laid out under `t` as they are in ringi's crate: ringi's tests
/// of the machine in `tests/machine/`, the examples and the files they share in `examples/`.
///
/// Each test of the machine is a request of its own, and several name the same kind (`expense`),
/// where a package holds one kind of each name: each is laid out named after its file
/// ([`named_after_file`]), with the proxies of the organization's data that name the kind it is
/// written as naming it too, and its vectors are made from the copy as its code is.
fn lay(t: &TempDir) {
    let ringi = here().join("../ringi");
    for dir in ["tests/machine", "examples/shared", "examples/org", "examples/masters"] {
        ritsu_testkit::tmp::copy_dir(&ringi.join(dir), &t.path().join(dir));
    }
    for e in std::fs::read_dir(ringi.join("examples")).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "ringi") {
            std::fs::copy(&p, t.path().join("examples").join(p.file_name().unwrap())).unwrap();
        }
    }
    let mut tests: Vec<PathBuf> = std::fs::read_dir(t.path().join("tests/machine")).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|x| x == "ringi")).collect();
    tests.sort();
    let renamed: Vec<(String, String)> = tests.iter().map(|p| named_after_file(p)).collect();
    // the `requests` of a row of `proxies.csv` names kinds: a test's name beside the one it is written as
    let proxies = t.path().join("examples/org/proxies.csv");
    let text = std::fs::read_to_string(&proxies).unwrap();
    let mut lines: Vec<String> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let Some((head, kinds)) = line.rsplit_once(',').filter(|_| i > 0) else {
            lines.push(line.to_string());
            continue;
        };
        let mut names: Vec<&str> = kinds.split(' ').collect();
        for (written, laid) in &renamed {
            if names.contains(&written.as_str()) && !names.contains(&laid.as_str()) {
                names.push(laid);
            }
        }
        lines.push(format!("{head},{}", names.join(" ")));
    }
    std::fs::write(&proxies, format!("{}\n", lines.join("\n"))).unwrap();
}

/// The request at `path` named after its file, its first line written again: `last.ja.ringi`'s
/// `request 経費(expense) v1` as `request 経費(last_ja) v1`, `once.ringi`'s `request expense v1` as
/// `request once v1`. Its ASCII name as written, and as laid out.
fn named_after_file(path: &Path) -> (String, String) {
    let src = std::fs::read_to_string(path).unwrap();
    let stem = path.file_name().unwrap().to_string_lossy().trim_end_matches(".ringi").replace('.', "_");
    let (first, rest) = src.split_once('\n').expect("a request has more than one line");
    let mut words = first.splitn(3, ' ');
    let (Some("request"), Some(name), Some(after)) = (words.next(), words.next(), words.next()) else { panic!("{}: {first}", path.display()) };
    let (written, laid) = match name.split_once('(') {
        Some((shown, ascii)) => (ascii.trim_end_matches(')').to_string(), format!("{shown}({stem})")),
        None => (name.to_string(), stem.clone()),
    };
    std::fs::write(path, format!("request {laid} {after}\n{rest}")).unwrap();
    (written, stem)
}

/// The ASCII name of the request at `request` under `root`, as its first line names it (`request
/// expense v1`, `request 経費(expense_ja) v1`): its module's, and its kind's in the package's list.
fn kind_of(root: &Path, request: &str) -> String {
    let src = std::fs::read_to_string(root.join(request)).unwrap();
    let name = src.lines().next().and_then(|l| l.split_whitespace().nth(1)).expect("a request names itself");
    name.split_once('(').map(|(_, a)| a.trim_end_matches(')').to_string()).unwrap_or_else(|| name.to_string())
}

/// What the generated code answers held to what the reference answers (ringi's DESIGN 10.4, its
/// PLAN D.4), the one place that says it: a refusal by its kind (`refused`), the place of the form
/// it names (`at`) and its words in both languages (`text`, `text_ja`), as they are, though a server
/// is held to the vectors by the kind and the place alone (DESIGN 12.4); anything else whole, the
/// words of the history's notes too, an object's keys in any order, a key whose value is null the
/// same as a key that is not there.
fn same(got: &Json, want: &Json) -> bool {
    if let (Some(g), Some(w)) = (got.get("err"), want.get("err")) {
        let at = |j: &Json| j.get("at").filter(|x| !x.is_null()).cloned();
        let words = |j: &Json| (j.get("text").and_then(Json::as_str).unwrap_or("").to_string(), j.get("text_ja").and_then(Json::as_str).unwrap_or("").to_string());
        return g.get("refused") == w.get("refused") && at(g) == at(w) && words(g) == words(w);
    }
    alike(got, want)
}

/// Two values alike, an object's keys in any order, a null the same as nothing.
fn alike(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::Obj(x), Json::Obj(y)) => {
            let x: Vec<&(String, Json)> = x.iter().filter(|(_, v)| !v.is_null()).collect();
            let y: Vec<&(String, Json)> = y.iter().filter(|(_, v)| !v.is_null()).collect();
            x.len() == y.len() && x.iter().all(|(k, v)| y.iter().any(|(k2, v2)| k == k2 && alike(v, v2)))
        }
        (Json::Arr(x), Json::Arr(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| alike(p, q)),
        _ => a == b,
    }
}

/// Vectors whose answers are held to these, by their kind of request and their `from`, beside the
/// four languages holding to them: a computation of a row past its range is a refusal of the form
/// at the row's cell (`at`, the place in ASCII names), said with the request's names of them and the
/// range as the file writes it, numbers grouped and units spelled as written, and the fields of the
/// row it is worked out from, to change.
const PINNED: [(&str, &str, &str); 4] = [
    (
        "purchase",
        "edge of lines.quantity, going up: 13",
        r#"{"err":{"refused":"form","text":"the `lines[0].subtotal` the form works out is outside its range (`>=1JPY <=100_000_000JPY`); change `quantity` or `price` (`lines`)","text_ja":"計算した `lines[0].subtotal` が、範囲（`>=1JPY <=100_000_000JPY`）の外です。`lines` の `quantity` か `price` を直してください","at":"lines[0].subtotal"}}"#,
    ),
    (
        "purchase",
        "edge of lines.price, going up: 8_333_334JPY",
        r#"{"err":{"refused":"form","text":"the `lines[0].subtotal` the form works out is outside its range (`>=1JPY <=100_000_000JPY`); change `quantity` or `price` (`lines`)","text_ja":"計算した `lines[0].subtotal` が、範囲（`>=1JPY <=100_000_000JPY`）の外です。`lines` の `quantity` か `price` を直してください","at":"lines[0].subtotal"}}"#,
    ),
    (
        "purchase_ja",
        "edge of 明細.数量, going up: 13",
        r#"{"err":{"refused":"form","text":"the `明細[0].小計` the form works out is outside its range (`>=1円 <=100_000_000円`); change `数量` or `単価` (`明細`)","text_ja":"計算した `明細[0].小計` が、範囲（`>=1円 <=100_000_000円`）の外です。`明細` の `数量` か `単価` を直してください","at":"lines[0].subtotal"}}"#,
    ),
    (
        "purchase_ja",
        "edge of 明細.単価, going up: 8_333_334円",
        r#"{"err":{"refused":"form","text":"the `明細[0].小計` the form works out is outside its range (`>=1円 <=100_000_000円`); change `数量` or `単価` (`明細`)","text_ja":"計算した `明細[0].小計` が、範囲（`>=1円 <=100_000_000円`）の外です。`明細` の `数量` か `単価` を直してください","at":"lines[0].subtotal"}}"#,
    ),
];

/// The answers of a language's driver held to the calls' ([`same`]): how many agree, and each that
/// does not (the first ten).
fn held(calls: &[Call], stdout: &str) -> (usize, Vec<String>) {
    let lines: Vec<&str> = stdout.lines().collect();
    let mut wrong = Vec::new();
    if lines.len() != calls.len() {
        wrong.push(format!("the driver printed {} lines for {} calls", lines.len(), calls.len()));
    }
    let mut agree = 0;
    for (c, line) in calls.iter().zip(&lines) {
        if ritsu_base::json::parse_nested(line, 4096).ok().is_some_and(|g| same(&g, &c.answer)) {
            agree += 1;
        } else if wrong.len() < 10 {
            wrong.push(format!("{} {}:\n  reference: {}\n  generated: {line}", c.entry, c.line, c.answer.compact()));
        }
    }
    (agree, wrong)
}

/// A vector as a call of an entry of the package's list of the kinds of request: the kind, the
/// entry (`start`, `next` or `worked`), the time (0 for `worked`, which takes none), and what it is
/// given as JSON texts, in the order the entry takes them; with what the reference answers.
struct Call {
    kind: String,
    entry: &'static str,
    now: i64,
    parts: Vec<String>,
    answer: Json,
    /// The vector's line, to say a call that does not agree by.
    line: String,
}

/// The calls as each driver reads them: a line each, its fields between tabs (a compact JSON text
/// holds no tab).
fn call_lines(calls: &[Call]) -> String {
    calls.iter().map(|c| format!("{}\t{}\t{}\t{}\n", c.kind, c.entry, c.now, c.parts.join("\t"))).collect()
}

/// The drivers: a program in each language that calls the package's list of the kinds of request
/// as a server does, a line of [`call_lines`] each, and prints what the entry answers (or the error,
/// as `{"error": …}`). Rust's crate is `generated`, Go's package is at `MODULE`.
const TS_DRIVER: &str = r#"import { readFileSync } from "node:fs";
import { kinds } from "./ticket_kinds";

for (const line of readFileSync(process.argv[2], "utf8").split("\n")) {
  if (line === "") continue;
  const [name, entry, now, ...parts] = line.split("\t");
  const k = kinds.find((x) => x.name === name);
  try {
    if (k === undefined) throw new Error("no such kind");
    if (entry === "start") console.log(k.start(parts[0], parts[1], BigInt(now)));
    else if (entry === "worked") console.log(k.worked(parts[0]));
    else console.log(k.next(parts[0], parts[1], parts[2], BigInt(now)));
  } catch (e) {
    console.log(JSON.stringify({ error: String(e) }));
  }
}
"#;

const PY_DRIVER: &str = r#"import json
import sys

from generated.ticket_kinds import KINDS

with open(sys.argv[1], encoding="utf-8") as f:
    for line in f:
        name, entry, now, *parts = line.rstrip("\n").split("\t")
        try:
            k = next(x for x in KINDS if x.name == name)
            if entry == "start":
                print(k.start(parts[0], parts[1], int(now)))
            elif entry == "worked":
                print(k.worked(parts[0]))
            else:
                print(k.next(parts[0], parts[1], parts[2], int(now)))
        except Exception as e:  # noqa: BLE001
            print(json.dumps({"error": repr(e)}))
"#;

fn go_driver() -> String {
    format!(
        r#"package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"strconv"
	"strings"

	"{MODULE}/ticketkinds"
)

func main() {{
	f, err := os.Open(os.Args[1])
	if err != nil {{
		panic(err)
	}}
	sc := bufio.NewScanner(f)
	sc.Buffer(make([]byte, 1<<20), 1<<28)
	for sc.Scan() {{
		fs := strings.Split(sc.Text(), "\t")
		now, _ := strconv.ParseInt(fs[2], 10, 64)
		var out json.RawMessage
		err := fmt.Errorf("no such kind")
		for _, k := range ticketkinds.Kinds {{
			if k.Name != fs[0] {{
				continue
			}}
			if fs[1] == "start" {{
				out, err = k.Start(json.RawMessage(fs[3]), json.RawMessage(fs[4]), now)
			}} else if fs[1] == "worked" {{
				out, err = k.Worked(json.RawMessage(fs[3]))
			}} else {{
				out, err = k.Next(json.RawMessage(fs[3]), json.RawMessage(fs[4]), json.RawMessage(fs[5]), now)
			}}
		}}
		if err != nil {{
			b, _ := json.Marshal(map[string]string{{"error": err.Error()}})
			fmt.Println(string(b))
			continue
		}}
		fmt.Println(string(out))
	}}
}}
"#
    )
}

const RUST_DRIVER: &str = r#"fn main() {
    let path = std::env::args().nth(1).expect("the file of the calls");
    let text = std::fs::read_to_string(path).expect("the file of the calls");
    for line in text.lines() {
        let fs: Vec<&str> = line.split('\t').collect();
        let now: i64 = fs[2].parse().expect("the time");
        let answer = match generated::ticket_kinds::KINDS.iter().find(|k| k.name == fs[0]) {
            None => Err(generated::ticket_kinds::EntryError { path: None, message: "no such kind".to_string() }),
            Some(k) if fs[1] == "start" => (k.start)(fs[3], fs[4], now),
            Some(k) if fs[1] == "worked" => (k.worked)(fs[3]),
            Some(k) => (k.next)(fs[3], fs[4], fs[5], now),
        };
        match answer {
            Ok(a) => println!("{a}"),
            Err(e) => println!("{{\"error\":{:?}}}", e.message),
        }
    }
}
"#;

/// The code `ritsu gen` writes for ringi's examples answers every vector of the reference
/// interpreter as the reference does, in TypeScript, Python, Go and Rust (ringi's PLAN D.4, D.5):
/// the same ticket and the same things to do, or a refusal of the same kind.
#[test]
fn the_code_of_the_examples_answers_as_the_reference_does() {
    let t = TempDir::new("gen-requests-examples");
    lay(&t);
    // the route of a request applies the rule it reads (`use rule`, ringi's DESIGN 6.4), and the code
    // of a rule holds what it applies, as rulec writes it: the package is made without the rule
    let o = t.path().join("out").to_string_lossy().into_owned();
    let (code, _, err) = ritsu_in(t.path(), &["gen", "examples/purchase.ringi", "--root", ".", "--out", &o, "--target", "python"]);
    assert_eq!(code, 0, "{err}");
    let route = std::fs::read_to_string(t.path().join("out/python/generated/rules/purchase_route.py")).unwrap();
    assert!(route.contains("# Applies: authority = ") && route.contains("# table authority:authority"), "{route}");
    std::fs::remove_dir_all(t.path().join("out")).unwrap();
    // a request whose code reads a rule the run is not given stops it, said of the request: the
    // route of the Japanese version reads it through the glue, as rulec does not take an amount in
    // 円 for the rule's JPY (ringi's PLAN 27)
    let (code, _, err) = ritsu_in(t.path(), &["gen", "examples/purchase.ja.ringi", "--root", ".", "--out", &o, "--target", "python"]);
    assert_eq!(
        (code, err.as_str()),
        (1, "error: `examples/purchase.ja.ringi` reads examples/shared/authority.rule, which is not one of the project's files; a package is made of the project's files, so give ritsu gen that file too\n")
    );
    assert!(!t.path().join("out").exists(), "nothing is written when the package is refused");
    answers_as_the_reference(&t, "examples", picked(EXAMPLES, EXAMPLES_FAST), READ);
}

/// The same of ringi's tests of the machine: the paths of a ticket the examples do not take.
#[test]
fn the_code_of_the_tests_of_the_machine_answers_as_the_reference_does() {
    let t = TempDir::new("gen-requests-machine");
    lay(&t);
    answers_as_the_reference(&t, "machine", picked(MACHINE, MACHINE_FAST), &[]);
}

/// A ticket passes, in one move, every stage before the one that waits, however many there are
/// (ringi's DESIGN 7.3): here seventy that only circulate, and the code answers as the reference
/// does.
#[test]
fn a_route_of_many_stages_is_walked_in_one_move() {
    let t = TempDir::new("gen-requests-long");
    lay(&t);
    std::fs::write(t.path().join("examples/long.ringi"), long_request(70)).unwrap();
    answers_as_the_reference(&t, "long", &["examples/long.ringi"], &[]);
}

/// A request named with a word kaname reserves (`match`) and written with words the languages it is
/// lowered to read as something else (ringi's DESIGN 6.4): a choice named as a type of rulec's
/// (`date`) whose values a cell of rulec's reads otherwise (`after`, `by`), rows named `count` with a
/// field `max`, fields named as the glue's own names (`g`, `form`, `org`, `y`), a way to choose an
/// approver named `after` and a table named `up`. Beside it, a request whose names, joined into the
/// names the lowered files and the glue declare, come to the same names ([`CLASH`]), one whose
/// calendars and rules come to the names the glue declares for a calendar's days ([`DAYS`]), and one
/// named in capitals, with a calendar, fields and a stage named so, which the lowered dates files
/// take in small letters ([`CAPS`]), one whose names start with `_`, end with `_`, or have two `_`
/// in a row ([`UNDER`]), and one whose calendars' files would be one module or one package of the
/// generated code ([`CALS`]). Each is lowered under another name; the code answers as the
/// reference does in every language, and what `--target requests` writes says none of the names
/// given in their place, but for the functions of the screens, which call each kind's values.rule
/// by them (a choice's values too).
#[test]
fn names_the_languages_read_otherwise_are_lowered_under_others() {
    let t = TempDir::new("gen-requests-words");
    lay(&t);
    std::fs::write(t.path().join("examples/match.ringi"), WORDS).unwrap();
    std::fs::write(t.path().join("examples/clash.ringi"), CLASH).unwrap();
    std::fs::write(t.path().join("examples/days.ringi"), DAYS).unwrap();
    std::fs::write(t.path().join("examples/caps.ringi"), CAPS).unwrap();
    std::fs::write(t.path().join("examples/under.ringi"), UNDER).unwrap();
    std::fs::write(t.path().join("examples/cals.ringi"), CALS).unwrap();
    let requests = ["examples/match.ringi", "examples/clash.ringi", "examples/days.ringi", "examples/caps.ringi", "examples/under.ringi", "examples/cals.ringi"];
    answers_as_the_reference(&t, "words", &requests, READ);
    // each entry of the kind named in capitals in Rust's list (`start`, `next` and `worked`) says
    // it is named so, which Rust would warn of (`non_snake_case`), and no other kind's do
    let kinds = std::fs::read_to_string(t.path().join("out/rust/ticket_kinds.rs")).unwrap();
    let entries = ["start", "next", "worked"];
    for entry in entries {
        assert!(kinds.contains(&format!("#[allow(non_snake_case)]\nfn {entry}_capsDays(")), "{kinds}");
    }
    assert_eq!(kinds.matches("#[allow(non_snake_case)]").count(), entries.len(), "{kinds}");
    let dir = t.path().join("out/requests");
    let written = files_under(&dir);
    assert!(!written.is_empty(), "--target requests writes nothing");
    let given_in_place = [
        "match_value", "date_value", "after_value", "by_value", "count_value", "max_value", "g_value", "form_value", "org_value", "y_value", "after_approver", "up_value",
        // of `CLASH`
        "check_by_value", "option_value", "fired_value", "rule_error_value", "xL_value", "check_approver2", "headOffice_approver", "val_x_y_value", "cell_a_b_c_value", "row_vals_x_value",
        // of `DAYS`
        "open_after_value", "open_office_value", "opened_office_value", "terms_1_value", "open_1_value",
        // of `CAPS`
        "capsdays", "firstday", "ndays", "backon", "firstb_due", "mainoffice",
    ];
    // of `UNDER`, each as a word of its own: `\n_low` in a JSON string is the value `_low` after a
    // line's end
    let given_as_words = [
        "n_level", "n_low", "n_twice", "n_items", "n_points", "n_remark_", "n_remark__shown", "n_boss", "n_head", "n_climb", "n_third", "n_auth_finance", "start_on", "check_two", "n_span",
    ];
    let says = |text: &str, name: &str| {
        text.match_indices(name).any(|(i, _)| {
            let before = text[..i].chars().next_back().is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == '\\'));
            let after = text[i + name.len()..].chars().next().is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'));
            before && after
        })
    };
    // the functions of the screens hold each kind's values.rule as rulec writes it, in the names the
    // lowered files give, and call it by them; what the screens, their catalog and the rest say is
    // in the request's names (the module of `--extra wasm`, which holds the code, is no text)
    for f in written.iter().filter(|f| *f != "a2ui/functions.js" && *f != "kinds.wasm") {
        let mut text = std::fs::read_to_string(dir.join(f)).unwrap();
        // but the name the door knows each kind's module by, which is the lowered files' (`match`
        // is `match_value`): what a server calls, beside the kind's own name
        if f == "wasm.json" {
            let mut about = ritsu_base::json::parse(&text).unwrap();
            if let Json::Obj(fields) = &mut about
                && let Some((_, Json::Arr(kinds))) = fields.iter_mut().find(|(k, _)| k == "kinds")
            {
                for k in kinds.iter_mut() {
                    if let Json::Obj(kf) = k {
                        kf.retain(|(key, _)| key != "module");
                    }
                }
            }
            text = about.pretty();
        }
        for given in given_in_place {
            assert!(!text.contains(given), "requests/{f} says `{given}`, a name the lowered files give in place of the request's");
        }
        for given in given_as_words {
            assert!(!says(&text, given), "requests/{f} says `{given}`, a name the lowered files give in place of the request's");
        }
    }
    // the choice `date` is given to the rule by the names it writes `after` and `by` under
    let functions = std::fs::read_to_string(dir.join("a2ui/functions.js")).unwrap();
    for named in [r#""after": "after_value""#, r#""by": "by_value""#] {
        assert!(functions.contains(named), "{named} is not in requests/a2ui/functions.js");
    }
    // and the fields and values of `UNDER` that start with `_` by the names with `n` before them: a
    // computed field, a value of a choice, and the output that says whether a field is shown
    for named in [r#""_twice": "n_twice""#, r#""_low": "n_low""#, r#""shown": "n_remark__shown""#] {
        assert!(functions.contains(named), "{named} is not in requests/a2ui/functions.js");
    }
}

/// The request of [`names_the_languages_read_otherwise_are_lowered_under_others`].
const WORDS: &str = r#"request match v1 "Words the languages read otherwise"
description "A request named and written with words the languages it is lowered to read as something else (fictional rules)"

use organization people from "shared/company.ringi"
use calendar office from "shared/office.cal"

form
  date  : choice "Kind of day"
    after "After"
    by    "By"
    usual "Usual"
  over  : number "Nights"  range >=1 <=10
  count : rows "Items"  count >=1 <=5
    max : money[JPY] "Price"  range >=1JPY <=1_000JPY
    y   : text "What"  count <=50
  total : money[JPY] "Total" = sum(count.max)  range >=1JPY <=5_000JPY
  g     : text "Destination"  count <=100
  form  : text "Note"  count <=100  shown when date is usual
  org   : number "Seats"  range >=0 <=9  required when total > 1_000JPY

route
| date      | total      | -> check | decide |
| after, by | -          | finance  | after  |
| usual     | <=1_000JPY | finance  |        |
| usual     | >1_000JPY  | finance  | after  |

who
  finance = team accounting
  after   = manager of requester
  top     = head of requester.division

every stage
  due in 3 business days at end of day
  when late escalate by up, then expire
  when empty escalate, then skip

table up
| approver | -> goes_to |
| finance  | top        |
| after    | top        |
| top      | top        |

ticket
  self approval  not allowed
  same approver approves once

examples
| date  | total    | -> check | decide |
| by    | 100JPY   | finance  | after  |
| usual | 2_000JPY | finance  | after  |

scenario a_day_after
  requester e1201
  form date = after, over = 2, count = [{max: 500JPY, y: "Pen"}], g = "Osaka"
| at               | event   | by    | -> stage | status      |
| 2026-11-16 09:00 | submit  | e1201 | check    | in progress |
| 2026-11-16 10:00 | approve | e1001 | decide   | in progress |
| 2026-11-16 11:00 | approve | e1101 |          | approved    |
"#;

/// The other request of [`names_the_languages_read_otherwise_are_lowered_under_others`], whose names
/// joined into others come to names it or the languages already have (ringi's DESIGN 6.4): a choice
/// named as the enum of a stage's output (`check_by`, the enum of `check`'s); choices named as types
/// the code rulec generates declares or reads (`option`, `fired`, `rule_error`), which the form's
/// `shown when` reads; two values of a choice and two ways to choose an approver that are one name
/// in Pascal case (`x_l` and `xL`, `head_office` and `headOffice`); a way named as a stage (`check`)
/// beside one named with `_approver` after it; and fields and rows whose names make one name of the
/// glue (`x_y` and the field `y` of the rows `x` are both `val_x_y`, the field `b_c` of `a` and the
/// field `c` of `a_b` are both `cell_a_b_c`, and the rows `x` and `vals_x` are both `row_vals_x`).
const CLASH: &str = r#"request clash v1 "Names made of two names"
description "A request whose names, joined into the names its lowered files and its code declare, come to the same names (fictional rules)"

use organization people from "shared/company.ringi"
use calendar office from "shared/office.cal"

form
  check_by   : choice "Checked by"
    desk   "Desk"
    post   "Post"
    portal "Portal"
  option     : choice "Option"
    plain "Plain"
    rush  "Rush"
  fired      : choice "Fired"
    no  "No"
    yes "Yes"
  rule_error : choice "Rule error"
    clean "Clean"
    seen  "Seen"
  fit        : choice "Fit"
    x_l "XL"
    xL  "xL"
  x_y    : text "Note on the items"  count <=40  optional
  x      : rows "Items"  count >=1 <=3
    y : text "What"  count <=40  optional
    p : number "How many"  range >=1 <=9
  a      : rows "Parts"  count >=1 <=3
    b_c : number "Count"  range >=1 <=9
  a_b    : rows "Boxes"  count >=1 <=3
    c : number "Width"  range >=1 <=9
  vals_x : rows "Values"  count >=1 <=3
    q : number "Weight"  range >=1 <=9
  ok     : rows "Checks"  count >=1 <=3
    n : number "Points"  range >=1 <=9
  total  : number "Points in all" = sum(ok.n)  range >=1 <=99
  note   : text "Note"  count <=40  optional  shown when option is rush and fired is yes and rule_error is seen and fit is xL

route
| check_by | -> check | decide         |
| desk     | check    | head_office    |
| post     | manager  | check_approver |
| portal   | manager  | headOffice     |

who
  check          = team accounting
  check_approver = team finance
  manager        = manager of requester
  head_office    = team general_affairs
  headOffice     = team legal

every stage
  due in 3 business days at end of day
  when late escalate, then expire
  when empty escalate, then skip

ticket
  self approval  not allowed
  same approver approves once

examples
| check_by | -> check | decide      |
| desk     | check    | head_office |
| portal   | manager  | headOffice  |

scenario two_names_in_one
  requester e1201
  form check_by = desk, option = plain, fired = no, rule_error = clean, fit = x_l, x = [{p: 1}], a = [{b_c: 1}], a_b = [{c: 2}], vals_x = [{q: 3}], ok = [{n: 4}]
| at               | event   | by    | -> stage | status      |
| 2026-11-16 09:00 | submit  | e1201 | check    | in progress |
| 2026-11-16 10:00 | approve | e1001 | decide   | in progress |
| 2026-11-16 11:00 | approve | e5001 |          | approved    |
"#;

/// A request whose calendars and rules, joined into the names the glue declares for a calendar's
/// days, come to the same names: a calendar named `after` (whose business days would be counted by
/// `open_after`, the glue's own function), rules read by `open_office` and `opened_office` (the
/// names of the function and of the file of the open days of the calendar `office`), and by
/// `terms_1` and `open_1` (the files of the deadlines and of the business days of the calendar read
/// second). The amount the rules read may be left empty, so the route reads their outputs as inputs
/// the glue gives it (`fed by`), not as values it works out with `apply` (ringi's DESIGN 6.4).
const DAYS: &str = r#"request days v1 "Names made of a calendar's name"
description "A request whose calendars and rules, joined into the names its code declares for a calendar's days, come to the same names (fictional rules)"

use organization people from "shared/company.ringi"
use calendar office from "shared/office.cal"
use calendar after from "shared/office.cal"
use rule open_office from "shared/authority.rule"
use rule opened_office from "shared/authority.rule"
use rule terms_1 from "shared/authority.rule"
use rule open_1 from "shared/authority.rule"

form
  kind   : choice from open_office.kind "Kind"
    goods    "Goods and services"
    capex    "Capital expenditure"
    software "Software"
  amount : money[JPY] "Amount"  range >=1JPY <=100_000_000JPY  optional
  start  : date "Start"  range >=2026-04-01 <=2027-03-31  open in office

route
  first    manager
  finance  manager  when open_office.finance
  board    manager  when opened_office.board
  check    manager  when terms_1.finance
    due in 2 business days in after at end of day
    when late expire
  last     manager  when open_1.board

who
  manager = manager of requester

every stage
  due in 3 business days in office at end of day
  when late escalate, then expire
  when empty escalate, then skip

ticket
  self approval  not allowed
  send back      to the requester, at most 2 times
  resubmit       within 5 business days in after
  pause          at most 5 business days in office

scenario approved_stage_by_stage
  requester e1201
  form kind = goods, amount = 2_000_000JPY, start = 2026-11-16
| at               | event   | by    | -> stage | status      |
| 2026-11-16 10:00 | submit  | e1201 | first    | in progress |
| 2026-11-16 16:00 | approve | e1101 | finance  | in progress |
| 2026-11-17 10:00 | approve | e1101 | check    | in progress |
| 2026-11-17 16:00 | approve | e1101 |          | approved    |
"#;

/// A request named in capitals, with a calendar, fields and a stage named so: the names of its
/// lowered dates files, of their inputs and of their dates, which koyomi takes only in small letters.
const CAPS: &str = r#"request capsDays v1 "Names in capitals"
description "A request whose names, in capitals, make the names of its lowered dates files, of their inputs and of their dates (fictional rules)"

use organization people from "shared/company.ringi"
use calendar mainOffice from "shared/office.cal"

form
  firstDay : date "First day"  range >=2026-04-01 <=2027-03-31  open in mainOffice
  nDays    : number "Business days"  range >=1 <=20
  backOn   : date "Back on" = firstDay + nDays business days in mainOffice

route
  firstB   manager
    due in 2 business days at end of day
    when late expire
  secondB  manager  when nDays > 5

who
  manager = manager of requester

every stage
  due in 3 business days at end of day
  when late escalate, then expire
  when empty escalate, then skip

ticket
  self approval  not allowed
  pause          at most 5 business days

scenario approved_stage_by_stage
  requester e1201
  form firstDay = 2026-11-16, nDays = 8
| at               | event   | by    | -> stage | status      |
| 2026-11-16 10:00 | submit  | e1201 | firstB   | in progress |
| 2026-11-16 16:00 | approve | e1101 | secondB  | in progress |
| 2026-11-17 10:00 | approve | e1101 |          | approved    |
"#;

/// A request whose names start with `_`, end with `_`, or have two `_` in a row (ringi's DESIGN
/// 6.4): a choice and its values, computed fields, rows and their fields, dates, a field shown on a
/// condition, stages, ways to choose an approver, a named table and the alias of a rule. The code
/// rulec generates for Python holds a rule's outputs and an element's fields in a `NamedTuple`,
/// which takes no field that starts with `_`, and rustc warns of two `_` in a row in the names of
/// the code koyomi generates: the lowered files name them otherwise, and the request's code and
/// JSON keep the names it writes.
const UNDER: &str = r#"request under v1 "Names with underscores"
description "A request whose names start with `_`, end with `_` or have two `_` in a row, which the code generated in some languages takes only under other names (fictional rules)"

use organization people from "shared/company.ringi"
use calendar office from "shared/office.cal"
use rule _auth from "shared/authority.rule"

form
  _level      : choice "Level"
    _low     "Low"
    high_    "High"
    mid__dle "Middle"
  kind        : choice from _auth.kind "Kind of purchase"
    goods    "Goods and services"
    capex    "Capital expenditure"
    software "Software"
  amount      : money[JPY] "Amount"  range >=1JPY <=100_000_000JPY
  _count      : number "Count"  range >=1 <=9
  _twice      : number "Twice" = _count * 2  range >=2 <=18
  _next       : number "Next" = _count + 1  range >=2 <=10
  _items      : rows "Items"  count >=1 <=5
    _points : number "Points"  range >=1 <=9
    note_   : text "Note"  count <=10  optional
  _sum        : number "Points in all" = sum(_items._points)  range >=1 <=45
  start__on   : date "Start"  range >=2026-04-01 <=2027-03-31  open in office
  _span       : number "Business days"  range >=1 <=20
  back_on_    : date "Back on" = start__on + _span business days in office
  _remark_    : text "Remark"  count <=10  optional  shown when _level is _low
  other__note : text "Other note"  count <=10  optional

route
  first       _boss
  check__two  _boss  when _twice > 10
    due in 2 business days at end of day
    when late expire
  _third      _boss  when _sum > 5
    due in 4 business days at 17:00
    when late expire
  finance     _boss  when _auth.finance

who
  _boss = manager of requester
  _head = head of requester.division

every stage
  due in 3 business days at end of day
  when late escalate by _climb, then expire
  when empty escalate, then skip

table _climb
| approver | -> goes_to |
| _boss    | _head      |
| _head    | _head      |

ticket
  self approval  not allowed
  pause          at most 5 business days

scenario approved_stage_by_stage
  requester e1201
  form _level = _low, kind = goods, amount = 2_000_000JPY, _count = 6, _items = [{_points: 3}, {_points: 4, note_: "two"}], start__on = 2026-11-16, _span = 2, _remark_ = "low", other__note = "x"
| at               | event   | by    | -> stage   | status      |
| 2026-11-16 10:00 | submit  | e1201 | first      | in progress |
| 2026-11-16 16:00 | approve | e1101 | check__two | in progress |
| 2026-11-17 10:00 | approve | e1101 | _third     | in progress |
| 2026-11-17 16:00 | approve | e1101 | finance    | in progress |
| 2026-11-18 10:00 | approve | e1101 |            | approved    |
"#;

/// A request whose calendars are named so that the files of their days, in the code generated from
/// them, would be one module or one package (ringi's DESIGN 6.4): `ab`, `aB` and `a_b` (one name in
/// small letters with no `_`), `x` and `ed_x` (`opened_x` and `open_ed_x` are one package of
/// Go's), one that starts with `_`, one that ends with `_`, and one with two `_` in a row. Each is
/// a calendar a date of the form is open in, a deadline is counted in, or the business days of a
/// pause, of the requester's turn or of a computed date are.
const CALS: &str = r#"request cals v1 "Calendars named alike"
description "A request whose calendars are named so that the files of their days would be one module or one package of the generated code (fictional rules)"

use organization people from "shared/company.ringi"
use calendar ab from "shared/office.cal"
use calendar aB from "shared/office.cal"
use calendar a_b from "shared/office.cal"
use calendar x from "shared/office.cal"
use calendar ed_x from "shared/office.cal"
use calendar _office from "shared/office.cal"
use calendar office_ from "shared/office.cal"
use calendar c__d from "shared/office.cal"

form
  day1 : date "Day 1"  range >=2026-04-01 <=2027-03-31  open in ab
  day2 : date "Day 2"  range >=2026-04-01 <=2027-03-31  open in aB
  day3 : date "Day 3"  range >=2026-04-01 <=2027-03-31  open in a_b
  day4 : date "Day 4"  range >=2026-04-01 <=2027-03-31  open in x
  day5 : date "Day 5"  range >=2026-04-01 <=2027-03-31  open in _office
  day6 : date "Day 6"  range >=2026-04-01 <=2027-03-31  open in c__d
  span : number "Business days"  range >=1 <=20
  back : date "Back" = day1 + span business days in office_

route
  first   manager
  second  manager  when day1 > 2026-12-01
    due in 2 business days in aB at end of day
    when late expire
  third   manager  when day2 > 2026-12-01
    due in 2 business days in a_b at end of day
    when late expire
  fourth  manager  when day3 > 2026-12-01
    due in 2 business days in office_ at end of day
    when late expire
  fifth   manager  when day4 > 2026-12-01
    due in 2 business days in _office at end of day
    when late expire

who
  manager = manager of requester

every stage
  due in 3 business days in ab at end of day
  when late escalate, then expire
  when empty escalate, then skip

ticket
  self approval  not allowed
  send back      to the requester, at most 2 times
  pause          at most 5 business days in x
  resubmit       within 5 business days in ed_x

scenario approved_stage_by_stage
  requester e1201
  form day1 = 2026-12-02, day2 = 2026-12-02, day3 = 2026-11-16, day4 = 2026-11-16, day5 = 2026-11-16, day6 = 2026-11-16, span = 3
| at               | event   | by    | -> stage | status      |
| 2026-11-16 10:00 | submit  | e1201 | first    | in progress |
| 2026-11-16 16:00 | approve | e1101 | second   | in progress |
| 2026-11-17 10:00 | approve | e1101 | third    | in progress |
| 2026-11-17 16:00 | approve | e1101 |          | approved    |
"#;

/// A request whose route has `n` stages that only circulate, then one the requester's manager
/// approves, with a scenario that submits it and approves it.
fn long_request(n: usize) -> String {
    let stages: String = (1..=n).map(|i| format!("  c{i:02}  circulate to operations\n")).collect();
    format!(
        r#"request long v1 "Long request"
description "A route of many stages that only circulate, then the requester's manager (fictional rules)"

use organization org from "shared/company.ringi"
use calendar office from "shared/office.cal"

form
  what : text "What"  count >=1 <=100

route
{stages}  decide  manager
    when empty escalate, then skip

who
  manager    = manager of requester
  operations = team it_operations

every stage
  due in 3 business days at end of day
  when late escalate, then expire

ticket
  self approval  not allowed

scenario long_walk
  requester e1201
  form what = "A chair"
| at               | event   | by    | -> stage | status      |
| 2026-11-16 10:00 | submit  | e1201 | decide   | in progress |
| 2026-11-16 11:00 | approve | e1101 |          | approved    |
"#
    )
}

/// The packages of `requests` under `t` (with the files of `read`), in the four languages and
/// `--target requests`, held to the reference: the vectors `--target requests` writes are
/// `ritsu ringi vectors`', and each language's code answers them as the reference does, its
/// refusals in the reference's words. `tag` keeps the drivers of Rust of one group from another's.
fn answers_as_the_reference(t: &TempDir, tag: &str, requests: &[&str], read: &[&str]) {
    let out = t.path().join("out");
    let o = out.to_string_lossy().into_owned();
    let mut args = vec!["gen"];
    args.extend(requests);
    args.extend(read);
    args.extend(["--root", ".", "--out", &o, "--module", MODULE, "--catalog", CATALOG]);
    for target in ["typescript", "python", "go", "rust", "requests"] {
        args.extend(["--target", target]);
    }
    // the package of Rust as one module of Wasm too (ringi's DESIGN 12.6), each vector through it
    let wasm = wasm_ready();
    if wasm {
        args.extend(["--extra", "wasm"]);
    }
    let (code, stdout, err) = ritsu_in(t.path(), &args);
    assert_eq!(code, 0, "{stdout}{err}");
    assert!(!stdout.contains("is not written"), "a request's code is not written:\n{stdout}");
    assert!(out.join("typescript/glue/ticket.ts").is_file(), "the machine is not written:\n{stdout}");
    let mut calls = Vec::new();
    for request in requests {
        let kind = kind_of(t.path(), request);
        // a package of Go is named with no `_` (`expenseja`)
        let go = kind.replace('_', "");
        for f in [format!("typescript/glue/{kind}.ts"), format!("python/generated/glue/{kind}.py"), format!("go/glue/{go}/{go}.go"), format!("rust/glue/{kind}.rs"), format!("requests/types/{kind}.json")] {
            assert!(out.join(&f).is_file(), "{f} is not written:\n{stdout}");
        }
        // the vectors of the reference interpreter, as `--target requests` writes them and as the
        // code takes and answers them
        let (code, vectors, err) = ritsu_in(t.path(), &["ringi", "vectors", request]);
        assert_eq!(code, 0, "{err}");
        let written = std::fs::read_to_string(out.join(format!("requests/vectors/{kind}.jsonl"))).unwrap();
        assert!(written == vectors, "the vectors --target requests writes of {request} are not those of `ritsu ringi vectors`");
        let before = calls.len();
        let mut pinned: Vec<&str> = PINNED.iter().filter(|p| p.0 == kind).map(|p| p.1).collect();
        for line in vectors.lines() {
            let v = ritsu_base::json::parse(line).expect("a vector is a line of JSON");
            let (entry, input, answer) = ringi::lower::shape::call_of(&v).expect("a vector");
            let from = v.get("from").and_then(Json::as_str).unwrap_or_default();
            if let Some((.., want)) = PINNED.iter().find(|p| p.0 == kind && p.1 == from) {
                assert!(same(&answer, &ritsu_base::json::parse(want).unwrap()), "{kind}, {from}: the reference answers\n  {}\nnot\n  {want}", answer.compact());
                pinned.retain(|p| *p != from);
            }
            let names: &[&str] = if entry == "start" { &["submit", "organization"] } else { &["ticket", "event", "organization"] };
            let parts = names.iter().map(|n| input.get(n).cloned().unwrap_or(Json::Null).compact()).collect();
            let now = input.get("now").and_then(Json::as_int).and_then(|n| i64::try_from(n).ok()).expect("a vector has its time");
            calls.push(Call { kind: kind.clone(), entry, now, parts, answer, line: line.chars().take(300).collect() });
        }
        assert!(calls.len() > before, "{request} has no vector");
        assert!(pinned.is_empty(), "{kind} has no vector {pinned:?}");
        // what the form works out (ringi's DESIGN 13.7): the vectors of the entry `worked`, the
        // reference's answers as `--target requests` writes them
        let worked = std::fs::read_to_string(out.join(format!("requests/vectors/worked/{kind}.jsonl"))).unwrap();
        let before = calls.len();
        for line in worked.lines() {
            let v = ritsu_base::json::parse(line).expect("a vector of `worked` is a line of JSON");
            let (Some(input), Some(answer)) = (v.get("input"), v.get("answer")) else { panic!("{kind}: a vector of `worked` with no input or no answer: {line}") };
            calls.push(Call { kind: kind.clone(), entry: "worked", now: 0, parts: vec![input.compact()], answer: answer.clone(), line: line.chars().take(300).collect() });
        }
        assert!(calls.len() > before, "{request} has no vector of `worked`");
    }
    let n = calls.len();
    let report = |target: &str, stdout: &str| {
        let (agree, wrong) = held(&calls, stdout);
        assert!(wrong.is_empty(), "the {target} answers otherwise than the reference:\n{}", wrong.join("\n"));
        println!("{target}: {agree} of {n} vectors of {} requests agree with the reference through the list of the kinds of request, their refusals in the reference's words", requests.len());
    };
    let lines = call_lines(&calls);
    let tools = workspace().join("crates/kaname/tools");
    let tsc = tools.join("node_modules/.bin/tsc");
    if ritsu_testkit::need(Need::Node) {
        if tsc.exists() {
            let dir = out.join("typescript");
            std::fs::write(dir.join("driver.ts"), TS_DRIVER).unwrap();
            std::fs::write(dir.join("calls.tsv"), &lines).unwrap();
            let tsconfig = serde_json::json!({
                "compilerOptions": {
                    "strict": true, "erasableSyntaxOnly": true, "module": "commonjs", "target": "es2022", "rootDir": ".", "outDir": "dist",
                    "skipLibCheck": true, "types": ["node"], "typeRoots": [tools.join("node_modules/@types")]
                },
                "include": ["**/*.ts"],
                "exclude": ["dist", "node_modules"]
            });
            std::fs::write(dir.join("tsconfig.json"), serde_json::to_string_pretty(&tsconfig).unwrap()).unwrap();
            let o = ritsu_testkit::run::finish(Command::new(&tsc).arg("-p").arg("tsconfig.json").current_dir(&dir), RUN, "tsc");
            assert!(o.ok, "tsc:\n{}", o.both());
            type_check_typescript_rules(&dir, &tsc);
            let o = ritsu_testkit::run::finish(Command::new("node").arg("dist/driver.js").arg("calls.tsv").current_dir(&dir), RUN, "node, running the driver of TypeScript");
            assert!(o.ok, "node:\n{}", o.both());
            report("TypeScript", &o.stdout);
        } else {
            ritsu_testkit::skip("tsc is not installed in crates/kaname/tools (npm ci --prefix crates/kaname/tools); the TypeScript of the request is not run");
        }
    }
    if ritsu_testkit::need(Need::Python) {
        match mypy() {
            Some((mypy, python, tigerbeetle)) => {
                type_check_python(&out.join("python"), &mypy, &python, std::slice::from_ref(&tigerbeetle));
                println!("mypy --strict passes the Python of the package");
            }
            None => ritsu_testkit::skip("mypy, Temporal's Python SDK (crates/dandori/tools/temporal-python) or tigerbeetle (crates/chobo/tools/runner) is not installed; the Python of the request is not type-checked"),
        }
    }
    if ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 is not installed; the Python of the request is not run") {
        let dir = out.join("python");
        std::fs::write(dir.join("driver.py"), PY_DRIVER).unwrap();
        std::fs::write(dir.join("calls.tsv"), &lines).unwrap();
        let o = ritsu_testkit::run::finish(Command::new("python3").arg("driver.py").arg("calls.tsv").current_dir(&dir), RUN, "python3, running the driver of Python");
        assert!(o.ok, "python3:\n{}", o.both());
        report("Python", &o.stdout);
    }
    if ready(Need::Go, have_go, "go is not installed; the Go of the request is not run") {
        match go_module() {
            Some(gomod) => {
                type_check_go(&t.path().join("go-vet"), &out.join("go"), &gomod);
                println!("go vet and gofmt pass the Go of the package");
                let dir = t.path().join("go-requests");
                go_module_in(&dir, &out.join("go"), &gomod);
                std::fs::create_dir_all(dir.join("driver")).unwrap();
                std::fs::write(dir.join("driver/main.go"), go_driver()).unwrap();
                std::fs::write(dir.join("calls.tsv"), &lines).unwrap();
                let bin = dir.join("driver.bin");
                let o = ritsu_testkit::run::finish(go_in(&dir).args(["build", "-o"]).arg(&bin).arg("./driver"), LIMIT, "go build");
                assert!(o.ok, "go build:\n{}", o.both());
                let o = ritsu_testkit::run::finish(Command::new(&bin).arg("calls.tsv").current_dir(&dir), RUN, "the driver of Go");
                assert!(o.ok, "the driver:\n{}", o.both());
                report("Go", &o.stdout);
            }
            None => ritsu_testkit::skip("the go.mod of crates/dandori/tools/temporal-go or crates/chobo/tools/runner/go is missing; the Go of the request is not run"),
        }
    }
    if ritsu_testkit::need(Need::Rustc) {
        // the package's crate, with a driver beside it
        let dir = out.join("rust");
        rust_crate(&dir, &format!("requests-{tag}"));
        rust_lints(&dir, "ritsu-rust");
        println!("the package builds as a crate of Rust with -D warnings");
        let bin = format!("driver_requests_{tag}");
        let toml = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        std::fs::write(dir.join("Cargo.toml"), format!("{toml}\n[[bin]]\nname = \"{bin}\"\npath = \"driver/main.rs\"\n")).unwrap();
        std::fs::create_dir_all(dir.join("driver")).unwrap();
        std::fs::write(dir.join("driver/main.rs"), RUST_DRIVER).unwrap();
        std::fs::write(dir.join("calls.tsv"), &lines).unwrap();
        let o = ritsu_testkit::run::finish(cargo_in(&dir, "ritsu-rust").args(["build", "--offline", "--quiet", "--bin", &bin]), LIMIT, "cargo build of the driver of Rust");
        assert!(o.ok, "cargo build of the driver:\n{}", o.both());
        let o = ritsu_testkit::run::finish(Command::new(rust_target("ritsu-rust").join("debug").join(&bin)).arg("calls.tsv").current_dir(&dir), RUN, "the driver of Rust");
        assert!(o.ok, "the driver:\n{}", o.both());
        report("Rust", &o.stdout);
    }
    if wasm {
        let calls = out.join("calls.tsv");
        std::fs::write(&calls, &lines).unwrap();
        let (_, printed) = wasm_calls(&t.path().join(format!("wasm-{tag}")), &out.join("requests"), &calls);
        report("Wasm", &printed);
    }
}

/// The files `--target requests` writes for the expense and the leave, by their paths under
/// `<out>/requests/`, in the order it lists them: each kind's description, the API, the tools of
/// an agent, the organization's schema and columns and the vectors of finding, each kind's
/// vectors and those of its entry `worked`, each kind's five screens in English and in Japanese,
/// and the catalog and the functions of the screens.
fn requests_files() -> Vec<String> {
    let mut out: Vec<String> = [
        "types/expense.json",
        "types/leave.json",
        "openapi.json",
        "mcp/tools.json",
        "organization/schema.json",
        "organization/columns.md",
        "organization/find.jsonl",
        "vectors/expense.jsonl",
        "vectors/worked/expense.jsonl",
        "vectors/leave.jsonl",
        "vectors/worked/leave.jsonl",
    ]
    .map(String::from)
    .to_vec();
    for kind in ["expense", "leave"] {
        for screen in ["form", "card", "list", "mine", "history", "offered"] {
            for lang in ["en", "ja"] {
                out.push(format!("a2ui/{kind}/{screen}.{lang}.json"));
            }
        }
    }
    out.extend(["a2ui/catalog.json", "a2ui/functions.js"].map(String::from));
    out
}

/// A flow of dandori's that asks for the leave a trip needs through the server's API, reading the
/// `openapi.json` `--target requests` writes (ringi's DESIGN 15.4): the form a ticket is opened with
/// is the kind's, its range included, and the answer is the ticket.
const TRIP_FLOW: &str = r#"workflow trip v1
description "Asks for the leave a trip needs, through the server of the requests"

use openapi requests from "../out/requests/openapi.json"
  url "https://requests.example.com"

enum Kind = paid | sick | special
enum Status = in_progress | paused | with_requester | approved | rejected | withdrawn | expired

record Leave
  kind      : Kind
  first_day : string
  days      : int  range >=1 <=20
  reason    : string?
  cover     : string?

record Ticket
  id     : string
  status : Status

record Moved
  ticket : Ticket

inputs
  leave : Leave

outputs
  ticket : string
  status : Status

task submit_leave(form: Leave) -> Moved
  http POST requests "/requests/leave/tickets"
  key

flow
  let moved = submit_leave(form: leave)
  succeed ticket = moved.ticket.id, status = moved.ticket.status
"#;

/// SIGTERM of `ritsu` reaches cargo's own process group, so the build does not wait
/// for the tool that would otherwise run on, and the scratch directory goes with the failed run.
/// The waits are long enough for a machine busy with the other tests (a debug build of ritsu checks
/// and writes the kind for over ten seconds before it starts cargo), and end as soon as what they
/// wait for happens; ritsu's error output is kept to say why it stopped before cargo, if it does.
#[cfg(unix)]
#[test]
fn a_stopped_build_stops_cargo_and_cleans_its_scratch() {
    if !wasm_ready() {
        return;
    }
    let t = TempDir::new("gen-requests-wasm-kill");
    lay(&t);
    let scratch = here().parent().unwrap().parent().unwrap().join("target").join(format!("wasm-kill-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();
    let fake = scratch.join("fake-cargo");
    let log = scratch.join("cargo.pid");
    std::fs::write(
        &fake,
        r#"#!/bin/sh
if [ "$1" = "-V" ]; then echo 'cargo 1.0.0 (fake)'; exit 0; fi
echo $$ > "$FAKE_CARGO_LOG"
while true; do sleep 1; done
"#,
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(t.path()).args([
        "gen", "examples/leave.ringi", "--root", ".", "--out", "out", "--target", "requests", "--catalog", CATALOG, "--extra", "wasm",
    ]);
    for v in common::LANG_VARS {
        c.env_remove(v);
    }
    let errors = scratch.join("ritsu.err");
    c.env("TMPDIR", &scratch).env("CARGO", &fake).env("FAKE_CARGO_LOG", &log).stderr(std::fs::File::create(&errors).unwrap());
    let mut child = c.spawn().expect("could not run ritsu");
    let mut cargo_pid = None;
    for _ in 0..3000 {
        if let Ok(text) = std::fs::read_to_string(&log)
            && let Some(pid) = text.lines().next().and_then(|x| x.trim().parse::<i32>().ok())
        {
            cargo_pid = Some(pid);
            break;
        }
        if child.try_wait().unwrap().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let Some(cargo_pid) = cargo_pid else {
        let ended = child.try_wait().unwrap();
        let _ = child.kill();
        let said = std::fs::read_to_string(&errors).unwrap_or_default();
        match ended {
            Some(status) => panic!("ritsu ended ({status}) before it started the fake cargo:\n{said}"),
            None => panic!("the build did not reach the fake cargo in 300 seconds:\n{said}"),
        }
    };
    unsafe extern "C" { fn kill(pid: i32, sig: i32) -> i32; }
    assert_eq!(unsafe { kill(child.id() as i32, 15) }, 0);
    let mut finished = false;
    for _ in 0..600 {
        if child.try_wait().unwrap().is_some() {
            finished = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if !finished {
        unsafe { kill(-cargo_pid, 15) };
        let _ = child.kill();
        panic!("ritsu still waited for cargo after SIGTERM");
    }
    assert!(!std::fs::read_dir(&scratch).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().starts_with("ritsu-wasm-")), "a scratch directory was left under {}", scratch.display());
    let _ = std::fs::remove_dir_all(&scratch);
}

/// The vectors `--target requests` wrote into `requests` of each kind, as calls of the list of the
/// kinds of request (as [`answers_as_the_reference`] reads them from the reference).
fn vector_calls(requests: &Path, kinds: &[&str]) -> Vec<Call> {
    let mut calls = Vec::new();
    for kind in kinds {
        let vectors = std::fs::read_to_string(requests.join(format!("vectors/{kind}.jsonl"))).unwrap();
        for line in vectors.lines() {
            let v = ritsu_base::json::parse(line).expect("a vector is a line of JSON");
            let (entry, input, answer) = ringi::lower::shape::call_of(&v).expect("a vector");
            let names: &[&str] = if entry == "start" { &["submit", "organization"] } else { &["ticket", "event", "organization"] };
            let parts = names.iter().map(|n| input.get(n).cloned().unwrap_or(Json::Null).compact()).collect();
            let now = input.get("now").and_then(Json::as_int).and_then(|n| i64::try_from(n).ok()).expect("a vector has its time");
            calls.push(Call { kind: kind.to_string(), entry, now, parts, answer, line: line.chars().take(300).collect() });
        }
        let worked = std::fs::read_to_string(requests.join(format!("vectors/worked/{kind}.jsonl"))).unwrap();
        for line in worked.lines() {
            let v = ritsu_base::json::parse(line).expect("a vector of `worked` is a line of JSON");
            let (Some(input), Some(answer)) = (v.get("input"), v.get("answer")) else { panic!("{kind}: a vector of `worked` with no input or no answer: {line}") };
            calls.push(Call { kind: kind.to_string(), entry: "worked", now: 0, parts: vec![input.compact()], answer: answer.clone(), line: line.chars().take(300).collect() });
        }
    }
    calls
}

/// The lines of `stdout` that say a path, by what they say of it (`generated: `), the paths as
/// the run printed them.
fn said_of(stdout: &str, what: &str) -> Vec<String> {
    stdout.lines().filter_map(|l| l.strip_prefix(what)).map(str::to_string).collect()
}

/// `--extra wasm` (ringi's DESIGN 12.6): the package of Rust is built as one module of Wasm,
/// `requests/kinds.wasm`, with how a server calls it, `requests/wasm.json`; in a place of its own
/// when `--target rust` is not given, which writes no `<out>/rust`. Every vector of the kinds
/// answers through the module as the reference answers, as a run in English and one in Japanese
/// write it; the same input builds the same bytes, with `--target rust` beside or not; `--check`
/// holds the files to the sources with no build and no toolchain on its way, and says a module, a
/// digest and a list of kinds changed by hand stale, as the host refuses or misanswers them; a run
/// with no `--extra` says what an earlier one wrote for it, and leaves it; and with no cargo, or no
/// standard library of wasm32, nothing is written and the run says what to install.
#[test]
fn wasm_is_built_alike_held_by_check_and_refused_without_its_tools() {
    if !wasm_ready() {
        return;
    }
    let t = TempDir::new("gen-requests-wasm");
    lay(&t);
    let kinds = ["expense", "leave"];
    let args_of = |out: &str, more: &[&str]| -> Vec<String> {
        let mut a = vec!["gen", "examples/leave.ringi", "examples/expense.ringi", "--root", ".", "--out", out, "--target", "requests", "--catalog", CATALOG, "--extra", "wasm"];
        a.extend(more);
        a.into_iter().map(String::from).collect()
    };
    let run = |args: &[String]| ritsu_in(t.path(), &args.iter().map(String::as_str).collect::<Vec<_>>());
    // the first build is made with its scratch directory under a tree that plants the two files a
    // shared temporary directory could hold: a toolchain choice cargo cannot use because the build
    // names its toolchain, and a configuration it cannot read because cargo runs from `/`
    let scratch = here().parent().unwrap().parent().unwrap().join("target").join(format!("wasm-planted-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(scratch.join(".cargo")).unwrap();
    std::fs::write(scratch.join("rust-toolchain.toml"), "[toolchain]\nchannel = \"1.99.1\"\n").unwrap();
    std::fs::write(scratch.join(".cargo/config.toml"), "this is no TOML table\n").unwrap();
    let (code, stdout, err) = ritsu_in_with(t.path(), &args_of("out", &[]).iter().map(String::as_str).collect::<Vec<_>>(), &[("TMPDIR", scratch.as_path())]);
    assert_eq!(code, 0, "{stdout}{err}");
    assert!(!std::fs::read_dir(&scratch).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().starts_with("ritsu-wasm-")), "a scratch directory was left under {}", scratch.display());
    let _ = std::fs::remove_dir_all(&scratch);
    let req = t.path().join("out/requests");
    assert!(!t.path().join("out/rust").exists(), "--extra wasm wrote <out>/rust with no --target rust:\n{stdout}");
    let module = std::fs::read(req.join("kinds.wasm")).unwrap();
    let about_text = std::fs::read_to_string(req.join("wasm.json")).unwrap();
    let about = ritsu_base::json::parse(&about_text).unwrap();
    assert!(about_text.starts_with("{\n  \"//\": \"Code generated by ritsu "), "{about_text}");
    assert_eq!(about.get("wasm").and_then(Json::as_int), Some(1));
    assert_eq!(about.get("contract").and_then(Json::as_int), Some(i128::from(ringi::lower::shape::CONTRACT)));
    assert_eq!(about.get("lang").and_then(Json::as_str), Some("en"));
    assert_eq!(about.get("file").and_then(Json::as_str), Some("kinds.wasm"));
    assert_eq!(about.get("sha256").and_then(Json::as_str), Some(ritsu_base::sha256::hex(&module).as_str()));
    assert_eq!(about.get("exports").unwrap().compact(), r#"{"call":"call","realloc":"cabi_realloc","post_return":"cabi_post_call","memory":"memory"}"#);
    let listed: Vec<(String, i128)> = about.get("kinds").and_then(Json::as_arr).unwrap().iter().map(|k| (k.get("name").and_then(Json::as_str).unwrap().to_string(), k.get("version").and_then(Json::as_int).unwrap())).collect();
    for (name, version) in &listed {
        let described = ritsu_base::json::parse(&std::fs::read_to_string(req.join(format!("types/{name}.json"))).unwrap()).unwrap();
        assert_eq!(described.get("version").and_then(Json::as_int), Some(*version), "{name}");
    }
    assert_eq!(listed.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), kinds);
    let built = about.get("built").unwrap();
    assert!(built.get("rustc").and_then(Json::as_str).is_some_and(|r| r.starts_with("rustc ")), "{}", built.compact());
    // every vector through the module, as the reference answers
    let calls = vector_calls(&req, &kinds);
    std::fs::write(t.path().join("calls.tsv"), call_lines(&calls)).unwrap();
    let answered = |requests: &Path| -> (String, usize, Vec<String>) {
        let (lang, printed) = wasm_calls(&t.path().join("host"), requests, &t.path().join("calls.tsv"));
        let (agree, wrong) = held(&calls, &printed);
        (lang, agree, wrong)
    };
    let (lang, agree, wrong) = answered(&req);
    assert!(wrong.is_empty(), "the Wasm answers otherwise than the reference:\n{}", wrong.join("\n"));
    assert_eq!((lang.as_str(), agree), ("en", calls.len()));
    println!("Wasm (en): {agree} of {} vectors of {} kinds agree with the reference", calls.len(), kinds.len());
    // the same input, the same module, whether the crate is built where <out>/rust is written or not
    let (code, stdout, err) = run(&args_of("again", &["--target", "rust"]));
    assert_eq!(code, 0, "{stdout}{err}");
    assert!(t.path().join("again/rust/Cargo.toml").is_file());
    for f in ["kinds.wasm", "wasm.json"] {
        assert!(std::fs::read(req.join(f)).unwrap() == std::fs::read(t.path().join("again/requests").join(f)).unwrap(), "{f} is built otherwise from the same input");
    }
    // a run in Japanese: the entries' sentences in Japanese, and every vector answered the same
    let (code, stdout, err) = run(&args_of("ja", &["--lang", "ja"]));
    assert_eq!(code, 0, "{stdout}{err}");
    let ja = t.path().join("ja/requests");
    assert_eq!(ritsu_base::json::parse(&std::fs::read_to_string(ja.join("wasm.json")).unwrap()).unwrap().get("lang").and_then(Json::as_str), Some("ja"));
    let (lang, agree, wrong) = answered(&ja);
    assert!(wrong.is_empty(), "the Wasm in Japanese answers otherwise than the reference:\n{}", wrong.join("\n"));
    assert_eq!((lang.as_str(), agree), ("ja", calls.len()));
    println!("Wasm (ja): {agree} of {} vectors agree with the reference", calls.len());
    // `--check` builds nothing: no cargo and no rustc are on its way
    let nothing = t.path().join("no-tools");
    std::fs::create_dir_all(&nothing).unwrap();
    let no_tools: [(&str, Option<&std::ffi::OsStr>); 3] = [("PATH", Some(nothing.as_os_str())), ("CARGO", Some(std::ffi::OsStr::new("/nonexistent/cargo"))), ("RUSTC", Some(std::ffi::OsStr::new("/nonexistent/rustc")))];
    let check = |out: &str| {
        let args = args_of(out, &["--check"]);
        ritsu_with(t.path(), &args.iter().map(String::as_str).collect::<Vec<_>>(), &no_tools)
    };
    let (code, stdout, err) = check("out");
    assert_eq!(code, 0, "--check after the run that wrote it:\n{stdout}{err}");
    // a module changed by hand: each place of a sentence a vector answers, a letter of it changed
    let words: Vec<String> = calls.iter().filter_map(|c| c.answer.get("err")).filter_map(|e| e.get("text").and_then(Json::as_str)).filter(|s| s.len() > 20 && s.is_ascii()).map(str::to_string).collect();
    let word = words.iter().find(|w| module.windows(w.len()).any(|x| x == w.as_bytes())).expect("a sentence a vector answers, in the module");
    let mut changed = module.clone();
    let letter = word.bytes().position(|b| b.is_ascii_alphabetic()).expect("a letter in the sentence");
    let places: Vec<usize> = module.windows(word.len()).enumerate().filter(|(_, x)| *x == word.as_bytes()).map(|(i, _)| i + letter).collect();
    for at in places {
        changed[at] ^= 0x20;
    }
    assert!(changed != module);
    let with_sha = |sha: &str| about_text.replace(&ritsu_base::sha256::hex(&module), sha);
    let restore = || {
        std::fs::write(req.join("kinds.wasm"), &module).unwrap();
        std::fs::write(req.join("wasm.json"), &about_text).unwrap();
    };
    // alone, `--check` says it stale, and the host refuses it by its digest
    std::fs::write(req.join("kinds.wasm"), &changed).unwrap();
    let (code, stdout, err) = check("out");
    assert_eq!(code, 1, "{stdout}{err}");
    assert!(said_of(&stdout, "generated file is stale or hand-edited: ").iter().any(|p| p.ends_with("kinds.wasm")), "{stdout}");
    let host = t.path().join("host/wasm_host.mjs");
    let refused = || {
        let o = ritsu_testkit::run::finish(Command::new("node").arg(&host).arg(&req).arg(t.path().join("calls.tsv")), RUN, "node, running the host of the Wasm");
        assert!(!o.ok && o.stderr.contains("the module is not the one wasm.json says"), "{}", o.both());
    };
    refused();
    // with its digest written in wasm.json too, it answers otherwise than the reference: what
    // holds a module to its sources is a build of them, which the vectors hold to the reference
    // (ringi's DESIGN 12.6), and which `--check`, building nothing, does not see
    std::fs::write(req.join("wasm.json"), with_sha(&ritsu_base::sha256::hex(&changed))).unwrap();
    let (_, agree, wrong) = answered(&req);
    assert!(agree < calls.len() && !wrong.is_empty(), "the module changed answers as before");
    let (code, stdout, err) = check("out");
    assert_eq!(code, 0, "{stdout}{err}");
    // a digest changed by hand: the host refuses the module, and `--check` says wasm.json stale
    std::fs::write(req.join("kinds.wasm"), &module).unwrap();
    std::fs::write(req.join("wasm.json"), with_sha(&"0".repeat(64))).unwrap();
    let (code, stdout, err) = check("out");
    assert_eq!(code, 1, "{stdout}{err}");
    assert!(said_of(&stdout, "generated file is stale or hand-edited: ").iter().any(|p| p.ends_with("wasm.json")), "{stdout}");
    refused();
    // a kind taken out of wasm.json: its calls find no kind, and `--check` says wasm.json stale
    let mut dropped = about.clone();
    if let Json::Obj(fields) = &mut dropped
        && let Some((_, Json::Arr(ks))) = fields.iter_mut().find(|(k, _)| k == "kinds")
    {
        ks.retain(|k| k.get("name").and_then(Json::as_str) != Some("leave"));
    }
    std::fs::write(req.join("wasm.json"), format!("{}\n", dropped.pretty())).unwrap();
    let (code, stdout, err) = check("out");
    assert_eq!(code, 1, "{stdout}{err}");
    assert!(said_of(&stdout, "generated file is stale or hand-edited: ").iter().any(|p| p.ends_with("wasm.json")), "{stdout}");
    let (_, agree, wrong) = answered(&req);
    let leave = calls.iter().filter(|c| c.kind == "leave").count();
    assert!(agree == calls.len() - leave && !wrong.is_empty(), "the calls of a kind wasm.json does not list are answered");
    restore();
    let (code, stdout, err) = check("out");
    assert_eq!(code, 0, "{stdout}{err}");
    // `--format json` names the extras of the run, and each file with what became of it
    let args = args_of("out", &["--check", "--format", "json"]);
    let (code, stdout, err) = ritsu_with(t.path(), &args.iter().map(String::as_str).collect::<Vec<_>>(), &no_tools);
    assert_eq!(code, 0, "{stdout}{err}");
    let j = ritsu_base::json::parse(&stdout).unwrap();
    assert_eq!(j.get("extras").unwrap().compact(), r#"["wasm"]"#);
    let statuses: Vec<String> = j
        .get("files")
        .and_then(Json::as_arr)
        .unwrap()
        .iter()
        .filter(|f| f.get("path").and_then(Json::as_str).is_some_and(|p| p.ends_with("kinds.wasm") || p.ends_with("wasm.json")))
        .map(|f| f.get("state").and_then(Json::as_str).unwrap_or("?").to_string())
        .collect();
    assert_eq!(statuses, ["same", "same"], "{stdout}");
    // a run that does not ask for it says the files an earlier one wrote for it, and leaves them
    let (code, stdout, err) = run(&args_of("out", &[])[..args_of("out", &[]).len() - 2]);
    assert_eq!(code, 0, "{stdout}{err}");
    let left = said_of(&stdout, "generated no more, and still there: ");
    assert!(left.iter().any(|p| p.ends_with("kinds.wasm")) && left.iter().any(|p| p.ends_with("wasm.json")), "{stdout}");
    assert!(stdout.contains("note: `--extra wasm` or `--extra bundle` writes these files, and this run does not ask for it"), "{stdout}");
    assert!(std::fs::read(req.join("kinds.wasm")).unwrap() == module && std::fs::read_to_string(req.join("wasm.json")).unwrap() == about_text);
    // no cargo: nothing written, and what to install said, in English and in Japanese
    for (lang, said) in [("en", "`--extra wasm` cannot find cargo, so nothing is written; install Rust's toolchain (https://rustup.rs)"), ("ja", "`--extra wasm` は、cargo が見つからないので、何も書きません。Rust のツールチェーンを入れてください（https://rustup.rs）")] {
        let mut args = vec!["--lang".to_string(), lang.to_string()];
        args.extend(args_of(&format!("no-cargo-{lang}"), &[]));
        let (code, stdout, err) = ritsu_with(t.path(), &args.iter().map(String::as_str).collect::<Vec<_>>(), &no_tools);
        assert_eq!(code, 2, "{stdout}{err}");
        assert!(err.contains(said), "{err}");
        assert!(!t.path().join(format!("no-cargo-{lang}")).exists(), "something is written with no cargo");
    }
    // a rustc with no standard library of wasm32: nothing written, and `rustup target add` said
    let fake = t.path().join("fake-rustc");
    let empty = t.path().join("no-libs");
    std::fs::create_dir_all(&empty).unwrap();
    std::fs::write(&fake, format!("#!/bin/sh\nif [ \"$1\" = \"-V\" ]; then echo 'rustc 1.0.0 (fake)'; exit 0; fi\necho '{}'\n", empty.display())).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let args = args_of("no-wasm32", &[]);
    let (code, stdout, err) = ritsu_with(t.path(), &args.iter().map(String::as_str).collect::<Vec<_>>(), &[("RUSTC", Some(fake.as_os_str()))]);
    assert_eq!(code, 2, "{stdout}{err}");
    assert!(err.contains("finds no standard library of wasm32-unknown-unknown for rustc 1.0.0 (fake), so nothing is written; install it with `rustup target add wasm32-unknown-unknown`"), "{err}");
    assert!(!t.path().join("no-wasm32").exists(), "something is written with no standard library of wasm32");
    // what it is an extra of, and what an extra needs
    for (args, said) in [
        (vec!["gen", "examples/leave.ringi", "--root", ".", "--out", "x", "--target", "go", "--extra", "wasm"], "error: `--extra wasm` is an extra of `requests`, and there is no `--target requests`\n"),
        (vec!["gen", "examples/leave.ringi", "--root", ".", "--out", "x", "--target", "requests", "--catalog", CATALOG, "--extra", "bundle"], "error: `--extra bundle` needs `--extra wasm`: a bundle carries the Wasm a server loads with no build of its own\n"),
        (vec!["gen", "examples/leave.ringi", "--root", ".", "--out", "x", "--target", "requests", "--catalog", CATALOG, "--extra", "wasm", "--diff-from", "out/requests"], "error: `--diff-from` goes with `--extra bundle`\n"),
    ] {
        let (code, stdout, err) = ritsu_in(t.path(), &args);
        assert_eq!((code, err.as_str()), (2, said), "{stdout}");
        assert!(!t.path().join("x").exists());
    }
}

/// Python's reader of a zip, run with `-I`: `testzip`, then each entry's name, the SHA-256 and the
/// size of what it reads, and whether it is a directory.
const PY_UNZIP: &str = r#"import hashlib
import sys
import zipfile

with zipfile.ZipFile(sys.argv[1]) as z:
    bad = z.testzip()
    if bad is not None:
        sys.exit("testzip finds a bad file: " + bad)
    for i in z.infolist():
        b = z.read(i)
        print(i.filename + "\t" + hashlib.sha256(b).hexdigest() + "\t" + str(len(b)) + "\t" + str(i.is_dir()).lower())
"#;

/// Go's (archive/zip), printing the same; reading each entry to its end checks its CRC-32.
const GO_UNZIP: &str = r#"package main

import (
	"archive/zip"
	"crypto/sha256"
	"fmt"
	"io"
	"os"
)

func main() {
	r, err := zip.OpenReader(os.Args[1])
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	defer r.Close()
	for _, f := range r.File {
		rc, err := f.Open()
		if err != nil {
			fmt.Fprintln(os.Stderr, f.Name, err)
			os.Exit(1)
		}
		b, err := io.ReadAll(rc)
		rc.Close()
		if err != nil {
			fmt.Fprintln(os.Stderr, f.Name, err)
			os.Exit(1)
		}
		fmt.Printf("%s\t%x\t%d\t%v\n", f.Name, sha256.Sum256(b), len(b), f.FileInfo().IsDir())
	}
}
"#;

/// The zip at `zip` read by readers other than ritsu's: Python's zipfile and Go's archive/zip, each
/// file the same bytes as `want` and none other, no directory among them; and `unzip -t`, which
/// finds no error. `dir` is where their programs are written.
fn read_elsewhere(dir: &Path, zip: &Path, want: &std::collections::BTreeMap<String, Vec<u8>>) {
    std::fs::create_dir_all(dir).unwrap();
    let wanted: Vec<String> = want.iter().map(|(p, b)| format!("{p}\t{}\t{}\tfalse", ritsu_base::sha256::hex(b), b.len())).collect();
    let listed = |stdout: &str| {
        let mut l: Vec<String> = stdout.lines().map(str::to_string).collect();
        l.sort();
        l
    };
    if ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 is not installed; the zip is not read by Python's zipfile") {
        std::fs::write(dir.join("unzip.py"), PY_UNZIP).unwrap();
        let o = ritsu_testkit::run::finish(Command::new("python3").arg("-I").arg(dir.join("unzip.py")).arg(zip).env("PYTHONIOENCODING", "utf-8"), RUN, "python3's zipfile");
        assert!(o.ok, "Python's zipfile does not read {}:\n{}", zip.display(), o.both());
        assert_eq!(listed(&o.stdout), wanted, "Python's zipfile reads {} otherwise", zip.display());
        println!("Python's zipfile reads the {} files of {} as written", want.len(), zip.display());
    }
    if ready(Need::Go, have_go, "go is not installed; the zip is not read by Go's archive/zip") {
        let g = dir.join("go");
        std::fs::create_dir_all(&g).unwrap();
        std::fs::write(g.join("go.mod"), "module unzip\n\ngo 1.21\n").unwrap();
        std::fs::write(g.join("main.go"), GO_UNZIP).unwrap();
        let bin = g.join("unzip.bin");
        let o = ritsu_testkit::run::finish(go_in(&g).args(["build", "-o"]).arg(&bin).arg("."), LIMIT, "go build of the reader of zip");
        assert!(o.ok, "go build:\n{}", o.both());
        let o = ritsu_testkit::run::finish(Command::new(&bin).arg(zip), RUN, "Go's archive/zip");
        assert!(o.ok, "Go's archive/zip does not read {}:\n{}", zip.display(), o.both());
        assert_eq!(listed(&o.stdout), wanted, "Go's archive/zip reads {} otherwise", zip.display());
        println!("Go's archive/zip reads the {} files of {} as written", want.len(), zip.display());
    }
    if Command::new("unzip").arg("-v").output().is_ok_and(|o| o.status.success()) {
        let o = ritsu_testkit::run::finish(Command::new("unzip").arg("-tq").arg(zip), RUN, "unzip -t");
        assert!(o.ok && o.stdout.contains("No errors detected"), "unzip -t finds fault with {}:\n{}", zip.display(), o.both());
    } else {
        ritsu_testkit::skip("unzip is not installed; the zip is not tested by unzip -t");
    }
}

/// `--extra bundle` (ringi's DESIGN 12.6): `<out>/requests.zip` holds every file `<out>/requests/`
/// holds, the same bytes, and nothing else; with them, the files the kinds are made from
/// (`source/`, listed in `bundle.json` with their digests), a request's `.ringi` and every file of
/// a language it reads, and none of the organization's or the masters' data. Python's zipfile,
/// Go's archive/zip and `unzip -t` read it as written, and so a zip of an empty file, of large
/// ones stored and compressed, and of names in Japanese; the same input writes the same bytes;
/// `--check` says a file of it changed by hand; `--diff-from` puts what `ringi diff` says of each
/// kind against the bundle before into `diff/`, the same pages from the zip as from a directory of
/// it; a `--diff-from` that is no bundle is refused, and a run that does not ask for the bundle
/// says its files, and leaves them.
#[test]
fn a_bundle_holds_the_requests_and_other_readers_read_it() {
    use std::collections::BTreeMap;
    if !wasm_ready() {
        return;
    }
    let t = TempDir::new("gen-requests-bundle");
    lay(&t);
    // a kind of request in a file named in Japanese, which the bundle's paths hold
    std::fs::copy(t.path().join("examples/leave.ringi"), t.path().join("examples/休暇.ringi")).unwrap();
    let run_gen = |out: &str, more: &[&str]| -> (i32, String, String) {
        let mut a = vec!["gen", "examples/休暇.ringi", "examples/expense.ringi", "--root", ".", "--out", out, "--target", "requests", "--catalog", CATALOG, "--extra", "wasm", "--extra", "bundle"];
        a.extend(more);
        ritsu_in(t.path(), &a)
    };
    let (code, stdout, err) = run_gen("out", &[]);
    assert_eq!(code, 0, "{stdout}{err}");
    let (req, zip_path) = (t.path().join("out/requests"), t.path().join("out/requests.zip"));
    let zip = std::fs::read(&zip_path).unwrap();
    let on_disk: BTreeMap<String, Vec<u8>> = files_under(&req).into_iter().map(|f| (f.clone(), std::fs::read(req.join(&f)).unwrap())).collect();
    let held: BTreeMap<String, Vec<u8>> = ritsu::package::zip::read(&zip).unwrap_or_else(|e| panic!("{}", e.en)).into_iter().collect();
    assert_eq!(held.keys().collect::<Vec<_>>(), on_disk.keys().collect::<Vec<_>>(), "the zip holds other files than requests/");
    assert!(held == on_disk, "a file of the zip is not the one in requests/");
    for f in ["kinds.wasm", "wasm.json", "bundle.json", "types/leave.json", "openapi.json", "source/examples/休暇.ringi", "source/examples/expense.ringi", "source/examples/shared/company.ringi", "source/examples/shared/office.cal"] {
        assert!(held.contains_key(f), "the bundle holds no {f}");
    }
    let about = ritsu_base::json::parse(&String::from_utf8(held["bundle.json"].clone()).unwrap()).unwrap();
    assert_eq!(about.get("bundle").and_then(Json::as_int), Some(1));
    assert!(about.get("diff").unwrap().is_null());
    let sources: Vec<(String, String)> = about.get("sources").and_then(Json::as_arr).unwrap().iter().map(|s| (s.get("path").and_then(Json::as_str).unwrap().to_string(), s.get("sha256").and_then(Json::as_str).unwrap().to_string())).collect();
    let under: Vec<&String> = held.keys().filter(|p| p.starts_with("source/")).collect();
    assert_eq!(sources.iter().map(|(p, _)| p).collect::<Vec<_>>(), under, "bundle.json lists other files than source/ holds");
    for (p, sha) in &sources {
        assert_eq!(*sha, ritsu_base::sha256::hex(&held[p]), "{p}");
        let rel = p.strip_prefix("source/").unwrap();
        assert!(held[p] == std::fs::read(t.path().join(rel)).unwrap(), "{p} is not the project's file");
        assert!(ritsu_project::project::kind_of(rel).is_some(), "{p} is no file of a language");
        assert!(!rel.starts_with("examples/org/") && !rel.starts_with("examples/masters/"), "the bundle holds data: {p}");
    }
    println!("the bundle holds the {} files of requests/, {} of them the files the kinds are made from", held.len(), sources.len());
    read_elsewhere(&t.path().join("read-bundle"), &zip_path, &held);
    // what the examples' bundle does not hold: an empty file, large files stored and compressed, and
    // names in Japanese
    let mut x: u64 = 0x2545_f491_4f6c_dd1d;
    let mut noise = |n: usize| -> Vec<u8> {
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                x as u8
            })
            .collect()
    };
    let lines: String = (0..120_000).map(|i| format!("{{\"line\":{i},\"word\":\"{}\"}}\n", ["申請", "承認", "差し戻し", "却下"][i % 4])).collect();
    let mut odd = noise(1 << 20);
    odd.extend(b"abc".repeat(300_000));
    odd.extend(noise(1 << 20));
    let made: BTreeMap<String, Vec<u8>> = [
        ("empty".to_string(), Vec::new()),
        ("空のディレクトリの下/空".to_string(), Vec::new()),
        ("日本語の名前/申請の種類.md".to_string(), "# 申請の種類\n\n経費の申請と休暇の申請。\n".repeat(50).into_bytes()),
        ("large/noise.bin".to_string(), noise(3 << 20)),
        ("large/lines.jsonl".to_string(), lines.into_bytes()),
        ("large/mixed.bin".to_string(), odd),
        ("one".to_string(), b"x".to_vec()),
    ]
    .into_iter()
    .collect();
    let z = ritsu::package::zip::write(&made.iter().map(|(p, b)| (p.clone(), b.clone())).collect::<Vec<_>>()).unwrap_or_else(|e| panic!("{}", e.en));
    let made_path = t.path().join("made.zip");
    std::fs::write(&made_path, &z).unwrap();
    assert!(ritsu::package::zip::read(&z).unwrap_or_else(|e| panic!("{}", e.en)).into_iter().collect::<BTreeMap<_, _>>() == made);
    read_elsewhere(&t.path().join("read-made"), &made_path, &made);
    // the same input, the same zip
    let (code, stdout, err) = run_gen("again", &[]);
    assert_eq!(code, 0, "{stdout}{err}");
    assert!(std::fs::read(t.path().join("again/requests.zip")).unwrap() == zip, "the zip is written otherwise from the same input");
    // `--check`: clean, then a file of the bundle changed by hand
    let (code, stdout, err) = run_gen("out", &["--check"]);
    assert_eq!(code, 0, "{stdout}{err}");
    let copy = req.join("source/examples/expense.ringi");
    let original = std::fs::read(&copy).unwrap();
    std::fs::write(&copy, [original.as_slice(), b"\n"].concat()).unwrap();
    let (code, stdout, err) = run_gen("out", &["--check"]);
    assert_eq!(code, 1, "{stdout}{err}");
    let stale = said_of(&stdout, "generated file is stale or hand-edited: ");
    assert!(stale.len() == 1 && stale[0].ends_with("source/examples/expense.ringi"), "{stdout}");
    std::fs::write(&copy, &original).unwrap();
    let mut changed = zip.clone();
    changed[40] ^= 1;
    std::fs::write(&zip_path, &changed).unwrap();
    let (code, stdout, err) = run_gen("out", &["--check"]);
    assert_eq!(code, 1, "{stdout}{err}");
    let stale = said_of(&stdout, "generated file is stale or hand-edited: ");
    assert!(stale.len() == 1 && stale[0].ends_with("requests.zip"), "{stdout}");
    std::fs::write(&zip_path, &zip).unwrap();
    // `--diff-from`: the leave's route changed, against the bundle before
    let src = std::fs::read_to_string(t.path().join("examples/休暇.ringi")).unwrap();
    std::fs::write(t.path().join("examples/休暇.ringi"), src.replace("route standard(long: days > 5)", "route standard(long: days > 3)")).unwrap();
    let (code, stdout, err) = run_gen("next", &["--diff-from", "out/requests.zip"]);
    assert_eq!(code, 0, "{stdout}{err}");
    let next = t.path().join("next/requests");
    let about = ritsu_base::json::parse(&std::fs::read_to_string(next.join("bundle.json")).unwrap()).unwrap();
    let diff = about.get("diff").unwrap();
    assert_eq!(diff.get("base").and_then(Json::as_str), Some(ritsu_base::sha256::hex(&zip).as_str()));
    assert_eq!(
        diff.get("kinds").unwrap().compact(),
        r#"[{"name":"expense","from":1,"to":1,"differs":false,"en":"diff/expense.en.md","ja":"diff/expense.ja.md"},{"name":"leave","from":1,"to":1,"differs":true,"en":"diff/leave.en.md","ja":"diff/leave.ja.md"}]"#
    );
    let page = |f: &str| std::fs::read_to_string(next.join(f)).unwrap();
    assert!(page("diff/leave.en.md").contains("### Requests where “Business days” is from 4 to 5: the stage “head” (dept_head) comes"), "{}", page("diff/leave.en.md"));
    assert!(page("diff/leave.ja.md").contains("`examples/休暇.ringi` → `examples/休暇.ringi`"), "{}", page("diff/leave.ja.md"));
    assert!(page("diff/expense.ja.md").contains("二つは同じことを言い、同じに答えます。"), "{}", page("diff/expense.ja.md"));
    assert!(ritsu::package::zip::read(&std::fs::read(t.path().join("next/requests.zip")).unwrap()).unwrap().iter().any(|(p, _)| p == "diff/leave.ja.md"));
    // the bundle before as a directory: the same pages; bundle.json names no zip
    let args = ["gen", "examples/休暇.ringi", "examples/expense.ringi", "--root", ".", "--out", "next", "--target", "requests", "--catalog", CATALOG, "--extra", "wasm", "--extra", "bundle", "--diff-from", "out/requests", "--check", "--format", "json"];
    let (code, stdout, err) = ritsu_in(t.path(), &args);
    assert_eq!(code, 1, "{stdout}{err}");
    let j = ritsu_base::json::parse(&stdout).unwrap();
    let status = |end: &str| j.get("files").and_then(Json::as_arr).unwrap().iter().find(|f| f.get("path").and_then(Json::as_str).is_some_and(|p| p.ends_with(end))).and_then(|f| f.get("state").and_then(Json::as_str)).unwrap_or("none").to_string();
    assert_eq!([status("diff/leave.en.md"), status("diff/leave.ja.md"), status("diff/expense.en.md"), status("requests/bundle.json"), status("requests.zip")], ["same", "same", "same", "stale", "stale"], "{stdout}");
    assert_eq!(j.get("extras").unwrap().compact(), r#"["wasm","bundle"]"#);
    // a `--diff-from` that is no bundle
    for (from, said) in [
        ("examples/leave.ringi", "error: `--diff-from examples/leave.ringi` cannot be read as a bundle: it does not end with the end of a central directory with no comment, as a zip ritsu writes does\n"),
        ("examples", "error: `--diff-from examples` cannot be read as a bundle: it holds no `wasm.json`\n"),
    ] {
        let (code, stdout, err) = run_gen("nothing", &["--diff-from", from]);
        assert_eq!((code, err.as_str()), (2, said), "{stdout}");
        assert!(!t.path().join("nothing").exists());
    }
    // a run that does not ask for the bundle says its files, and leaves them
    let args = ["gen", "examples/休暇.ringi", "examples/expense.ringi", "--root", ".", "--out", "next", "--target", "requests", "--catalog", CATALOG, "--extra", "wasm", "--check"];
    let (code, stdout, err) = ritsu_in(t.path(), &args);
    let left = said_of(&stdout, "generated no more, and still there: ");
    for end in ["requests.zip", "requests/bundle.json", "requests/source/examples/休暇.ringi", "requests/diff/leave.ja.md"] {
        assert!(left.iter().any(|p| p.ends_with(end)), "{end} is not said:\n{stdout}{err}");
    }
    assert_eq!(code, 0, "{stdout}{err}");
    assert!(t.path().join("next/requests.zip").is_file() && next.join("diff/leave.ja.md").is_file());
}

/// `--target requests` writes, once for the project, what is apart from any language (ringi's
/// DESIGN 12.2): each kind's description and vectors, the server's API with an operation of each
/// kind, the tools of an agent, the schema and the columns of the organization's data, the vectors
/// of finding a person, a team or a row of a master (`organization/find.jsonl`, apart from the kinds'
/// `vectors/`), and each kind's screens with their catalog and functions (DESIGN 13). It is a target as the others are:
/// listed in `--format json` with the file each comes from, written again the same, a file it wrote
/// before for a kind no longer given said to be left by `--check` and taken away (its vectors, which
/// have no head, with its description), `--check` clean after it writes; and the same in a
/// run in Japanese, as none of its files is in the run's language. The API reads as
/// OpenAPI 3.1: its references lead to what it holds, and dandori reads it in a flow, its ranges
/// and its operations held to the flow's. (What the screens hold is ringi's `tests/a2ui.rs`'s.)
#[test]
fn the_requests_target_describes_the_kinds_once() {
    let t = TempDir::new("gen-requests-target");
    lay(&t);
    let out = t.path().join("out");
    let o = out.to_string_lossy().into_owned();
    let version = env!("CARGO_PKG_VERSION");
    // what a kind no longer given left: a description with the head, its vectors and those of its
    // entry `worked` (which have none), and its screens
    for dir in ["types", "vectors/worked", "a2ui/gone"] {
        std::fs::create_dir_all(out.join("requests").join(dir)).unwrap();
    }
    let head = format!("{{\n  \"//\": \"Code generated by ritsu {version}. DO NOT EDIT.\"\n}}\n");
    std::fs::write(out.join("requests/types/gone.json"), &head).unwrap();
    std::fs::write(out.join("requests/vectors/gone.jsonl"), "{}\n").unwrap();
    std::fs::write(out.join("requests/vectors/worked/gone.jsonl"), "{}\n").unwrap();
    std::fs::write(out.join("requests/a2ui/gone/form.en.json"), &head).unwrap();
    let gone = ["a2ui/gone/form.en.json", "types/gone.json", "vectors/gone.jsonl", "vectors/worked/gone.jsonl"];
    let args = ["gen", "examples/leave.ringi", "examples/expense.ringi", "--root", ".", "--out", &o, "--target", "requests", "--catalog", CATALOG];
    // `--check` says they are left, and takes nothing away
    let (code, stdout, err) = ritsu_in(t.path(), &[&args[..], &["--check", "--format", "json"]].concat());
    assert_eq!(code, 1, "{stdout}{err}");
    let checked = ritsu_base::json::parse(&stdout).expect("--format json writes one object");
    let left: Vec<String> = checked
        .get("files")
        .and_then(Json::as_arr)
        .expect("the files")
        .iter()
        .filter(|f| f.get("state").and_then(Json::as_str) == Some("left"))
        .map(|f| f.get("path").and_then(Json::as_str).unwrap().strip_prefix(&format!("{o}/requests/")).expect("under <out>/requests").to_string())
        .collect();
    assert_eq!(left, gone, "{stdout}");
    assert!(gone.iter().all(|f| out.join("requests").join(f).is_file()), "--check takes a file away");
    let (code, stdout, err) = ritsu_in(t.path(), &[&args[..], &["--format", "json"]].concat());
    assert_eq!(code, 0, "{stdout}{err}");
    let answer = ritsu_base::json::parse(&stdout).expect("--format json writes one object");
    assert_eq!(answer.get("targets").map(Json::compact), Some(r#"["requests"]"#.to_string()));
    let listed: Vec<(String, String, String)> = answer
        .get("files")
        .and_then(Json::as_arr)
        .expect("the files")
        .iter()
        .map(|f| {
            assert_eq!(f.get("target").and_then(Json::as_str), Some("requests"));
            let path = f.get("path").and_then(Json::as_str).unwrap().strip_prefix(&format!("{o}/requests/")).expect("under <out>/requests").to_string();
            (path, f.get("from").and_then(Json::as_str).unwrap_or("-").to_string(), f.get("state").and_then(Json::as_str).unwrap().to_string())
        })
        .collect();
    let want: Vec<(String, String, String)> = requests_files()
        .iter()
        .map(|p| {
            let from = if p.contains("expense") { "examples/expense.ringi" } else if p.contains("leave") { "examples/leave.ringi" } else { "-" };
            (p.to_string(), from.to_string(), "written".to_string())
        })
        .chain(gone.map(|p| (p.to_string(), "-".to_string(), "removed".to_string())))
        .collect();
    assert_eq!(listed, want);
    assert!(!out.join("requests/a2ui/gone").exists(), "the directory of the screens of a kind no longer given is left");
    let written = files_under(&out);
    let mut expected: Vec<String> = requests_files().iter().map(|p| format!("requests/{p}")).collect();
    expected.sort();
    assert_eq!(written, expected, "--target requests writes its files alone, in <out>/requests/");
    // a run in Japanese writes the same files: none of them is in the run's language
    let ja = t.path().join("out-ja");
    let o_ja = ja.to_string_lossy().into_owned();
    let (code, stdout, err) = ritsu_in(t.path(), &["--lang", "ja", "gen", "examples/leave.ringi", "examples/expense.ringi", "--root", ".", "--out", &o_ja, "--target", "requests", "--catalog", CATALOG]);
    assert_eq!(code, 0, "{stdout}{err}");
    assert_eq!(files_under(&ja), expected);
    for f in &expected {
        assert!(std::fs::read(out.join(f)).unwrap() == std::fs::read(ja.join(f)).unwrap(), "{f} is written otherwise in a run in Japanese");
    }
    // each kind's vectors are the reference's
    for (kind, request) in [("leave", "examples/leave.ringi"), ("expense", "examples/expense.ringi")] {
        let (code, vectors, err) = ritsu_in(t.path(), &["ringi", "vectors", request]);
        assert_eq!(code, 0, "{err}");
        assert!(std::fs::read_to_string(out.join(format!("requests/vectors/{kind}.jsonl"))).unwrap() == vectors, "the vectors of {kind}");
    }
    // the API, as OpenAPI 3.1 reads: each reference leads to what the document holds, each operation
    // has its own ID, and each kind's operations say the request they are of
    let api = ritsu_base::json::parse(&std::fs::read_to_string(out.join("requests/openapi.json")).unwrap()).unwrap();
    assert_eq!(api.get("openapi").and_then(Json::as_str), Some("3.1.0"));
    let mut refs = Vec::new();
    references(&api, &mut refs);
    for r in &refs {
        let pointer = r.strip_prefix("#/").unwrap_or_else(|| panic!("a reference outside the document: {r}"));
        assert!(pointer.split('/').try_fold(&api, |j, k| j.get(k)).is_some(), "the reference {r} leads nowhere");
    }
    let mut ids: Vec<&str> = Vec::new();
    for (path, item) in api.get("paths").and_then(Json::as_obj).unwrap() {
        for (method, op) in item.as_obj().unwrap() {
            let id = op.get("operationId").and_then(Json::as_str).unwrap_or_else(|| panic!("{method} {path} has no operationId"));
            assert!(!ids.contains(&id), "two operations are {id}");
            ids.push(id);
            if path.starts_with("/requests/") {
                let kind = path.split('/').nth(2).unwrap();
                assert_eq!(op.get("x-ritsu-request").and_then(|r| r.get("name")).and_then(Json::as_str), Some(kind), "{method} {path}");
            }
        }
    }
    // the tools of an agent: each an operation of the API, its input reading alone
    let tools = ritsu_base::json::parse(&std::fs::read_to_string(out.join("requests/mcp/tools.json")).unwrap()).unwrap();
    for tool in tools.get("tools").and_then(Json::as_arr).unwrap() {
        let name = tool.get("name").and_then(Json::as_str).unwrap();
        assert!(ids.contains(&name), "the tool {name} is no operation of the API");
        let input = tool.get("inputSchema").unwrap();
        let mut refs = Vec::new();
        references(input, &mut refs);
        for r in refs {
            let pointer = r.strip_prefix("#/").unwrap_or_else(|| panic!("{name}: a reference outside its input: {r}"));
            assert!(pointer.split('/').try_fold(input, |j, k| j.get(k)).is_some(), "{name}: the reference {r} leads nowhere");
        }
    }
    // dandori reads the API in a flow: what the flow sends and reads is what the operation takes and
    // answers, a range wider than the form's is not, and a kind the API has no operation of is not
    std::fs::create_dir_all(t.path().join("flows")).unwrap();
    std::fs::write(t.path().join("flows/trip.flow"), TRIP_FLOW).unwrap();
    let (code, stdout, err) = ritsu_in(t.path(), &["check", "flows/trip.flow"]);
    assert_eq!(code, 0, "{stdout}{err}");
    for (name, from, to, said) in [
        ("wide", "range >=1 <=20", "range >=1 <=30", "in `days`, it is `>=1 <=30`, and `>=1 <=20` is taken"),
        ("vacation", "/requests/leave/tickets", "/requests/vacation/tickets", "`requests` has no POST /requests/vacation/tickets"),
    ] {
        let flow = TRIP_FLOW.replace("workflow trip v1", &format!("workflow {name} v1")).replace(from, to);
        std::fs::write(t.path().join(format!("flows/{name}.flow")), flow).unwrap();
        let (code, stdout, err) = ritsu_in(t.path(), &["check", &format!("flows/{name}.flow")]);
        assert!(code == 1 && stdout.contains("E016") && stdout.contains(said), "{name}: {stdout}{err}");
    }
    // the same files again, and nothing to write
    let (code, stdout, err) = ritsu_in(t.path(), &[&args[..], &["--check"]].concat());
    assert_eq!(code, 0, "{stdout}{err}");
    // what each file says, each kind's vectors aside (they are held to the reference above); the
    // vectors of finding are the reference's answers, and are read here
    for f in requests_files().iter().filter(|f| !f.starts_with("vectors/")) {
        let text = std::fs::read_to_string(out.join("requests").join(f)).unwrap();
        ritsu_testkit::golden(here().join("tests/golden/requests").join(f), &text.replace(version, "<version>"));
    }
}

/// The `ritsu.mod` of [`a_library_holds_the_kinds_and_requests_is_written_once_beside`]: a module
/// with a library in each language, written in them and in `requests`.
/// A flow a request starts once it is approved, that reads the ticket through the server's API
/// `--target requests` writes.
const FOLLOW_FLOW: &str = r#"workflow follow v1
description "Reads the ticket it is started for through the server of the requests, and answers the state it is in"

use openapi requests from "../out/requests/openapi.json"
  url "https://requests.example.com"

enum Status = in_progress | paused | with_requester | approved | rejected | withdrawn | expired

record Ticket
  id     : string
  status : Status

record Seen
  ticket : Ticket

inputs
  request_id : string

outputs
  status : Status

task get_ticket(id: string) -> Seen
  http GET requests "/tickets/{id}"

flow
  let seen = get_ticket(id: request_id)
  succeed status = seen.ticket.status
"#;

/// A flow that reads the API `--target requests` writes is checked against the API the run writes
/// (ringi's DESIGN 9.3): on the first run, with nothing written yet, a request that starts such a
/// flow, and reads a gate that guards an operation of the API and names the flow's workflow, is
/// written in one run; and with an API written before that takes more than the requests now do,
/// the run holds the flow to the one it writes, says so, and writes nothing.
#[test]
fn a_flow_that_reads_the_api_is_held_to_the_one_the_run_writes() {
    let t = TempDir::new("gen-requests-api-laid");
    lay(&t);
    let out = t.path().join("out");
    let o = out.to_string_lossy().into_owned();
    let doc = out.join("requests/openapi.json");
    std::fs::create_dir_all(t.path().join("flows")).unwrap();
    std::fs::write(t.path().join("flows/follow.flow"), FOLLOW_FLOW).unwrap();
    let leave_file = t.path().join("examples/leave.ringi");
    let leave = std::fs::read_to_string(&leave_file).unwrap();
    let started = leave
        .replacen("use route standard", "use flow follow from \"../flows/follow.flow\"\nuse route standard", 1)
        .replacen("  on approved    post", "  on approved    launch follow(request_id: ticket.id)\n  on approved    post", 1);
    assert!(started.contains("launch follow"), "the leave example is not as this test reads it");
    std::fs::write(&leave_file, &started).unwrap();
    // the gate the request reads guards the operation that reads a ticket, of the API the run
    // writes (`use openapi`), and names the flow's workflow (`workflow … from`), whose check reads
    // the flow too
    let gate_file = t.path().join("examples/shared/company.gate");
    let gate = std::fs::read_to_string(&gate_file).unwrap();
    let see = "action see\n  description \"See a ticket one is not on: the requester's side and the approvers' side see theirs without asking\"\n";
    let named = gate
        .replacen("namespace Ringi\n", "namespace Ringi\n\nuse openapi requests from \"../../out/requests/openapi.json\"\n", 1)
        .replacen(see, &format!("{see}  guards requests get_ticket\n"), 1)
        .replacen("resource Team\n", "workflow follow from \"../../flows/follow.flow\"\n  description \"Reads the ticket it is started for through the server\"\n\nresource Team\n", 1);
    assert!(named.contains("use openapi") && named.contains("guards requests") && named.contains("workflow follow"), "the gate is not as this test reads it");
    std::fs::write(&gate_file, named).unwrap();
    let write = |files: &[&str]| ritsu_in(t.path(), &[&["gen"][..], files, &["--root", ".", "--out", &o, "--target", "requests", "--catalog", CATALOG]].concat());
    // the first run, nothing written: the request, the flow it starts and its gate pass, and the API
    // is written
    assert!(!out.exists());
    let (code, stdout, err) = write(&["examples/leave.ringi"]);
    assert_eq!(code, 0, "the first run: {stdout}{err}");
    // (from the root: the gate's workflow is outside the request's directory, sekisho's E208)
    let (code, stdout, err) = ritsu_in(t.path(), &["check", "examples/leave.ringi", "--root", "."]);
    assert_eq!(code, 0, "the request, its flow and its gate, on the API written: {stdout}{err}");
    // an API written when the requests took more days: a flow that sends that many passes on it ...
    let wide = TRIP_FLOW.replace("workflow trip v1", "workflow wide v1").replace("range >=1 <=20", "range >=1 <=30");
    std::fs::write(t.path().join("flows/wide.flow"), wide).unwrap();
    std::fs::write(&leave_file, started.replace("range >=1 <=20", "range >=1 <=30")).unwrap();
    let (code, stdout, err) = write(&["examples/leave.ringi", "flows/wide.flow"]);
    assert_eq!(code, 0, "{stdout}{err}");
    let before = std::fs::read(&doc).unwrap();
    let (code, stdout, err) = ritsu_in(t.path(), &["check", "flows/wide.flow"]);
    assert_eq!(code, 0, "{stdout}{err}");
    // ... and once the requests take fewer, the run that writes the API holds the flow to the API
    // it writes, not to the one on the disk, and writes nothing
    std::fs::write(&leave_file, &started).unwrap();
    let (code, stdout, err) = write(&["examples/leave.ringi", "flows/wide.flow"]);
    assert!(code == 1 && err.contains("dandori E016") && err.contains("in `days`, it is `>=1 <=30`, and `>=1 <=20` is taken"), "{stdout}{err}");
    assert!(std::fs::read(&doc).unwrap() == before, "a run that stops writes the API");
    // the flow that sends what the requests take passes, and the API it was checked on is written
    std::fs::write(t.path().join("flows/trip.flow"), TRIP_FLOW).unwrap();
    let (code, stdout, err) = write(&["examples/leave.ringi", "flows/trip.flow"]);
    assert_eq!(code, 0, "{stdout}{err}");
    let (code, stdout, err) = ritsu_in(t.path(), &["check", "flows/wide.flow"]);
    assert!(code == 1 && stdout.contains("E016"), "the API written is the one the run checked the flows on: {stdout}{err}");
}

const LIBRARY_MOD: &str = r#"module example.com/acme/requests
version 1.0.0
description "Kinds of request of a fictional company"
license "MIT OR Apache-2.0"
library typescript "@acme/requests"
library python "acme-requests"
library go "example.com/acme/requests-go"
target typescript python go requests
catalog "https://requests.example.com/a2ui/catalog.json"
"#;

/// A module of requests written as libraries (`ritsu gen --library`, the root's DESIGN 17.16;
/// ringi's DESIGN 12.2): each language's library holds the list of the kinds of request, which
/// reads the glue by the library's own name, and is reached from the library's entry; what
/// `requests` writes is no language's, and is written once beside the libraries in
/// `<out>/requests/`, with no `library` line of its own. `--format json` lists the files written,
/// each under its target's directory, and `--check` finds nothing to write.
#[test]
fn a_library_holds_the_kinds_and_requests_is_written_once_beside() {
    let t = TempDir::new("gen-requests-library");
    let examples = here().join("../ringi/examples");
    let module = t.path().join("requests");
    for dir in ["shared", "org", "masters"] {
        ritsu_testkit::tmp::copy_dir(&examples.join(dir), &module.join(dir));
    }
    std::fs::copy(examples.join("leave.ringi"), module.join("leave.ringi")).unwrap();
    std::fs::write(module.join("ritsu.mod"), LIBRARY_MOD).unwrap();
    let out = t.path().join("out");
    let o = out.to_string_lossy().into_owned();
    let store = t.path().join("store");
    let library = |more: &[&str]| {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
        c.current_dir(&module).args(["gen", "--library", "--out", &o]).args(more).env(ritsu_base::modules::store::VARIABLE, &store);
        for v in common::LANG_VARS {
            c.env_remove(v);
        }
        let r = c.output().expect("could not run ritsu");
        (r.status.code().unwrap_or(-1), String::from_utf8_lossy(&r.stdout).into_owned(), String::from_utf8_lossy(&r.stderr).into_owned())
    };
    let (code, stdout, err) = library(&["--format", "json"]);
    assert_eq!(code, 0, "{stdout}{err}");
    let answer = ritsu_base::json::parse(&stdout).expect("--format json writes one object");
    assert_eq!(answer.get("targets").map(Json::compact), Some(r#"["typescript","python","go","requests"]"#.to_string()));
    let mut listed: Vec<String> = answer
        .get("files")
        .and_then(Json::as_arr)
        .expect("the files")
        .iter()
        .map(|f| {
            let target = f.get("target").and_then(Json::as_str).unwrap();
            let path = f.get("path").and_then(Json::as_str).unwrap().strip_prefix(&format!("{o}/")).expect("under <out>").to_string();
            assert_eq!(f.get("state").and_then(Json::as_str), Some("written"), "{path}");
            assert!(path.starts_with(&format!("{target}/")), "{path} is listed as a file of {target}");
            path
        })
        .collect();
    listed.sort();
    assert_eq!(listed, files_under(&out), "--format json lists the files written");
    // each language's library holds the list of the kinds, which reads the glue by the library's name
    for (f, reads) in [
        ("typescript/ticket_kinds.ts", "from \"./glue/leave\""),
        ("python/acme_requests/ticket_kinds.py", "from acme_requests.glue import leave"),
        ("go/ticketkinds/ticketkinds.go", "\"example.com/acme/requests-go/glue/leave\""),
        ("typescript/index.ts", "export * as ticket_kinds from \"./ticket_kinds\""),
    ] {
        let text = std::fs::read_to_string(out.join(f)).unwrap_or_else(|_| panic!("{f} is not written"));
        assert!(text.contains(reads), "{f} does not say `{reads}`");
    }
    // requests once, beside the libraries: its files and no other
    let requests: Vec<String> = listed.iter().filter_map(|p| p.strip_prefix("requests/")).map(str::to_string).collect();
    let mut want: Vec<String> = requests_files().into_iter().filter(|p| !p.contains("expense")).collect();
    want.sort();
    assert_eq!(requests, want, "what `requests` writes");
    for f in ["openapi.json", "mcp/tools.json", "organization/schema.json", "organization/columns.md", "organization/find.jsonl", "vectors/leave.jsonl", "vectors/worked/leave.jsonl", "a2ui/catalog.json", "a2ui/functions.js"] {
        let at: Vec<&String> = listed.iter().filter(|p| p.ends_with(&format!("/{f}"))).collect();
        assert_eq!(at.len(), 1, "{f} is written once: {at:?}");
    }
    // the screens name the catalog by the `catalog` line of ritsu.mod
    let form = std::fs::read_to_string(out.join("requests/a2ui/leave/form.en.json")).unwrap();
    assert!(form.contains(&format!("\"@catalogId\": \"{CATALOG}\"")), "the screens do not name the catalog of ritsu.mod");
    let (code, vectors, err) = ritsu_in(&module, &["ringi", "vectors", "leave.ringi"]);
    assert_eq!(code, 0, "{err}");
    assert!(std::fs::read_to_string(out.join("requests/vectors/leave.jsonl")).unwrap() == vectors, "the vectors of the leave");
    // the same files again, and nothing to write
    let (code, stdout, err) = library(&["--check"]);
    assert_eq!((code, stdout.as_str()), (0, ""), "{err}");
}

/// The screens of `requests` name ritsu's catalog by its ID (ringi's DESIGN 13.2): a name for the
/// project's catalog written as a URL the project owns, which need not be where the catalog is
/// served and is the same in every environment, so only the project knows it: `--catalog <url>`,
/// else the `catalog` line of the module's `ritsu.mod`, `--catalog` first. With neither, `ritsu gen`
/// says how to give it and what it is, and writes nothing; an ID that is no http or https URL with
/// a host, or that has a user, a query or a fragment in it, is refused as the line is; and
/// `--catalog` without `requests` is refused too, for no screen would name it.
#[test]
fn the_screens_name_the_catalog_they_are_given() {
    let t = TempDir::new("gen-requests-catalog");
    lay(&t);
    let out = t.path().join("out");
    let o = out.to_string_lossy().into_owned();
    for (more, said) in [
        (
            &["--target", "requests"][..],
            "`requests` writes the screens too, which need the ID of the catalog they name: give it with `--catalog <url>`, or in a module with the line `catalog \"<url>\"` of its ritsu.mod. The ID is a name for this project's catalog, written as a URL you own. The place `requests/a2ui/catalog.json` is served at is the easiest to follow, but the two need not be the same. Use the same value in every environment (what is generated is not made apart for each)",
        ),
        (&["--target", "requests", "--catalog", "catalog.json"], "`--catalog` cannot be used: the catalog's ID `catalog.json` is an http or https URL with a host"),
        (&["--target", "requests", "--catalog", "https://requests.example.com/a2ui/catalog.json?v=2"], "has a query (from the `?` on) or a fragment (from the `#` on) in it"),
        (&["--target", "requests", "--catalog", "https://user:secret@requests.example.com/a2ui/catalog.json"], "has a user or a password in it"),
        (&["--target", "typescript", "--catalog", CATALOG], "`--catalog` goes with `--target requests`"),
    ] {
        let (code, stdout, err) = ritsu_in(t.path(), &[&["gen", "examples/leave.ringi", "--root", ".", "--out", &o][..], more].concat());
        assert!(code == 2 && format!("{stdout}{err}").contains(said), "{more:?}: {code}\n{stdout}{err}");
        assert!(!out.exists(), "{more:?}: something is written");
    }
    // a module with the line: the screens name its URL, and `--catalog`'s when it is given
    let module = t.path().join("module");
    for dir in ["shared", "org", "masters"] {
        ritsu_testkit::tmp::copy_dir(&t.path().join("examples").join(dir), &module.join(dir));
    }
    std::fs::copy(t.path().join("examples/leave.ringi"), module.join("leave.ringi")).unwrap();
    std::fs::write(module.join("ritsu.mod"), "module example.com/acme/requests\nversion 1.0.0\ncatalog \"https://screens.example.com/catalog.json\"\n").unwrap();
    for (more, named) in [(&[][..], "https://screens.example.com/catalog.json"), (&["--catalog", CATALOG], CATALOG)] {
        let (code, stdout, err) = ritsu_in(&module, &[&["gen", "leave.ringi", "--out", &o, "--target", "requests"][..], more].concat());
        assert_eq!(code, 0, "{more:?}: {stdout}{err}");
        let form = std::fs::read_to_string(out.join("requests/a2ui/leave/form.en.json")).unwrap();
        assert!(form.contains(&format!("\"@catalogId\": \"{named}\"")), "{more:?}: the screens do not name {named}");
        let functions = std::fs::read_to_string(out.join("requests/a2ui/functions.js")).unwrap();
        assert!(functions.contains(&format!("export const catalogId = \"{named}\";")), "{more:?}: the functions do not say {named}");
    }
}

/// A request whose number's range a screen cannot hold exactly (E704).
const PARTS: &str = r#"request parts v1 "Parts request"

use organization org from "shared/company.ringi"

form
  items : rows "Items"  count >=1 <=10
    what   : text "What"  count >=1 <=100
    serial : number "Serial number"  range >=1 <=99_999_999_999_999_999

route
  check  finance
    when empty escalate

who
  finance = team finance

ticket
  self approval  not allowed
"#;

/// A request whose `labels ja` has no label for a field the screens show (E703).
const TRIP: &str = r#"request trip v1 "Trip request"

use organization org from "shared/company.ringi"

form
  city   : text "City"  count >=1 <=100
  nights : number "Nights"  range >=1 <=30

route
  check  finance
    when empty escalate

who
  finance = team finance

ticket
  self approval  not allowed

labels ja
  request        "出張"
  city           "行き先"
  stage check    "確かめ"
  who finance    "財務"
"#;

/// A request whose screens alone cannot be made (E704) passes `ritsu check`: `ritsu gen --target
/// requests` stops on it saying the request cannot be made into that package, as a file a target
/// cannot be made from is said, not that it does not pass check (ringi's DESIGN 13.3), in English, in
/// Japanese and in JSON; nothing is written. The targets of the languages are made from it.
#[test]
fn what_the_screens_alone_refuse_is_said_as_the_target_refuses_it() {
    let t = TempDir::new("gen-requests-screens");
    lay(&t);
    std::fs::write(t.path().join("examples/parts.ringi"), PARTS).unwrap();
    let files = ["examples/parts.ringi"];
    let (code, stdout, err) = ritsu_in(t.path(), &[&["check"][..], &files, &["--root", "."]].concat());
    assert_eq!(code, 0, "ritsu check: {stdout}{err}");
    let out = t.path().join("out");
    let o = out.to_string_lossy().into_owned();
    let asked = [&["gen"][..], &files, &["--root", ".", "--out", &o, "--target", "requests", "--catalog", CATALOG]].concat();
    for (lang, last) in [
        ("en", "error: `examples/parts.ringi` cannot be made into the requests package"),
        ("ja", "エラー: `examples/parts.ringi` からは requests のパッケージを生成できません"),
    ] {
        let (code, stdout, err) = ritsu_in(t.path(), &[&asked[..], &["--lang", lang]].concat());
        let said = format!("{stdout}{err}");
        assert_eq!(code, 1, "{lang}: {said}");
        assert!(said.contains("E704]: examples/parts.ringi:8:1:"), "{lang}: {said}");
        assert_eq!(said.trim_end().lines().last(), Some(last), "{lang}: {said}");
        assert!(!out.exists(), "{lang}: something is written");
    }
    let (code, stdout, _) = ritsu_in(t.path(), &[&asked[..], &["--format", "json"]].concat());
    assert_eq!(code, 1, "{stdout}");
    let j = ritsu_base::json::parse(&stdout).unwrap();
    assert_eq!(j.get("error").and_then(|e| e.get("message")).and_then(Json::as_str), Some("`examples/parts.ringi` cannot be made into the requests package"), "{stdout}");
    let codes: Vec<&str> = j.get("diagnostics").and_then(Json::as_arr).unwrap().iter().filter_map(|d| d.get("code").and_then(Json::as_str)).collect();
    assert_eq!(codes, ["E704"], "{stdout}");
    assert!(!out.exists(), "json: something is written");
    let (code, stdout, err) = ritsu_in(t.path(), &[&["gen"][..], &files, &["--root", ".", "--out", &o, "--target", "python"]].concat());
    assert_eq!(code, 0, "--target python: {stdout}{err}");
}

/// A label the screens show that a language the file declares labels in lacks (E703) is said by
/// `ritsu check`, at the `labels` line (ringi's DESIGN 4.8): the file says what its pages are in, and
/// it is found where the file is checked, not first where its screens are made.
#[test]
fn a_label_the_screens_lack_is_said_by_the_check() {
    let t = TempDir::new("gen-requests-labels");
    lay(&t);
    std::fs::write(t.path().join("examples/trip.ringi"), TRIP).unwrap();
    for lang in ["en", "ja"] {
        let (code, stdout, err) = ritsu_in(t.path(), &["check", "examples/trip.ringi", "--root", ".", "--lang", lang]);
        let said = format!("{stdout}{err}");
        assert_eq!(code, 1, "{lang}: {said}");
        assert!(said.contains("E703]: examples/trip.ringi:19:1:") && said.contains("`nights`"), "{lang}: {said}");
    }
}

/// A request whose code is not written yet (here one that reads no organization, whose tickets have
/// no one to go to) is said whichever targets leave it out, a warning each: the packages of
/// languages leave its tickets' code out, and `requests` does not describe it, alone as with them
/// (ringi's DESIGN 12.1).
#[test]
fn a_request_left_out_is_said_by_the_requests_target_too() {
    let t = TempDir::new("gen-requests-left-out");
    std::fs::write(
        t.path().join("bare.ringi"),
        "request bare v1 \"Bare\"\n\nform\n  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY\n\nroute\n  check  finance\n\nwho\n  finance = team finance\n\nticket\n  self approval  not allowed\n",
    )
    .unwrap();
    let (code, stdout, err) = ritsu_in(t.path(), &["check", "bare.ringi", "--root", "."]);
    assert_eq!(code, 0, "ritsu check: {stdout}{err}");
    for (targets, en, ja) in [
        (
            &["requests"][..],
            "warning: `requests` does not describe `bare.ringi` (the request reads no organization); the other parts are written",
            "警告: `bare.ringi` は `requests` の説明に入りません（申請の種類が組織を読みません）。ほかの部分は書きます",
        ),
        (
            &["python", "requests"][..],
            "warning: The code of the tickets of `bare.ringi` is not written, nor does `requests` describe it (the request reads no organization); the other parts are written",
            "警告: `bare.ringi` のチケットのコードは書かず、`requests` の説明にも入りません（申請の種類が組織を読みません）。ほかの部分は書きます",
        ),
        (
            &["python"][..],
            "warning: The code of the tickets of `bare.ringi` is not written (the request reads no organization); the other parts are written",
            "警告: `bare.ringi` のチケットのコードは書きません（申請の種類が組織を読みません）。ほかの部分は書きます",
        ),
    ] {
        let mut asked: Vec<&str> = vec!["gen", "bare.ringi", "--root", ".", "--out", "out"];
        for t in targets {
            asked.extend(["--target", t]);
        }
        if targets.contains(&"requests") {
            asked.extend(["--catalog", CATALOG]);
        }
        for (lang, want) in [("en", en), ("ja", ja)] {
            let (code, stdout, err) = ritsu_in(t.path(), &[&asked[..], &["--lang", lang]].concat());
            assert_eq!(code, 0, "{targets:?} {lang}: {stdout}{err}");
            let said: Vec<&str> = stdout.lines().filter(|l| !l.starts_with("generated:") && !l.starts_with("生成しました:")).collect();
            assert_eq!(said, [want], "{targets:?} {lang}: {stdout}{err}");
        }
        assert!(!t.path().join("out/requests").exists(), "{targets:?}: the request is described");
        let (code, stdout, _) = ritsu_in(t.path(), &[&asked[..], &["--format", "json"]].concat());
        assert_eq!(code, 0, "{targets:?} json: {stdout}");
        let j = ritsu_base::json::parse(&stdout).unwrap();
        let said: Vec<(&str, &str, &str)> = j
            .get("diagnostics")
            .and_then(Json::as_arr)
            .unwrap()
            .iter()
            .map(|d| (d.get("file").and_then(Json::as_str).unwrap_or(""), d.get("severity").and_then(Json::as_str).unwrap_or(""), d.get("message").and_then(Json::as_str).unwrap_or("")))
            .collect();
        assert_eq!(said, [("bare.ringi", "warning", en.trim_start_matches("warning: "))], "{targets:?} json: {stdout}");
        // `requests` alone writes nothing, for it describes no kind
        if t.path().join("out").exists() {
            std::fs::remove_dir_all(t.path().join("out")).unwrap();
        }
    }
}

/// The files under `dir`, by their paths from it, in order.
fn files_under(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                todo.push(p);
            } else {
                out.push(p.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out.sort();
    out
}

/// Every `$ref` under `j`.
fn references(j: &Json, out: &mut Vec<String>) {
    match j {
        Json::Obj(o) => {
            for (k, v) in o {
                match (k.as_str(), v.as_str()) {
                    ("$ref", Some(r)) => out.push(r.to_string()),
                    _ => references(v, out),
                }
            }
        }
        Json::Arr(a) => a.iter().for_each(|x| references(x, out)),
        _ => {}
    }
}

/// A computation the generated code cannot work out in int64 stops `ritsu gen` (ringi's DESIGN 4.4,
/// 9.2): the check says E201 at the field, whether it writes a range the check reads it within or
/// none, and nothing is written.
#[test]
fn a_computation_past_int64_in_the_generated_code_stops_gen() {
    for (name, range) in [("ranged", "  range >=1JPY <=100_000_000JPY"), ("open", "")] {
        let t = TempDir::new("gen-requests-int64");
        let src = format!(
            "request big v1 \"Big purchase\"\n\nform\n  lines  : rows \"Lines\"  count >=1 <=30\n    quantity : number \"Quantity\"  range >=1 <=1_000_000\n    price    : money[JPY] \"Unit price\"  range >=1JPY <=1_000_000_000_000JPY\n    subtotal : money[JPY] \"Subtotal\" = quantity * price  range >=1JPY <=100_000_000JPY\n  amount : money[JPY] \"Amount\" = sum(lines.subtotal){range}\n\nroute\n  check  finance\n\nwho\n  finance = team finance\n\nticket\n  self approval  not allowed\n"
        );
        std::fs::write(t.path().join("big.ringi"), src).unwrap();
        let out = t.path().join("out");
        let o = out.to_string_lossy().into_owned();
        let (code, stdout, err) = ritsu_in(t.path(), &["gen", "big.ringi", "--root", ".", "--out", &o, "--target", "python"]);
        assert_ne!(code, 0, "{name}: {stdout}{err}");
        let said = format!("{stdout}{err}");
        assert!(said.contains("E201") && said.contains("big.ringi:8:") && said.contains("past int64"), "{name}: {said}");
        assert!(said.contains("nothing is generated"), "{name}: {said}");
        assert!(!out.exists() || std::fs::read_dir(&out).unwrap().next().is_none(), "{name}: something is written");
    }
}

/// A host of `<out>/requests/kinds.wasm` in Node, as a server is one (ringi's DESIGN 12.6): it reads
/// `wasm.json`, refuses a module whose SHA-256 is not the one written there or that imports
/// anything, and calls each line of a file of [`call_lines`] through the door as `wasm.json` says
/// (the function and the parts of the entry, `now` where it takes one, the answer of a kind whose
/// form works nothing out). Each part is first held to what the crate's own entries hold it to
/// (kaname's DESIGN 16.4): nested no deeper than 512, counted outside its strings, then one value
/// of JSON. It prints the language of `wasm.json`, then a line for each call: the value, or
/// `{kind, error, path}` (what the door says fails or refuses, by its kind).
const KINDS_HOST: &str = r#"import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";

const [dir, calls] = process.argv.slice(2);
const about = JSON.parse(readFileSync(dir + "/wasm.json", "utf8"));
const bytes = readFileSync(dir + "/" + about.file);
if (createHash("sha256").update(bytes).digest("hex") !== about.sha256) throw new Error("the module is not the one wasm.json says");
const mod = new WebAssembly.Module(bytes);
if (WebAssembly.Module.imports(mod).length !== 0) throw new Error("the module imports " + JSON.stringify(WebAssembly.Module.imports(mod)));
const ex = new WebAssembly.Instance(mod, {}).exports;
const [call, realloc, post, memory] = [ex[about.exports.call], ex[about.exports.realloc], ex[about.exports.post_return], ex[about.exports.memory]];
const enc = new TextEncoder();
const dec = new TextDecoder("utf-8", { fatal: true });
// numbers as they are written, so that an integer past 2^53 is printed as the door wrote it
const read = (s) => JSON.parse(s, (_k, v, ctx) => (typeof v === "number" ? JSON.rawJSON(ctx.source) : v));
function door(line) {
  const b = enc.encode(line);
  const p = realloc(0, 0, 1, b.length);
  new Uint8Array(memory.buffer, p, b.length).set(b);
  const r = call(p, b.length);
  const v = new DataView(memory.buffer);
  const s = dec.decode(new Uint8Array(memory.buffer, v.getUint32(r, true), v.getUint32(r + 4, true)).slice());
  post(r);
  return read(s);
}
function deep(text) {
  let depth = 0;
  let inString = false;
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i);
    if (inString) {
      if (c === 92) i++;
      else if (c === 34) inString = false;
    } else if (c === 34) inString = true;
    else if (c === 91 || c === 123) {
      if (++depth > 512) return true;
    } else if ((c === 93 || c === 125) && depth > 0) depth--;
  }
  return false;
}
const ja = about.lang === "ja";
const said = (name, fun, tooDeep) =>
  ja
    ? "関数 `" + fun + "` の引数 `" + name + "` は、" + (tooDeep ? "512 段より深く入れ子になっています" : "一つの JSON の値ではありません")
    : "The argument `" + name + "` of `" + fun + "` " + (tooDeep ? "nests deeper than 512" : "is not one value of JSON");
console.log(about.lang);
for (const line of readFileSync(calls, "utf8").split("\n")) {
  if (line === "") continue;
  const [name, entry, now, ...parts] = line.split("	");
  const k = about.kinds.find((x) => x.name === name);
  if (k === undefined) {
    console.log(JSON.stringify({ kind: "other", error: "no such kind: " + name, path: null }));
    continue;
  }
  const f = entry === "start" ? k.start : entry === "worked" ? k.worked : k.next;
  let wrong = null;
  const ins = [];
  for (const [i, n] of f.in.entries()) {
    if (deep(parts[i])) wrong = { kind: "input", error: said(n, f.function, true), path: n };
    else {
      try {
        JSON.parse(parts[i]);
      } catch {
        wrong = { kind: "input", error: said(n, f.function, false), path: n };
      }
    }
    if (wrong !== null) break;
    ins.push(JSON.stringify(n) + ":" + parts[i]);
  }
  if (wrong !== null) {
    console.log(JSON.stringify(wrong));
    continue;
  }
  if (f.now) ins.push('"now":' + now);
  const a = door('{"module":' + JSON.stringify(k.module) + ',"function":' + JSON.stringify(f.function) + ',"in":{' + ins.join(",") + "}}");
  if ("out" in a) console.log(JSON.stringify("answer" in f ? f.answer : a.out));
  else if ("fails" in a) console.log(JSON.stringify({ kind: a.fails.kind, error: a.fails.message, path: a.fails.path ?? null }));
  else if ("refused" in a) console.log(JSON.stringify({ kind: a.refused, error: a.error, path: a.path ?? null }));
  else console.log(JSON.stringify({ kind: "other", error: JSON.stringify(a), path: null }));
}
"#;

/// Whether the Wasm of the requests is built and run: rustc's level, with the standard library of
/// wasm32-unknown-unknown, and Node's (a SKIP saying which is missing when not).
fn wasm_ready() -> bool {
    ready(Need::Rustc, have_wasm32, "the standard library of wasm32-unknown-unknown is not installed (rustup target add wasm32-unknown-unknown); the Wasm of the requests is not built") && ritsu_testkit::need(Need::Node)
}

/// The calls of `calls` (a file of [`call_lines`]) through `<requests>/kinds.wasm` by [`KINDS_HOST`],
/// in `dir`: the language it says, and a line for each call.
fn wasm_calls(dir: &Path, requests: &Path, calls: &Path) -> (String, String) {
    std::fs::create_dir_all(dir).unwrap();
    let host = dir.join("wasm_host.mjs");
    std::fs::write(&host, KINDS_HOST).unwrap();
    let o = ritsu_testkit::run::finish(Command::new("node").arg(&host).arg(requests).arg(calls), RUN, "node, running the host of the Wasm");
    assert!(o.ok, "the host of the Wasm:\n{}", o.both());
    let (lang, rest) = o.stdout.split_once('\n').unwrap_or((o.stdout.as_str(), ""));
    (lang.to_string(), rest.to_string())
}

/// `ritsu`, run in `dir` as [`ritsu_in`] runs it, with `env` set (a value of None taken away).
fn ritsu_with(dir: &Path, args: &[&str], env: &[(&str, Option<&std::ffi::OsStr>)]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(dir).args(args);
    for v in common::LANG_VARS {
        c.env_remove(v);
    }
    for (k, v) in env {
        match v {
            Some(v) => c.env(k, v),
            None => c.env_remove(k),
        };
    }
    let o = c.output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The drivers of [`json_out_of_the_contract_is_said_alike_in_the_four_languages`]: as [`TS_DRIVER`]
/// and the others, but first the language the list of the kinds says its entries' sentences are in,
/// and what stops a call printed as `{"kind": …, "error": …, "path": …}`, the kind, the message and
/// the place of the error of kaname (`other` for any other error; Rust's entries give the sentence and
/// the place, its `EntryError`), the place null where the error has none.
const TS_WRONG_DRIVER: &str = r#"import { readFileSync } from "node:fs";
import { KanameError } from "./glue/kaname";
import { kinds, lang } from "./ticket_kinds";

console.log(lang);
for (const line of readFileSync(process.argv[2], "utf8").split("\n")) {
  if (line === "") continue;
  const [name, entry, now, ...parts] = line.split("\t");
  const k = kinds.find((x) => x.name === name);
  try {
    if (k === undefined) throw new Error("no such kind");
    if (entry === "start") console.log(k.start(parts[0], parts[1], BigInt(now)));
    else if (entry === "worked") console.log(k.worked(parts[0]));
    else console.log(k.next(parts[0], parts[1], parts[2], BigInt(now)));
  } catch (e) {
    console.log(JSON.stringify(e instanceof KanameError ? { kind: e.kind, error: e.message, path: e.path ?? null } : { kind: "other", error: String(e), path: null }));
  }
}
"#;

const PY_WRONG_DRIVER: &str = r#"import json
import sys

from generated.glue.kaname import KanameError
from generated.ticket_kinds import KINDS, LANG

print(LANG)
with open(sys.argv[1], encoding="utf-8") as f:
    for line in f:
        name, entry, now, *parts = line.rstrip("\n").split("\t")
        try:
            k = next(x for x in KINDS if x.name == name)
            if entry == "start":
                print(k.start(parts[0], parts[1], int(now)))
            elif entry == "worked":
                print(k.worked(parts[0]))
            else:
                print(k.next(parts[0], parts[1], parts[2], int(now)))
        except KanameError as e:
            print(json.dumps({"kind": e.kind, "error": e.message, "path": e.path}))
        except Exception as e:  # noqa: BLE001
            print(json.dumps({"kind": "other", "error": repr(e), "path": None}))
"#;

fn go_wrong_driver() -> String {
    format!(
        r#"package main

import (
	"bufio"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"strconv"
	"strings"

	"{MODULE}/glue/kaname"
	"{MODULE}/ticketkinds"
)

func main() {{
	f, err := os.Open(os.Args[1])
	if err != nil {{
		panic(err)
	}}
	sc := bufio.NewScanner(f)
	sc.Buffer(make([]byte, 1<<20), 1<<28)
	fmt.Println(ticketkinds.Lang)
	for sc.Scan() {{
		fs := strings.Split(sc.Text(), "\t")
		now, _ := strconv.ParseInt(fs[2], 10, 64)
		var out json.RawMessage
		err := fmt.Errorf("no such kind")
		for _, k := range ticketkinds.Kinds {{
			if k.Name != fs[0] {{
				continue
			}}
			if fs[1] == "start" {{
				out, err = k.Start(json.RawMessage(fs[3]), json.RawMessage(fs[4]), now)
			}} else if fs[1] == "worked" {{
				out, err = k.Worked(json.RawMessage(fs[3]))
			}} else {{
				out, err = k.Next(json.RawMessage(fs[3]), json.RawMessage(fs[4]), json.RawMessage(fs[5]), now)
			}}
		}}
		if err != nil {{
			said := map[string]any{{"kind": "other", "error": err.Error(), "path": nil}}
			var k *kaname.KanameError
			if errors.As(err, &k) {{
				said = map[string]any{{"kind": k.Kind, "error": k.Message, "path": nil}}
				if k.Path != "" {{
					said["path"] = k.Path
				}}
			}}
			b, _ := json.Marshal(said)
			fmt.Println(string(b))
			continue
		}}
		fmt.Println(string(out))
	}}
}}
"#
    )
}

const RUST_WRONG_DRIVER: &str = r#"fn main() {
    let path = std::env::args().nth(1).expect("the file of the calls");
    let text = std::fs::read_to_string(path).expect("the file of the calls");
    println!("{}", generated::ticket_kinds::LANG);
    for line in text.lines() {
        let fs: Vec<&str> = line.split('\t').collect();
        let now: i64 = fs[2].parse().expect("the time");
        let answer = match generated::ticket_kinds::KINDS.iter().find(|k| k.name == fs[0]) {
            None => Err(generated::ticket_kinds::EntryError { path: None, message: "no such kind".to_string() }),
            Some(k) if fs[1] == "start" => (k.start)(fs[3], fs[4], now),
            Some(k) if fs[1] == "worked" => (k.worked)(fs[3]),
            Some(k) => (k.next)(fs[3], fs[4], fs[5], now),
        };
        match answer {
            Ok(a) => println!("{a}"),
            Err(e) => println!("{{\"error\":{},\"path\":{}}}", quoted(&e.message), e.path.as_deref().map_or("null".to_string(), quoted)),
        }
    }
}

/// A string as JSON writes it.
fn quoted(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}
"#;

/// The place a break at the pointer `at` of the argument `arg` is at, as an error holds it: the
/// argument's name, then `.key` for each key and `[i]` for each index (`organization.members[0].primary`).
fn place(arg: &str, at: &str) -> String {
    let mut out = arg.to_string();
    for step in at.split('/').skip(1) {
        match step.parse::<usize>() {
            Ok(i) => out.push_str(&format!("[{i}]")),
            Err(_) => out.push_str(&format!(".{step}")),
        }
    }
    out
}

/// What the entries of the package's list of the kinds of request say of JSON out of the contract
/// (ringi's DESIGN 12.4, kaname's DESIGN 16.4): the first request of the leave the reference answers
/// and the first approval, each broken at one place (a key taken out, a value of another type, a
/// word of no enum, an event of no name, a text that is no JSON), given to the code of the four
/// languages, in English and in Japanese, and a time outside its range. Each says the same sentence:
/// the place from the argument's name, what is there and what it must be, an error of kaname of the
/// kind `input` (Rust's entries an `EntryError`), and holds the same place apart from the sentence
/// (`path`: the argument's name, then each key and index the break is at); what is in the contract is
/// answered. Each list of the kinds says the language of its sentences (`lang`, `LANG`, `Lang`) as
/// the one it was written in.
#[test]
fn json_out_of_the_contract_is_said_alike_in_the_four_languages() {
    use serde_json::json;
    let t = TempDir::new("gen-requests-wrong");
    lay(&t);
    let kind = kind_of(t.path(), "examples/leave.ringi");
    let (code, vectors, err) = ritsu_in(t.path(), &["ringi", "vectors", "examples/leave.ringi"]);
    assert_eq!(code, 0, "{err}");
    let (mut submit, mut approve) = (None, None);
    for line in vectors.lines() {
        let v = ritsu_base::json::parse(line).unwrap();
        let (entry, input, answer) = ringi::lower::shape::call_of(&v).unwrap();
        let ok = answer.get("ok").is_some();
        if entry == "start" && ok && submit.is_none() {
            submit = Some(input);
        } else if entry == "next" && ok && approve.is_none() && input.get("event").and_then(|e| e.get("approve")).is_some() {
            approve = Some(input);
        }
    }
    let (submit, approve) = (submit.expect("a request the reference takes"), approve.expect("an approval the reference takes"));
    // what the entry `worked` takes (ringi's DESIGN 13.7): the form of that request, and no edit
    let form = submit.get("submit").and_then(|s| s.get("form")).cloned().expect("a submit's form");
    let worked = Json::obj([("input", Json::obj([("form", form), ("edited", Json::Null)])), ("now", Json::int(0))]);
    let part = |input: &Json, n: &str| -> serde_json::Value { serde_json::from_str(&input.get(n).cloned().unwrap_or(Json::Null).compact()).unwrap() };
    enum Do {
        Keep,
        Drop,
        Set(serde_json::Value),
        Raw(&'static str),
        /// The value at the place written as given, as an encoder of JSON may write it.
        Spell(String),
        /// The key of the place written twice: the first value, then the later (None: the part's own),
        /// which every reader reads (as JavaScript's `JSON.parse` does).
        Twice(Option<serde_json::Value>, Option<serde_json::Value>),
        /// The time given instead of the request's, the parts kept.
        Now(i64),
    }
    let long = "a".repeat(45);
    let statuses = r#""in_progress", "paused", "with_requester", "approved", "rejected", "withdrawn", "expired""#;
    let statuses_ja = r#""in_progress"、"paused"、"with_requester"、"approved"、"rejected"、"withdrawn"、"expired""#;
    let events = r#""resubmit", "approve", "reject", "send_back", "retract", "pause", "resume", "withdraw", "discard", "edit", "add_approver", "hand_over", "comment", "remind", "late", "resubmit_late", "pause_ends", "agent_answered", "note_ready", "flow_failed""#;
    let events_ja = events.replace("\", \"", "\"、\"");
    let clocks = r#""remind", "late", "resubmit_late", "pause_ends""#;
    let clocks_ja = clocks.replace("\", \"", "\"、\"");
    // the case, the entry and the part it breaks, where and how; what is said in English and in
    // Japanese (None: the answer of the contract)
    let cases: Vec<(&str, &str, &str, &str, Do, Option<String>, Option<String>)> = vec![
        ("as given", "start", "submit", "", Do::Keep, None, None),
        (
            "a key taken out: submit.requester",
            "start",
            "submit",
            "/requester",
            Do::Drop,
            Some("The argument `submit.requester` of `start` is missing, and must be a string".into()),
            Some("関数 `start` の引数 `submit.requester` がありません。文字列が要ります".into()),
        ),
        (
            "a key taken out: submit.form",
            "start",
            "submit",
            "/form",
            Do::Drop,
            Some("The argument `submit.form` of `start` is missing, and must be an object".into()),
            Some("関数 `start` の引数 `submit.form` がありません。オブジェクトが要ります".into()),
        ),
        (
            "text for a number: submit.form.days",
            "start",
            "submit",
            "/form/days",
            Do::Set(json!("3")),
            Some("The argument `submit.form.days` of `start` is \"3\", and must be an integer".into()),
            Some("関数 `start` の引数 `submit.form.days` が \"3\" です。整数が要ります".into()),
        ),
        (
            "a word for a number: submit.form.days",
            "start",
            "submit",
            "/form/days",
            Do::Set(json!("three")),
            Some("The argument `submit.form.days` of `start` is \"three\", and must be an integer".into()),
            Some("関数 `start` の引数 `submit.form.days` が \"three\" です。整数が要ります".into()),
        ),
        (
            "a fraction for a number: submit.form.days",
            "start",
            "submit",
            "/form/days",
            Do::Set(json!(2.5)),
            Some("The argument `submit.form.days` of `start` is 2.5, and must be an integer".into()),
            Some("関数 `start` の引数 `submit.form.days` が 2.5 です。整数が要ります".into()),
        ),
        // a whole number in another spelling is the number
        ("a whole number with a point: submit.form.days", "start", "submit", "/form/days", Do::Set(json!(3.0)), None, None),
        ("a whole number with an exponent: submit.form.days", "start", "submit", "/form/days", Do::Spell("3e0".into()), None, None),
        (
            "a long text for a number: submit.form.days",
            "start",
            "submit",
            "/form/days",
            Do::Set(json!(long)),
            Some(format!("The argument `submit.form.days` of `start` is \"{}\u{2026}\", and must be an integer", "a".repeat(40))),
            Some(format!("関数 `start` の引数 `submit.form.days` が \"{}\u{2026}\" です。整数が要ります", "a".repeat(40))),
        ),
        (
            "no date: submit.form.first_day",
            "start",
            "submit",
            "/form/first_day",
            Do::Set(json!("2026-13-40")),
            Some("The argument `submit.form.first_day` of `start` is 2026-13-40, outside its range >=0001-01-01 <=9999-12-31".into()),
            Some("関数 `start` の引数 `submit.form.first_day` の 2026-13-40 は、範囲 >=0001-01-01 <=9999-12-31 の外です".into()),
        ),
        (
            "a date not written YYYY-MM-DD: submit.form.first_day",
            "start",
            "submit",
            "/form/first_day",
            Do::Set(json!("2026-4-1")),
            Some("The argument `submit.form.first_day` of `start` is \"2026-4-1\", and must be a date written YYYY-MM-DD".into()),
            Some("関数 `start` の引数 `submit.form.first_day` が \"2026-4-1\" です。YYYY-MM-DD と書いた日付が要ります".into()),
        ),
        (
            "a number for text: submit.requester",
            "start",
            "submit",
            "/requester",
            Do::Set(json!(5)),
            Some("The argument `submit.requester` of `start` is 5, and must be a string".into()),
            Some("関数 `start` の引数 `submit.requester` が 5 です。文字列が要ります".into()),
        ),
        (
            "a number for a choice: submit.form.kind",
            "start",
            "submit",
            "/form/kind",
            Do::Set(json!(5)),
            Some("The argument `submit.form.kind` of `start` is 5, and must be a string".into()),
            Some("関数 `start` の引数 `submit.form.kind` が 5 です。文字列が要ります".into()),
        ),
        (
            "a field of an element broken: organization.people[0].name",
            "start",
            "organization",
            "/people/0/name",
            Do::Set(json!(5)),
            Some("The argument `organization.people[0].name` of `start` is 5, and must be a string".into()),
            Some("関数 `start` の引数 `organization.people[0].name` が 5 です。文字列が要ります".into()),
        ),
        (
            "an object for a list: organization.teams",
            "start",
            "organization",
            "/teams",
            Do::Set(json!({})),
            Some("The argument `organization.teams` of `start` is an object, and must be an array".into()),
            Some("関数 `start` の引数 `organization.teams` がオブジェクトです。配列が要ります".into()),
        ),
        (
            "text for a bool: organization.members[0].primary",
            "start",
            "organization",
            "/members/0/primary",
            Do::Set(json!("yes")),
            Some("The argument `organization.members[0].primary` of `start` is \"yes\", and must be true or false".into()),
            Some("関数 `start` の引数 `organization.members[0].primary` が \"yes\" です。true か false が要ります".into()),
        ),
        (
            "null for text: submit.requester",
            "start",
            "submit",
            "/requester",
            Do::Set(json!(null)),
            Some("The argument `submit.requester` of `start` is null, and must be a string".into()),
            Some("関数 `start` の引数 `submit.requester` が null です。文字列が要ります".into()),
        ),
        // a string that is no Unicode: an escape of a lone surrogate, which every entry reads as it
        // is (Go's own reader would read U+FFFD), so that the module's check refuses it; a sentence
        // that shows such a string shows U+FFFD in the surrogate's place
        (
            "a lone surrogate: submit.requester",
            "start",
            "submit",
            "/requester",
            Do::Spell(r#""e\ud8001""#.into()),
            Some("The argument `submit.requester` of `start` is no well-formed string of Unicode".into()),
            Some("関数 `start` の引数 `submit.requester` は、Unicode の文字列として正しくありません".into()),
        ),
        (
            "a lone low surrogate after a pair: submit.requester",
            "start",
            "submit",
            "/requester",
            Do::Spell(r#""\ud83d\ude00\udc00""#.into()),
            Some("The argument `submit.requester` of `start` is no well-formed string of Unicode".into()),
            Some("関数 `start` の引数 `submit.requester` は、Unicode の文字列として正しくありません".into()),
        ),
        (
            "a lone surrogate for a number: submit.form.days",
            "start",
            "submit",
            "/form/days",
            Do::Spell(r#""\ud800""#.into()),
            Some("The argument `submit.form.days` of `start` is \"\u{fffd}\", and must be an integer".into()),
            Some("関数 `start` の引数 `submit.form.days` が \"\u{fffd}\" です。整数が要ります".into()),
        ),
        // a key written twice: the later is read
        ("a key written twice, the later of the contract: submit.requester", "start", "submit", "/requester", Do::Twice(Some(json!(5)), None), None, None),
        (
            "a key written twice, the later out of it: submit.requester",
            "start",
            "submit",
            "/requester",
            Do::Twice(None, Some(json!(5))),
            Some("The argument `submit.requester` of `start` is 5, and must be a string".into()),
            Some("関数 `start` の引数 `submit.requester` が 5 です。文字列が要ります".into()),
        ),
        // an integer of more digits than int64 holds, as it is written
        (
            "a long integer for text: submit.requester",
            "start",
            "submit",
            "/requester",
            Do::Spell(format!("1{}", "0".repeat(24))),
            Some(format!("The argument `submit.requester` of `start` is 1{}, and must be a string", "0".repeat(24))),
            Some(format!("関数 `start` の引数 `submit.requester` が 1{} です。文字列が要ります", "0".repeat(24))),
        ),
        (
            "no JSON: submit",
            "start",
            "submit",
            "",
            Do::Raw("{"),
            Some("The argument `submit` of `start` is not one value of JSON".into()),
            Some("関数 `start` の引数 `submit` は、一つの JSON の値ではありません".into()),
        ),
        (
            "text for a time: ticket.stages[0].due",
            "next",
            "ticket",
            "/stages/0/due",
            Do::Set(json!("tomorrow")),
            Some("The argument `ticket.stages[0].due` of `next` is \"tomorrow\", and must be an integer".into()),
            Some("関数 `next` の引数 `ticket.stages[0].due` が \"tomorrow\" です。整数が要ります".into()),
        ),
        (
            "text for a whole number: ticket.round",
            "next",
            "ticket",
            "/round",
            Do::Set(json!("1")),
            Some("The argument `ticket.round` of `next` is \"1\", and must be an integer".into()),
            Some("関数 `next` の引数 `ticket.round` が \"1\" です。整数が要ります".into()),
        ),
        (
            "past the machine's limit: ticket.round",
            "next",
            "ticket",
            "/round",
            Do::Set(json!(1_000_001)),
            Some("The argument `ticket.round` of `next` is 1_000_001, outside its range >=1 <=1_000_000".into()),
            Some("関数 `next` の引数 `ticket.round` の 1_000_001 は、範囲 >=1 <=1_000_000 の外です".into()),
        ),
        (
            "past int64: ticket.round",
            "next",
            "ticket",
            "/round",
            Do::Set(json!(1e30)),
            Some("The argument `ticket.round` of `next` is outside int64".into()),
            Some("関数 `next` の引数 `ticket.round` が int64 の外にあります".into()),
        ),
        // an integer past int64 written as an integer, which TypeScript and Python can hold: past it
        // by one, far past it, and of more digits than Python reads from a text (4300)
        (
            "an integer past int64: ticket.round",
            "next",
            "ticket",
            "/round",
            Do::Spell("9223372036854775808".into()),
            Some("The argument `ticket.round` of `next` is outside int64".into()),
            Some("関数 `next` の引数 `ticket.round` が int64 の外にあります".into()),
        ),
        (
            "a negative integer past int64: ticket.round",
            "next",
            "ticket",
            "/round",
            Do::Spell("-100000000000000000000".into()),
            Some("The argument `ticket.round` of `next` is outside int64".into()),
            Some("関数 `next` の引数 `ticket.round` が int64 の外にあります".into()),
        ),
        (
            "an integer of 5000 digits: ticket.round",
            "next",
            "ticket",
            "/round",
            Do::Spell(format!("1{}", "0".repeat(4999))),
            Some("The argument `ticket.round` of `next` is outside int64".into()),
            Some("関数 `next` の引数 `ticket.round` が int64 の外にあります".into()),
        ),
        (
            "a key taken out: ticket.status",
            "next",
            "ticket",
            "/status",
            Do::Drop,
            Some(format!("The argument `ticket.status` of `next` is missing, and must be one of {statuses}")),
            Some(format!("関数 `next` の引数 `ticket.status` がありません。{statuses_ja} のどれかが要ります")),
        ),
        (
            "a word not of the contract: ticket.status",
            "next",
            "ticket",
            "/status",
            Do::Set(json!("done")),
            Some(format!("The argument `ticket.status` of `next` is \"done\", and must be one of {statuses}")),
            Some(format!("関数 `next` の引数 `ticket.status` が \"done\" です。{statuses_ja} のどれかが要ります")),
        ),
        (
            "an event not of the contract",
            "next",
            "event",
            "",
            Do::Set(json!({"aprove": {"by": "e1101"}})),
            Some(format!("The argument `event` of `next` is an object with the key \"aprove\", and must have one key of {events}")),
            Some(format!("関数 `next` の引数 `event` がキー \"aprove\" のオブジェクトです。キーを {events_ja} のどれか一つだけ持つオブジェクトが要ります")),
        ),
        (
            "a name for an event of a value",
            "next",
            "event",
            "",
            Do::Set(json!("approve")),
            Some(format!("The argument `event` of `next` is \"approve\", and must be one of {clocks}, or an object with one key of {events}")),
            Some(format!("関数 `next` の引数 `event` が \"approve\" です。{clocks_ja} のどれか、またはキーを {events_ja} のどれか一つだけ持つオブジェクトが要ります")),
        ),
        (
            "an event of two names",
            "next",
            "event",
            "",
            Do::Set(json!({"approve": {"by": "e1101"}, "reject": {"by": "e1101"}})),
            Some(format!("The argument `event` of `next` is an object with 2 keys, and must have one key of {events}")),
            Some(format!("関数 `next` の引数 `event` がキーを 2 個持つオブジェクトです。キーを {events_ja} のどれか一つだけ持つオブジェクトが要ります")),
        ),
        ("an event's key written twice, the later of the contract", "next", "event", "/approve", Do::Twice(Some(json!(5)), None), None, None),
        (
            "a key taken out: event.approve.by",
            "next",
            "event",
            "/approve/by",
            Do::Drop,
            Some("The argument `event.approve.by` of `next` is missing, and must be a string".into()),
            Some("関数 `next` の引数 `event.approve.by` がありません。文字列が要ります".into()),
        ),
        (
            "no JSON: organization",
            "next",
            "organization",
            "",
            Do::Raw(""),
            Some("The argument `organization` of `next` is not one value of JSON".into()),
            Some("関数 `next` の引数 `organization` は、一つの JSON の値ではありません".into()),
        ),
        (
            "two values of JSON: organization",
            "next",
            "organization",
            "",
            Do::Raw("{} {}"),
            Some("The argument `organization` of `next` is not one value of JSON".into()),
            Some("関数 `next` の引数 `organization` は、一つの JSON の値ではありません".into()),
        ),
        // the entry `worked`: a value of the form's type is answered whatever its field's range,
        // and what is no value of it is said as the other entries say it
        ("worked: as given", "worked", "input", "", Do::Keep, None, None),
        ("worked: a value outside its field's range: input.form.days", "worked", "input", "/form/days", Do::Set(json!(1000)), None, None),
        ("worked: a key taken out: input.edited", "worked", "input", "/edited", Do::Drop, None, None),
        (
            "worked: a key taken out: input.form",
            "worked",
            "input",
            "/form",
            Do::Drop,
            Some("The argument `input.form` of `worked_out` is missing, and must be an object".into()),
            Some("関数 `worked_out` の引数 `input.form` がありません。オブジェクトが要ります".into()),
        ),
        (
            "worked: text for a number: input.form.days",
            "worked",
            "input",
            "/form/days",
            Do::Set(json!("3")),
            Some("The argument `input.form.days` of `worked_out` is \"3\", and must be an integer".into()),
            Some("関数 `worked_out` の引数 `input.form.days` が \"3\" です。整数が要ります".into()),
        ),
        (
            "worked: no JSON: input",
            "worked",
            "input",
            "",
            Do::Raw("{"),
            Some("The argument `input` of `worked_out` is not one value of JSON".into()),
            Some("関数 `worked_out` の引数 `input` は、一つの JSON の値ではありません".into()),
        ),
        // an argument nested as deep as the entries read, and one deeper (kaname's DESIGN 16.4): the
        // deeper is said so in every language before it is read as JSON, so that one opened past the
        // depth and never closed is said so too; brackets in a string, after an escaped quote, are
        // no nesting
        (
            "nested 512 deep: submit",
            "start",
            "submit",
            "",
            Do::Spell(format!("{}{}", "[".repeat(512), "]".repeat(512))),
            Some("The argument `submit` of `start` is an array, and must be an object".into()),
            Some("関数 `start` の引数 `submit` が配列です。オブジェクトが要ります".into()),
        ),
        (
            "nested 513 deep: submit",
            "start",
            "submit",
            "",
            Do::Spell(format!("{}{}", "[".repeat(513), "]".repeat(513))),
            Some("The argument `submit` of `start` nests deeper than 512".into()),
            Some("関数 `start` の引数 `submit` は、512 段より深く入れ子になっています".into()),
        ),
        (
            "opened 513 deep and never closed: organization",
            "start",
            "organization",
            "",
            Do::Spell(r#"{"a":"#.repeat(513)),
            Some("The argument `organization` of `start` nests deeper than 512".into()),
            Some("関数 `start` の引数 `organization` は、512 段より深く入れ子になっています".into()),
        ),
        (
            "brackets in a string: submit.requester",
            "start",
            "submit",
            "/requester",
            Do::Spell(format!(r#"["\"{}\\"]"#, "[{".repeat(600))),
            Some("The argument `submit.requester` of `start` is an array, and must be a string".into()),
            Some("関数 `start` の引数 `submit.requester` が配列です。文字列が要ります".into()),
        ),
        // the time, an integer outside its range, which the module's check of the entry says
        (
            "a time before 1970: now",
            "start",
            "now",
            "",
            Do::Now(-1),
            Some("The argument `now` of `start` is -1, outside its range >=0 <=253_402_300_799".into()),
            Some("関数 `start` の引数 `now` の -1 は、範囲 >=0 <=253_402_300_799 の外です".into()),
        ),
    ];
    let mut lines = String::new();
    for (name, entry, arg, at, what, _, _) in &cases {
        let (input, args): (&Json, &[&str]) = match *entry {
            "start" => (&submit, &["submit", "organization"]),
            "worked" => (&worked, &["input"]),
            _ => (&approve, &["ticket", "event", "organization"]),
        };
        let mut parts: Vec<String> = Vec::new();
        for a in args {
            let mut v = part(input, a);
            let text = match what {
                _ if a != arg => v.to_string(),
                Do::Keep | Do::Now(_) => v.to_string(),
                Do::Raw(r) => r.to_string(),
                Do::Set(x) if at.is_empty() => x.to_string(),
                Do::Set(x) => {
                    *v.pointer_mut(at).unwrap_or_else(|| panic!("{name}: {at}")) = x.clone();
                    v.to_string()
                }
                Do::Spell(spelled) => {
                    *v.pointer_mut(at).unwrap_or_else(|| panic!("{name}: {at}")) = json!("\u{0}spelled");
                    let text = v.to_string();
                    assert_eq!(text.matches(r#""\u0000spelled""#).count(), 1, "{name}: {text}");
                    text.replace(r#""\u0000spelled""#, spelled)
                }
                Do::Twice(first, later) => {
                    let own = v.pointer(at).cloned().unwrap_or_else(|| panic!("{name}: {at}"));
                    let key = serde_json::to_string(at.rsplit_once('/').unwrap().1).unwrap();
                    *v.pointer_mut(at).unwrap() = json!("\u{0}spelled");
                    let text = v.to_string();
                    let placed = format!(r#"{key}:"\u0000spelled""#);
                    assert_eq!(text.matches(&placed).count(), 1, "{name}: {text}");
                    let (first, later) = (first.clone().unwrap_or(own.clone()), later.clone().unwrap_or(own));
                    text.replace(&placed, &format!("{key}:{first},{key}:{later}"))
                }
                Do::Drop => {
                    let (parent, key) = at.rsplit_once('/').unwrap();
                    let p = if parent.is_empty() { &mut v } else { v.pointer_mut(parent).unwrap() };
                    p.as_object_mut().unwrap().remove(key).unwrap_or_else(|| panic!("{name}: {at}"));
                    v.to_string()
                }
            };
            parts.push(text);
        }
        let now = match what {
            Do::Now(n) => i128::from(*n),
            _ => input.get("now").and_then(Json::as_int).unwrap(),
        };
        lines.push_str(&format!("{kind}\t{entry}\t{now}\t{}\n", parts.join("\t")));
    }
    // the Wasm of the crate, called as a server calls it ([`KINDS_HOST`])
    let wasm = wasm_ready();
    for lang in ["en", "ja"] {
        let out = t.path().join(format!("out-{lang}"));
        let o = out.to_string_lossy().into_owned();
        let mut args = vec!["gen", "examples/leave.ringi", "--root", ".", "--out", &o, "--module", MODULE, "--lang", lang];
        for target in ["typescript", "python", "go", "rust"] {
            args.extend(["--target", target]);
        }
        if wasm {
            args.extend(["--target", "requests", "--catalog", CATALOG, "--extra", "wasm"]);
        }
        let (code, stdout, err) = ritsu_in(t.path(), &args);
        assert_eq!(code, 0, "{stdout}{err}");
        // each line a driver prints held to its case: the answer of the contract, or the sentence; what
        // each target says otherwise, said at the end for all four
        let said_otherwise = std::cell::RefCell::new(Vec::new());
        let check = |target: &str, stdout: &str| {
            let mut printed: Vec<&str> = stdout.lines().collect();
            assert_eq!(printed.first().copied(), Some(lang), "{target} ({lang}) says its entries' sentences are in another language:\n{stdout}");
            printed.remove(0);
            assert_eq!(printed.len(), cases.len(), "{target} ({lang}) printed {} lines for {} calls:\n{stdout}", printed.len(), cases.len());
            let mut wrong = Vec::new();
            for ((name, _, arg, at, _, en, ja), line) in cases.iter().zip(&printed) {
                let got = ritsu_base::json::parse_nested(line, 4096).unwrap_or_else(|e| panic!("{target} ({lang}), {name}: {e:?}: {line}"));
                let want = if lang == "en" { en } else { ja };
                let path = place(arg, at);
                let agrees = match want {
                    // the answer of the contract: a ticket's, or what the form works out
                    None => got.get("ok").is_some() || got.get("fields").is_some(),
                    Some(text) => {
                        got.get("error").and_then(Json::as_str) == Some(text.as_str())
                            && (target == "Rust" || got.get("kind").and_then(Json::as_str) == Some("input"))
                            && got.get("path").and_then(Json::as_str) == Some(path.as_str())
                    }
                };
                if !agrees {
                    wrong.push(format!("{name}:\n  expected: {} (at {path})\n  printed:  {line}", want.as_deref().unwrap_or("the answer of the contract")));
                }
            }
            if !wrong.is_empty() {
                said_otherwise.borrow_mut().push(format!("{target} ({lang}) says otherwise:\n{}", wrong.join("\n")));
                return;
            }
            println!("{target} ({lang}): {} calls out of the contract and {} in it are said and answered as the contract says", cases.iter().filter(|c| c.5.is_some()).count(), cases.iter().filter(|c| c.5.is_none()).count());
        };
        let tools = workspace().join("crates/kaname/tools");
        let tsc = tools.join("node_modules/.bin/tsc");
        if ritsu_testkit::need(Need::Node) {
            if tsc.exists() {
                let dir = out.join("typescript");
                std::fs::write(dir.join("wrong.ts"), TS_WRONG_DRIVER).unwrap();
                std::fs::write(dir.join("calls.tsv"), &lines).unwrap();
                let tsconfig = serde_json::json!({
                    "compilerOptions": {
                        "strict": true, "erasableSyntaxOnly": true, "module": "commonjs", "target": "es2022", "rootDir": ".", "outDir": "dist",
                        "skipLibCheck": true, "types": ["node"], "typeRoots": [tools.join("node_modules/@types")]
                    },
                    "include": ["**/*.ts"],
                    "exclude": ["dist", "node_modules"]
                });
                std::fs::write(dir.join("tsconfig.json"), serde_json::to_string_pretty(&tsconfig).unwrap()).unwrap();
                let o = ritsu_testkit::run::finish(Command::new(&tsc).arg("-p").arg("tsconfig.json").current_dir(&dir), RUN, "tsc");
                assert!(o.ok, "tsc:\n{}", o.both());
                type_check_typescript_rules(&dir, &tsc);
                let o = ritsu_testkit::run::finish(Command::new("node").arg("dist/wrong.js").arg("calls.tsv").current_dir(&dir), RUN, "node");
                assert!(o.ok, "node:\n{}", o.both());
                check("TypeScript", &o.stdout);
            } else {
                ritsu_testkit::skip("tsc is not installed in crates/kaname/tools (npm ci --prefix crates/kaname/tools); the TypeScript of the request is not run");
            }
        }
        if ritsu_testkit::need(Need::Python) {
            match mypy() {
                Some((mypy, python, tigerbeetle)) => type_check_python(&out.join("python"), &mypy, &python, std::slice::from_ref(&tigerbeetle)),
                None => ritsu_testkit::skip("mypy, Temporal's Python SDK (crates/dandori/tools/temporal-python) or tigerbeetle (crates/chobo/tools/runner) is not installed; the Python of the request is not type-checked"),
            }
        }
        if ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 is not installed; the Python of the request is not run") {
            let dir = out.join("python");
            std::fs::write(dir.join("wrong.py"), PY_WRONG_DRIVER).unwrap();
            std::fs::write(dir.join("calls.tsv"), &lines).unwrap();
            let o = ritsu_testkit::run::finish(Command::new("python3").arg("wrong.py").arg("calls.tsv").current_dir(&dir), RUN, "python3");
            assert!(o.ok, "python3:\n{}", o.both());
            check("Python", &o.stdout);
        }
        if ready(Need::Go, have_go, "go is not installed; the Go of the request is not run") {
            match go_module() {
                Some(gomod) => {
                    type_check_go(&t.path().join(format!("go-vet-{lang}")), &out.join("go"), &gomod);
                    let dir = t.path().join(format!("go-wrong-{lang}"));
                    go_module_in(&dir, &out.join("go"), &gomod);
                    std::fs::create_dir_all(dir.join("driver")).unwrap();
                    std::fs::write(dir.join("driver/main.go"), go_wrong_driver()).unwrap();
                    std::fs::write(dir.join("calls.tsv"), &lines).unwrap();
                    let bin = dir.join("driver.bin");
                    let o = ritsu_testkit::run::finish(go_in(&dir).args(["build", "-o"]).arg(&bin).arg("./driver"), LIMIT, "go build");
                    assert!(o.ok, "go build:\n{}", o.both());
                    let o = ritsu_testkit::run::finish(Command::new(&bin).arg("calls.tsv").current_dir(&dir), RUN, "the driver of Go");
                    assert!(o.ok, "the driver:\n{}", o.both());
                    check("Go", &o.stdout);
                }
                None => ritsu_testkit::skip("the go.mod of crates/dandori/tools/temporal-go or crates/chobo/tools/runner/go is missing; the Go of the request is not run"),
            }
        }
        if ritsu_testkit::need(Need::Rustc) {
            let dir = out.join("rust");
            rust_crate(&dir, &format!("requests-wrong-{lang}"));
            let bin = format!("driver_requests_wrong_{lang}");
            let toml = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
            std::fs::write(dir.join("Cargo.toml"), format!("{toml}\n[[bin]]\nname = \"{bin}\"\npath = \"driver/main.rs\"\n")).unwrap();
            std::fs::create_dir_all(dir.join("driver")).unwrap();
            std::fs::write(dir.join("driver/main.rs"), RUST_WRONG_DRIVER).unwrap();
            std::fs::write(dir.join("calls.tsv"), &lines).unwrap();
            let o = ritsu_testkit::run::finish(cargo_in(&dir, "ritsu-rust").args(["build", "--offline", "--quiet", "--bin", &bin]), LIMIT, "cargo build of the driver");
            assert!(o.ok, "cargo build of the driver:\n{}", o.both());
            let o = ritsu_testkit::run::finish(Command::new(rust_target("ritsu-rust").join("debug").join(&bin)).arg("calls.tsv").current_dir(&dir), RUN, "the driver of Rust");
            assert!(o.ok, "the driver:\n{}", o.both());
            check("Rust", &o.stdout);
        }
        if wasm {
            let calls = out.join("calls.tsv");
            std::fs::write(&calls, &lines).unwrap();
            let (said, printed) = wasm_calls(&t.path().join(format!("wasm-wrong-{lang}")), &out.join("requests"), &calls);
            check("Wasm", &format!("{said}\n{printed}"));
        }
        let said_otherwise = said_otherwise.take();
        assert!(said_otherwise.is_empty(), "{}", said_otherwise.join("\n"));
    }
}

/// Requests named as names Go's file of entries holds (its functions `order` and `pairs`, the
/// standard package `strings` it imports) are built: the file names each module by a name of its
/// own (`m0`, …), as the files of TypeScript and Python do (kaname's DESIGN 16.4).
#[test]
fn requests_named_as_the_names_of_the_go_entries_are_built() {
    let t = TempDir::new("gen-requests-go-names");
    lay(&t);
    let leave = std::fs::read_to_string(t.path().join("examples/leave.ringi")).unwrap();
    let names = ["order", "pairs", "strings"];
    let mut given = Vec::new();
    for name in names {
        let renamed = leave.replacen("request leave v1 ", &format!("request {name} v1 "), 1);
        assert_ne!(renamed, leave, "the leave example names itself otherwise");
        std::fs::write(t.path().join(format!("examples/{name}.ringi")), renamed).unwrap();
        given.push(format!("examples/{name}.ringi"));
    }
    let out = t.path().join("out");
    let o = out.to_string_lossy().into_owned();
    let mut args: Vec<&str> = vec!["gen"];
    args.extend(given.iter().map(String::as_str));
    args.extend(["--root", ".", "--out", &o, "--module", MODULE, "--target", "go"]);
    let (code, stdout, err) = ritsu_in(t.path(), &args);
    assert_eq!(code, 0, "{stdout}{err}");
    let entries = std::fs::read_to_string(out.join("go/ticketkinds/ticketkinds.go")).unwrap();
    for name in names {
        let path = format!("\"{MODULE}/glue/{name}\"");
        let line = entries.lines().find(|l| l.ends_with(&path)).unwrap_or_else(|| panic!("the entries import no {path}:\n{entries}"));
        assert!(line.trim_start().starts_with('m'), "the module {name} is imported by its own name: {line:?}");
    }
    if ready(Need::Go, have_go, "go is not installed; the Go of the requests is not built") {
        match go_module() {
            Some(gomod) => type_check_go(&t.path().join("go-vet"), &out.join("go"), &gomod),
            None => ritsu_testkit::skip("the go.mod of crates/dandori/tools/temporal-go or crates/chobo/tools/runner/go is missing; the Go of the requests is not built"),
        }
    }
}

/// `j` itself, or what its `$ref` (`#/components/schemas/…`) leads to in the API `api`.
fn deref<'a>(api: &'a serde_json::Value, mut j: &'a serde_json::Value) -> &'a serde_json::Value {
    while let Some(r) = j.get("$ref").and_then(serde_json::Value::as_str) {
        j = api.pointer(r.trim_start_matches('#')).unwrap_or_else(|| panic!("`{r}` leads nowhere"));
    }
    j
}

/// Whether the schema `s` of the API `api` takes null.
fn takes_null(api: &serde_json::Value, s: &serde_json::Value) -> bool {
    use serde_json::Value;
    let s = deref(api, s);
    match s.get("type") {
        Some(Value::String(t)) => t == "null",
        Some(Value::Array(ts)) => ts.iter().any(|t| t == "null"),
        _ => match ["anyOf", "oneOf"].iter().find_map(|k| s.get(*k).and_then(Value::as_array)) {
            Some(xs) => xs.iter().any(|x| takes_null(api, x)),
            None => true,
        },
    }
}

/// The changes of one place each of a body the schema `s` of the API `api` describes, which holds
/// `v` at the pointer `at`: each key of the schema left out (where `v` has it) and given null, each
/// with its place, whether it is left out, and whether the schema takes it; and so within the value of
/// a key that is an object, and within the first row of a list of objects.
fn one_place_changes(api: &serde_json::Value, s: &serde_json::Value, v: &serde_json::Value, at: &str, out: &mut Vec<(String, bool, bool)>) {
    use serde_json::Value;
    let s = deref(api, s);
    let required: Vec<&str> = s.get("required").and_then(Value::as_array).map(|r| r.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let Some(props) = s.get("properties").and_then(Value::as_object) else { return };
    for (key, p) in props {
        let here = format!("{at}/{key}");
        let given = v.get(key);
        if given.is_some() {
            out.push((here.clone(), true, !required.contains(&key.as_str())));
        }
        out.push((here.clone(), false, takes_null(api, p)));
        let p = deref(api, p);
        match given {
            Some(o @ Value::Object(_)) if p.get("properties").is_some() => one_place_changes(api, p, o, &here, out),
            Some(Value::Array(rows)) => {
                if let (Some(first @ Value::Object(_)), Some(row)) = (rows.first(), p.get("items"))
                    && deref(api, row).get("properties").is_some()
                {
                    one_place_changes(api, row, first, &format!("{here}/0"), out);
                }
            }
            _ => {}
        }
    }
}

/// `openapi.json` and the entries read a body alike (ringi's DESIGN 12.4): each operation with a
/// body, made by a call of the vectors of the expense and the leave (one the reference takes where
/// there is one), its body changed at one place: each key of its schema (but the submit's
/// `requester`, which the server writes for a person who makes their own request), each field of its
/// form and of the form's first row, and each key of its revision, left out where the call has it and
/// given null. Where the schema takes the change (a key it does not require; null where its type
/// takes null), the entry answers it as a call of the contract, which the machine may refuse (409);
/// where it does not (400), the entry stops it, with an error of the kind `input`. The revision is the
/// one place the API holds narrower than the entry, as 12.4 says: the entry reads a revision left out
/// or null as none, which the machine refuses as stale, and a revision's `stage` left out as null.
/// The operations that work a form out (`worked_<kind>`, `ticket_worked_<kind>`, ringi's DESIGN 13.7)
/// are held to the entry `worked` alike, from the input of its vectors with the most keys: each key
/// of `form` and `edited`, and of the first row. Python's entries stand for the four languages',
/// which read alike ([`json_out_of_the_contract_is_said_alike_in_the_four_languages`]).
#[test]
fn the_api_requires_what_the_entries_stop_without() {
    use serde_json::Value;
    if !ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 is not installed; the entries are not held to the API") {
        return;
    }
    let t = TempDir::new("gen-requests-required");
    lay(&t);
    let requests = ["examples/expense.ringi", "examples/leave.ringi"];
    let out = t.path().join("out");
    let o = out.to_string_lossy().into_owned();
    let args = [&["gen"][..], &requests, &["--root", ".", "--out", &o, "--target", "python", "--target", "requests", "--catalog", CATALOG]].concat();
    let (code, stdout, err) = ritsu_in(t.path(), &args);
    assert_eq!(code, 0, "{stdout}{err}");
    let api: Value = serde_json::from_str(&std::fs::read_to_string(out.join("requests/openapi.json")).unwrap()).unwrap();
    // each kind's calls: the entry, its parts as JSON, its time and whether the reference takes it
    let mut calls: Vec<(String, String, Vec<Value>, i128, bool)> = Vec::new();
    let mut kinds = Vec::new();
    for request in requests {
        let kind = kind_of(t.path(), request);
        for line in std::fs::read_to_string(out.join(format!("requests/vectors/{kind}.jsonl"))).unwrap().lines() {
            let v = ritsu_base::json::parse(line).unwrap();
            let (entry, input, answer) = ringi::lower::shape::call_of(&v).unwrap();
            let names: &[&str] = if entry == "start" { &["submit", "organization"] } else { &["ticket", "event", "organization"] };
            let parts = names.iter().map(|n| serde_json::from_str(&input.get(n).cloned().unwrap_or(Json::Null).compact()).unwrap()).collect();
            calls.push((kind.clone(), entry.to_string(), parts, input.get("now").and_then(Json::as_int).unwrap(), answer.get("ok").is_some()));
        }
        kinds.push(kind);
    }
    // each kind's input of the entry `worked` with the most keys, of its form and of what an
    // approver gives
    fn keys(v: &serde_json::Value) -> usize {
        match v {
            serde_json::Value::Object(o) => o.len() + o.values().map(keys).sum::<usize>(),
            serde_json::Value::Array(a) => a.iter().map(keys).sum(),
            _ => 0,
        }
    }
    let worked: Vec<(String, Value)> = kinds
        .iter()
        .map(|kind| {
            let text = std::fs::read_to_string(out.join(format!("requests/vectors/worked/{kind}.jsonl"))).unwrap();
            let inputs = text.lines().map(|l| serde_json::from_str::<Value>(l).unwrap()["input"].clone());
            (kind.clone(), inputs.max_by_key(keys).unwrap_or_else(|| panic!("{kind} has no vector of `worked`")))
        })
        .collect();
    // the changes: the operation, the kind, the place in its body and how it is changed, whether the
    // schema takes it, whether it is a change of the revision the entry takes; and the line of the call
    let mut cases: Vec<(String, String, String, bool, bool, bool)> = Vec::new();
    let mut lines = String::new();
    let mut ops = 0;
    for (path, item) in api.get("paths").and_then(Value::as_object).unwrap() {
        for op in item.as_object().unwrap().values() {
            let Some(schema) = op.pointer("/requestBody/content/application~1json/schema") else { continue };
            let id = op.get("operationId").and_then(Value::as_str).unwrap();
            let steps: Vec<&str> = path.trim_start_matches('/').split('/').collect();
            // `/requests/<kind>/worked`, `/requests/<kind>/tickets/{id}/worked`: the body is what the
            // entry `worked` takes, whole
            if steps.first() == Some(&"requests") && steps.last() == Some(&"worked") {
                let (kind, input) = worked.iter().find(|(k, _)| k == steps[1]).unwrap_or_else(|| panic!("{path}: no kind {}", steps[1]));
                ops += 1;
                let mut places = Vec::new();
                one_place_changes(&api, schema, input, "", &mut places);
                for (place, left_out, takes) in places {
                    let mut changed = input.clone();
                    let (parent, key) = place.rsplit_once('/').unwrap();
                    let Some(Value::Object(holder)) = changed.pointer_mut(parent) else { panic!("{id}: {place}") };
                    if left_out {
                        holder.remove(key);
                    } else {
                        holder.insert(key.to_string(), Value::Null);
                    }
                    cases.push((id.to_string(), kind.clone(), place, left_out, takes, false));
                    lines.push_str(&format!("{kind}\tworked\t0\t{changed}\n"));
                }
                continue;
            }
            // `/requests/<kind>/tickets`, `/requests/<kind>/tickets/{id}/<event>`: one kind's;
            // `/tickets/{id}/<event>`: any kind's
            let (of, event) = match steps[..] {
                ["requests", kind, "tickets"] => (vec![kind.to_string()], "submit"),
                ["requests", kind, "tickets", _, event] => (vec![kind.to_string()], event),
                ["tickets", _, event] => (kinds.clone(), event),
                _ => panic!("{path}: an operation with a body of no ticket"),
            };
            let made = |c: &&(String, String, Vec<Value>, i128, bool)| of.contains(&c.0) && c.2.get(if event == "submit" { 0 } else { 1 }).is_some_and(|p| if event == "submit" { c.1 == "start" } else { p.get(event).is_some() });
            let (kind, entry, parts, now, _) = calls.iter().filter(made).find(|c| c.4).or_else(|| calls.iter().find(made)).unwrap_or_else(|| panic!("no vector makes `{id}`"));
            ops += 1;
            // the body: what `start` takes whole, or the value of the event `next` takes
            let (part, base) = if event == "submit" { (0, String::new()) } else { (1, format!("/{event}")) };
            let mut places = Vec::new();
            one_place_changes(&api, schema, parts[part].pointer(&base).unwrap(), "", &mut places);
            for (place, left_out, takes) in places {
                if event == "submit" && place == "/requester" {
                    continue;
                }
                let mut changed = parts.clone();
                let at = format!("{base}{place}");
                let (parent, key) = at.rsplit_once('/').unwrap();
                let Some(Value::Object(holder)) = changed[part].pointer_mut(parent) else { panic!("{id}: {at}") };
                if left_out {
                    holder.remove(key);
                } else {
                    holder.insert(key.to_string(), Value::Null);
                }
                let narrower = place == "/revision" || (place == "/revision/stage" && left_out);
                cases.push((id.to_string(), kind.clone(), place, left_out, takes, narrower));
                lines.push_str(&format!("{kind}\t{entry}\t{now}\t{}\n", changed.iter().map(Value::to_string).collect::<Vec<_>>().join("\t")));
            }
        }
    }
    let dir = out.join("python");
    std::fs::write(dir.join("wrong.py"), PY_WRONG_DRIVER).unwrap();
    std::fs::write(dir.join("calls.tsv"), &lines).unwrap();
    let o = ritsu_testkit::run::finish(Command::new("python3").arg("wrong.py").arg("calls.tsv").current_dir(&dir), RUN, "python3");
    assert!(o.ok, "python3:\n{}", o.both());
    let printed: Vec<&str> = o.stdout.lines().skip(1).collect();
    assert_eq!(printed.len(), cases.len(), "{}", o.stdout);
    let mut wrong = Vec::new();
    let (mut taken, mut stopped, mut narrower) = (0, 0, 0);
    for ((id, kind, place, left_out, takes, wider), line) in cases.iter().zip(&printed) {
        let (left_out, takes, wider) = (*left_out, *takes, *wider);
        let got: Value = serde_json::from_str(line).unwrap_or_else(|e| panic!("{id} ({kind}) {place}: {e}: {line}"));
        let stops = got.get("kind").and_then(Value::as_str) == Some("input");
        let answers = got.get("ok").is_some() || got.get("err").is_some() || got.get("fields").is_some();
        let how = if left_out { "left out" } else { "null" };
        let fine = match (takes, wider) {
            (_, true) => !takes && answers,
            (true, false) => answers,
            (false, false) => stops,
        };
        if !fine {
            let schema = if takes { "the schema takes it" } else { "the schema refuses it (400)" };
            let why = if wider { ", as the API holds a revision narrower than the entry" } else { "" };
            wrong.push(format!("{id} ({kind}), `{place}` {how}: {schema}{why}, and the entry prints {line}"));
        }
        match (takes, wider) {
            (_, true) => narrower += 1,
            (true, _) => taken += 1,
            _ => stopped += 1,
        }
    }
    assert!(wrong.is_empty(), "{} of {} changes are read otherwise:\n{}", wrong.len(), cases.len(), wrong.join("\n"));
    assert!(taken > 0 && stopped > 0 && narrower > 0, "{taken} {stopped} {narrower}");
    println!("{} changes of the bodies of {ops} operations: {taken} the schema takes and the entry answers, {stopped} it refuses and the entry stops, {narrower} of the revision the API alone refuses", cases.len());
}
