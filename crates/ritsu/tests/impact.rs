//! `ritsu impact`, English first and the twin with Japanese names beside it. Each case is a git
//! repository the test makes (`ritsu_testkit::git`) from the example in `examples/impact` or
//! `examples/impact.ja`, which has no `ritsu.mod`; the working tree or a second tag is the head.
//!
//! - Code written by hand alone: no answer moves. With a record of the claims, `unclaimed_lines` is
//!   the lines no claim ran; with none, every changed line.
//! - A table of a rule: the answers move, the owners come out, the gate that reads the rule moves with
//!   it. Text, Markdown and JSON held to golden files (`tests/golden/impact`; `RITSU_BLESS=1` writes
//!   them), in each language.
//! - A permit of a gate, a calendar the gate reads through a dates file, and a file nothing reads,
//!   which shows no difference on a gate with a date condition.
//! - An error only the head has (exit 1), what cannot be compared (exit 2), JSON printed to the end
//!   in each.
//! - The route rule: where a change goes, an input that is no fact, a type that is not the fact's, a
//!   value outside the input's range, a rule that does not pass its check. The promise the example's
//!   rule keeps.
//! - The files: how each is sorted, what git ignores, what a language whose contents are not compared
//!   counts as; the generated files with a `target` line; a project is left as it was and no claim runs.
//! - What stops a run, in each language; a root given, and one below the top of the repository.
//! - What the report cannot vouch for: a file it knows nothing of, a generated file nothing checked, a
//!   line the record holds no statement on, a rename. The example rule sends each to a person.
//! - The pages show what the example prints, and every fact.

use ritsu_testkit::git::{GitServer, Options, Repo};
use ritsu_testkit::{TempDir, skip};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const LANG_VARS: [&str; 10] = ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG", "CHOBO_LANG", "GEAS_LANG", "YUEN_LANG", "SAKAI_LANG", "SEKISHO_LANG", "KANAME_LANG"];

/// A repository of the test's, and what it needs to stay.
struct Desk {
    repo: Repo,
    _server: GitServer,
    _t: TempDir,
}

fn no_git() -> bool {
    if Command::new("git").arg("--version").output().is_ok_and(|o| o.status.success()) {
        return false;
    }
    skip("git is not on the PATH; `ritsu impact` compares two revisions of a repository");
    true
}

fn copy_into(repo: &Repo, from: &Path, under: &str) {
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let name = e.file_name().to_string_lossy().into_owned();
        let rel = if under.is_empty() { name } else { format!("{under}/{name}") };
        if e.file_type().unwrap().is_dir() {
            copy_into(repo, &e.path(), &rel);
        } else {
            repo.write(&rel, std::fs::read(e.path()).unwrap());
        }
    }
}

impl Desk {
    /// An empty repository.
    fn empty(what: &str) -> Desk {
        let t = TempDir::new(what);
        let server = GitServer::start(&t.path().join("git"), Options::hosted());
        let repo = server.repo("acme/desk");
        Desk { repo, _server: server, _t: t }
    }

    /// The example in `examples/<dir>`, committed and tagged `base`.
    fn example(what: &str, dir: &str) -> Desk {
        let d = Desk::empty(what);
        copy_into(&d.repo, &here().join("examples").join(dir), "");
        d.repo.commit("the desk");
        d.repo.tag("base");
        d
    }

    fn en(what: &str) -> Desk {
        Desk::example(what, "impact")
    }

    fn ja(what: &str) -> Desk {
        Desk::example(what, "impact.ja")
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
        c.current_dir(&self.repo.work).args(args);
        for v in LANG_VARS {
            c.env_remove(v);
        }
        c
    }

    /// The exit code, what was printed and what was said on stderr.
    fn ritsu(&self, args: &[&str]) -> (i32, String, String) {
        let o = self.command(args).output().unwrap();
        (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
    }

    /// `ritsu impact --base base <args> --format json`: the exit code, the JSON, what was said on stderr.
    fn json(&self, args: &[&str]) -> (i32, Value, String) {
        let mut all = vec!["impact", "--base", "base", "--format", "json"];
        all.extend(args);
        let (code, out, err) = self.ritsu(&all);
        let v = serde_json::from_str(&out).unwrap_or_else(|e| panic!("not JSON ({e}), exit {code}:\n{out}\n{err}"));
        (code, v, err)
    }

    /// A file of the working tree with `from` replaced by `to`; it has to be there. Not added.
    fn edit(&self, file: &str, from: &str, to: &str) {
        let at = self.repo.work.join(file);
        let s = std::fs::read_to_string(&at).unwrap();
        assert!(s.contains(from), "{file} has no `{from}`:\n{s}");
        std::fs::write(at, s.replacen(from, to, 1)).unwrap();
    }

    /// A file of the working tree written. Not added.
    fn put(&self, file: &str, text: &str) {
        let at = self.repo.work.join(file);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, text).unwrap();
    }

    fn append(&self, file: &str, text: &str) {
        let at = self.repo.work.join(file);
        let mut s = std::fs::read_to_string(&at).unwrap();
        s.push_str(text);
        std::fs::write(at, s).unwrap();
    }

    /// What a person does after looking at what changed: the `.req` written again.
    fn review(&self, req: &str, by: &str) {
        let (code, out, err) = self.ritsu(&["yuen", "review", req, "--all", "--by", by, "--date", "2027-01-05"]);
        assert_eq!(code, 0, "yuen review:\n{out}\n{err}");
    }

    /// The table of the example's rule moved, from 20 items to 30.
    fn move_the_band(&self) {
        for (from, to) in [("| <=20  | small          |", "| <=30  | small          |"), ("| >20   | large          |", "| >30   | large          |"), ("| 20    | small   |", "| 30    | small   |"), ("| 21    | large   |", "| 31    | large   |")] {
            self.edit("refund_band.rule", from, to);
        }
    }

    fn status(&self) -> String {
        self.repo.git(&["status", "--porcelain", "--untracked-files=all", "--ignored"])
    }
}

fn fact(v: &Value, name: &str) -> i64 {
    v["facts"][name].as_i64().unwrap_or_else(|| panic!("the fact {name} is no number: {}", v["facts"]))
}

fn yes(v: &Value, name: &str) -> bool {
    v["facts"][name].as_bool().unwrap_or_else(|| panic!("the fact {name} is no bool: {}", v["facts"]))
}

/// The facts of an answer that moved are all 0.
fn nothing_moved(v: &Value) {
    for name in ["rule_changes", "date_changes", "allow_gained", "allow_lost", "policies_moved", "book_changes", "interface_changes", "interface_breaks"] {
        assert_eq!(fact(v, name), 0, "{name}: {}", v["facts"]);
    }
}

fn the_route(v: &Value) -> (&str, bool) {
    let o = &v["route"]["outputs"];
    (o["route"].as_str().unwrap(), o["person_reads"].as_bool().unwrap())
}

/// What a golden file should not hold: the version of ritsu, which every release raises.
fn masked(s: &str) -> String {
    s.replace(&format!("\"ritsu\": \"{}\"", env!("CARGO_PKG_VERSION")), "\"ritsu\": \"<version>\"")
}

fn golden(name: &str, got: &str, failures: &mut Vec<String>) {
    if let Err(e) = ritsu_testkit::golden::check(&here().join("tests/golden/impact").join(name), &masked(got)) {
        failures.push(e);
    }
}

// ── Code written by hand ────────────────────────────────────────────────

#[test]
fn code_written_by_hand_moves_no_answer() {
    if no_git() {
        return;
    }
    let d = Desk::en("impact-code");
    d.edit("app.py", "    return f\"Refunds for {name}\"\n", "    return f\"The refunds of {name}\"\n\n\ndef bye():\n    return 1\n");
    let (code, v, err) = d.json(&["--route", "route.rule"]);
    assert_eq!(code, 0, "{err}");
    nothing_moved(&v);
    assert_eq!(fact(&v, "code_files"), 1);
    assert_eq!(fact(&v, "code_lines"), 3, "one line edited and two added; the blank lines are not counted");
    assert_eq!(fact(&v, "spec_files") + fact(&v, "generated_files") + fact(&v, "other_files"), 0);
    // this project has no `.geas`: every changed line is one no claim is known to run
    assert!(!yes(&v, "claims_known"));
    assert_eq!(fact(&v, "unclaimed_lines"), 3);
    assert!(v["claims"]["note"].as_str().unwrap().contains(".geas"), "{}", v["claims"]);
    // no requirement names the file
    assert_eq!(fact(&v, "requirements_unknown"), 1, "{}", v["requirements"]);
    assert_eq!(fact(&v, "requirements_moved"), 0);
    assert_eq!(the_route(&v), ("owner_reads", true));
    assert!(v["markdown"].as_str().unwrap().contains("No answer moved"), "{}", v["markdown"]);
}

/// The greeter of yuen's example: its server is covered by a record of the claims, and no `.req`.
fn greeter(what: &str) -> Desk {
    let d = Desk::empty(what);
    let g = here().join("../yuen/examples/greeter");
    let read = |f: &str| std::fs::read_to_string(g.join(f)).unwrap();
    let server = read("server.py");
    // the base differs on two lines: one the "rejects an empty name" claim runs, and one no claim runs
    assert!(server.contains("self._send(400, \"name required\")") && server.contains("            self._send(404, \"not found\")\n\n\nport"));
    d.repo.write("server.py", server.replace("            self._send(404, \"not found\")\n\n\nport", "            self._send(404, \"no such path\")\n\n\nport"));
    d.repo.write("greeter.geas", read("greeter.geas"));
    d.repo.write(".geas/greeter.map.jsonl", read(".geas/greeter.map.jsonl"));
    d.repo.commit("the greeter");
    d.repo.tag("base");
    // the head is the code of the record after the change
    d.put("server.py", &server.replace("self._send(400, \"name required\")", "self._send(400, \"a name is required\")"));
    d.put(".geas/greeter.map.jsonl", &read("changes/after.map.jsonl"));
    d
}

#[test]
fn a_record_of_the_claims_says_which_lines_no_claim_runs() {
    if no_git() {
        return;
    }
    let d = greeter("impact-record");
    let (code, v, err) = d.json(&[]);
    assert_eq!(code, 0, "{err}\n{}", v["markdown"]);
    nothing_moved(&v);
    assert_eq!(fact(&v, "code_lines"), 2);
    assert!(yes(&v, "claims_known"), "{}", v["claims"]);
    assert_eq!(fact(&v, "claims_touched"), 1, "{}", v["claims"]);
    assert_eq!(v["claims"]["touched"][0]["name"], "rejects an empty name");
    assert_eq!(fact(&v, "unclaimed_lines"), 1, "only the line no claim runs: {}", v["claims"]);
    let u = &v["claims"]["unclaimed"][0];
    assert_eq!((u["file"].as_str(), u["lines"].as_str(), u["count"].as_i64()), (Some("server.py"), Some("47"), Some(1)), "{u}");

    // a language geas keeps no record of: its lines are run by no claim that is known
    d.put("tool.rb", "puts 1\nputs 2\n");
    let (_, v, _) = d.json(&[]);
    assert!(yes(&v, "claims_known"));
    assert_eq!(fact(&v, "code_lines"), 4);
    assert_eq!(fact(&v, "unclaimed_lines"), 3, "{}", v["claims"]);
    std::fs::remove_file(d.repo.work.join("tool.rb")).unwrap();

    // a record that does not fit the code says so, and exit does not change
    d.append("server.py", "print(1)\n");
    let (code, v, _) = d.json(&[]);
    assert_eq!(code, 0);
    assert!(!yes(&v, "claims_known"), "{}", v["claims"]);
    assert!(v["claims"]["specs"][0]["why"].as_str().is_some(), "{}", v["claims"]);
    assert_eq!(fact(&v, "unclaimed_lines"), fact(&v, "code_lines"));

    // the head a revision: the record and the code are read from it
    let d = greeter("impact-record-head");
    d.repo.git(&["add", "-A"]);
    d.repo.commit("the change");
    d.repo.tag("after");
    let (code, v, err) = d.json(&["--head", "after"]);
    assert_eq!(code, 0, "{err}\n{}", v["markdown"]);
    assert!(yes(&v, "claims_known"), "{}", v["claims"]);
    assert_eq!(fact(&v, "claims_touched"), 1, "{}", v["claims"]);
    assert_eq!(fact(&v, "unclaimed_lines"), 1, "{}", v["claims"]);

    // no record at all: every changed line
    let d = greeter("impact-no-record");
    std::fs::remove_file(d.repo.work.join(".geas/greeter.map.jsonl")).unwrap();
    let (code, v, _) = d.json(&[]);
    assert_eq!(code, 0);
    assert!(!yes(&v, "claims_known"));
    assert_eq!(fact(&v, "claims_touched"), 0);
    assert_eq!(fact(&v, "unclaimed_lines"), 2);
    let u = &v["claims"]["unclaimed"][0];
    assert_eq!(u["why"], "no_record", "{u}");
}

#[test]
fn a_line_the_record_holds_no_statement_on_is_one_no_claim_is_known_to_run() {
    if no_git() {
        return;
    }
    let d = greeter("impact-no-statement");
    // what coverage marks no line of: a table a statement is written over. A comment is left out.
    let at = d.repo.work.join("server.py");
    let mut server = std::fs::read_to_string(&at).unwrap();
    let line = server.lines().count() as u32 + 2;
    server.push_str("\n# what the codes are called\nNAMES = {\n    404: \"no such path\",\n}\n");
    std::fs::write(&at, &server).unwrap();
    let blob = d.repo.git(&["hash-object", "server.py"]);
    let record = d.repo.work.join(".geas/greeter.map.jsonl");
    let was = std::fs::read_to_string(&record).unwrap();
    let old = was.split("\"blob\":\"").nth(1).and_then(|r| r.split('"').next()).unwrap().to_string();
    std::fs::write(&record, was.replace(&old, blob.trim())).unwrap();

    let (code, v, err) = d.json(&[]);
    assert_eq!(code, 0, "{err}\n{}", v["markdown"]);
    assert!(yes(&v, "claims_known"), "{}", v["claims"]);
    assert_eq!(fact(&v, "code_lines"), 6, "{}", v["files"]);
    assert_eq!(fact(&v, "unclaimed_lines"), 4, "the line no claim runs, and the three of the table: {}", v["claims"]);
    let said = v["claims"]["unclaimed"].as_array().unwrap().iter().find(|u| u["why"] == "no_statement").expect("the table is said");
    assert_eq!((said["file"].as_str(), said["count"].as_i64()), (Some("server.py"), Some(3)), "{said}");
    assert_eq!(said["lines"].as_str(), Some(format!("{}-{}", line + 1, line + 3).as_str()), "{said}");
    assert!(v["markdown"].as_str().unwrap().contains("a table, a constant"), "{}", v["markdown"]);
}

#[test]
fn a_file_with_no_line_in_the_diff_is_one_no_claim_is_known_to_run() {
    if no_git() {
        return;
    }
    let d = Desk::en("impact-renamed");
    d.repo.git(&["mv", "app.py", "app_windows.py"]);
    let (code, v, err) = d.json(&["--route", "route.rule"]);
    assert_eq!(code, 0, "{err}\n{}", v["markdown"]);
    assert_eq!((fact(&v, "code_files"), fact(&v, "code_lines"), fact(&v, "unclaimed_lines")), (1, 1, 1), "{}", v["files"]);
    assert_eq!(the_route(&v), ("owner_reads", true));

    // with a record that fits: nothing in it says what a new name does to a build
    let d = greeter("impact-renamed-record");
    d.repo.write("helper.py", "VALUE = 1\n");
    d.repo.git(&["add", "-A"]);
    d.repo.commit("a helper");
    d.repo.tag("helper");
    d.repo.git(&["mv", "helper.py", "helper_old.py"]);
    let blob = d.repo.git(&["hash-object", "helper_old.py"]);
    let line = format!("{{\"file\":\"helper_old.py\",\"blob\":\"{}\",\"lang\":\"python\",\"code\":\"1\"}}\n{{\"claim\":\"greets by name\",\"target\":\"api\",\"file\":\"helper_old.py\",\"ran\":\"1\"}}\n", blob.trim());
    d.append(".geas/greeter.map.jsonl", &line);
    let (code, out, err) = d.ritsu(&["impact", "--base", "helper", "--format", "json"]);
    assert_eq!(code, 0, "{out}\n{err}");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert!(yes(&v, "claims_known"), "{}", v["claims"]);
    let said = v["claims"]["unclaimed"].as_array().unwrap().iter().find(|u| u["why"] == "no_lines").expect("the rename is said");
    assert_eq!(said["file"], "helper_old.py", "{said}");
    assert_eq!(fact(&v, "unclaimed_lines"), 1, "{}", v["claims"]);
}

#[test]
fn what_the_report_cannot_vouch_for_goes_to_a_person() {
    if no_git() {
        return;
    }
    let d = Desk::en("impact-kinds");
    let route = |d: &Desk| -> Value { d.json(&["--route", "route.rule"]).1 };

    // prose nothing is known to read
    for doc in ["README.md", "README.ja.md", "docs/guide.md", "CONTRIBUTING.md", "notes.rst"] {
        d.put(doc, "Some prose.\n");
        let v = route(&d);
        assert_eq!((fact(&v, "doc_files"), fact(&v, "other_files")), (1, 0), "{doc}: {}", v["files"]);
        assert_eq!(the_route(&v), ("merge", false), "{doc}");
        std::fs::remove_file(d.repo.work.join(doc)).unwrap();
    }

    // a file with a role in how the project is built or worked on, and a contract a spec could read
    for other in ["AGENTS.md", ".tool/notes.md", "api/contract.json", "Makefile", "go.mod", "requirements.txt", "Dockerfile", ".github/workflows/impact.yml", "LICENSE"] {
        d.put(other, "something\n");
        let v = route(&d);
        assert_eq!((fact(&v, "doc_files"), fact(&v, "other_files")), (0, 1), "{other}: {}", v["files"]);
        assert_eq!(the_route(&v), ("owner_reads", true), "{other}");
        assert_eq!(fact(&v, "undecided"), 0, "{other}");
        std::fs::remove_file(d.repo.work.join(other)).unwrap();
    }

    // a file whose head says a generator wrote it, with nothing that held it to that: code in disguise
    d.put("wired.py", "# Code generated by nothing. DO NOT EDIT.\nprint(1)\n");
    let v = route(&d);
    assert_eq!((fact(&v, "generated_files"), fact(&v, "generated_unchecked"), fact(&v, "code_lines")), (1, 1, 0), "{}", v["files"]);
    assert_eq!(the_route(&v), ("owner_reads", true));
    std::fs::remove_file(d.repo.work.join("wired.py")).unwrap();

    // what `ritsu gen --check` holds is `generated/`, and nothing else
    let d = Desk::en("impact-kinds-target");
    d.put("ritsu.mod", "module example.com/desk\n\ntarget python\n");
    let (code, out, err) = d.ritsu(&["gen"]);
    assert_eq!(code, 0, "{out}\n{err}");
    d.repo.git(&["add", "-A"]);
    d.repo.commit("generated");
    d.repo.tag("generated");
    d.put("wired.py", "# Code generated by nothing. DO NOT EDIT.\nprint(1)\n");
    let (_, out, err) = d.ritsu(&["impact", "--base", "generated", "--route", "route.rule", "--format", "json"]);
    let v: Value = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}\n{err}"));
    assert!(yes(&v, "generated_checked") && !yes(&v, "generated_stale"), "{}", v["generated"]);
    assert_eq!(fact(&v, "generated_unchecked"), 1, "{}", v["files"]);
    assert_eq!(the_route(&v), ("owner_reads", true));

    // what a requirement says, with no answer moved: its owner reads it
    let d = Desk::en("impact-requirement");
    d.edit("refunds.req", "A refund of up to 20 items is small", "A refund of at most 20 items is small");
    let v = route(&d);
    assert_eq!((fact(&v, "spec_files"), fact(&v, "rule_changes")), (1, 0), "{}", v["facts"]);
    assert!(fact(&v, "requirements_moved") >= 1, "{}", v["facts"]);
    assert_ne!(the_route(&v), ("merge", false), "{}", v["facts"]);
}

#[test]
fn no_claim_runs_and_nothing_is_written() {
    if no_git() {
        return;
    }
    let d = greeter("impact-no-run");
    let before = d.status();
    let (code, _, err) = d.ritsu(&["impact", "--base", "base"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(d.status(), before, "the project is as it was");
    assert!(!d.repo.work.join(".geas/greeter.journal.jsonl").exists(), "no claim ran");
}

// ── A rule ──────────────────────────────────────────────────────────────

#[test]
fn a_table_of_a_rule_moves_the_answers_and_says_the_owner() {
    if no_git() {
        return;
    }
    let mut failures = Vec::new();
    // English
    let d = Desk::en("impact-rule");
    d.move_the_band();
    d.review("refunds.req", "development");
    let (code, v, err) = d.json(&["--route", "route.rule"]);
    assert_eq!(code, 0, "{err}\n{}", v["markdown"]);
    assert_eq!(fact(&v, "rule_changes"), 1, "{}", v["facts"]);
    assert_eq!(fact(&v, "allow_gained"), 1, "the gate that reads the rule: {}", v["facts"]);
    assert_eq!(fact(&v, "date_changes"), 0);
    assert_eq!(fact(&v, "new_errors"), 0);
    assert_eq!(fact(&v, "undecided"), 0);
    assert!(fact(&v, "requirements_moved") >= 1);
    assert_eq!(v["requirements"]["owners"], serde_json::json!(["finance"]));
    assert_eq!(the_route(&v), ("owner_reads", true));
    assert_eq!(v["route"]["file"], "route.rule");
    assert_eq!(v["base"]["rev"], "base");
    assert_eq!(v["head"]["rev"], Value::Null);
    for (args, ext) in [(vec!["--route", "route.rule"], "txt"), (vec!["--route", "route.rule", "--format", "markdown"], "md"), (vec!["--route", "route.rule", "--format", "json"], "json")] {
        let mut all = vec!["impact", "--base", "base"];
        all.extend(args);
        let (code, out, err) = d.ritsu(&all);
        assert_eq!(code, 0, "{err}");
        golden(&format!("rule.en.{ext}"), &out, &mut failures);
        if ext == "md" {
            let outside: String = out.split('`').step_by(2).collect();
            assert!(!outside.contains('@'), "every @ of a comment is in code:\n{out}");
        }
    }
    // Japanese
    let d = Desk::ja("impact-rule-ja");
    for (from, to) in [("| <=20 | 少数           |", "| <=30 | 少数           |"), ("| >20  | 多数           |", "| >30  | 多数           |"), ("| 20   | 少数    |", "| 30   | 少数    |"), ("| 21   | 多数    |", "| 31   | 多数    |")] {
        d.edit("refund_band.ja.rule", from, to);
    }
    d.review("refunds.ja.req", "開発");
    for (args, ext) in [(vec!["--route", "route.ja.rule"], "txt"), (vec!["--route", "route.ja.rule", "--format", "markdown"], "md"), (vec!["--route", "route.ja.rule", "--format", "json"], "json")] {
        let mut all = vec!["impact", "--base", "base", "--lang", "ja"];
        all.extend(args);
        let (code, out, err) = d.ritsu(&all);
        assert_eq!(code, 0, "{err}");
        golden(&format!("rule.ja.{ext}"), &out, &mut failures);
        if ext == "json" {
            let v: Value = serde_json::from_str(&out).unwrap();
            // the route's outputs and values are the rule's public names, whatever the language
            assert_eq!(the_route(&v), ("owner_reads", true), "{}", v["route"]);
            assert_eq!(v["requirements"]["owners"], serde_json::json!(["経理"]));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_change_no_one_has_looked_at_is_an_error_only_the_head_has() {
    if no_git() {
        return;
    }
    let d = Desk::en("impact-unreviewed");
    d.move_the_band();
    let (code, v, _) = d.json(&["--route", "route.rule"]);
    assert_eq!(code, 1, "{}", v["markdown"]);
    assert_eq!(fact(&v, "new_errors"), 2, "{}", v["check"]["new"]);
    assert_eq!(v["check"]["new"][0]["code"], "E303");
    assert_eq!(v["check"]["new"][0]["tool"], "yuen");
    assert_eq!(fact(&v, "rule_changes"), 1, "what moved is said all the same");
    assert_eq!(the_route(&v), ("stop", true));
    let md = v["markdown"].as_str().unwrap();
    assert!(md.contains("E303") && md.contains("exit 1"), "{md}");
    // the same on text
    let (code, out, _) = d.ritsu(&["impact", "--base", "base"]);
    assert_eq!(code, 1);
    assert!(out.contains("error[yuen E303] refunds.req:"), "{out}");
}

#[test]
fn an_error_both_sides_have_is_not_new_where_its_line_moved() {
    if no_git() {
        return;
    }
    let d = Desk::en("impact-old-error");
    d.move_the_band();
    d.repo.git(&["add", "-A"]);
    d.repo.commit("no one looked");
    d.repo.tag("stale");
    let s = std::fs::read_to_string(d.repo.work.join("refunds.req")).unwrap();
    let (head, rest) = s.split_once('\n').unwrap();
    d.put("refunds.req", &format!("{head}\n# a comment above the links\n{rest}"));
    let (code, out, err) = d.ritsu(&["impact", "--base", "stale", "--format", "json"]);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(code, 0, "{err}\n{}", v["markdown"]);
    assert_eq!(fact(&v, "new_errors"), 0, "{}", v["check"]["new"]);
    assert_eq!(v["check"]["head"]["errors"], 2);
    assert_eq!(v["check"]["gone"], serde_json::json!([]));
}

// ── A gate ──────────────────────────────────────────────────────────────

#[test]
fn a_permit_of_a_gate() {
    if no_git() {
        return;
    }
    // more is allowed: a manager has no period any more
    let d = Desk::en("impact-gate");
    d.edit("refunds.gate", "  principal in manager\n  action refund_order\n  when in_period\n", "  principal in manager\n  action refund_order\n");
    d.review("refunds.req", "development");
    let (code, v, _) = d.json(&["--route", "route.rule"]);
    assert_eq!(code, 0, "{}", v["markdown"]);
    assert!(fact(&v, "allow_gained") >= 1, "{}", v["facts"]);
    assert_eq!(fact(&v, "allow_lost"), 0);
    assert_eq!(fact(&v, "rule_changes") + fact(&v, "date_changes"), 0);
    assert_eq!(v["requirements"]["owners"], serde_json::json!(["finance"]));
    assert_eq!(the_route(&v), ("owner_reads", true));

    // less is allowed: a manager is held to what a clerk is
    let d = Desk::en("impact-gate-lost");
    d.edit("refunds.gate", "  principal in manager\n  action refund_order\n  when in_period\n", "  principal in manager\n  action refund_order\n  when size is small\n  when in_period\n");
    d.review("refunds.req", "development");
    let (code, v, _) = d.json(&["--route", "route.rule"]);
    assert_eq!(code, 0, "{}", v["markdown"]);
    assert!(fact(&v, "allow_lost") >= 1, "{}", v["facts"]);
    assert_eq!(fact(&v, "allow_gained"), 0);
    assert_eq!(the_route(&v), ("owner_reads", true), "an answer that moved does not go straight to merge");
}

#[test]
fn a_calendar_the_gate_reads_moves_the_dates_and_the_gate() {
    if no_git() {
        return;
    }
    let d = Desk::en("impact-cal");
    d.append("office.cal", "closed 2027-03-03 \"Stocktaking\"\n");
    d.review("refunds.req", "development");
    let (code, v, _) = d.json(&["--route", "route.rule"]);
    assert_eq!(code, 0, "{}", v["markdown"]);
    assert!(fact(&v, "date_changes") >= 1, "{}", v["facts"]);
    assert!(fact(&v, "allow_gained") >= 1, "the last day is later for a gate that compares today with it: {}", v["facts"]);
    assert_eq!(v["files"][0]["path"], "office.cal");
    assert_eq!(v["requirements"]["owners"], serde_json::json!(["finance"]));
    assert_eq!(the_route(&v), ("owner_reads", true));
}

#[test]
fn a_file_nothing_reads_shows_no_difference_on_a_gate_with_a_date_condition() {
    if no_git() {
        return;
    }
    let d = Desk::en("impact-unrelated");
    d.put("README.md", "The desk.\n");
    let (code, v, _) = d.json(&["--route", "route.rule"]);
    assert_eq!(code, 0, "{}", v["markdown"]);
    nothing_moved(&v);
    assert_eq!(fact(&v, "doc_files"), 1);
    assert_eq!(fact(&v, "undecided"), 0);
    assert_eq!(fact(&v, "requirements_unknown"), 0, "a file that is no spec or code asks no requirement");
    assert_eq!(the_route(&v), ("merge", false));
    let gate = v["behavior"].as_array().unwrap().iter().find(|b| b["file"] == "refunds.gate").expect("the gate is compared");
    assert_eq!(gate["result"]["total"]["gained"], 0);
    assert_eq!(gate["result"]["total"]["lost"], 0);
    assert_eq!(gate["result"]["total"]["reasons"], 0);

    // a comment in the dates file it reads, and its description: nothing it rests on moves
    d.edit("refund_terms.cal", "  + 30 days\n", "  + 30 days   # after the payment\n");
    let (_, v, _) = d.json(&[]);
    assert_eq!(fact(&v, "date_changes"), 0, "{}", v["facts"]);
    assert_eq!(fact(&v, "allow_gained") + fact(&v, "allow_lost") + fact(&v, "policies_moved"), 0, "{}", v["facts"]);

    // nothing changed at all
    let d = Desk::en("impact-nothing");
    let (code, v, _) = d.json(&[]);
    assert_eq!(code, 0);
    nothing_moved(&v);
    assert_eq!(v["files"], serde_json::json!([]));
    assert_eq!(v["route"], Value::Null);

    // nothing changed, and a file that does not pass on both sides: there is no difference to compare
    let d = Desk::en("impact-nothing-broken");
    d.edit("refund_band.rule", "| <=20  | small          |", "| <=20  | tiny           |");
    d.repo.git(&["add", "-A"]);
    d.repo.commit("broken");
    d.repo.tag("broken");
    let (code, out, err) = d.ritsu(&["impact", "--base", "broken", "--format", "json"]);
    assert_eq!(code, 0, "{err}\n{out}");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(fact(&v, "undecided"), 0, "{}", v["undecided"]);
    assert!(v["check"]["head"]["errors"].as_i64().unwrap() >= 1);
}

// ── What makes the exit ─────────────────────────────────────────────────

#[test]
fn an_error_only_the_head_has_makes_exit_1_and_the_json_goes_to_its_end() {
    if no_git() {
        return;
    }
    let d = Desk::en("impact-error");
    d.edit("refund_band.rule", "| <=20  | small          |", "| <=20  | tiny           |");
    let (code, v, _) = d.json(&["--route", "route.rule"]);
    assert_eq!(code, 1, "{}", v["markdown"]);
    assert!(fact(&v, "new_errors") >= 1);
    assert!(v["check"]["new"].as_array().unwrap().iter().any(|n| n["tool"] == "rulec"), "{}", v["check"]["new"]);
    assert_eq!(the_route(&v), ("stop", true));
    for key in ["files", "check", "behavior", "interface", "requirements", "claims", "generated", "facts", "route", "markdown"] {
        assert!(v.get(key).is_some(), "{key} is in the JSON");
    }
}

#[test]
fn what_cannot_be_compared_makes_exit_2_and_the_json_goes_to_its_end() {
    if no_git() {
        return;
    }
    // the base's rule does not pass its check, and the head's does
    let d = Desk::en("impact-cannot");
    d.edit("refund_band.rule", "| <=20  | small          |", "| <=20  | tiny           |");
    d.repo.git(&["add", "-A"]);
    d.repo.commit("broken");
    d.repo.tag("broken");
    d.edit("refund_band.rule", "| <=20  | tiny           |", "| <=20  | small          |");
    let (code, out, err) = d.ritsu(&["impact", "--base", "broken", "--format", "json", "--route", "route.rule"]);
    assert_eq!(code, 2, "{err}\n{out}");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert!(fact(&v, "undecided") >= 1, "{}", v["facts"]);
    assert_eq!(fact(&v, "new_errors"), 0, "{}", v["check"]["new"]);
    assert!(v["check"]["gone"].as_array().unwrap().iter().any(|n| n["tool"] == "rulec"), "{}", v["check"]["gone"]);
    // what could not be decided goes to a person; the exit says so too
    assert_eq!(the_route(&v), ("owner_reads", true));
    let what: Vec<&str> = v["undecided"].as_array().unwrap().iter().map(|u| u["what"].as_str().unwrap()).collect();
    assert!(what.contains(&"refund_band.rule"), "{what:?}");
    assert!(v["markdown"].as_str().unwrap().contains("exit 2"), "{}", v["markdown"]);
    let (code, out, _) = d.ritsu(&["impact", "--base", "broken"]);
    assert_eq!(code, 2);
    assert!(out.contains("could not be compared or decided:"), "{out}");
    let (code, out, _) = d.ritsu(&["impact", "--base", "broken", "--lang", "ja"]);
    assert_eq!(code, 2);
    assert!(out.contains("比べられないもの、決められないもの"), "{out}");
}

// ── The route rule ──────────────────────────────────────────────────────

/// A route rule whose inputs are `inputs` and which always answers `merge`.
fn a_rule(inputs: &str) -> String {
    format!("rule wrong(wrong) v1\ndescription \"A route rule of the test\"\n\nenum destination(destination) = merge | stop\n\ninputs\n{inputs}\n\noutputs\n  route : destination\n\ntable route\npolicy unique\n| -> route : destination |\n| merge                  |\n")
}

#[test]
fn a_route_rule_takes_only_the_facts_by_their_types() {
    if no_git() {
        return;
    }
    let d = Desk::en("impact-route-wrong");
    let cases: [(&str, &str, &str, &str, &str); 4] = [
        ("  moved_rules : number  range >=0", "is named after no fact. The names there are: spec_files, generated_files,", "は、事実の名前ではありません。使える名前: spec_files, generated_files,", "generated_stale, generated_checked", "generated_stale, generated_checked"),
        ("  rule_changes : bool", "the input `rule_changes` of the route rule `wrong.rule` is bool, and the fact `rule_changes` is a count (number)", "の入力 `rule_changes` は bool ですが、事実 `rule_changes` は数（number）です", "", ""),
        ("  claims_known : number  range >=0", "is number, and the fact `claims_known` is a yes or no (bool)", "は number ですが、事実 `claims_known` は真偽（bool）です", "", ""),
        ("  spec_files : money[GBP, incl_tax]  range >=0GBP <=10GBP", "is money[GBP, incl_tax], and the fact `spec_files` is a count (number)", "は money[GBP, incl_tax] ですが、事実 `spec_files` は数（number）です", "", ""),
    ];
    d.repo.write("keep", "");
    for (input, en, ja, en2, ja2) in cases {
        d.put("wrong.rule", &a_rule(input));
        let (code, out, err) = d.ritsu(&["impact", "--base", "base", "--route", "wrong.rule"]);
        assert_eq!(code, 2, "{input}\n{out}\n{err}");
        assert_eq!(out, "", "nothing is walked before the rule is read");
        assert!(err.contains(en) && err.contains(en2), "{input}: {err}");
        let (code, _, err) = d.ritsu(&["impact", "--base", "base", "--route", "wrong.rule", "--lang", "ja"]);
        assert_eq!(code, 2);
        assert!(err.contains(ja) && err.contains(ja2), "{input}: {err}");
    }

    // a rule that does not pass its check: what it says is given, with where
    d.put("wrong.rule", "rule wrong(wrong) v1\ndescription \"No outputs\"\n");
    let (code, _, err) = d.ritsu(&["impact", "--base", "base", "--route", "wrong.rule"]);
    assert_eq!(code, 2);
    assert!(err.contains("the route rule `wrong.rule` does not pass its check") && err.contains("wrong.rule"), "{err}");
    // no such file
    let (code, _, err) = d.ritsu(&["impact", "--base", "base", "--route", "nowhere.rule"]);
    assert_eq!(code, 2);
    assert!(err.contains("there is no route rule `nowhere.rule`"), "{err}");
    let (code, _, err) = d.ritsu(&["impact", "--base", "base", "--route", "nowhere.rule", "--lang", "ja"]);
    assert_eq!(code, 2);
    assert!(err.contains("経路の規則 `nowhere.rule` がありません"), "{err}");
}

#[test]
fn a_value_outside_a_range_is_said_and_not_rounded() {
    if no_git() {
        return;
    }
    let d = Desk::en("impact-route-range");
    d.move_the_band();
    d.review("refunds.req", "development");
    d.put("narrow.rule", &a_rule("  allow_gained : number  range >=0 <=0"));
    let (code, out, err) = d.ritsu(&["impact", "--base", "base", "--route", "narrow.rule", "--format", "json"]);
    assert_eq!(code, 2, "{out}\n{err}");
    assert!(err.contains("the fact `allow_gained` is 1, outside the range (>=0 <=0) of the input `allow_gained`") && err.contains("not rounded"), "{err}");
    // the report comes to its end, with no route
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["route"], Value::Null);
    assert_eq!(fact(&v, "allow_gained"), 1);
    let (code, out, err) = d.ritsu(&["impact", "--base", "base", "--route", "narrow.rule", "--lang", "ja"]);
    assert_eq!(code, 2);
    assert!(err.contains("事実 `allow_gained` の値 1 は") && err.contains("範囲（>=0 <=0）の外です"), "{err}");
    assert!(out.contains("経路の規則に事実を渡せない"), "{out}");
}

#[test]
fn the_route_of_the_example_sends_no_change_that_moved_an_answer_to_merge() {
    if no_git() {
        return;
    }
    let d = Desk::en("impact-promise");
    let (code, out, err) = d.ritsu(&["check", "route.rule"]);
    assert_eq!(code, 0, "{out}\n{err}");
    // a table that sends such a change to merge: the second table has no row for it, and the check stops
    d.edit("route.rule", "| false           | 0          | >=1   | -      | owner_reads            |", "| false           | 0          | >=1   | -      | merge                  |");
    let s = std::fs::read_to_string(d.repo.work.join("route.rule")).unwrap();
    let kept: Vec<&str> = s.lines().filter(|l| !l.ends_with("| owner_reads | true            |")).collect();
    d.put("route.rule", &format!("{}\n", kept.join("\n")));
    let (code, out, err) = d.ritsu(&["check", "route.rule"]);
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(format!("{out}{err}").contains("E101"), "{out}\n{err}");
}

// ── The files ───────────────────────────────────────────────────────────

fn file_of<'a>(v: &'a Value, path: &str) -> &'a Value {
    v["files"].as_array().unwrap().iter().find(|f| f["path"] == path).unwrap_or_else(|| panic!("{path} is not among {}", v["files"]))
}

#[test]
fn each_file_is_what_its_head_or_its_name_says() {
    if no_git() {
        return;
    }
    let d = Desk::en("impact-kinds");
    d.repo.write("notes.txt", "a\nb\nc\nd\ne\nf\ng\nh\n");
    d.repo.write("gone.py", "print(1)\n");
    d.repo.write("old.rule", "rule old(old) v1\ndescription \"A rule that goes\"\n\nenum e(e) = a | b\n\ninputs\n  x : number  range >=0 <=9\n\noutputs\n  y : e\n\ntable y\npolicy unique\n| x   | -> y : e |\n| <=4 | a        |\n| >4  | b        |\n");
    d.repo.write(".gitignore", "ignored.py\n");
    d.repo.commit("more");
    d.repo.tag("more");
    d.put("gen.py", "# Code generated by ritsu 0.1.0. DO NOT EDIT.\nx = 1\n");
    d.put("page.html", "<!-- Generated by sekisho 0.1.0 from a.gate -->\n<p>a</p>\n");
    d.put("gen.ts", "// Code generated by rulec 0.1.0. DO NOT EDIT.\nexport const x = 1;\n");
    d.put("mine.ts", "export const y = 1;\n");
    d.put("ignored.py", "print(2)\n");
    d.put("@scope.txt", "a\n");
    std::fs::remove_file(d.repo.work.join("gone.py")).unwrap();
    std::fs::remove_file(d.repo.work.join("old.rule")).unwrap();
    std::fs::rename(d.repo.work.join("notes.txt"), d.repo.work.join("notes2.txt")).unwrap();
    d.repo.git(&["add", "-A"]);
    d.repo.commit("after");
    d.repo.tag("after");
    let (code, out, err) = d.ritsu(&["impact", "--base", "more", "--head", "after", "--format", "json"]);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(code, 0, "{err}\n{}", v["markdown"]);
    assert_eq!(v["head"]["rev"], "after");
    for (path, change, kind) in [("gen.py", "added", "generated"), ("page.html", "added", "generated"), ("gen.ts", "added", "generated"), ("mine.ts", "added", "code"), ("gone.py", "removed", "code"), ("old.rule", "removed", "spec"), ("notes2.txt", "renamed", "other"), ("@scope.txt", "added", "other")] {
        let f = file_of(&v, path);
        assert_eq!((f["change"].as_str(), f["kind"].as_str()), (Some(change), Some(kind)), "{f}");
    }
    assert_eq!(file_of(&v, "notes2.txt")["from"], "notes.txt");
    assert_eq!(file_of(&v, "mine.ts")["lines"], 1);
    assert_eq!(file_of(&v, "gone.py")["lines"], 1);
    assert!(!v["files"].as_array().unwrap().iter().any(|f| f["path"] == "ignored.py"), "git ignores it: {}", v["files"]);
    assert_eq!(fact(&v, "generated_files"), 3);
    assert_eq!(fact(&v, "code_lines"), 2, "generated files are not code: {}", v["facts"]);
    // a rule that goes: one change of the answers, and it can break what read it
    assert_eq!(fact(&v, "rule_changes"), 1, "{}", v["facts"]);
    assert_eq!(fact(&v, "interface_breaks"), 1, "{}", v["facts"]);
    let md = v["markdown"].as_str().unwrap();
    let outside: String = md.split('`').step_by(2).collect();
    assert!(!outside.contains('@'), "every @ of a comment is in code:\n{md}");

    // the working tree: what git does not ignore and does not track comes as added
    let d = Desk::en("impact-untracked");
    d.put(".gitignore", "ignored.py\n");
    d.put("ignored.py", "print(2)\n");
    d.put("new.py", "print(1)\nprint(2)\n");
    d.put("with space.txt", "a\n");
    let (_, v, _) = d.json(&[]);
    assert_eq!(file_of(&v, "new.py")["change"], "added");
    assert_eq!(file_of(&v, "with space.txt")["kind"], "other");
    assert!(!v["files"].as_array().unwrap().iter().any(|f| f["path"] == "ignored.py"), "{}", v["files"]);
    assert_eq!(fact(&v, "code_lines"), 2);
}

#[test]
fn a_language_whose_contents_are_not_compared_is_counted() {
    if no_git() {
        return;
    }
    let d = Desk::en("impact-opaque");
    d.repo.write("n.flow", "workflow n v1\ndescription \"A flow\"\n\nflow\n  pass\n");
    d.repo.write("m.kn", "module m v1\ndescription \"A module\"\n\nfn doubled(n: number range >=0 <=10) -> number\n  n + n\n");
    d.repo.commit("a flow and a module");
    d.repo.tag("two");
    d.edit("n.flow", "\"A flow\"", "\"The flow\"");
    d.edit("m.kn", "\"A module\"", "\"The module\"");
    let (code, out, err) = d.ritsu(&["impact", "--base", "two", "--route", "route.rule", "--format", "json"]);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(code, 0, "{err}\n{}", v["markdown"]);
    assert_eq!(fact(&v, "opaque_specs"), 2);
    assert_eq!(fact(&v, "spec_files"), 2);
    assert_eq!(fact(&v, "rule_changes") + fact(&v, "date_changes") + fact(&v, "allow_gained") + fact(&v, "allow_lost"), 0);
    assert_eq!(the_route(&v), ("owner_reads", true), "they are read by a person");
    assert!(v["markdown"].as_str().unwrap().contains("whose contents are not compared"), "{}", v["markdown"]);
}

#[test]
fn the_generated_files_are_held_to_what_gen_writes_when_there_is_a_target() {
    if no_git() {
        return;
    }
    // no `target` line
    let d = Desk::en("impact-no-target");
    let (_, v, _) = d.json(&[]);
    assert!(!yes(&v, "generated_checked") && !yes(&v, "generated_stale"));
    assert!(v["generated"]["note"].as_str().unwrap().contains("target"), "{}", v["generated"]);

    // a `target` line
    let d = Desk::en("impact-target");
    d.put("ritsu.mod", "module example.com/desk\n\ntarget python\n");
    let (code, out, err) = d.ritsu(&["gen"]);
    assert_eq!(code, 0, "{out}\n{err}");
    d.repo.git(&["add", "-A"]);
    d.repo.commit("generated");
    d.repo.tag("generated");
    let impact = |args: &[&str]| -> (i32, Value) {
        let mut all = vec!["impact", "--base", "generated", "--format", "json"];
        all.extend(args);
        let (code, out, err) = d.ritsu(&all);
        (code, serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}\n{err}")))
    };
    let (code, v) = impact(&[]);
    assert_eq!(code, 0, "{}", v["markdown"]);
    assert!(yes(&v, "generated_checked") && !yes(&v, "generated_stale"), "{}", v["generated"]);
    d.move_the_band();
    d.review("refunds.req", "development");
    let (code, v) = impact(&["--route", "route.rule"]);
    assert_eq!(code, 0, "where a rule's generated code is stale decides no exit: {}", v["markdown"]);
    assert!(yes(&v, "generated_checked") && yes(&v, "generated_stale"), "{}", v["generated"]);
    assert_eq!(v["generated"]["stale"][0], "generated/python/generated/rules/refund_band.py");
    assert_eq!(the_route(&v), ("stop", true));

    // two revisions: the working tree is not what is asked about
    let (code, out, err) = d.ritsu(&["impact", "--base", "generated", "--head", "generated", "--format", "json"]);
    assert_eq!(code, 0, "{err}");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert!(!yes(&v, "generated_checked"));
    assert!(v["generated"]["note"].as_str().unwrap().contains("not checked"), "{}", v["generated"]);
}

// ── What stops a run ────────────────────────────────────────────────────

#[test]
fn what_stops_a_run_in_each_language() {
    if no_git() {
        return;
    }
    let d = Desk::en("impact-stops");
    let cases: [(&[&str], &str, &str); 5] = [
        (&["impact"], "`--base <revision>` is required", "`--base <git の版>`"),
        (&["impact", "--base", "no-such-tag"], "no-such-tag", "no-such-tag"),
        (&["impact", "--base", "base", "--format", "pdf"], "--format", "--format"),
        (&["impact", "--base", "base", "--root", "app.py"], "`--root app.py` is not a directory", "`--root app.py` はディレクトリではありません"),
        (&["impact", "--base", "base", "--budget", "many"], "--budget", "--budget"),
    ];
    for (args, en, ja) in cases {
        let (code, out, err) = d.ritsu(args);
        assert_eq!(code, 2, "{args:?}\n{out}\n{err}");
        assert!(err.contains(en), "{args:?}: {err}");
        let mut with: Vec<&str> = args.to_vec();
        with.extend(["--lang", "ja"]);
        let (code, _, err) = d.ritsu(&with);
        assert_eq!(code, 2);
        assert!(err.contains(ja), "{with:?}: {err}");
    }
    // not a repository
    let t = TempDir::new("impact-not-a-repo");
    let o = Command::new(env!("CARGO_BIN_EXE_ritsu")).current_dir(t.path()).args(["impact", "--base", "HEAD"]).env("GIT_CEILING_DIRECTORIES", t.path()).output().unwrap();
    assert_eq!(o.status.code(), Some(2), "{}", String::from_utf8_lossy(&o.stderr));
}

#[test]
fn the_root_can_be_given_and_help_names_the_facts() {
    if no_git() {
        return;
    }
    let d = Desk::en("impact-root");
    d.move_the_band();
    d.review("refunds.req", "development");
    let root = d.repo.work.display().to_string();
    let elsewhere = TempDir::new("impact-elsewhere");
    let o = Command::new(env!("CARGO_BIN_EXE_ritsu")).current_dir(elsewhere.path()).args(["impact", "--base", "base", "--root", &root, "--format", "json"]).output().unwrap();
    let v: Value = serde_json::from_str(&String::from_utf8_lossy(&o.stdout)).unwrap();
    assert_eq!(fact(&v, "rule_changes"), 1, "{}", v["facts"]);
    assert_eq!(o.status.code(), Some(0));
    assert!(!String::from_utf8_lossy(&o.stdout).contains(&root), "no absolute path of the runner's:\n{}", String::from_utf8_lossy(&o.stdout));

    // a project below the top of its repository: the paths are the project's, and what is outside is not
    let d2 = Desk::empty("impact-below");
    d2.repo.write("top.txt", "top\n");
    copy_into(&d2.repo, &here().join("examples/impact"), "project");
    d2.repo.commit("a project below");
    d2.repo.tag("base");
    d2.append("top.txt", "more\n");
    d2.put("project/new.py", "print(1)\n");
    let (code, out, err) = d2.ritsu(&["impact", "--base", "base", "--root", "project", "--route", "project/route.rule", "--format", "json"]);
    assert_eq!(code, 0, "{err}\n{out}");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["files"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap()).collect::<Vec<_>>(), ["new.py"], "{}", v["files"]);
    assert_eq!(v["route"]["outputs"]["route"], "owner_reads");

    // a module of its own below the root: not read by this project's check, so not compared, and said
    d2.put("project/ritsu.mod", "module example.com/desk\n");
    d2.repo.git(&["add", "-A"]);
    d2.repo.commit("a module");
    d2.repo.tag("moduled");
    d2.append("project/refund_band.rule", "# a comment\n");
    let (code, out, _) = d2.ritsu(&["impact", "--base", "moduled", "--format", "json"]);
    assert_eq!(code, 2, "{out}");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["undecided"][0]["what"], "project/refund_band.rule", "{}", v["undecided"]);
    assert!(v["undecided"][0]["why"].as_str().unwrap().contains("--root"), "{}", v["undecided"]);

    let (code, out, _) = d.ritsu(&["impact", "--help"]);
    assert_eq!(code, 0);
    for name in ["unclaimed_lines", "claims_known", "generated_stale", "allow_gained", "--route", "--base", "--head"] {
        assert!(out.contains(name), "{name} is on the help: {out}");
    }
    let (code, out, _) = d.ritsu(&["--help"]);
    assert_eq!(code, 0);
    assert!(out.contains("impact"), "{out}");
}

// ── The pages ───────────────────────────────────────────────────────────

/// The fences of a page: what follows each opening fence, and its lines.
fn fences(text: &str) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    let mut open: Option<(String, Vec<String>)> = None;
    for line in text.lines() {
        match (&mut open, line.strip_prefix("```")) {
            (None, Some(info)) => open = Some((info.to_string(), Vec::new())),
            (Some(_), Some(_)) => out.extend(open.take()),
            (Some((_, lines)), None) => lines.push(line.to_string()),
            (None, None) => {}
        }
    }
    out
}

#[test]
fn the_pages_show_what_the_example_prints_and_every_fact() {
    let repo = here().join("../..");
    let read = |p: PathBuf| std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let golden_json: Value = serde_json::from_str(&read(here().join("tests/golden/impact/rule.en.json"))).unwrap();
    let facts: Vec<&String> = golden_json["facts"].as_object().unwrap().keys().collect();
    assert_eq!(facts.len(), 26);
    for (page, printed, rule, shown) in [
        ("website/docs/impact.md", "rule.en.txt", "examples/impact/route.rule", 3),
        ("website/docs-ja/impact.md", "rule.ja.txt", "examples/impact.ja/route.ja.rule", 3),
        ("README.md", "rule.en.txt", "examples/impact/route.rule", 1),
        ("README.ja.md", "rule.ja.txt", "examples/impact.ja/route.ja.rule", 1),
    ] {
        let text = read(repo.join(page));
        let printed = read(here().join("tests/golden/impact").join(printed));
        let rule = read(here().join(rule));
        let mut outputs = 0;
        for (info, lines) in fences(&text) {
            if info == "rule" && lines.iter().any(|l| l.contains("-> route : destination") || l.contains("-> 経路 : 行き先")) {
                for l in lines.iter().filter(|l| !l.is_empty()) {
                    assert!(rule.lines().any(|r| r == l), "{page}: `{l}` is no line of the rule");
                }
            } else if info.is_empty() && lines.iter().any(|l| l.starts_with("ritsu impact") && (l.contains('→'))) {
                outputs += 1;
                for l in lines.iter().filter(|l| !l.is_empty()) {
                    assert!(printed.lines().any(|r| r == l), "{page}: `{l}` is not what the example prints");
                }
            } else if info.is_empty() && lines.iter().all(|l| l.starts_with("      ")) && !lines.is_empty() {
                outputs += 1;
                for l in &lines {
                    assert!(printed.lines().any(|r| r == l), "{page}: `{l}` is not what the example prints");
                }
            }
        }
        assert_eq!(outputs, shown, "{page}: the blocks of output");
        if page.starts_with("website") {
            for name in &facts {
                assert!(text.contains(&format!("`{name}`")), "{page} lists {name}");
            }
            assert!(text.contains(&format!("uses: i2y/ritsu@v{}", env!("CARGO_PKG_VERSION"))), "{page}: the version of the workflow");
        }
    }
}
